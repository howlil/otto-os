use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use otto_core::{
    CMD_APP_ACTIVATE, CMD_APP_HIDE, CMD_APP_HIDE_OTHERS, CMD_APP_QUIT, CMD_APP_SHOW_ALL,
    CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS,
    CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND, CMD_CC_WIFI,
    CMD_EDIT_COPY, CMD_EDIT_CUT, CMD_EDIT_FIND, CMD_EDIT_PASTE, CMD_EDIT_REDO, CMD_EDIT_SELECT_ALL,
    CMD_EDIT_UNDO, CMD_FILE_NEW, CMD_FILE_NEW_FOLDER, CMD_FILE_NEW_TAB, CMD_FILE_OPEN,
    CMD_FILE_SAVE, CMD_FILE_SAVE_AS, CMD_HELP_F1, CMD_LOCK, CMD_QUIT, CMD_RESTART, CMD_SETTINGS,
    CMD_SHOW_DESKTOP, CMD_SHUTDOWN, CMD_SLEEP, CMD_START, CMD_TASK_VIEW, CMD_TASKMGR,
    CMD_TILE_LEFT, CMD_TILE_RIGHT, CMD_VIEW_ACTUAL_SIZE, CMD_VIEW_AS_DETAILS, CMD_VIEW_AS_ICONS,
    CMD_VIEW_AS_LIST, CMD_VIEW_FULLSCREEN, CMD_VIEW_ZOOM_IN, CMD_VIEW_ZOOM_OUT, CMD_WIN_CLOSE,
    CMD_WIN_MAXIMIZE, CMD_WIN_MINIMIZE, CommandOutcome, WindowCommand, WindowIdentity,
    WindowMenuState,
};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::AddFontResourceW;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MOD_ALT, MOD_NOREPEAT, RegisterHotKey, SendInput, UnregisterHotKey,
    VIRTUAL_KEY, VK_DOWN, VK_ESCAPE, VK_F1, VK_LBUTTON, VK_LCONTROL, VK_LSHIFT, VK_LWIN,
    VK_OEM_MINUS, VK_OEM_PLUS, VK_RBUTTON, VK_RETURN, VK_TAB, VK_UP, VK_VOLUME_DOWN, VK_VOLUME_UP,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetForegroundWindow, GetWindowRect, HWND_BROADCAST, HWND_TOPMOST, IDYES, IsWindow,
    IsWindowVisible, MB_DEFBUTTON2, MB_ICONWARNING, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    MessageBoxW, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SendNotifyMessageW, SetForegroundWindow, SetWindowPos, ShowWindow, WM_FONTCHANGE,
};
use windows::core::{PCWSTR, w};

use crate::media::{MediaCommand, send_media_command};
use crate::window_model::{
    application_of, execute_window_command, hide_application, hide_other_applications,
    inspect_window, quit_application, show_application,
};

const FOCUS_POLL_ATTEMPTS: u32 = 3;
const FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub const HOTKEY_TOGGLE_NOTCH: i32 = 1;
pub const HOTKEY_TOGGLE_POMODORO: i32 = 2;

const fn vk_char(ch: u8) -> VIRTUAL_KEY {
    VIRTUAL_KEY(ch as u16)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuNavEvent {
    Up,
    Down,
    Enter,
    Escape,
    Char(char),
}

static LAST_NAV_KEY: AtomicU32 = AtomicU32::new(0);

fn is_key_down(key: i32) -> bool {
    (unsafe { GetAsyncKeyState(key) } as u16 & 0x8000) != 0
}

#[must_use]
pub fn is_escape_down() -> bool {
    is_key_down(i32::from(VK_ESCAPE.0))
}

#[must_use]
pub fn poll_menu_events() -> (bool, Option<MenuNavEvent>) {
    let mouse_down = is_key_down(i32::from(VK_LBUTTON.0)) || is_key_down(i32::from(VK_RBUTTON.0));
    if is_escape_down() {
        return (mouse_down, Some(MenuNavEvent::Escape));
    }
    let (pressed, event) = if is_key_down(i32::from(VK_UP.0)) {
        (u32::from(VK_UP.0), Some(MenuNavEvent::Up))
    } else if is_key_down(i32::from(VK_DOWN.0)) {
        (u32::from(VK_DOWN.0), Some(MenuNavEvent::Down))
    } else if is_key_down(i32::from(VK_RETURN.0)) {
        (u32::from(VK_RETURN.0), Some(MenuNavEvent::Enter))
    } else {
        (b'A'..=b'Z')
            .find(|letter| is_key_down(i32::from(*letter)))
            .map_or((0, None), |letter| {
                (u32::from(letter), Some(MenuNavEvent::Char(letter as char)))
            })
    };
    let previous = LAST_NAV_KEY.swap(pressed, Ordering::Relaxed);
    if pressed != 0 && pressed != previous {
        (mouse_down, event)
    } else {
        (mouse_down, None)
    }
}

pub fn signal_command_failure() {
    unsafe {
        let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(
            windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
        );
    }
}

pub fn trim_working_set() {
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

pub fn register_notch_hotkeys(hwnd: isize) {
    let handle = HWND(hwnd as *mut c_void);
    let modifiers = MOD_ALT | MOD_NOREPEAT;
    unsafe {
        let _ = RegisterHotKey(
            Some(handle),
            HOTKEY_TOGGLE_NOTCH,
            modifiers,
            u32::from(b'N'),
        );
        let _ = RegisterHotKey(
            Some(handle),
            HOTKEY_TOGGLE_POMODORO,
            modifiers,
            u32::from(b'P'),
        );
    }
}

pub fn unregister_notch_hotkeys(hwnd: isize) {
    let handle = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = UnregisterHotKey(Some(handle), HOTKEY_TOGGLE_NOTCH);
        let _ = UnregisterHotKey(Some(handle), HOTKEY_TOGGLE_POMODORO);
    }
}

pub fn play_pomodoro_chime() {
    unsafe {
        let _ = PlaySoundW(
            w!("SystemNotification"),
            None,
            SND_ALIAS | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

pub fn load_apple_fonts() {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let font_dir = PathBuf::from(local_app_data).join("Microsoft\\Windows\\Fonts");
    let mut loaded = false;
    for file in [
        "SF-Pro-Text-Regular.otf",
        "SF-Pro-Text-Medium.otf",
        "SF-Pro-Text-Semibold.otf",
    ] {
        let path = font_dir.join(file);
        if path.exists() {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                if AddFontResourceW(PCWSTR(wide.as_ptr())) > 0 {
                    loaded = true;
                }
            }
        }
    }
    if loaded {
        unsafe {
            let _ = SendNotifyMessageW(HWND_BROADCAST, WM_FONTCHANGE, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn open_start_menu() {
    send_key_chord(&[VK_LWIN]);
}

pub fn open_quick_settings() {
    send_key_chord(&[VK_LWIN, vk_char(b'A')]);
}

pub fn open_calendar_notifications() {
    send_key_chord(&[VK_LWIN, vk_char(b'N')]);
}

pub fn open_spotlight_search() {
    send_key_chord(&[VK_LWIN, vk_char(b'S')]);
}

pub fn open_task_view() {
    send_key_chord(&[VK_LWIN, VK_TAB]);
}

pub fn adjust_volume(wheel_delta: i16) {
    if crate::audio::step_system_volume(wheel_delta).is_some() {
        return;
    }
    if wheel_delta > 0 {
        send_key_chord(&[VK_VOLUME_UP]);
    } else if wheel_delta < 0 {
        send_key_chord(&[VK_VOLUME_DOWN]);
    }
}

#[must_use]
pub fn toggle_tray_overflow(anchor_center_x: i32, bar_height: i32, screen_width: i32) -> bool {
    unsafe {
        let Ok(overflow) = FindWindowW(w!("NotifyIconOverflowWindow"), PCWSTR::null()) else {
            return false;
        };
        if overflow.0.is_null() {
            return false;
        }
        if IsWindowVisible(overflow).as_bool() {
            let _ = ShowWindow(overflow, SW_HIDE);
            return true;
        }
        let _ = ShowWindow(overflow, SW_SHOWNOACTIVATE);
        let mut rect = RECT::default();
        let _ = GetWindowRect(overflow, &mut rect);
        let width = (rect.right - rect.left).max(160);
        let x = (anchor_center_x - width / 2).clamp(8, (screen_width - width - 8).max(8));
        let _ = SetWindowPos(
            overflow,
            Some(HWND_TOPMOST),
            x,
            bar_height + 6,
            0,
            0,
            SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = SetForegroundWindow(overflow);
        true
    }
}

fn report_media(accepted: bool) {
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
    const fn of(outcome: CommandOutcome) -> Self {
        Self {
            outcome,
            quit_requested: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PowerAction {
    Restart,
    ShutDown,
}

fn confirm_power_action(action: PowerAction) -> bool {
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

fn run_power_action(action: PowerAction) -> CommandOutcome {
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

fn execute_system_menu_command(cmd: usize) -> Option<MenuCommandResult> {
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

fn window_command_for(cmd: usize) -> Option<WindowCommand> {
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

fn shortcut_for(cmd: usize) -> Option<Vec<VIRTUAL_KEY>> {
    let control = |key: VIRTUAL_KEY| vec![VK_LCONTROL, key];
    let control_shift = |key: VIRTUAL_KEY| vec![VK_LCONTROL, VK_LSHIFT, key];
    Some(match cmd {
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

fn send_shortcut_to(
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

fn execute_application_command(
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

fn restore_focus(target_hwnd: isize) -> bool {
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

pub fn open_downloads_folder() {
    shell_exec(w!("shell:Downloads"), PCWSTR::null());
}

pub fn open_path(path: &Path) {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    shell_exec(PCWSTR(wide.as_ptr()), PCWSTR::null());
}

fn shell_exec(file: PCWSTR, params: PCWSTR) {
    unsafe {
        let _ = ShellExecuteW(None, w!("open"), file, params, PCWSTR::null(), SW_SHOW);
    }
}

pub(crate) fn send_key_chord(keys: &[VIRTUAL_KEY]) {
    let mut inputs = Vec::with_capacity(keys.len() * 2);
    for &vk in keys {
        inputs.push(key_input(vk, KEYBD_EVENT_FLAGS(0)));
    }
    for &vk in keys.iter().rev() {
        inputs.push(key_input(vk, KEYEVENTF_KEYUP));
    }
    unsafe {
        let _ = SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

const fn key_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{execute_top_bar_command, shortcut_for, window_command_for};
    use otto_core::{
        APPLICATION_MENUS, AppMenuState, CMD_APP_HIDE, CMD_APP_QUIT, CMD_EDIT_PASTE,
        CMD_FILE_NEW_FOLDER, CMD_FILE_NEW_TAB, CMD_SHOW_DESKTOP, CMD_TASK_VIEW,
        CMD_VIEW_ACTUAL_SIZE, CMD_VIEW_AS_ICONS, CMD_VIEW_FULLSCREEN, CMD_WIN_CLOSE, Capability,
        CommandOutcome, MenuContext, MenuRow, WindowCommand, WindowIdentity, WindowMenuState,
        build_top_bar_menu_rows,
    };

    const AVAILABLE_WINDOW: WindowMenuState = WindowMenuState {
        close: Capability::Available,
        minimize: Capability::Available,
        zoom: Capability::Available,
        full_screen: Capability::Available,
        tile: Capability::Available,
        shortcuts: Capability::Available,
        zoomed: false,
        full_screen_active: false,
    };
    const AVAILABLE_APP: AppMenuState = AppMenuState {
        hide: Capability::Available,
        hide_others: Capability::Available,
        show_all: Capability::Available,
        quit: Capability::Available,
    };

    #[test]
    fn every_enabled_application_menu_row_has_an_executor() {
        for file_manager in [false, true] {
            let context = MenuContext {
                app_name: "App",
                file_manager,
                tabbed: true,
                window: Some(AVAILABLE_WINDOW),
                app: AVAILABLE_APP,
            };
            for target in APPLICATION_MENUS {
                for row in build_top_bar_menu_rows(target, &context) {
                    let MenuRow::Item {
                        id, enabled: true, ..
                    } = row
                    else {
                        continue;
                    };
                    let routed = window_command_for(id).is_some()
                        || shortcut_for(id).is_some()
                        || matches!(id, CMD_TASK_VIEW | CMD_SHOW_DESKTOP);
                    assert!(routed, "{target:?} row {id} has no executor");
                }
            }
        }
    }

    #[test]
    fn distinct_labels_route_to_distinct_actions() {
        assert_ne!(
            shortcut_for(CMD_FILE_NEW_FOLDER),
            shortcut_for(CMD_FILE_NEW_TAB)
        );
        assert_ne!(
            shortcut_for(CMD_VIEW_AS_ICONS),
            shortcut_for(CMD_VIEW_ACTUAL_SIZE)
        );
        assert_eq!(
            window_command_for(CMD_VIEW_FULLSCREEN),
            Some(WindowCommand::ToggleFullScreen)
        );
        assert_eq!(
            window_command_for(CMD_WIN_CLOSE),
            Some(WindowCommand::Close)
        );
        assert!(shortcut_for(CMD_WIN_CLOSE).is_none());
        assert!(window_command_for(CMD_APP_QUIT).is_none());
    }

    #[test]
    fn commands_for_a_window_that_no_longer_exists_reach_no_other_application() {
        let gone = WindowIdentity {
            hwnd: 0x7FFF_FFF0,
            process_id: 0xFFFF_FFF0,
            thread_id: 0xFFFF_FFF1,
            generation: 1,
        };
        let own = std::process::id();
        for cmd in [CMD_EDIT_PASTE, CMD_WIN_CLOSE, CMD_APP_QUIT, CMD_APP_HIDE] {
            let result = execute_top_bar_command(cmd, Some(gone), own);
            assert_eq!(result.outcome, CommandOutcome::TargetGone, "command {cmd}");
            assert!(!result.quit_requested);
        }
        let untargeted = execute_top_bar_command(CMD_EDIT_PASTE, None, own);
        assert_eq!(untargeted.outcome, CommandOutcome::Unsupported);
    }
}
