use otto_core::{
    ControlCenterPort, OttoSettingKind, SettingsAction, SettingsHit, UPDATE_CHECKING_LABEL,
};
use otto_platform::{WindowsControlCenter, read_launch_at_login};
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

fn apply_action(runtime: &RuntimeContext, action: SettingsAction) {
    match action {
        SettingsAction::Toggle(kind) => {
            crate::control_center::toggle_setting(runtime, kind);
            if kind == OttoSettingKind::LaunchAtLogin {
                with_window(runtime, |window| {
                    window.launch_at_login = read_launch_at_login();
                });
            }
        }
        SettingsAction::CycleDockIconSize => crate::control_center::cycle_dock_icon_size(runtime),
        SettingsAction::CycleMenuBarHide => crate::control_center::cycle_menu_bar_hide(runtime),
        SettingsAction::Open(page) => {
            with_window(runtime, |window| window.select_page(page));
            refresh_model(runtime);
        }
        SettingsAction::OpenLogs => {
            let _ = otto_platform::open_logs_folder();
        }
        SettingsAction::CheckForUpdates => start_update_check(runtime),
        SettingsAction::OpenReleases => WindowsControlCenter.open_releases_page(),
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
                window.content.row(section, row).and_then(|row| row.action)
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
