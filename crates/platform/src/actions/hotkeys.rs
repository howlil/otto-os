use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY, VK_F2,
    VK_OEM_COMMA, VK_SPACE,
};

pub const HOTKEY_TOGGLE_NOTCH: i32 = 1;
pub const HOTKEY_TOGGLE_POMODORO: i32 = 2;
pub const HOTKEY_FOCUS_BAR: i32 = 3;
pub const HOTKEY_CONTROL_CENTER: i32 = 4;
pub const HOTKEY_SPOTLIGHT: i32 = 5;
pub const HOTKEY_TOGGLE_TOP_BAR: i32 = 6;
pub const HOTKEY_TOGGLE_DOCK: i32 = 7;
pub const HOTKEY_QUICK_CAPTURE: i32 = 8;
pub const HOTKEY_OTTO_SETTINGS: i32 = 9;

pub(super) type HotkeyBinding = (
    i32,
    windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS,
    u32,
);

pub(super) fn notch_hotkey_bindings() -> [HotkeyBinding; 5] {
    let safe_chord = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
    [
        (HOTKEY_TOGGLE_NOTCH, safe_chord, u32::from(b'N')),
        (HOTKEY_TOGGLE_POMODORO, safe_chord, u32::from(b'P')),
        (
            HOTKEY_FOCUS_BAR,
            MOD_CONTROL | MOD_NOREPEAT,
            u32::from(VK_F2.0),
        ),
        (HOTKEY_CONTROL_CENTER, safe_chord, u32::from(b'C')),
        (HOTKEY_SPOTLIGHT, safe_chord, u32::from(VK_SPACE.0)),
    ]
}

pub(super) fn capture_hotkey_bindings() -> [HotkeyBinding; 1] {
    [(
        HOTKEY_QUICK_CAPTURE,
        MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
        u32::from(b'Q'),
    )]
}

pub(super) fn shell_hotkey_bindings() -> [HotkeyBinding; 2] {
    let chord = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
    [
        (HOTKEY_TOGGLE_TOP_BAR, chord, u32::from(b'B')),
        (HOTKEY_TOGGLE_DOCK, chord, u32::from(b'D')),
    ]
}

pub(super) fn settings_hotkey_bindings() -> [HotkeyBinding; 1] {
    [(
        HOTKEY_OTTO_SETTINGS,
        MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
        u32::from(VK_OEM_COMMA.0),
    )]
}

pub(super) const fn vk_char(ch: u8) -> VIRTUAL_KEY {
    VIRTUAL_KEY(ch as u16)
}

pub(super) fn register_hotkeys(hwnd: isize, bindings: &[HotkeyBinding]) -> bool {
    let handle = HWND(hwnd as *mut c_void);
    let mut all_registered = true;
    for &(id, modifiers, vk) in bindings {
        if let Err(error) = unsafe { RegisterHotKey(Some(handle), id, modifiers, vk) } {
            all_registered = false;
            crate::diagnostics::log_error(
                "hotkeys",
                &format!("failed to register hotkey id {id} ({modifiers:?}, vk={vk:#x}): {error}"),
            );
        }
    }
    all_registered
}

pub(super) fn unregister_hotkeys(hwnd: isize, bindings: &[HotkeyBinding]) {
    let handle = HWND(hwnd as *mut c_void);
    for &(id, _, _) in bindings {
        let _ = unsafe { UnregisterHotKey(Some(handle), id) };
    }
}

pub fn register_capture_hotkey(hwnd: isize) {
    register_hotkeys(hwnd, &capture_hotkey_bindings());
}

pub fn unregister_capture_hotkey(hwnd: isize) {
    unregister_hotkeys(hwnd, &capture_hotkey_bindings());
}

pub fn register_notch_hotkeys(hwnd: isize) {
    register_hotkeys(hwnd, &notch_hotkey_bindings());
}

pub fn unregister_notch_hotkeys(hwnd: isize) {
    unregister_hotkeys(hwnd, &notch_hotkey_bindings());
}

#[must_use]
pub fn register_shell_hotkeys(hwnd: isize) -> bool {
    register_hotkeys(hwnd, &shell_hotkey_bindings())
}

pub fn unregister_shell_hotkeys(hwnd: isize) {
    unregister_hotkeys(hwnd, &shell_hotkey_bindings());
}

#[must_use]
pub fn register_settings_hotkey(hwnd: isize) -> bool {
    register_hotkeys(hwnd, &settings_hotkey_bindings())
}

pub fn unregister_settings_hotkey(hwnd: isize) {
    unregister_hotkeys(hwnd, &settings_hotkey_bindings());
}
