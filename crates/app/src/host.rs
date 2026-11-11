use std::cell::RefCell;
use std::fs;
use std::mem::size_of;
use std::path::PathBuf;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, HCURSOR, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW,
    TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WNDCLASSEXW, WNDPROC, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result, w};

use crate::{bar, dock, genie, menu, window_controls};
use otto_platform::{replace_file_atomically, shutdown_media_task};

const CONFIG_FILE: &str = "plugins.txt";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PluginId {
    PopupMenu,
    TopBar,
    Dock,
    WindowControls,
    Genie,
    ControlCenter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PluginState {
    Disabled,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Copy)]
struct PluginDescriptor {
    id: PluginId,
    required: bool,
    dependencies: &'static [PluginId],
}

const NO_DEPENDENCIES: &[PluginId] = &[];
const POPUP_DEPENDENCY: &[PluginId] = &[PluginId::PopupMenu];
const WINDOW_CONTROLS_DEPENDENCIES: &[PluginId] = &[PluginId::PopupMenu, PluginId::Genie];

const PLUGIN_DESCRIPTORS: &[PluginDescriptor] = &[
    PluginDescriptor {
        id: PluginId::PopupMenu,
        required: true,
        dependencies: NO_DEPENDENCIES,
    },
    PluginDescriptor {
        id: PluginId::TopBar,
        required: true,
        dependencies: POPUP_DEPENDENCY,
    },
    PluginDescriptor {
        id: PluginId::Genie,
        required: false,
        dependencies: NO_DEPENDENCIES,
    },
    PluginDescriptor {
        id: PluginId::Dock,
        required: false,
        dependencies: POPUP_DEPENDENCY,
    },
    PluginDescriptor {
        id: PluginId::WindowControls,
        required: false,
        dependencies: WINDOW_CONTROLS_DEPENDENCIES,
    },
    PluginDescriptor {
        id: PluginId::ControlCenter,
        required: false,
        dependencies: &[PluginId::PopupMenu, PluginId::TopBar],
    },
];

impl PluginId {
    const fn name(self) -> &'static str {
        match self {
            Self::PopupMenu => "popup_menu",
            Self::TopBar => "top_bar",
            Self::Dock => "dock",
            Self::WindowControls => "window_controls",
            Self::Genie => "genie",
            Self::ControlCenter => "control_center",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "popup_menu" => Some(Self::PopupMenu),
            "top_bar" => Some(Self::TopBar),
            "dock" => Some(Self::Dock),
            "window_controls" | "traffic_light" => Some(Self::WindowControls),
            "genie" => Some(Self::Genie),
            "control_center" => Some(Self::ControlCenter),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct PluginConfig {
    enabled: Vec<PluginId>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: vec![
                PluginId::PopupMenu,
                PluginId::TopBar,
                PluginId::Dock,
                PluginId::WindowControls,
                PluginId::Genie,
                PluginId::ControlCenter,
            ],
        }
    }
}

impl PluginConfig {
    fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&contents)
    }

    fn parse(contents: &str) -> Self {
        let Some(enabled) = contents
            .lines()
            .find_map(|line| line.strip_prefix("enabled="))
        else {
            return Self::default();
        };
        let mut config = Self {
            enabled: Vec::new(),
        };
        for name in enabled.split(',') {
            if let Some(plugin) = PluginId::parse(name)
                && !config.enabled.contains(&plugin)
            {
                config.enabled.push(plugin);
            }
        }
        config.ensure_required();
        config
    }

    fn save(&self) -> Result<()> {
        let Some(path) = config_path() else {
            return Ok(());
        };
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        fs::create_dir_all(parent).map_err(io_error)?;
        let enabled = self
            .enabled
            .iter()
            .map(|plugin| plugin.name())
            .collect::<Vec<_>>()
            .join(",");
        replace_file_atomically(&path, format!("version=1\nenabled={enabled}\n").as_bytes())
    }

    fn ensure_required(&mut self) {
        if !self.enabled.contains(&PluginId::PopupMenu) {
            self.enabled.push(PluginId::PopupMenu);
        }
        if !self.enabled.contains(&PluginId::TopBar) {
            self.enabled.push(PluginId::TopBar);
        }
    }

    fn is_enabled(&self, plugin: PluginId) -> bool {
        self.enabled.contains(&plugin)
    }

    fn set_enabled(&mut self, plugin: PluginId, enabled: bool) {
        if enabled {
            if !self.enabled.contains(&plugin) {
                self.enabled.push(plugin);
            }
        } else {
            self.enabled.retain(|candidate| *candidate != plugin);
        }
        self.ensure_required();
    }
}

struct Surfaces {
    bar: HWND,
    panel: HWND,
    menu: HWND,
    dock: Option<HWND>,
    window_controls: Option<HWND>,
    genie: Option<HWND>,
}

struct Host {
    instance: HINSTANCE,
    cursor: HCURSOR,
    ex_style: WINDOW_EX_STYLE,
    blur: bool,
    config: PluginConfig,
    surfaces: Option<Surfaces>,
    started: Vec<PluginId>,
    states: Vec<(PluginId, PluginState)>,
    shutting_down: bool,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

pub fn start(blur: bool) -> Result<()> {
    let instance: HINSTANCE =
        unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? }.into();
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };
    let mut host = Host {
        instance,
        cursor,
        ex_style: WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
        blur,
        config: PluginConfig::load(),
        surfaces: None,
        started: Vec::new(),
        states: PLUGIN_DESCRIPTORS
            .iter()
            .map(|descriptor| (descriptor.id, PluginState::Disabled))
            .collect(),
        shutting_down: false,
    };
    if let Err(error) = host.initialize() {
        host.shutdown();
        return Err(error);
    }
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    Ok(())
}

pub fn run_message_loop() {
    unsafe {
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

pub fn apply_cli_overrides() -> Result<()> {
    let overrides = std::env::args().filter_map(|arg| {
        if let Some(name) = arg.strip_prefix("--enable-plugin=") {
            return PluginId::parse(name).map(|plugin| (plugin, true));
        }
        arg.strip_prefix("--disable-plugin=")
            .and_then(PluginId::parse)
            .map(|plugin| (plugin, false))
    });
    for (plugin, enabled) in overrides {
        set_plugin_enabled(plugin, enabled)?;
    }
    Ok(())
}

pub fn stop() {
    HOST.with(|cell| {
        if let Some(mut host) = cell.borrow_mut().take() {
            host.shutdown();
        }
    });
}

pub fn is_plugin_enabled(plugin: PluginId) -> bool {
    HOST.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|host| host.config.is_enabled(plugin))
    })
}

pub fn set_plugin_enabled(plugin: PluginId, enabled: bool) -> Result<()> {
    let result = HOST.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(host) = guard.as_mut() else {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto host is not running",
            ));
        };
        host.set_plugin_enabled(plugin, enabled)
    });
    if result.is_ok() && plugin == PluginId::ControlCenter {
        bar::refresh_plugin_layout();
    }
    result
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("otto-os").join(CONFIG_FILE))
}

fn io_error(_error: std::io::Error) -> Error {
    Error::new(
        windows::Win32::Foundation::E_FAIL,
        "Unable to persist Otto plugin configuration",
    )
}

fn descriptor(plugin: PluginId) -> PluginDescriptor {
    PLUGIN_DESCRIPTORS
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == plugin)
        .unwrap_or(PluginDescriptor {
            id: plugin,
            required: false,
            dependencies: NO_DEPENDENCIES,
        })
}

impl Host {
    fn set_state(&mut self, plugin: PluginId, state: PluginState) {
        if let Some((_, current)) = self
            .states
            .iter_mut()
            .find(|(candidate, _)| *candidate == plugin)
        {
            *current = state;
        }
    }

    fn initialize(&mut self) -> Result<()> {
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
        self.surfaces = Some(Surfaces {
            bar,
            panel,
            menu,
            dock: None,
            window_controls: None,
            genie: None,
        });

        menu::start(menu.0 as isize)?;
        self.started.push(PluginId::PopupMenu);
        self.set_state(PluginId::PopupMenu, PluginState::Running);
        bar::start(
            bar.0 as isize,
            panel.0 as isize,
            self.blur,
            self.config.is_enabled(PluginId::ControlCenter),
        )?;
        self.started.push(PluginId::TopBar);
        self.set_state(PluginId::TopBar, PluginState::Running);
        if self.config.is_enabled(PluginId::ControlCenter) {
            self.set_state(PluginId::ControlCenter, PluginState::Running);
        }

        for plugin in [PluginId::Genie, PluginId::Dock, PluginId::WindowControls] {
            if self.config.is_enabled(plugin)
                && let Err(error) = self.start_optional(plugin)
            {
                report_plugin_failure(plugin, &error);
            }
        }
        Ok(())
    }

    fn start_optional(&mut self, plugin: PluginId) -> Result<()> {
        let descriptor = descriptor(plugin);
        if !descriptor
            .dependencies
            .iter()
            .all(|dependency| self.started.contains(dependency))
        {
            self.set_state(plugin, PluginState::Failed);
            self.config.set_enabled(plugin, false);
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Plugin dependency is not running",
            ));
        }
        self.set_state(plugin, PluginState::Starting);
        let surface = match self.create_surface(plugin) {
            Ok(surface) => surface,
            Err(error) => {
                self.set_state(plugin, PluginState::Failed);
                self.config.set_enabled(plugin, false);
                return Err(error);
            }
        };
        let result = match plugin {
            PluginId::Dock => dock::start(surface.0 as isize),
            PluginId::WindowControls => window_controls::start(surface.0 as isize),
            PluginId::Genie => genie::start(surface.0 as isize),
            PluginId::PopupMenu | PluginId::TopBar | PluginId::ControlCenter => Ok(()),
        };
        if let Err(error) = result {
            self.destroy_surface(plugin);
            self.config.set_enabled(plugin, false);
            self.set_state(plugin, PluginState::Failed);
            return Err(error);
        }
        self.started.push(plugin);
        self.set_state(plugin, PluginState::Running);
        Ok(())
    }

    fn set_plugin_enabled(&mut self, plugin: PluginId, enabled: bool) -> Result<()> {
        if descriptor(plugin).required {
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "The Otto top bar is required",
            ));
        }
        if plugin == PluginId::ControlCenter {
            self.config.set_enabled(plugin, enabled);
            if enabled {
                self.set_state(plugin, PluginState::Running);
            } else {
                menu::close();
                self.set_state(plugin, PluginState::Disabled);
            }
            return self.config.save();
        }
        if enabled == self.config.is_enabled(plugin) {
            return Ok(());
        }
        if enabled {
            if plugin == PluginId::WindowControls && !self.config.is_enabled(PluginId::Genie) {
                return Err(Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    "Window Controls require Genie",
                ));
            }
            self.config.set_enabled(plugin, true);
            if let Err(error) = self.start_optional(plugin) {
                self.config.set_enabled(plugin, false);
                return Err(error);
            }
        } else {
            if plugin == PluginId::Genie && self.config.is_enabled(PluginId::WindowControls) {
                return Err(Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    "Disable Window Controls before Genie",
                ));
            }
            self.stop_plugin(plugin);
            self.config.set_enabled(plugin, false);
        }
        self.config.save()
    }

    fn create_surface(&mut self, plugin: PluginId) -> Result<HWND> {
        if self.surfaces.is_none() {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surfaces are not initialized",
            ));
        }
        let surface = match plugin {
            PluginId::Dock => create_overlay_window(
                self.instance,
                self.cursor,
                w!("OttoDockBar"),
                Some(dock_window_proc),
                self.ex_style,
            )?,
            PluginId::WindowControls => create_overlay_window(
                self.instance,
                self.cursor,
                w!("OttoWindowControls"),
                Some(window_controls_window_proc),
                self.ex_style,
            )?,
            PluginId::Genie => create_overlay_window(
                self.instance,
                self.cursor,
                w!("OttoGenie"),
                Some(genie_window_proc),
                self.ex_style,
            )?,
            PluginId::PopupMenu | PluginId::TopBar | PluginId::ControlCenter => {
                return Err(Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    "Plugin does not own a dynamic surface",
                ));
            }
        };
        let surfaces = self.surfaces.as_mut().ok_or_else(|| {
            Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surfaces are not initialized",
            )
        })?;
        match plugin {
            PluginId::Dock => surfaces.dock = Some(surface),
            PluginId::WindowControls => surfaces.window_controls = Some(surface),
            PluginId::Genie => surfaces.genie = Some(surface),
            PluginId::PopupMenu | PluginId::TopBar | PluginId::ControlCenter => {}
        }
        Ok(surface)
    }

    fn destroy_surface(&mut self, plugin: PluginId) {
        let Some(surfaces) = self.surfaces.as_mut() else {
            return;
        };
        let surface = match plugin {
            PluginId::Dock => surfaces.dock.take(),
            PluginId::WindowControls => surfaces.window_controls.take(),
            PluginId::Genie => surfaces.genie.take(),
            PluginId::PopupMenu | PluginId::TopBar | PluginId::ControlCenter => None,
        };
        if let Some(surface) = surface {
            unsafe {
                let _ = DestroyWindow(surface);
            }
        }
    }

    fn stop_plugin(&mut self, plugin: PluginId) {
        self.set_state(plugin, PluginState::Stopping);
        match plugin {
            PluginId::PopupMenu => menu::stop(),
            PluginId::TopBar => bar::stop(),
            PluginId::Dock => dock::stop(),
            PluginId::WindowControls => window_controls::stop(),
            PluginId::Genie => genie::stop(),
            PluginId::ControlCenter => menu::close(),
        }
        self.started.retain(|started| *started != plugin);
        self.destroy_surface(plugin);
        self.set_state(plugin, PluginState::Disabled);
    }

    fn shutdown(&mut self) {
        self.shutting_down = true;
        let started = std::mem::take(&mut self.started);
        for plugin in started.into_iter().rev() {
            self.set_state(plugin, PluginState::Stopping);
            match plugin {
                PluginId::PopupMenu => menu::stop(),
                PluginId::TopBar => bar::stop(),
                PluginId::Dock => dock::stop(),
                PluginId::WindowControls => window_controls::stop(),
                PluginId::Genie => genie::stop(),
                PluginId::ControlCenter => menu::close(),
            }
            self.destroy_surface(plugin);
            self.set_state(plugin, PluginState::Disabled);
        }
        if let Some(surfaces) = self.surfaces.take() {
            unsafe {
                let _ = DestroyWindow(surfaces.menu);
                let _ = DestroyWindow(surfaces.panel);
                let _ = DestroyWindow(surfaces.bar);
            }
        }
        for (_, state) in &mut self.states {
            *state = PluginState::Disabled;
        }
        shutdown_media_task();
    }
}

fn report_plugin_failure(plugin: PluginId, error: &Error) {
    let message = format!("Otto plugin {} failed to start: {error}\0", plugin.name());
    let wide: Vec<u16> = message.encode_utf16().collect();
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(PCWSTR(wide.as_ptr()));
    }
}

fn create_overlay_window(
    instance: HINSTANCE,
    cursor: HCURSOR,
    class_name: PCWSTR,
    wnd_proc: WNDPROC,
    ex_style: WINDOW_EX_STYLE,
) -> Result<HWND> {
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: wnd_proc,
            hInstance: instance,
            hCursor: cursor,
            lpszClassName: class_name,
            ..WNDCLASSEXW::default()
        };
        let _ = RegisterClassExW(&class);
        CreateWindowExW(
            ex_style,
            class_name,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
    }
}

fn should_post_quit(hwnd: isize) -> bool {
    HOST.with(|cell| {
        cell.borrow().as_ref().is_some_and(|host| {
            !host.shutting_down
                && host
                    .surfaces
                    .as_ref()
                    .is_some_and(|surfaces| surfaces.bar.0 as isize == hwnd)
        })
    })
}

unsafe extern "system" fn bar_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(result) = bar::handle_message(hwnd.0 as isize, message, wparam, lparam) {
        return result;
    }
    if message == WM_DESTROY && should_post_quit(hwnd.0 as isize) {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn dock_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(result) = dock::handle_message(hwnd.0 as isize, message, wparam, lparam) {
        return result;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn window_controls_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(result) = window_controls::handle_message(hwnd.0 as isize, message, wparam, lparam)
    {
        return result;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn genie_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(result) = genie::handle_message(hwnd.0 as isize, message, wparam, lparam) {
        return result;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn menu_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(result) = menu::handle_message(hwnd.0 as isize, message, wparam, lparam) {
        return result;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::{PluginConfig, PluginId};

    #[test]
    fn config_keeps_required_top_bar_and_ignores_unknown_plugins() {
        let config = PluginConfig::parse("version=1\nenabled=dock,unknown\n");
        assert!(config.is_enabled(PluginId::TopBar));
        assert!(config.is_enabled(PluginId::Dock));
        assert!(!config.is_enabled(PluginId::WindowControls));
    }

    #[test]
    fn config_defaults_all_plugins_when_enabled_field_is_missing() {
        let config = PluginConfig::parse("version=1\n");
        assert!(config.is_enabled(PluginId::TopBar));
        assert!(config.is_enabled(PluginId::ControlCenter));
    }
}
