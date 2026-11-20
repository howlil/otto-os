use std::ffi::c_void;
use std::thread;
use std::time::Duration;

use otto_core::{
    CMD_APP_ACTIVATE, CMD_APP_HIDE, CMD_APP_HIDE_OTHERS, CMD_APP_QUIT, CMD_APP_SETTINGS,
    CMD_APP_SHOW_ALL, CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY,
    CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND,
    CMD_CC_WIFI, CMD_DOCK_WINDOW_BASE, CMD_EDIT_COPY, CMD_EDIT_CUT, CMD_EDIT_FIND, CMD_EDIT_PASTE,
    CMD_EDIT_REDO, CMD_EDIT_SELECT_ALL, CMD_EDIT_UNDO, CMD_FILE_NEW, CMD_FILE_NEW_FOLDER,
    CMD_FILE_NEW_TAB, CMD_FILE_OPEN, CMD_FILE_SAVE, CMD_FILE_SAVE_AS, CMD_HELP_F1, CMD_LOCK,
    CMD_QUIT, CMD_RESTART, CMD_SETTINGS, CMD_SHOW_DESKTOP, CMD_SHUTDOWN, CMD_SLEEP, CMD_START,
    CMD_TASK_VIEW, CMD_TASKMGR, CMD_TILE_LEFT, CMD_TILE_RIGHT, CMD_VIEW_ACTUAL_SIZE,
    CMD_VIEW_AS_DETAILS, CMD_VIEW_AS_ICONS, CMD_VIEW_AS_LIST, CMD_VIEW_FULLSCREEN,
    CMD_VIEW_ZOOM_IN, CMD_VIEW_ZOOM_OUT, CMD_WIN_BRING_ALL_TO_FRONT, CMD_WIN_CLOSE,
    CMD_WIN_MAXIMIZE, CMD_WIN_MINIMIZE, CommandOutcome, WindowCommand, WindowIdentity,
    WindowMenuState,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_F1, VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PLUS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IDYES, IsWindow, MB_DEFBUTTON2, MB_ICONWARNING, MB_SETFOREGROUND,
    MB_TOPMOST, MB_YESNO, MessageBoxW, SetForegroundWindow,
};
use windows::core::{PCWSTR, w};

use crate::notch::media::{MediaCommand, send_media_command};
use crate::window::model::{
    application_of, execute_window_command, hide_application, hide_other_applications,
    inspect_window, quit_application, show_application,
};

use super::hotkeys::vk_char;
use super::keys::send_key_chord;
use super::shell::open_calendar_notifications;
use super::shell::open_start_menu;
use super::shell::open_task_view;
use super::shell::shell_exec;
use super::system::signal_command_failure;

pub(super) const FOCUS_POLL_ATTEMPTS: u32 = 3;
pub(super) const FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) fn report_media(accepted: bool) {
    if !accepted {
        signal_command_failure();
    }
}

pub fn execute_control_center_command(cmd: usize) {
    match cmd {
        CMD_CC_MEDIA_TOGGLE => report_media(send_media_command(MediaCommand::TogglePlayPause)),
        CMD_CC_MEDIA_NEXT => report_media(send_media_command(MediaCommand::Next)),
        CMD_CC_MEDIA_PREVIOUS => report_media(send_media_command(MediaCommand::Previous)),
        CMD_CC_CALENDAR => open_calendar_notifications(),
        CMD_CAL_SETTINGS => shell_exec(w!("ms-settings:dateandtime"), PCWSTR::null()),
        CMD_CC_WIFI => shell_exec(w!("ms-settings:network-wifi"), PCWSTR::null()),
        CMD_CC_BLUETOOTH => shell_exec(w!("ms-settings:bluetooth"), PCWSTR::null()),
        CMD_CC_FOCUS => shell_exec(w!("ms-settings:quiethours"), PCWSTR::null()),
        CMD_CC_DISPLAY => shell_exec(w!("ms-settings:display"), PCWSTR::null()),
        CMD_CC_SOUND => shell_exec(w!("ms-settings:sound"), PCWSTR::null()),
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuCommandResult {
    pub outcome: CommandOutcome,
    pub quit_requested: bool,
}

impl MenuCommandResult {
    pub(super) const fn of(outcome: CommandOutcome) -> Self {
        Self {
            outcome,
            quit_requested: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PowerAction {
    Restart,
    ShutDown,
}

pub(super) fn confirm_power_action(action: PowerAction) -> bool {
    let (text, title) = match action {
        PowerAction::Restart => (
            w!("Restart this computer now? Unsaved work in open applications may be lost."),
            w!("Restart"),
        ),
        PowerAction::ShutDown => (
            w!("Shut down this computer now? Unsaved work in open applications may be lost."),
            w!("Shut Down"),
        ),
    };
    let flags = MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2 | MB_TOPMOST | MB_SETFOREGROUND;
    unsafe { MessageBoxW(None, text, title, flags) == IDYES }
}

pub(super) fn run_power_action(action: PowerAction) -> CommandOutcome {
    if !confirm_power_action(action) {
        return CommandOutcome::Cancelled;
    }
    let arguments = match action {
        PowerAction::Restart => w!("/r /t 0"),
        PowerAction::ShutDown => w!("/s /t 0"),
    };
    shell_exec(w!("shutdown.exe"), arguments);
    CommandOutcome::Completed
}

pub(super) fn execute_system_menu_command(cmd: usize) -> Option<MenuCommandResult> {
    let outcome = match cmd {
        CMD_START => {
            open_start_menu();
            CommandOutcome::Completed
        }
        CMD_SETTINGS => {
            shell_exec(w!("ms-settings:"), PCWSTR::null());
            CommandOutcome::Completed
        }
        CMD_TASKMGR => {
            shell_exec(w!("taskmgr.exe"), PCWSTR::null());
            CommandOutcome::Completed
        }
        CMD_LOCK => {
            send_key_chord(&[VK_LWIN, vk_char(b'L')]);
            CommandOutcome::Completed
        }
        CMD_SLEEP => {
            if unsafe { SetSuspendState(false, false, false) } {
                CommandOutcome::Completed
            } else {
                CommandOutcome::Denied
            }
        }
        CMD_RESTART => run_power_action(PowerAction::Restart),
        CMD_SHUTDOWN => run_power_action(PowerAction::ShutDown),
        CMD_SHOW_DESKTOP => {
            send_key_chord(&[VK_LWIN, vk_char(b'D')]);
            CommandOutcome::Completed
        }
        CMD_TASK_VIEW => {
            open_task_view();
            CommandOutcome::Completed
        }
        CMD_QUIT => {
            return Some(MenuCommandResult {
                outcome: CommandOutcome::Completed,
                quit_requested: true,
            });
        }
        _ => return None,
    };
    Some(MenuCommandResult::of(outcome))
}

pub(super) fn window_command_for(cmd: usize) -> Option<WindowCommand> {
    match cmd {
        CMD_WIN_MINIMIZE => Some(WindowCommand::Minimize),
        CMD_WIN_MAXIMIZE => Some(WindowCommand::Zoom),
        CMD_WIN_CLOSE => Some(WindowCommand::Close),
        CMD_TILE_LEFT => Some(WindowCommand::TileLeft),
        CMD_TILE_RIGHT => Some(WindowCommand::TileRight),
        CMD_VIEW_FULLSCREEN => Some(WindowCommand::ToggleFullScreen),
        _ => None,
    }
}

pub(super) fn shortcut_for(cmd: usize) -> Option<Vec<VIRTUAL_KEY>> {
    let control = |key: VIRTUAL_KEY| vec![VK_LCONTROL, key];
    let control_shift = |key: VIRTUAL_KEY| vec![VK_LCONTROL, VK_LSHIFT, key];
    Some(match cmd {
        CMD_APP_SETTINGS => control(VK_OEM_COMMA),
        CMD_FILE_NEW => control(vk_char(b'N')),
        CMD_FILE_NEW_TAB => control(vk_char(b'T')),
        CMD_FILE_NEW_FOLDER => control_shift(vk_char(b'N')),
        CMD_FILE_OPEN => control(vk_char(b'O')),
        CMD_FILE_SAVE => control(vk_char(b'S')),
        CMD_FILE_SAVE_AS => control_shift(vk_char(b'S')),
        CMD_EDIT_UNDO => control(vk_char(b'Z')),
        CMD_EDIT_REDO => control(vk_char(b'Y')),
        CMD_EDIT_CUT => control(vk_char(b'X')),
        CMD_EDIT_COPY => control(vk_char(b'C')),
        CMD_EDIT_PASTE => control(vk_char(b'V')),
        CMD_EDIT_SELECT_ALL => control(vk_char(b'A')),
        CMD_EDIT_FIND => control(vk_char(b'F')),
        CMD_VIEW_ZOOM_IN => control(VK_OEM_PLUS),
        CMD_VIEW_ZOOM_OUT => control(VK_OEM_MINUS),
        CMD_VIEW_ACTUAL_SIZE => control(vk_char(b'0')),
        CMD_VIEW_AS_ICONS => control_shift(vk_char(b'2')),
        CMD_VIEW_AS_LIST => control_shift(vk_char(b'5')),
        CMD_VIEW_AS_DETAILS => control_shift(vk_char(b'6')),
        CMD_HELP_F1 => vec![VK_F1],
        _ => return None,
    })
}

pub(super) fn send_shortcut_to(
    target: WindowIdentity,
    keys: &[VIRTUAL_KEY],
    own_process_id: u32,
) -> CommandOutcome {
    let Some(window) = inspect_window(target.hwnd, own_process_id)
        .filter(|window| window.identity.is_same_window(&target))
    else {
        return CommandOutcome::TargetGone;
    };
    if let Some(blocked) = CommandOutcome::from_capability(WindowMenuState::of(&window).shortcuts) {
        return blocked;
    }
    if !restore_focus(target.hwnd) {
        return CommandOutcome::TimedOut;
    }
    send_key_chord(keys);
    CommandOutcome::Completed
}

pub(super) fn execute_application_command(
    cmd: usize,
    target: WindowIdentity,
    own_process_id: u32,
) -> Option<CommandOutcome> {
    if !matches!(
        cmd,
        CMD_APP_HIDE | CMD_APP_HIDE_OTHERS | CMD_APP_SHOW_ALL | CMD_APP_QUIT
    ) {
        return None;
    }
    let Some(context) = application_of(target, own_process_id) else {
        return Some(CommandOutcome::TargetGone);
    };
    Some(match cmd {
        CMD_APP_HIDE => hide_application(&context.app, own_process_id),
        CMD_APP_HIDE_OTHERS => hide_other_applications(&context.app.identity, own_process_id),
        CMD_APP_SHOW_ALL => show_application(&context.app, own_process_id),
        _ => quit_application(&context.app, own_process_id),
    })
}

#[must_use]
pub fn execute_top_bar_command(
    cmd: usize,
    target: Option<WindowIdentity>,
    own_process_id: u32,
) -> MenuCommandResult {
    if let Some(result) = execute_system_menu_command(cmd) {
        return result;
    }
    let Some(target) = target else {
        if cmd == CMD_APP_ACTIVATE {
            shell_exec(w!("explorer.exe"), PCWSTR::null());
            return MenuCommandResult::of(CommandOutcome::Completed);
        }
        return MenuCommandResult::of(CommandOutcome::Unsupported);
    };
    let outcome = if cmd == CMD_APP_ACTIVATE {
        execute_window_command(target, WindowCommand::Focus, own_process_id)
    } else if cmd == CMD_WIN_BRING_ALL_TO_FRONT {
        application_of(target, own_process_id).map_or(CommandOutcome::TargetGone, |context| {
            show_application(&context.app, own_process_id)
        })
    } else if (CMD_DOCK_WINDOW_BASE..CMD_DOCK_WINDOW_BASE + 64).contains(&cmd) {
        application_of(target, own_process_id)
            .and_then(|context| {
                context
                    .app
                    .primary_windows()
                    .nth(cmd - CMD_DOCK_WINDOW_BASE)
                    .map(|window| {
                        execute_window_command(
                            window.identity,
                            WindowCommand::Focus,
                            own_process_id,
                        )
                    })
            })
            .unwrap_or(CommandOutcome::TargetGone)
    } else if let Some(outcome) = execute_application_command(cmd, target, own_process_id) {
        outcome
    } else if let Some(command) = window_command_for(cmd) {
        execute_window_command(target, command, own_process_id)
    } else if let Some(keys) = shortcut_for(cmd) {
        send_shortcut_to(target, &keys, own_process_id)
    } else {
        CommandOutcome::Unsupported
    };
    MenuCommandResult::of(outcome)
}

pub(super) fn restore_focus(target_hwnd: isize) -> bool {
    if target_hwnd == 0 {
        return false;
    }
    let hwnd = HWND(target_hwnd as *mut c_void);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return false;
        }
        let _ = SetForegroundWindow(hwnd);
    }
    for _ in 0..FOCUS_POLL_ATTEMPTS {
        if unsafe { GetForegroundWindow() } == hwnd {
            return true;
        }
        thread::sleep(FOCUS_POLL_INTERVAL);
    }
    unsafe { GetForegroundWindow() == hwnd }
}
