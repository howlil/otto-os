use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use otto_core::{
    CMD_APP_ACTIVATE, CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY,
    CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND,
    CMD_CC_WIFI, CMD_EDIT_COPY, CMD_EDIT_CUT, CMD_EDIT_FIND, CMD_EDIT_PASTE, CMD_EDIT_REDO,
    CMD_EDIT_SELECT_ALL, CMD_EDIT_UNDO, CMD_FILE_NEW, CMD_FILE_NEW_TAB, CMD_FILE_OPEN,
    CMD_FILE_SAVE, CMD_FILE_SAVE_AS, CMD_HELP_F1, CMD_HELP_PALETTE, CMD_LOCK, CMD_QUIT,
    CMD_RESTART, CMD_SETTINGS, CMD_SHOW_DESKTOP, CMD_SHUTDOWN, CMD_SLEEP, CMD_START, CMD_TASK_VIEW,
    CMD_TASKMGR, CMD_TILE_LEFT, CMD_TILE_RIGHT, CMD_VIEW_ACTUAL_SIZE, CMD_VIEW_FULLSCREEN,
    CMD_VIEW_ZOOM_IN, CMD_VIEW_ZOOM_OUT, CMD_WIN_CLOSE, CMD_WIN_MAXIMIZE, CMD_WIN_MINIMIZE,
    HitTarget, TrafficLightAction,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::AddFontResourceW;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MOD_ALT, MOD_NOREPEAT, RegisterHotKey, SendInput, UnregisterHotKey,
    VIRTUAL_KEY, VK_ESCAPE, VK_F1, VK_F11, VK_LBUTTON, VK_LCONTROL, VK_LEFT, VK_LSHIFT, VK_LWIN,
    VK_OEM_MINUS, VK_OEM_PLUS, VK_RBUTTON, VK_RETURN, VK_RIGHT, VK_TAB, VK_VOLUME_DOWN,
    VK_VOLUME_UP,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, FindWindowW, GetCursorPos, GetWindowRect,
    HWND_BROADCAST, HWND_TOPMOST, IsWindowVisible, MF_SEPARATOR, MF_STRING, SW_HIDE, SW_SHOW,
    SW_SHOWNOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, SendNotifyMessageW, SetForegroundWindow,
    SetWindowPos, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    WM_FONTCHANGE,
};
use windows::core::{PCWSTR, w};

use crate::dock::execute_traffic_light;
use crate::media::{MediaCommand, send_media_command};

pub const HOTKEY_TOGGLE_NOTCH: i32 = 1;
pub const HOTKEY_TOGGLE_POMODORO: i32 = 2;

const fn vk_char(ch: u8) -> VIRTUAL_KEY {
    VIRTUAL_KEY(ch as u16)
}

#[must_use]
pub fn poll_menu_dismiss_keys() -> (bool, bool) {
    unsafe {
        let left_down = (GetAsyncKeyState(i32::from(VK_LBUTTON.0)) as u16 & 0x8000) != 0;
        let right_down = (GetAsyncKeyState(i32::from(VK_RBUTTON.0)) as u16 & 0x8000) != 0;
        let esc_down = (GetAsyncKeyState(i32::from(VK_ESCAPE.0)) as u16 & 0x8000) != 0;
        (left_down || right_down, esc_down)
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
    if crate::status::step_system_volume(wheel_delta).is_some() {
        return;
    }
    if wheel_delta > 0 {
        send_key_chord(&[VK_VOLUME_UP]);
    } else if wheel_delta < 0 {
        send_key_chord(&[VK_VOLUME_DOWN]);
    }
}

pub fn toggle_tray_overflow(anchor_center_x: i32, bar_height: i32, screen_width: i32) {
    unsafe {
        if let Ok(overflow) = FindWindowW(w!("NotifyIconOverflowWindow"), PCWSTR::null())
            && !overflow.0.is_null()
        {
            if IsWindowVisible(overflow).as_bool() {
                let _ = ShowWindow(overflow, SW_HIDE);
                return;
            }
            let _ = ShowWindow(overflow, SW_SHOWNOACTIVATE);
            let mut rect = RECT::default();
            let _ = GetWindowRect(overflow, &mut rect);
            let width = (rect.right - rect.left).max(160);
            let x = (anchor_center_x - width / 2).clamp(8, (screen_width - width - 8).max(8));
            let y = bar_height + 6;
            let _ = SetWindowPos(
                overflow,
                Some(HWND_TOPMOST),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(overflow);
        } else {
            send_key_chord(&[VK_LWIN, vk_char(b'B')]);
            send_key_chord(&[VK_RETURN]);
        }
    }
}

pub fn execute_control_center_command(cmd: usize) {
    match cmd {
        CMD_CC_MEDIA_TOGGLE => send_media_command(MediaCommand::TogglePlayPause),
        CMD_CC_MEDIA_NEXT => send_media_command(MediaCommand::Next),
        CMD_CC_MEDIA_PREVIOUS => send_media_command(MediaCommand::Previous),
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

#[must_use]
pub fn execute_system_menu_command(cmd: usize) -> bool {
    match cmd {
        CMD_START => {
            open_start_menu();
            false
        }
        CMD_SETTINGS => {
            shell_exec(w!("ms-settings:"), PCWSTR::null());
            false
        }
        CMD_TASKMGR => {
            send_key_chord(&[VK_LCONTROL, VK_LSHIFT, VK_ESCAPE]);
            false
        }
        CMD_LOCK => {
            send_key_chord(&[VK_LWIN, vk_char(b'L')]);
            false
        }
        CMD_SLEEP => {
            shell_exec(w!("rundll32.exe"), w!("powrprof.dll,SetSuspendState 0,1,0"));
            false
        }
        CMD_RESTART => {
            shell_exec(w!("shutdown.exe"), w!("/r /t 0"));
            false
        }
        CMD_SHUTDOWN => {
            shell_exec(w!("shutdown.exe"), w!("/s /t 0"));
            false
        }
        CMD_QUIT => true,
        _ => false,
    }
}

#[must_use]
pub fn execute_top_bar_command(cmd: usize, app_name: &str, target_hwnd: isize) -> bool {
    if matches!(
        cmd,
        CMD_START
            | CMD_SETTINGS
            | CMD_TASKMGR
            | CMD_LOCK
            | CMD_SLEEP
            | CMD_RESTART
            | CMD_SHUTDOWN
            | CMD_QUIT
    ) {
        return execute_system_menu_command(cmd);
    }
    dispatch_top_bar_command(cmd, app_name, target_hwnd);
    false
}

#[must_use]
pub fn show_apple_system_menu(hwnd: isize, x: i32, y: i32) -> bool {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return false;
        };
        let _ = AppendMenuW(menu, MF_STRING, CMD_START, w!("Open Start Menu"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_SETTINGS, w!("System Settings..."));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_TASKMGR,
            w!("Force Quit / Task Manager...\tCtrl+Shift+Esc"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_LOCK, w!("Lock Screen\tWin+L"));
        let _ = AppendMenuW(menu, MF_STRING, CMD_SLEEP, w!("Sleep"));
        let _ = AppendMenuW(menu, MF_STRING, CMD_RESTART, w!("Restart..."));
        let _ = AppendMenuW(menu, MF_STRING, CMD_SHUTDOWN, w!("Shut Down..."));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Quit Otto Bar"));

        let _ = SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            x,
            y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        execute_system_menu_command(selected.0 as usize)
    }
}

pub fn show_top_bar_menu(
    hwnd: isize,
    target: HitTarget,
    x: i32,
    y: i32,
    app_name: &str,
    target_hwnd: isize,
) {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        populate_top_bar_menu(menu, target, app_name);
        let _ = SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            x,
            y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        let cmd = selected.0 as usize;
        if cmd != 0 {
            dispatch_top_bar_command(cmd, app_name, target_hwnd);
        }
    }
}

unsafe fn append_menu_entries(
    menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    entries: &[(usize, PCWSTR)],
) {
    unsafe {
        for &(cmd, label) in entries {
            if cmd == 0 {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            } else {
                let _ = AppendMenuW(menu, MF_STRING, cmd, label);
            }
        }
    }
}

unsafe fn populate_top_bar_menu(
    menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    target: HitTarget,
    app_name: &str,
) {
    unsafe {
        match target {
            HitTarget::AppMenu => {
                let title_wide = to_wide(&format!("Show {app_name}"));
                let quit_wide = to_wide(&format!("Quit {app_name}\tAlt+F4"));
                append_menu_entries(
                    menu,
                    &[
                        (CMD_APP_ACTIVATE, PCWSTR(title_wide.as_ptr())),
                        (0, PCWSTR::null()),
                        (CMD_WIN_MINIMIZE, w!("Minimize\tWin+Down")),
                        (CMD_WIN_MAXIMIZE, w!("Zoom / Maximize\tWin+Up")),
                        (CMD_SHOW_DESKTOP, w!("Hide Others (Show Desktop)\tWin+D")),
                        (0, PCWSTR::null()),
                        (
                            CMD_TASKMGR,
                            w!("Force Quit Applications...\tCtrl+Shift+Esc"),
                        ),
                        (CMD_WIN_CLOSE, PCWSTR(quit_wide.as_ptr())),
                    ],
                );
            }
            HitTarget::FileMenu => append_menu_entries(
                menu,
                &[
                    (CMD_FILE_NEW, w!("New Window / Document\tCtrl+N")),
                    (CMD_FILE_NEW_TAB, w!("New Tab\tCtrl+T")),
                    (CMD_FILE_OPEN, w!("Open...\tCtrl+O")),
                    (0, PCWSTR::null()),
                    (CMD_FILE_SAVE, w!("Save\tCtrl+S")),
                    (CMD_FILE_SAVE_AS, w!("Save As...\tCtrl+Shift+S")),
                    (0, PCWSTR::null()),
                    (CMD_WIN_CLOSE, w!("Close Window\tAlt+F4")),
                ],
            ),
            HitTarget::EditMenu => append_menu_entries(
                menu,
                &[
                    (CMD_EDIT_UNDO, w!("Undo\tCtrl+Z")),
                    (CMD_EDIT_REDO, w!("Redo\tCtrl+Y")),
                    (0, PCWSTR::null()),
                    (CMD_EDIT_CUT, w!("Cut\tCtrl+X")),
                    (CMD_EDIT_COPY, w!("Copy\tCtrl+C")),
                    (CMD_EDIT_PASTE, w!("Paste\tCtrl+V")),
                    (CMD_EDIT_SELECT_ALL, w!("Select All\tCtrl+A")),
                    (0, PCWSTR::null()),
                    (CMD_EDIT_FIND, w!("Find...\tCtrl+F")),
                ],
            ),
            HitTarget::ViewMenu => append_menu_entries(
                menu,
                &[
                    (CMD_VIEW_FULLSCREEN, w!("Toggle Full Screen\tF11")),
                    (0, PCWSTR::null()),
                    (CMD_VIEW_ZOOM_IN, w!("Zoom In\tCtrl++")),
                    (CMD_VIEW_ZOOM_OUT, w!("Zoom Out\tCtrl+-")),
                    (CMD_VIEW_ACTUAL_SIZE, w!("Actual Size\tCtrl+0")),
                ],
            ),
            HitTarget::WindowMenu => append_menu_entries(
                menu,
                &[
                    (CMD_WIN_MINIMIZE, w!("Minimize\tWin+Down")),
                    (CMD_WIN_MAXIMIZE, w!("Zoom / Maximize\tWin+Up")),
                    (0, PCWSTR::null()),
                    (CMD_TILE_LEFT, w!("Tile Window to Left of Screen\tWin+Left")),
                    (
                        CMD_TILE_RIGHT,
                        w!("Tile Window to Right of Screen\tWin+Right"),
                    ),
                    (0, PCWSTR::null()),
                    (CMD_TASK_VIEW, w!("Mission Control (Task View)\tWin+Tab")),
                    (CMD_SHOW_DESKTOP, w!("Show Desktop\tWin+D")),
                ],
            ),
            HitTarget::HelpMenu => append_menu_entries(
                menu,
                &[
                    (
                        CMD_HELP_PALETTE,
                        w!("Command Palette / Search\tCtrl+Shift+P"),
                    ),
                    (CMD_HELP_F1, w!("Application Help\tF1")),
                ],
            ),
            _ => {}
        }
    }
}

fn dispatch_top_bar_command(cmd: usize, app_name: &str, target_hwnd: isize) {
    match cmd {
        CMD_APP_ACTIVATE => {
            if target_hwnd != 0 {
                restore_focus(target_hwnd);
            } else if app_name == "Finder" {
                shell_exec(w!("explorer.exe"), PCWSTR::null());
            }
        }
        CMD_WIN_MINIMIZE if target_hwnd != 0 => {
            execute_traffic_light(target_hwnd, TrafficLightAction::Minimize);
        }
        CMD_WIN_MAXIMIZE if target_hwnd != 0 => {
            execute_traffic_light(target_hwnd, TrafficLightAction::ToggleMaximize);
        }
        CMD_WIN_CLOSE if target_hwnd != 0 => {
            execute_traffic_light(target_hwnd, TrafficLightAction::Close);
        }
        CMD_SHOW_DESKTOP => send_key_chord(&[VK_LWIN, vk_char(b'D')]),
        CMD_TASKMGR => send_key_chord(&[VK_LCONTROL, VK_LSHIFT, VK_ESCAPE]),
        CMD_TASK_VIEW => open_task_view(),
        CMD_TILE_LEFT => send_to_target(target_hwnd, &[VK_LWIN, VK_LEFT]),
        CMD_TILE_RIGHT => send_to_target(target_hwnd, &[VK_LWIN, VK_RIGHT]),
        CMD_FILE_NEW => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'N')]),
        CMD_FILE_NEW_TAB => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'T')]),
        CMD_FILE_OPEN => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'O')]),
        CMD_FILE_SAVE => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'S')]),
        CMD_FILE_SAVE_AS => {
            send_to_target(target_hwnd, &[VK_LCONTROL, VK_LSHIFT, vk_char(b'S')]);
        }
        CMD_EDIT_UNDO => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'Z')]),
        CMD_EDIT_REDO => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'Y')]),
        CMD_EDIT_CUT => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'X')]),
        CMD_EDIT_COPY => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'C')]),
        CMD_EDIT_PASTE => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'V')]),
        CMD_EDIT_SELECT_ALL => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'A')]),
        CMD_EDIT_FIND => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'F')]),
        CMD_VIEW_FULLSCREEN => send_to_target(target_hwnd, &[VK_F11]),
        CMD_VIEW_ZOOM_IN => send_to_target(target_hwnd, &[VK_LCONTROL, VK_OEM_PLUS]),
        CMD_VIEW_ZOOM_OUT => send_to_target(target_hwnd, &[VK_LCONTROL, VK_OEM_MINUS]),
        CMD_VIEW_ACTUAL_SIZE => send_to_target(target_hwnd, &[VK_LCONTROL, vk_char(b'0')]),
        CMD_HELP_PALETTE => {
            send_to_target(target_hwnd, &[VK_LCONTROL, VK_LSHIFT, vk_char(b'P')]);
        }
        CMD_HELP_F1 => send_to_target(target_hwnd, &[VK_F1]),
        _ => {}
    }
}

fn restore_focus(target_hwnd: isize) {
    if target_hwnd != 0 {
        unsafe {
            let _ = SetForegroundWindow(HWND(target_hwnd as *mut c_void));
        }
        thread::sleep(Duration::from_millis(30));
    }
}

fn send_to_target(target_hwnd: isize, keys: &[VIRTUAL_KEY]) {
    restore_focus(target_hwnd);
    send_key_chord(keys);
}

fn shell_exec(file: PCWSTR, params: PCWSTR) {
    unsafe {
        let _ = ShellExecuteW(None, w!("open"), file, params, PCWSTR::null(), SW_SHOW);
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[must_use]
pub fn show_quit_menu(hwnd: isize) -> bool {
    let mut cursor = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut cursor);
    }
    show_apple_system_menu(hwnd, cursor.x, cursor.y)
}

fn send_key_chord(keys: &[VIRTUAL_KEY]) {
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
