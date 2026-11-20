use std::mem::size_of;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_DOWN, VK_ESCAPE, VK_LBUTTON, VK_LEFT, VK_RBUTTON,
    VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuNavEvent {
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
    Enter,
    Escape,
    Char(char),
}

pub(super) static LAST_NAV_KEY: AtomicU32 = AtomicU32::new(0);

pub(super) fn is_key_down(key: i32) -> bool {
    (unsafe { GetAsyncKeyState(key) } as u16 & 0x8000) != 0
}

#[must_use]
pub fn is_escape_down() -> bool {
    is_key_down(i32::from(VK_ESCAPE.0))
}

#[must_use]
pub fn is_shift_down() -> bool {
    is_key_down(i32::from(VK_SHIFT.0))
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
    } else if is_key_down(i32::from(VK_LEFT.0)) {
        (u32::from(VK_LEFT.0), Some(MenuNavEvent::Left))
    } else if is_key_down(i32::from(VK_RIGHT.0)) {
        (u32::from(VK_RIGHT.0), Some(MenuNavEvent::Right))
    } else if is_key_down(i32::from(VK_TAB.0)) {
        if is_shift_down() {
            (u32::from(VK_TAB.0) | 0x1000, Some(MenuNavEvent::BackTab))
        } else {
            (u32::from(VK_TAB.0), Some(MenuNavEvent::Tab))
        }
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

pub(super) const fn key_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
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
