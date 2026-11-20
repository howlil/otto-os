use super::messages::bar_window_proc;
use super::messages::menu_window_proc;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, SW_HIDE, ShowWindow, WINDOW_EX_STYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};
use windows::core::{Error, Result, w};

use crate::{bar, settings_window};
use otto_platform::{restore_hidden_windows, shutdown_background_tasks};

use super::Host;
use super::create_overlay_window;
use super::dependencies_enabled;
use super::dependencies_running;
use super::descriptor;
use super::plugins::PLUGIN_DESCRIPTORS;
use super::plugins::PluginConfig;
use super::plugins::PluginId;
use super::plugins::PluginState;
use super::plugins::SurfaceSlot;
use super::report_plugin_failure;
use super::reverse_lifecycle_order;
use super::runtime::Surfaces;
use super::surface;
use super::user_can_toggle;

impl Host {
    pub(super) fn sync_plugin_flags(&self) {
        self.runtime
            .top_bar_enabled
            .set(self.config.is_enabled(PluginId::TopBar));
        self.runtime
            .dock_enabled
            .set(self.config.is_enabled(PluginId::Dock));
        self.runtime
            .window_controls_enabled
            .set(self.config.is_enabled(PluginId::WindowControls));
        self.runtime
            .genie_enabled
            .set(self.config.is_enabled(PluginId::Genie));
    }

    pub(super) fn set_state(&mut self, plugin: PluginId, state: PluginState) {
        if let Some((_, current)) = self
            .states
            .iter_mut()
            .find(|(candidate, _)| *candidate == plugin)
        {
            *current = state;
        }
    }

    pub(super) fn initialize(&mut self) -> Result<()> {
        let bar = create_overlay_window(
            self.instance,
            self.cursor,
            w!("OttoNotchBar"),
            Some(bar_window_proc),
            self.ex_style,
        )?;
        let panel = match create_overlay_window(
            self.instance,
            self.cursor,
            w!("OttoNotchPanel"),
            Some(bar_window_proc),
            self.ex_style,
        ) {
            Ok(panel) => panel,
            Err(error) => {
                unsafe {
                    let _ = DestroyWindow(bar);
                }
                return Err(error);
            }
        };
        let menu = match create_overlay_window(
            self.instance,
            self.cursor,
            w!("OttoPopupMenu"),
            Some(menu_window_proc),
            self.ex_style,
        ) {
            Ok(menu) => menu,
            Err(error) => {
                unsafe {
                    let _ = DestroyWindow(panel);
                    let _ = DestroyWindow(bar);
                }
                return Err(error);
            }
        };
        self.runtime.bar_hwnd.set(bar.0 as isize);
        self.tray_icon = otto_platform::TrayIcon::register(bar.0 as isize);
        self.taskbar_created_message = self
            .tray_icon
            .as_ref()
            .and_then(otto_platform::TrayIcon::taskbar_created_message);
        self.recovery_hotkeys = otto_platform::register_shell_hotkeys(bar.0 as isize);
        let _ = otto_platform::register_settings_hotkey(bar.0 as isize);
        if self.runtime.hotkeys_enabled.get() {
            otto_platform::register_notch_hotkeys(bar.0 as isize);
        }
        self.surfaces = Some(Surfaces {
            bar,
            panel,
            menu,
            dock: None,
            window_controls: None,
            genie: None,
            control_center: None,
            spotlight: None,
        });
        bar::start(
            &self.runtime,
            surface(self, SurfaceSlot::Bar)?.0 as isize,
            surface(self, SurfaceSlot::Panel)?.0 as isize,
            self.blur,
            self.config.is_enabled(PluginId::ControlCenter),
        )?;

        for descriptor in PLUGIN_DESCRIPTORS {
            if !self.config.is_enabled(descriptor.id)
                || !dependencies_enabled(&self.config, descriptor.dependencies)
            {
                continue;
            }
            match self.start_plugin(descriptor.id) {
                Ok(()) => {}
                Err(error) if descriptor.required => return Err(error),
                Err(error) => report_plugin_failure(descriptor.id, &error),
            }
        }
        self.sync_plugin_flags();
        Ok(())
    }

    pub(super) fn start_plugin(&mut self, plugin: PluginId) -> Result<()> {
        let descriptor = descriptor(plugin);
        if self.started.contains(&plugin) {
            return Ok(());
        }
        if !dependencies_running(&self.started, descriptor.dependencies) {
            self.set_state(plugin, PluginState::Failed);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
                self.sync_plugin_flags();
            }
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Plugin dependency is not running",
            ));
        }
        self.set_state(plugin, PluginState::Starting);
        if descriptor.surface.is_some()
            && let Err(error) = self.create_surface(plugin)
        {
            self.set_state(plugin, PluginState::Failed);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
                self.sync_plugin_flags();
            }
            return Err(error);
        }
        let result = descriptor.start.map_or(Ok(()), |start| start(self));
        if let Err(error) = result {
            self.queue_surface_destroy(plugin);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
                self.sync_plugin_flags();
            }
            self.set_state(plugin, PluginState::Failed);
            return Err(error);
        }
        self.started.push(plugin);
        self.set_state(plugin, PluginState::Running);
        self.sync_plugin_flags();
        Ok(())
    }

    pub(super) fn set_plugin_enabled(
        &mut self,
        plugin: PluginId,
        enabled: bool,
        persist: impl FnOnce(&PluginConfig) -> Result<()>,
    ) -> Result<()> {
        if !user_can_toggle(plugin) {
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "This Otto plugin cannot be turned off",
            ));
        }
        if enabled == self.config.is_enabled(plugin) {
            return Ok(());
        }
        if enabled {
            self.enable_plugin(plugin, persist)
        } else {
            self.disable_plugin(plugin, persist)
        }
    }

    pub(super) fn enable_plugin(
        &mut self,
        plugin: PluginId,
        persist: impl FnOnce(&PluginConfig) -> Result<()>,
    ) -> Result<()> {
        self.config.set_enabled(plugin, true);
        if let Err(error) = self.start_plugin(plugin) {
            self.config.set_enabled(plugin, false);
            self.sync_plugin_flags();
            return Err(error);
        }
        if let Err(error) = persist(&self.config) {
            self.stop_plugin(plugin);
            self.config.set_enabled(plugin, false);
            self.sync_plugin_flags();
            return Err(error);
        }
        self.start_dependents(plugin);
        self.sync_plugin_flags();
        Ok(())
    }

    pub(super) fn start_dependents(&mut self, plugin: PluginId) {
        let mut restarted = vec![plugin];
        for descriptor in PLUGIN_DESCRIPTORS {
            let depends = descriptor
                .dependencies
                .iter()
                .any(|dependency| restarted.contains(dependency));
            if !depends
                || !self.config.is_enabled(descriptor.id)
                || self.started.contains(&descriptor.id)
            {
                continue;
            }
            match self.start_plugin(descriptor.id) {
                Ok(()) => restarted.push(descriptor.id),
                Err(error) => report_plugin_failure(descriptor.id, &error),
            }
        }
    }

    pub(super) fn dependents_first(&self, plugin: PluginId) -> Vec<PluginId> {
        let mut chain = vec![plugin];
        for candidate in &self.started {
            if descriptor(*candidate)
                .dependencies
                .iter()
                .any(|dependency| chain.contains(dependency))
            {
                chain.push(*candidate);
            }
        }
        chain.reverse();
        chain
    }

    pub(super) fn disable_plugin(
        &mut self,
        plugin: PluginId,
        persist: impl FnOnce(&PluginConfig) -> Result<()>,
    ) -> Result<()> {
        if plugin == PluginId::TopBar && !self.recovery_hotkeys {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Ctrl+Alt+B or Ctrl+Alt+D is taken by another app, so the Top Bar cannot be turned off safely",
            ));
        }
        self.config.set_enabled(plugin, false);
        if let Err(error) = persist(&self.config) {
            self.config.set_enabled(plugin, true);
            self.sync_plugin_flags();
            return Err(error);
        }
        for stopping in self.dependents_first(plugin) {
            self.stop_plugin(stopping);
        }
        self.sync_plugin_flags();
        Ok(())
    }

    pub(super) fn create_surface(&mut self, plugin: PluginId) -> Result<HWND> {
        if self.surfaces.is_none() {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surfaces are not initialized",
            ));
        }
        let Some(descriptor) = descriptor(plugin).surface else {
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Plugin does not own a dynamic surface",
            ));
        };
        #[cfg(test)]
        if self.instance.is_invalid() {
            let fake = HWND(100 as _);
            if let Some(surfaces) = self.surfaces.as_mut() {
                surfaces.store(descriptor.slot, fake)?;
                return Ok(fake);
            }
        }
        let base_ex_style = if descriptor.topmost {
            self.ex_style
        } else {
            WINDOW_EX_STYLE(self.ex_style.0 & !WS_EX_TOPMOST.0)
        };
        let ex_style = match descriptor.slot {
            SurfaceSlot::Genie => WINDOW_EX_STYLE(
                (base_ex_style.0 & !WS_EX_LAYERED.0)
                    | WS_EX_NOREDIRECTIONBITMAP.0
                    | WS_EX_TRANSPARENT.0,
            ),
            SurfaceSlot::Spotlight => WINDOW_EX_STYLE(base_ex_style.0 & !WS_EX_NOACTIVATE.0),
            _ => base_ex_style,
        };
        let surface = create_overlay_window(
            self.instance,
            self.cursor,
            descriptor.class_name,
            descriptor.wnd_proc,
            ex_style,
        )?;
        let surfaces = self.surfaces.as_mut().ok_or_else(|| {
            Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surfaces are not initialized",
            )
        })?;
        if let Err(error) = surfaces.store(descriptor.slot, surface) {
            unsafe {
                let _ = DestroyWindow(surface);
            }
            return Err(error);
        }
        match descriptor.slot {
            SurfaceSlot::Dock => self.runtime.dock_hwnd.set(surface.0 as isize),
            SurfaceSlot::WindowControls => {
                self.runtime.window_controls_hwnd.set(surface.0 as isize);
            }
            _ => {}
        }
        Ok(surface)
    }

    pub(super) fn queue_surface_destroy(&mut self, plugin: PluginId) {
        let Some(descriptor) = descriptor(plugin).surface else {
            return;
        };
        match descriptor.slot {
            SurfaceSlot::Dock => self.runtime.dock_hwnd.set(0),
            SurfaceSlot::WindowControls => self.runtime.window_controls_hwnd.set(0),
            _ => {}
        }
        let Some(surfaces) = self.surfaces.as_mut() else {
            return;
        };
        if let Some(surface) = surfaces.take(descriptor.slot) {
            #[cfg(test)]
            if self.instance.is_invalid() {
                return;
            }
            unsafe {
                let _ = ShowWindow(surface, SW_HIDE);
            }
            self.runtime.queue_destroy_hwnd(surface.0 as isize);
        }
    }

    pub(super) fn stop_plugin(&mut self, plugin: PluginId) {
        if !self.started.contains(&plugin) {
            return;
        }
        self.stop_started_plugin(plugin);
    }

    pub(super) fn stop_started_plugin(&mut self, plugin: PluginId) {
        self.set_state(plugin, PluginState::Stopping);
        (descriptor(plugin).stop)(&self.runtime);
        self.started.retain(|started| *started != plugin);
        self.queue_surface_destroy(plugin);
        self.set_state(plugin, PluginState::Disabled);
        self.sync_plugin_flags();
    }

    pub(super) fn shutdown(&mut self) -> Vec<HWND> {
        self.shutting_down = true;
        let started = std::mem::take(&mut self.started);
        for plugin in reverse_lifecycle_order(&started) {
            self.stop_started_plugin(plugin);
        }
        let allow_real_hwnd = {
            #[cfg(test)]
            {
                !self.instance.is_invalid()
            }
            #[cfg(not(test))]
            {
                true
            }
        };
        let settings_hwnd = settings_window::take_hwnd(&self.runtime);
        if allow_real_hwnd {
            self.runtime.queue_destroy_hwnd(settings_hwnd);
        }
        bar::stop(&self.runtime);
        self.tray_icon = None;
        let bar_hwnd = self.runtime.bar_hwnd.get();
        if allow_real_hwnd && bar_hwnd != 0 {
            otto_platform::unregister_notch_hotkeys(bar_hwnd);
            otto_platform::unregister_settings_hotkey(bar_hwnd);
            otto_platform::unregister_shell_hotkeys(bar_hwnd);
        }
        let _ = restore_hidden_windows();
        shutdown_background_tasks();
        if let Some(surfaces) = self.surfaces.take()
            && allow_real_hwnd
        {
            self.runtime.queue_destroy_hwnd(surfaces.menu.0 as isize);
            self.runtime.queue_destroy_hwnd(surfaces.panel.0 as isize);
            self.runtime.queue_destroy_hwnd(surfaces.bar.0 as isize);
        }
        for (_, state) in &mut self.states {
            *state = PluginState::Disabled;
        }
        self.runtime.take_pending_destroy_hwnds()
    }
}
