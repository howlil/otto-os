use crate::host::RuntimeContext;
use otto_core::{
    CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_SETTINGS, CMD_CAL_TODAY, CMD_CONTROL_CENTER,
    CMD_OPEN_LOGS, CMD_OTTO_SETTINGS, CMD_QUIT, CMD_TOGGLE_DOCK, CMD_TOGGLE_NOTCH,
    CMD_TOGGLE_TOP_BAR, Capability, CommandOutcome, ControlCenterAction, ControlCenterPage,
    DownloadsEffect, WindowCommand, WindowInfo, WindowMenuEffect, dock_overflow_choice,
    dock_settings_effect, downloads_effect, overflow_menu_target, shift_month, window_menu_effect,
};
use otto_platform::{
    current_process_id, execute_top_bar_command, open_downloads_folder, open_path,
    signal_command_failure, window_command_capability,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

use super::MenuSource;
use super::open::open_calendar;
use super::open::open_calendar_month;
use super::with_menu;

pub(super) fn report(outcome: CommandOutcome) {
    if !matches!(
        outcome,
        CommandOutcome::Completed | CommandOutcome::Cancelled
    ) {
        signal_command_failure();
    }
}

pub(super) fn execute_window_menu_command(
    runtime: &RuntimeContext,
    cmd: usize,
    window: Option<&WindowInfo>,
) -> bool {
    let own_pid = current_process_id();
    let minimize_available = window.is_some_and(|window| {
        window_command_capability(window.identity, WindowCommand::Minimize, own_pid)
            == Some(Capability::Available)
    });
    match window_menu_effect(cmd, minimize_available) {
        WindowMenuEffect::GenieMinimize => {
            if let Some(window) = window {
                let _ = crate::genie::minimize_window(runtime, window.identity.hwnd);
            }
            false
        }
        WindowMenuEffect::OpenOttoSettings => {
            crate::settings_window::open(runtime, None);
            false
        }
        WindowMenuEffect::OpenLogs => {
            let _ = otto_platform::open_logs_folder();
            false
        }
        WindowMenuEffect::Native(cmd) => {
            let result =
                execute_top_bar_command(cmd, window.map(|window| window.identity), own_pid);
            report(result.outcome);
            result.quit_requested
        }
    }
}

pub(super) fn dispatch_tray_selection(runtime: &RuntimeContext, from_tray: bool, cmd: usize) {
    match cmd {
        CMD_OTTO_SETTINGS => crate::settings_window::open(runtime, None),
        CMD_CONTROL_CENTER => crate::control_center::toggle_from_host(
            runtime,
            ControlCenterPage::Main,
            false,
            from_tray,
        ),
        CMD_TOGGLE_TOP_BAR => {
            runtime.queue_plugin_toggle(
                crate::host::PluginId::TopBar,
                !runtime.top_bar_enabled.get(),
            );
        }
        CMD_TOGGLE_NOTCH => {
            runtime
                .pending_notch_enabled
                .set(Some(!runtime.notch_enabled.get()));
        }
        CMD_TOGGLE_DOCK => {
            runtime.queue_plugin_toggle(crate::host::PluginId::Dock, !runtime.dock_enabled.get());
        }
        CMD_OPEN_LOGS => {
            let _ = otto_platform::open_logs_folder();
        }
        CMD_QUIT => {
            let bar_hwnd = runtime.bar_hwnd.get();
            if bar_hwnd != 0 {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(bar_hwnd as *mut _)),
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                    )
                };
            }
        }
        _ => {}
    }
}

pub(super) fn dispatch_selection(runtime: &RuntimeContext, source: MenuSource, cmd: usize) {
    match source {
        MenuSource::TopBar {
            bar_hwnd, window, ..
        } => {
            if execute_window_menu_command(runtime, cmd, window.as_ref()) {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(bar_hwnd as *mut _)),
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                    )
                };
            }
        }
        MenuSource::Overflow => {
            if let Some(target) = overflow_menu_target(cmd) {
                crate::bar::activate_hidden_target(runtime, target);
            }
        }
        MenuSource::Dock { entry, target_rect } => {
            crate::dock::execute_dock_menu_command(runtime, cmd, &entry, target_rect);
        }
        MenuSource::Calendar {
            year,
            month,
            anchor_right,
            anchor_y,
        } => match cmd {
            CMD_CAL_PREVIOUS | CMD_CAL_NEXT => {
                let delta = if cmd == CMD_CAL_NEXT { 1 } else { -1 };
                let (next_year, next_month) = shift_month(year, month, delta);
                open_calendar_month(runtime, next_year, next_month, anchor_right, anchor_y);
            }
            CMD_CAL_TODAY => open_calendar(runtime, anchor_right, anchor_y),
            CMD_CAL_SETTINGS => with_menu(runtime, |menu| {
                menu.control_center
                    .execute(ControlCenterAction::OpenCalendarSettings);
            }),
            _ => {}
        },
        MenuSource::WindowControls { window } => {
            let _ = execute_window_menu_command(runtime, cmd, Some(&window));
        }
        MenuSource::DockSettings => {
            if cmd == CMD_OTTO_SETTINGS {
                crate::settings_window::open(runtime, None);
            } else if let Some(change) = dock_settings_effect(cmd) {
                let next = runtime.dock_preferences.get().apply(change);
                runtime.pending_dock_preferences.set(Some(next));
            }
        }
        MenuSource::Tray { from_tray } => dispatch_tray_selection(runtime, from_tray, cmd),
        MenuSource::DockOverflow { entries } => {
            if let Some(index) = dock_overflow_choice(cmd, entries.len()) {
                crate::dock::activate_overflow_entry(runtime, &entries[index]);
            }
        }
        MenuSource::DownloadsStack { items } => match downloads_effect(cmd, items.len()) {
            Some(DownloadsEffect::OpenFolder) => open_downloads_folder(),
            Some(DownloadsEffect::OpenItem(index)) => open_path(&items[index].path),
            None => {}
        },
    }
}
