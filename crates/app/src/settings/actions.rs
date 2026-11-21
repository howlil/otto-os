use otto_core::{
    ClipboardPayload, ControlCenterPort, FocusActivityState, OttoSettingKind, SettingsAction,
    SettingsHit, UPDATE_CHECKING_LABEL, normalize_domain,
};
use otto_platform::{
    ClipboardRead, WindowsControlCenter, pick_blocked_app_executable, read_clipboard_payload,
    read_launch_at_login, save_focus_activity, signal_command_failure,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

use crate::host::RuntimeContext;
use crate::win_msg::UPDATE_TIMER_ID;

use super::lifecycle::repaint;
use super::{refresh_model, with_window};

const UPDATE_POLL_MS: u32 = 500;

fn start_update_check(runtime: &RuntimeContext) {
    let port = WindowsControlCenter;
    let (status, poll) = if port.start_update_check() {
        (Some(String::from(UPDATE_CHECKING_LABEL)), true)
    } else if let Some(cached) = port.poll_update_status() {
        (Some(cached), false)
    } else {
        (port.check_for_updates(), false)
    };
    let hwnd = with_window(runtime, |window| {
        window.update_status = status;
        window.hwnd
    });
    if let (Some(hwnd), true) = (hwnd, poll) {
        unsafe {
            SetTimer(
                Some(HWND(hwnd as *mut _)),
                UPDATE_TIMER_ID,
                UPDATE_POLL_MS,
                None,
            );
        }
    }
}

pub(super) fn poll_update_check(runtime: &RuntimeContext, hwnd: isize) {
    let finished = WindowsControlCenter
        .poll_update_status()
        .filter(|status| status != UPDATE_CHECKING_LABEL);
    let Some(status) = finished else {
        return;
    };
    unsafe {
        let _ = KillTimer(Some(HWND(hwnd as *mut _)), UPDATE_TIMER_ID);
    }
    with_window(runtime, |window| window.update_status = Some(status));
    repaint(runtime);
}

fn update_focus_activity(
    runtime: &RuntimeContext,
    mutate: impl FnOnce(&mut FocusActivityState) -> bool,
) {
    let updated = runtime.focus_activity.try_borrow_mut().is_ok_and(|mut fa| {
        let changed = mutate(&mut fa);
        if changed {
            if save_focus_activity(&fa) {
                fa.mark_saved();
            } else {
                signal_command_failure();
            }
        }
        changed
    });
    if updated {
        crate::bar::sync_focus_blocklist(runtime);
    }
}

fn apply_action(runtime: &RuntimeContext, action: SettingsAction) {
    match action {
        SettingsAction::Toggle(kind) => {
            super::state::toggle_setting(runtime, kind);
            if kind == OttoSettingKind::LaunchAtLogin {
                with_window(runtime, |window| {
                    window.launch_at_login = read_launch_at_login();
                });
            }
        }
        SettingsAction::CycleDockIconSize => super::state::cycle_dock_icon_size(runtime),
        SettingsAction::CycleMenuBarHide => super::state::cycle_menu_bar_hide(runtime),
        SettingsAction::Open(page) => {
            with_window(runtime, |window| window.select_page(page));
            refresh_model(runtime);
        }
        SettingsAction::OpenLogs => {
            let _ = otto_platform::open_logs_folder();
        }
        SettingsAction::CheckForUpdates => start_update_check(runtime),
        SettingsAction::OpenReleases => WindowsControlCenter.open_releases_page(),
        SettingsAction::ToggleFocusBlockNotifications => {
            update_focus_activity(runtime, |fa| {
                fa.blocklist.block_notifications = !fa.blocklist.block_notifications;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleFocusAllowCalls => {
            update_focus_activity(runtime, |fa| {
                fa.blocklist.allow_calls_during_focus = !fa.blocklist.allow_calls_during_focus;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleBlockedApp(app_key) => {
            update_focus_activity(runtime, |fa| {
                let _ = fa.blocklist.toggle_app(&app_key);
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleBlockedWebsite(domain) => {
            update_focus_activity(runtime, |fa| {
                let _ = fa.blocklist.toggle_website(&domain);
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::PickBlockedAppFile => {
            let owner = with_window(runtime, |window| window.hwnd).unwrap_or(0);
            if let Some(app_key) = pick_blocked_app_executable(owner) {
                update_focus_activity(runtime, |fa| {
                    let added = fa.blocklist.add_app(&app_key);
                    if added {
                        fa.mark_dirty();
                    }
                    added
                });
            }
        }
        SettingsAction::AddBlockedWebsiteFromClipboard => {
            let domain = match read_clipboard_payload() {
                ClipboardRead::Captured {
                    payload: ClipboardPayload::Text(text) | ClipboardPayload::Link(text),
                    ..
                } => normalize_domain(&text),
                _ => None,
            };
            if let Some(domain) = domain {
                update_focus_activity(runtime, |fa| {
                    let added = fa.blocklist.add_website(&domain);
                    if added {
                        fa.mark_dirty();
                    }
                    added
                });
            } else {
                signal_command_failure();
            }
        }
        SettingsAction::ResetFocusActivityStats => {
            update_focus_activity(runtime, |fa| {
                fa.reset_stats();
                true
            });
        }
    }
}

pub(super) fn activate(runtime: &RuntimeContext, hit: SettingsHit) {
    match hit {
        SettingsHit::Page(page) => {
            with_window(runtime, |window| window.select_page(page));
            refresh_model(runtime);
        }
        SettingsHit::Row { section, row } => {
            let action = with_window(runtime, |window| {
                window
                    .content
                    .row(section, row)
                    .and_then(|row| row.action.clone())
            })
            .flatten();
            if let Some(action) = action {
                apply_action(runtime, action);
            }
        }
        SettingsHit::Content => {}
    }
    repaint(runtime);
}
