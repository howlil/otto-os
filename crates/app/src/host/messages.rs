use super::HOST;
use super::MODAL_ACTION_ACTIVE;
use super::WND_PROC_CATCH_DEPTH;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, PostQuitMessage, WM_DESTROY, WM_HOTKEY, WNDPROC,
};

use crate::{bar, control_center, dock, genie, menu, settings_window, spotlight, window_controls};
use otto_core::{AppearancePreferences, DockPreferences, MenuBarPreferences};

use super::ModalActionGuard;
use super::plugins::PluginId;
use super::plugins::PluginState;
use super::reclaim_disabled_memory;
use super::runtime::RuntimeContext;
use super::set_dock_preferences;
use super::set_online_lyrics_enabled;
use super::set_plugin_enabled;
use super::should_post_quit;
use super::shutdown_session;
use super::sync_platform_tasks;
use super::update_preferences;
use super::with_host_read;

pub(super) struct PendingPreferences {
    pub(super) capture_visible: Option<bool>,
    pub(super) hotkeys_enabled: Option<bool>,
    pub(super) classic_logo: Option<bool>,
    pub(super) notch_enabled: Option<bool>,
    pub(super) dynamic_island: Option<bool>,
    pub(super) privacy_indicators: Option<bool>,
    pub(super) clipboard_history: Option<bool>,
    pub(super) drop_shelf: Option<bool>,
    pub(super) notch_hud: Option<bool>,
    pub(super) media_module: Option<bool>,
    pub(super) focus_module: Option<bool>,
    pub(super) background_cleanup: Option<bool>,
    pub(super) menu_bar: Option<MenuBarPreferences>,
    pub(super) quick_capture: Option<bool>,
    pub(super) onboarding_seen: Option<bool>,
    pub(super) appearance: Option<AppearancePreferences>,
}

impl PendingPreferences {
    pub(super) fn take_from(runtime: &RuntimeContext) -> Self {
        Self {
            capture_visible: runtime.pending_capture_visible.take(),
            hotkeys_enabled: runtime.pending_hotkeys_enabled.take(),
            classic_logo: runtime.pending_classic_logo.take(),
            notch_enabled: runtime.pending_notch_enabled.take(),
            dynamic_island: runtime.pending_dynamic_island.take(),
            privacy_indicators: runtime.pending_privacy_indicators.take(),
            clipboard_history: runtime.pending_clipboard_history.take(),
            drop_shelf: runtime.pending_drop_shelf.take(),
            notch_hud: runtime.pending_notch_hud.take(),
            media_module: runtime.pending_media_module.take(),
            focus_module: runtime.pending_focus_module.take(),
            background_cleanup: runtime.pending_background_cleanup.take(),
            menu_bar: runtime.pending_menu_bar.take(),
            quick_capture: runtime.pending_quick_capture.take(),
            onboarding_seen: runtime.pending_onboarding_seen.take(),
            appearance: runtime.pending_appearance.take(),
        }
    }

    pub(super) fn has_change(&self) -> bool {
        self.capture_visible.is_some()
            || self.hotkeys_enabled.is_some()
            || self.classic_logo.is_some()
            || self.notch_enabled.is_some()
            || self.dynamic_island.is_some()
            || self.privacy_indicators.is_some()
            || self.clipboard_history.is_some()
            || self.drop_shelf.is_some()
            || self.notch_hud.is_some()
            || self.media_module.is_some()
            || self.focus_module.is_some()
            || self.background_cleanup.is_some()
            || self.menu_bar.is_some()
            || self.quick_capture.is_some()
            || self.onboarding_seen.is_some()
            || self.appearance.is_some()
    }

    pub(super) fn affects_ui(&self) -> bool {
        self.capture_visible.is_some()
            || self.hotkeys_enabled.is_some()
            || self.classic_logo.is_some()
            || self.notch_enabled.is_some()
            || self.dynamic_island.is_some()
            || self.privacy_indicators.is_some()
            || self.clipboard_history.is_some()
            || self.drop_shelf.is_some()
            || self.notch_hud.is_some()
            || self.media_module.is_some()
            || self.focus_module.is_some()
            || self.background_cleanup.is_some()
            || self.menu_bar.is_some()
            || self.quick_capture.is_some()
            || self.appearance.is_some()
    }
}

pub(super) struct PendingDispatch {
    pub(super) online_lyrics: Option<bool>,
    pub(super) dock: Option<DockPreferences>,
    pub(super) plugin_toggles: Vec<(PluginId, bool)>,
    pub(super) preferences: PendingPreferences,
    pub(super) end_session: bool,
    pub(super) modal: Option<bar::PreparedModalAction>,
}

pub(super) fn dispatch_runtime_message(
    action: impl FnOnce(&RuntimeContext) -> Option<LRESULT>,
) -> Option<LRESULT> {
    let (result, pending) = HOST.with(|cell| {
        let guard = cell.try_borrow().ok()?;
        let host = guard.as_ref()?;
        let result = action(&host.runtime);
        let online_lyrics = host.runtime.pending_online_lyrics.take();
        let dock = host.runtime.pending_dock_preferences.take();
        let plugin_toggles = host.runtime.take_plugin_toggles();
        let preferences = PendingPreferences::take_from(&host.runtime);
        let end_session = host.runtime.pending_end_session.take();
        let raw_modal = host
            .runtime
            .pending_modal_action
            .try_borrow_mut()
            .ok()
            .and_then(|mut slot| slot.take());
        let modal = if !host.shutting_down && !end_session && !MODAL_ACTION_ACTIVE.with(Cell::get) {
            raw_modal.and_then(|modal| bar::prepare_modal_action(&host.runtime, modal))
        } else {
            None
        };
        Some((
            result,
            PendingDispatch {
                online_lyrics,
                dock,
                plugin_toggles,
                preferences,
                end_session,
                modal,
            },
        ))
    })?;
    let had_plugin_toggles = !pending.plugin_toggles.is_empty();
    for (plugin, enabled) in pending.plugin_toggles {
        if set_plugin_enabled(plugin, enabled).is_err() {
            otto_platform::signal_command_failure();
        }
    }
    let mut dock_changed = false;
    if let Some(preferences) = pending.dock {
        if set_dock_preferences(preferences).is_ok() {
            dock_changed = true;
            with_host_read(|host| dock::apply_preferences(&host.runtime, preferences));
        } else {
            otto_platform::signal_command_failure();
        }
    }
    let mut lyrics_changed = false;
    if let Some(enabled) = pending.online_lyrics {
        if set_online_lyrics_enabled(enabled).is_ok() {
            lyrics_changed = true;
            with_host_read(|host| bar::set_online_lyrics_enabled(&host.runtime, enabled));
        } else {
            otto_platform::signal_command_failure();
        }
    }
    let prefs_changed = apply_pending_preference_changes(&pending.preferences);
    if pending.preferences.background_cleanup.is_some() {
        with_host_read(|host| settings_window::refresh_process_audit_if_general(&host.runtime));
    }
    if had_plugin_toggles || dock_changed || lyrics_changed || prefs_changed {
        with_host_read(|host| control_center::repaint(&host.runtime));
        with_host_read(|host| settings_window::repaint(&host.runtime));
    }
    if let Some(prepared) = pending.modal {
        MODAL_ACTION_ACTIVE.with(|flag| flag.set(true));
        let _modal_guard = ModalActionGuard;
        let completed = bar::run_prepared_modal_action(prepared);
        with_host_read(|host| {
            if !host.shutting_down {
                bar::finish_modal_action(&host.runtime, completed);
            }
        });
    }
    if pending.end_session {
        shutdown_session();
        unsafe { PostQuitMessage(0) };
    }
    result
}

#[allow(clippy::too_many_lines)]
pub(super) fn apply_pending_preference_changes(pending: &PendingPreferences) -> bool {
    if pending.has_change()
        && let Ok(updated) = update_preferences(|prefs| {
            if let Some(visible) = pending.capture_visible {
                prefs.capture_visible = visible;
            }
            if let Some(enabled) = pending.hotkeys_enabled {
                prefs.hotkeys_enabled = enabled;
            }
            if let Some(classic) = pending.classic_logo {
                prefs.classic_logo = classic;
            }
            if let Some(notch_enabled) = pending.notch_enabled {
                prefs.notch_enabled = notch_enabled;
            }
            if let Some(dynamic_island) = pending.dynamic_island {
                prefs.dynamic_island = dynamic_island;
            }
            if let Some(privacy_indicators) = pending.privacy_indicators {
                prefs.privacy_indicators = privacy_indicators;
            }
            if let Some(clipboard_history) = pending.clipboard_history {
                prefs.clipboard_history = clipboard_history;
            }
            if let Some(drop_shelf) = pending.drop_shelf {
                prefs.drop_shelf = drop_shelf;
            }
            if let Some(notch_hud) = pending.notch_hud {
                prefs.notch_hud = notch_hud;
            }
            if let Some(media_module) = pending.media_module {
                prefs.media_module = media_module;
            }
            if let Some(focus_module) = pending.focus_module {
                prefs.focus_module = focus_module;
            }
            if let Some(background_cleanup) = pending.background_cleanup {
                prefs.background_cleanup = background_cleanup;
            }
            if let Some(menu_bar) = pending.menu_bar {
                prefs.menu_bar = menu_bar;
            }
            if let Some(quick_capture) = pending.quick_capture {
                prefs.quick_capture = quick_capture;
            }
            if let Some(seen) = pending.onboarding_seen {
                prefs.onboarding_seen = seen;
            }
            if let Some(appearance) = pending.appearance {
                prefs.appearance = appearance;
            }
        })
    {
        sync_pending_platform_modules(pending, &updated);
        with_host_read(|host| {
            if pending.capture_visible.is_some() {
                bar::set_capture_visible(&host.runtime, updated.capture_visible);
                dock::set_capture_visible(&host.runtime, updated.capture_visible);
                control_center::set_capture_visible(&host.runtime, updated.capture_visible);
                menu::set_capture_visible(&host.runtime, updated.capture_visible);
                spotlight::set_capture_visible(&host.runtime, updated.capture_visible);
            }
            if pending.hotkeys_enabled.is_some() {
                let bar_hwnd = host.runtime.bar_hwnd.get();
                if bar_hwnd != 0 {
                    if updated.hotkeys_enabled {
                        otto_platform::register_notch_hotkeys(bar_hwnd);
                    } else {
                        otto_platform::unregister_notch_hotkeys(bar_hwnd);
                    }
                }
                bar::set_hotkeys_enabled(&host.runtime, updated.hotkeys_enabled);
            }
            if pending.classic_logo.is_some() {
                bar::set_brand_icon(&host.runtime, updated.brand_icon());
            }
            if pending.notch_enabled.is_some() {
                bar::set_notch_enabled(&host.runtime, updated.notch_enabled);
            }
            if pending.dynamic_island.is_some() {
                let style = updated.notch_style();
                bar::set_notch_style(&host.runtime, style);
                window_controls::set_notch_style(&host.runtime, style);
            }
            if pending.privacy_indicators.is_some() {
                bar::set_privacy_indicators_enabled(&host.runtime, updated.privacy_indicators);
            }
            if pending.clipboard_history.is_some() {
                bar::set_clipboard_history_enabled(&host.runtime, updated.clipboard_history);
            }
            if pending.drop_shelf.is_some() {
                bar::set_drop_shelf_enabled(&host.runtime, updated.drop_shelf);
            }
            if pending.notch_hud.is_some() {
                bar::set_notch_hud_enabled(&host.runtime, updated.notch_hud);
            }
            if pending.media_module.is_some() {
                bar::set_media_module_enabled(&host.runtime, updated.media_module);
            }
            if pending.focus_module.is_some() {
                bar::set_focus_module_enabled(&host.runtime, updated.focus_module);
            }
            if pending.menu_bar.is_some() {
                host.runtime.menu_bar.set(updated.menu_bar);
                bar::set_menu_bar_preferences(&host.runtime, updated.menu_bar);
            }
            if pending.quick_capture.is_some() {
                bar::set_quick_capture_enabled(&host.runtime, updated.quick_capture);
            }
            if pending.appearance.is_some() {
                host.runtime.appearance_preferences.set(updated.appearance);
                crate::theme_state::sync_effective_glass(&host.runtime);
            }
            sync_platform_tasks(&host.config);
            if disabled_any_feature(pending) {
                reclaim_disabled_memory(host);
            }
        });
    }
    pending.affects_ui()
}

fn disabled_any_feature(pending: &PendingPreferences) -> bool {
    pending.notch_enabled == Some(false)
        || pending.privacy_indicators == Some(false)
        || pending.clipboard_history == Some(false)
        || pending.drop_shelf == Some(false)
        || pending.notch_hud == Some(false)
        || pending.media_module == Some(false)
        || pending.focus_module == Some(false)
        || pending.background_cleanup == Some(false)
        || pending.quick_capture == Some(false)
        || pending.menu_bar.is_some_and(|prefs| !prefs.show_background)
        || pending
            .appearance
            .is_some_and(|prefs| !prefs.style.uses_glass())
}

pub(super) fn sync_pending_platform_modules(
    pending: &PendingPreferences,
    updated: &otto_core::AppPreferences,
) {
    if pending.notch_enabled.is_some() || pending.privacy_indicators.is_some() {
        if updated.notch_enabled && updated.privacy_indicators {
            otto_platform::resume_privacy_task();
        } else {
            otto_platform::shutdown_privacy_task();
        }
    }
    if pending.notch_enabled.is_some() || pending.media_module.is_some() {
        if updated.notch_enabled && updated.media_module {
            otto_platform::resume_media_task();
        } else {
            otto_platform::shutdown_media_task();
        }
    }
    if pending.notch_enabled.is_some() || pending.notch_hud.is_some() {
        otto_platform::set_volume_hud_hook_enabled(updated.notch_enabled && updated.notch_hud);
    }
    if pending.background_cleanup.is_some() {
        otto_platform::set_background_cleanup(updated.background_cleanup);
    }
}

struct WndProcCatchGuard;

impl WndProcCatchGuard {
    fn enter() -> Self {
        WND_PROC_CATCH_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        Self
    }
}

impl Drop for WndProcCatchGuard {
    fn drop(&mut self) {
        WND_PROC_CATCH_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

pub(super) fn safe_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    handler: impl FnOnce() -> Option<LRESULT>,
) -> LRESULT {
    let _catch_guard = WndProcCatchGuard::enter();
    let result = catch_unwind(AssertUnwindSafe(handler));
    if let Ok(Some(result)) = result {
        return result;
    }
    if message == WM_DESTROY && should_post_quit(hwnd.0 as isize) {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

pub(super) fn handle_taskbar_created(message: u32) -> bool {
    if message == 0 {
        return false;
    }
    let mut handled = false;
    with_host_read(|host| {
        if host.taskbar_created_message == Some(message) {
            if let Some(tray) = host.tray_icon.as_ref() {
                let _ = tray.re_add();
            }
            dock::on_taskbar_created(&host.runtime);
            handled = true;
        }
    });
    handled
}

pub(super) fn handle_tray_icon_event(
    runtime: &RuntimeContext,
    event: otto_platform::TrayIconEvent,
) {
    match event {
        otto_platform::TrayIconEvent::LeftClick => settings_window::toggle(runtime),
        otto_platform::TrayIconEvent::RightClick => {
            let (cx, cy) = otto_platform::cursor_position();
            menu::open_tray_menu(runtime, cx, cy, true);
        }
    }
}

pub(super) unsafe extern "system" fn bar_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        if handle_taskbar_created(message) {
            return Some(LRESULT(0));
        }
        dispatch_runtime_message(|runtime| {
            if message == otto_platform::WM_APP_TRAY_ICON {
                if let Some(event) = otto_platform::classify_tray_icon_message(lparam.0) {
                    handle_tray_icon_event(runtime, event);
                }
                return Some(LRESULT(0));
            }
            if message == WM_HOTKEY && handle_host_hotkey(runtime, wparam.0 as i32) {
                return Some(LRESULT(0));
            }
            bar::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) fn handle_host_hotkey(runtime: &RuntimeContext, hotkey_id: i32) -> bool {
    match hotkey_id {
        otto_platform::HOTKEY_TOGGLE_TOP_BAR => {
            runtime.queue_plugin_toggle(PluginId::TopBar, !runtime.top_bar_enabled.get());
        }
        otto_platform::HOTKEY_TOGGLE_DOCK => {
            runtime.queue_plugin_toggle(PluginId::Dock, !runtime.dock_enabled.get());
        }
        otto_platform::HOTKEY_OTTO_SETTINGS => {
            bar::leave_keyboard_focus(runtime);
            settings_window::toggle(runtime);
        }
        otto_platform::HOTKEY_CONTROL_CENTER => {
            bar::leave_keyboard_focus(runtime);
            control_center::toggle_from_host(
                runtime,
                otto_core::ControlCenterPage::Main,
                true,
                false,
            );
        }
        otto_platform::HOTKEY_SPOTLIGHT => {
            bar::leave_keyboard_focus(runtime);
            let trigger = bar::search_slot_trigger(runtime);
            spotlight::toggle_from_hotkey(runtime, trigger);
        }
        _ => return false,
    }
    true
}

pub(crate) const SETTINGS_WND_PROC: WNDPROC = Some(settings_window_proc);

pub(crate) fn module_status(id: otto_core::ModuleId) -> Option<otto_core::ModuleStatus> {
    use otto_core::{ModuleId, ModuleStatus};
    let plugin = match id {
        ModuleId::TopBar => PluginId::TopBar,
        ModuleId::Dock => PluginId::Dock,
        ModuleId::WindowControls => PluginId::WindowControls,
        ModuleId::Genie => PluginId::Genie,
        ModuleId::Spotlight => PluginId::Spotlight,
        ModuleId::ControlCenter => PluginId::ControlCenter,
        _ => return None,
    };
    let mut status = None;
    with_host_read(|host| {
        status = host
            .states
            .iter()
            .find(|(candidate, _)| *candidate == plugin)
            .map(|(_, state)| match state {
                PluginState::Running => ModuleStatus::Running,
                PluginState::Starting => ModuleStatus::Starting,
                PluginState::Failed => ModuleStatus::Failed,
                PluginState::Disabled | PluginState::Stopping => ModuleStatus::Off,
            });
    });
    status
}

pub(super) unsafe extern "system" fn settings_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            settings_window::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn dock_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            dock::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn window_controls_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            window_controls::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn genie_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            genie::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn focus_shield_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            crate::focus_shield::handle_message(runtime, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn control_center_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            control_center::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn spotlight_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            spotlight::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

pub(super) unsafe extern "system" fn menu_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            menu::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}
