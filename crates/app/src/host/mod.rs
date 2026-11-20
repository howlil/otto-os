use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::path::PathBuf;

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, HCURSOR,
    IDC_ARROW, LoadCursorW, MSG, RegisterClassExW, TranslateMessage, WINDOW_EX_STYLE, WNDCLASSEXW,
    WNDPROC, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result};

use crate::{bar, control_center, dock, genie, menu, spotlight, window_controls};
use otto_core::{AppPreferences, DockPreferences};
use otto_platform::resume_background_tasks;

#[cfg(test)]
mod fitness_tests;
mod lifecycle;
mod messages;
mod plugins;
mod runtime;
pub(crate) mod win_msg;

pub(crate) use self::messages::SETTINGS_WND_PROC;
pub(crate) use self::messages::module_status;
use self::plugins::CONFIG_FILE;
use self::plugins::PLUGIN_DESCRIPTORS;
use self::plugins::PluginConfig;
use self::plugins::PluginDescriptor;
pub use self::plugins::PluginId;
use self::plugins::PluginState;
use self::plugins::SurfaceSlot;
pub(crate) use self::runtime::RuntimeContext;
use self::runtime::Surfaces;

struct Host {
    instance: HINSTANCE,
    cursor: HCURSOR,
    ex_style: WINDOW_EX_STYLE,
    blur: bool,
    config: PluginConfig,
    recovery_hotkeys: bool,
    tray_icon: Option<otto_platform::TrayIcon>,
    taskbar_created_message: Option<u32>,
    surfaces: Option<Surfaces>,
    started: Vec<PluginId>,
    states: Vec<(PluginId, PluginState)>,
    shutting_down: bool,
    runtime: RuntimeContext,
}

thread_local! {
    pub(super) static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    pub(super) static MODAL_ACTION_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

struct ModalActionGuard;

impl Drop for ModalActionGuard {
    fn drop(&mut self) {
        MODAL_ACTION_ACTIVE.with(|flag| flag.set(false));
    }
}

fn busy_host_error() -> Error {
    Error::new(windows::Win32::Foundation::E_FAIL, "Otto host is busy")
}

fn not_running_error() -> Error {
    Error::new(
        windows::Win32::Foundation::E_FAIL,
        "Otto host is not running",
    )
}

fn with_host_read(action: impl FnOnce(&Host)) {
    HOST.with(|cell| {
        if let Ok(guard) = cell.try_borrow()
            && let Some(host) = guard.as_ref()
        {
            action(host);
        }
    });
}

fn with_host_mut<T>(action: impl FnOnce(&mut Host) -> Result<T>) -> Result<T> {
    HOST.with(|cell| {
        let mut guard = cell.try_borrow_mut().map_err(|_| busy_host_error())?;
        let Some(host) = guard.as_mut() else {
            return Err(not_running_error());
        };
        action(host)
    })
}

fn destroy_hwnds(hwnds: &[HWND]) {
    for &hwnd in hwnds {
        if !hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

pub fn start(blur: bool) -> Result<()> {
    resume_background_tasks();
    let instance: HINSTANCE =
        unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? }.into();
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };
    let config = PluginConfig::load();
    let online_lyrics_enabled = config.online_lyrics;
    let mut host = Host {
        instance,
        cursor,
        ex_style: WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
        blur,
        config,
        recovery_hotkeys: false,
        tray_icon: None,
        taskbar_created_message: None,
        surfaces: None,
        started: Vec::new(),
        states: PLUGIN_DESCRIPTORS
            .iter()
            .map(|descriptor| (descriptor.id, PluginState::Disabled))
            .collect(),
        shutting_down: false,
        runtime: RuntimeContext::new(online_lyrics_enabled),
    };
    host.runtime
        .system_appearance
        .set(otto_platform::read_system_appearance());
    host.runtime.dock_preferences.set(host.config.dock);
    host.runtime.apply_preferences(host.config.preferences);
    host.runtime.blur_supported.set(blur);
    host.runtime.glass_enabled.set(
        blur && host.config.preferences.liquid_glass
            && host.runtime.system_appearance.get().contrast.is_none(),
    );
    if !host.config.preferences.media_module {
        otto_platform::shutdown_media_task();
    }
    if !host.config.preferences.notch_enabled || !host.config.preferences.privacy_indicators {
        otto_platform::shutdown_privacy_task();
    }
    otto_platform::set_volume_hud_hook_enabled(
        host.config.preferences.notch_enabled && host.config.preferences.notch_hud,
    );
    otto_platform::set_background_cleanup(host.config.preferences.background_cleanup);
    host.sync_plugin_flags();
    if let Err(error) = host.initialize() {
        let hwnds = host.shutdown();
        destroy_hwnds(&hwnds);
        return Err(error);
    }
    let pending_hwnds = host.runtime.take_pending_destroy_hwnds();
    destroy_hwnds(&pending_hwnds);
    if let Some(seen) = host.runtime.pending_onboarding_seen.take() {
        host.config.preferences.onboarding_seen = seen;
        let _ = host.config.save();
        host.runtime.onboarding_seen.set(seen);
    }
    HOST.with(|cell| {
        let mut guard = cell.try_borrow_mut().map_err(|_| busy_host_error())?;
        *guard = Some(host);
        Ok(())
    })
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
    let hwnds = HOST.with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else {
            return Vec::new();
        };
        guard.as_mut().map(Host::shutdown).unwrap_or_default()
    });
    destroy_hwnds(&hwnds);
    let host = HOST.with(|cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut guard| guard.take())
    });
    drop(host);
    otto_render::release_shared_factories();
}

pub fn shutdown_session() {
    otto_platform::uninstall_keyboard_hook();
    stop();
    let _ = otto_platform::restore_shell_from_recovery();
    otto_platform::uninstall_window_event_hooks();
}

pub(crate) fn log_present_error<E: std::fmt::Display>(
    surface: &str,
    result: std::result::Result<(), E>,
) {
    if let Err(error) = result {
        otto_platform::log_error("render", &format!("{surface} present failed: {error}"));
    }
}

pub fn set_plugin_enabled(plugin: PluginId, enabled: bool) -> Result<()> {
    let mut pending_hwnds = Vec::new();
    let result = with_host_mut(|host| {
        let outcome = host.set_plugin_enabled(plugin, enabled, PluginConfig::save);
        pending_hwnds = host.runtime.take_pending_destroy_hwnds();
        outcome
    });
    destroy_hwnds(&pending_hwnds);
    if result.is_ok() {
        match plugin {
            PluginId::ControlCenter => {
                with_host_read(|host| {
                    bar::refresh_plugin_layout(
                        &host.runtime,
                        host.config.is_enabled(PluginId::ControlCenter),
                    );
                });
            }
            PluginId::Dock => {
                with_host_read(|host| {
                    bar::set_dock_taskbar_hidden(
                        &host.runtime,
                        enabled && host.config.dock.hide_taskbar,
                    );
                });
            }
            PluginId::TopBar => {
                with_host_read(|host| {
                    window_controls::sync_bar_height(&host.runtime);
                });
            }
            _ => {}
        }
    }
    result
}

pub fn set_online_lyrics_enabled(enabled: bool) -> Result<()> {
    with_host_mut(|host| {
        let previous = host.config.online_lyrics;
        host.config.online_lyrics = enabled;
        if let Err(error) = host.config.save() {
            host.config.online_lyrics = previous;
            return Err(error);
        }
        host.runtime.online_lyrics_enabled.set(enabled);
        Ok(())
    })
}

pub fn set_dock_preferences(preferences: DockPreferences) -> Result<()> {
    let result = with_host_mut(|host| {
        let previous = host.config.dock;
        host.config.dock = preferences;
        if let Err(error) = host.config.save() {
            host.config.dock = previous;
            return Err(error);
        }
        host.runtime.dock_preferences.set(preferences);
        Ok(())
    });
    if result.is_ok() {
        with_host_read(|host| {
            bar::set_dock_taskbar_hidden(
                &host.runtime,
                host.config.is_enabled(PluginId::Dock) && host.config.dock.hide_taskbar,
            );
        });
    }
    result
}

fn update_preferences(update: impl FnOnce(&mut AppPreferences)) -> Result<AppPreferences> {
    with_host_mut(|host| {
        let previous = host.config.preferences;
        update(&mut host.config.preferences);
        if let Err(error) = host.config.save() {
            host.config.preferences = previous;
            return Err(error);
        }
        let updated = host.config.preferences;
        host.runtime.apply_preferences(updated);
        Ok(updated)
    })
}

fn config_path() -> Option<PathBuf> {
    otto_platform::data_dir().map(|root| root.join(CONFIG_FILE))
}

fn io_error(_error: std::io::Error) -> Error {
    Error::new(
        windows::Win32::Foundation::E_FAIL,
        "Unable to persist Otto plugin configuration",
    )
}

fn descriptor(plugin: PluginId) -> &'static PluginDescriptor {
    PLUGIN_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.id == plugin)
        .expect("every PluginId must have a descriptor")
}

fn dependencies_running(started: &[PluginId], dependencies: &[PluginId]) -> bool {
    dependencies
        .iter()
        .all(|dependency| started.contains(dependency))
}

fn dependencies_enabled(config: &PluginConfig, dependencies: &[PluginId]) -> bool {
    dependencies
        .iter()
        .all(|dependency| config.is_enabled(*dependency))
}

fn user_can_toggle(plugin: PluginId) -> bool {
    plugin == PluginId::TopBar || !descriptor(plugin).required
}

fn reverse_lifecycle_order(started: &[PluginId]) -> impl Iterator<Item = PluginId> + '_ {
    started.iter().rev().copied()
}

fn surface(host: &Host, slot: SurfaceSlot) -> Result<HWND> {
    host.surfaces
        .as_ref()
        .and_then(|surfaces| surfaces.get(slot))
        .ok_or_else(|| {
            Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surface is not initialized",
            )
        })
}

fn start_popup_menu(host: &Host) -> Result<()> {
    menu::start(&host.runtime, surface(host, SurfaceSlot::Menu)?.0 as isize)
}

#[allow(clippy::unnecessary_wraps)]
fn start_top_bar(host: &Host) -> Result<()> {
    bar::set_top_bar_enabled(&host.runtime, true);
    Ok(())
}

fn start_dock(host: &Host) -> Result<()> {
    dock::start(&host.runtime, surface(host, SurfaceSlot::Dock)?.0 as isize)
}

fn start_window_controls(host: &Host) -> Result<()> {
    window_controls::start(
        &host.runtime,
        surface(host, SurfaceSlot::WindowControls)?.0 as isize,
    )
}

fn start_genie(host: &Host) -> Result<()> {
    genie::start(&host.runtime, surface(host, SurfaceSlot::Genie)?.0 as isize)
}

fn start_control_center(host: &Host) -> Result<()> {
    control_center::start(
        &host.runtime,
        surface(host, SurfaceSlot::ControlCenter)?.0 as isize,
    );
    Ok(())
}

fn start_spotlight(host: &Host) -> Result<()> {
    spotlight::start(
        &host.runtime,
        surface(host, SurfaceSlot::Spotlight)?.0 as isize,
    );
    Ok(())
}

fn stop_popup_menu(runtime: &RuntimeContext) {
    menu::stop(runtime);
}

fn stop_top_bar(runtime: &RuntimeContext) {
    menu::close(runtime);
    bar::set_top_bar_enabled(runtime, false);
}

fn stop_dock(runtime: &RuntimeContext) {
    dock::stop(runtime);
}

fn stop_window_controls(runtime: &RuntimeContext) {
    window_controls::stop(runtime);
}

fn stop_genie(runtime: &RuntimeContext) {
    genie::stop(runtime);
}

fn stop_control_center(runtime: &RuntimeContext) {
    control_center::stop(runtime);
}

fn stop_spotlight(runtime: &RuntimeContext) {
    spotlight::stop(runtime);
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
        cell.try_borrow().ok().is_some_and(|guard| {
            guard.as_ref().is_some_and(|host| {
                !host.shutting_down
                    && host
                        .surfaces
                        .as_ref()
                        .is_some_and(|surfaces| surfaces.bar.0 as isize == hwnd)
            })
        })
    })
}

#[cfg(test)]
mod tests;
