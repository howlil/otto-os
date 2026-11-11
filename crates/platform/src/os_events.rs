use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, FindWindowW, HHOOK, KBDLLHOOKSTRUCT, SW_HIDE, SetWindowsHookExW, ShowWindow,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};
use windows::core::{PCWSTR, w};

use crate::status::{step_system_volume, toggle_system_mute};

static HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

pub fn install_os_event_hooks() -> bool {
    if !HOOK.load(Ordering::SeqCst).is_null() {
        return true;
    }
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0);
        match hook {
            Ok(h) => {
                HOOK.store(h.0, Ordering::SeqCst);
                true
            }
            Err(_) => false,
        }
    }
}

pub fn uninstall_os_event_hooks() {
    let prev = HOOK.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !prev.is_null() {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(prev.cast()));
        }
    }
}

pub fn suppress_native_osd() {
    unsafe {
        if let Ok(hud) = FindWindowW(w!("NativeHUD"), PCWSTR::null())
            && !hud.0.is_null()
        {
            let _ = ShowWindow(hud, SW_HIDE);
        }
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && (wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize) {
        let kbd = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kbd.vkCode as u16;
        if vk == VK_VOLUME_UP.0 {
            let _ = step_system_volume(120);
            return LRESULT(1);
        }
        if vk == VK_VOLUME_DOWN.0 {
            let _ = step_system_volume(-120);
            return LRESULT(1);
        }
        if vk == VK_VOLUME_MUTE.0 {
            let _ = toggle_system_mute();
            return LRESULT(1);
        }
    }
    let hook = HOOK.load(Ordering::Relaxed);
    unsafe { CallNextHookEx(Some(HHOOK(hook.cast())), code, wparam, lparam) }
}
