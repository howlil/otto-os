use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, AtomicPtr, AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, FindWindowW, HHOOK,
    KBDLLHOOKSTRUCT, OBJID_WINDOW, PostMessageW, SW_HIDE, SetWindowsHookExW, ShowWindow,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    WM_KEYDOWN, WM_SYSKEYDOWN,
};
use windows::core::{PCWSTR, w};

use crate::status::{step_system_volume, toggle_system_mute};

static HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static WIN_EVENT_HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static WIN_EVENT_TARGET: AtomicIsize = AtomicIsize::new(0);
static WIN_EVENT_MSG: AtomicU32 = AtomicU32::new(0);

pub fn install_window_tracking_hook(target_hwnd: isize, message: u32) -> bool {
    WIN_EVENT_TARGET.store(target_hwnd, Ordering::SeqCst);
    WIN_EVENT_MSG.store(message, Ordering::SeqCst);
    if !WIN_EVENT_HOOK.load(Ordering::SeqCst).is_null() {
        return true;
    }
    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.0.is_null() {
            false
        } else {
            WIN_EVENT_HOOK.store(hook.0, Ordering::SeqCst);
            true
        }
    }
}

pub fn uninstall_window_tracking_hook() {
    let prev = WIN_EVENT_HOOK.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !prev.is_null() {
        unsafe {
            let _ = UnhookWinEvent(HWINEVENTHOOK(prev));
        }
    }
    WIN_EVENT_TARGET.store(0, Ordering::SeqCst);
    WIN_EVENT_MSG.store(0, Ordering::SeqCst);
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if id_object != OBJID_WINDOW.0 {
        return;
    }
    if event == EVENT_SYSTEM_FOREGROUND || event == EVENT_OBJECT_LOCATIONCHANGE {
        let target = WIN_EVENT_TARGET.load(Ordering::Relaxed);
        let msg = WIN_EVENT_MSG.load(Ordering::Relaxed);
        if target != 0 && msg != 0 {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(target as *mut _)),
                    msg,
                    WPARAM(hwnd.0 as usize),
                    LPARAM(event as isize),
                );
            }
        }
    }
}

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
