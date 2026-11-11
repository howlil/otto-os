use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, FindWindowW,
    GetForegroundWindow, IsWindow, OBJID_WINDOW, PostMessageW, SW_HIDE, ShowWindow,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};
use windows::core::{PCWSTR, w};

static FOREGROUND_HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static LOCATION_HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SHOW_HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static WIN_EVENT_TARGET: AtomicIsize = AtomicIsize::new(0);
static WIN_EVENT_MSG: AtomicU32 = AtomicU32::new(0);
static SYNC_PENDING: AtomicBool = AtomicBool::new(false);
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static LAST_POLLED_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

#[must_use]
pub fn poll_foreground_change() -> bool {
    let current = unsafe { GetForegroundWindow() }.0 as isize;
    if current == 0 || LAST_POLLED_FOREGROUND.swap(current, Ordering::AcqRel) == current {
        return false;
    }
    LAST_FOREGROUND.store(current, Ordering::Release);
    true
}

pub(crate) fn current_foreground() -> HWND {
    let announced = HWND(LAST_FOREGROUND.load(Ordering::Acquire) as *mut _);
    if !announced.0.is_null() && unsafe { IsWindow(Some(announced)) }.as_bool() {
        return announced;
    }
    unsafe { GetForegroundWindow() }
}

fn install_hook(slot: &AtomicPtr<c_void>, event: u32) -> bool {
    if !slot.load(Ordering::SeqCst).is_null() {
        return true;
    }
    let hook = unsafe {
        SetWinEventHook(
            event,
            event,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.0.is_null() {
        return false;
    }
    slot.store(hook.0, Ordering::SeqCst);
    true
}

fn remove_hook(slot: &AtomicPtr<c_void>) {
    let previous = slot.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !previous.is_null() {
        unsafe {
            let _ = UnhookWinEvent(HWINEVENTHOOK(previous));
        }
    }
}

pub fn install_window_tracking_hook(target_hwnd: isize, message: u32) -> bool {
    WIN_EVENT_TARGET.store(target_hwnd, Ordering::SeqCst);
    WIN_EVENT_MSG.store(message, Ordering::SeqCst);
    SYNC_PENDING.store(false, Ordering::SeqCst);
    let installed = install_hook(&FOREGROUND_HOOK, EVENT_SYSTEM_FOREGROUND)
        && install_hook(&LOCATION_HOOK, EVENT_OBJECT_LOCATIONCHANGE)
        && install_hook(&SHOW_HOOK, EVENT_OBJECT_SHOW);
    if !installed {
        uninstall_window_tracking_hook();
    }
    installed
}

pub fn uninstall_window_tracking_hook() {
    remove_hook(&FOREGROUND_HOOK);
    remove_hook(&LOCATION_HOOK);
    remove_hook(&SHOW_HOOK);
    WIN_EVENT_TARGET.store(0, Ordering::SeqCst);
    WIN_EVENT_MSG.store(0, Ordering::SeqCst);
    SYNC_PENDING.store(false, Ordering::SeqCst);
    LAST_FOREGROUND.store(0, Ordering::SeqCst);
    LAST_POLLED_FOREGROUND.store(0, Ordering::SeqCst);
}

pub fn acknowledge_window_sync() {
    SYNC_PENDING.store(false, Ordering::Release);
}

fn post_window_sync(source: HWND, event: u32) {
    let target = WIN_EVENT_TARGET.load(Ordering::Relaxed);
    let message = WIN_EVENT_MSG.load(Ordering::Relaxed);
    if target == 0 || message == 0 || SYNC_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            message,
            WPARAM(source.0 as usize),
            LPARAM(event as isize),
        )
    };
    if posted.is_err() {
        SYNC_PENDING.store(false, Ordering::Release);
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _event_time: u32,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if id_object != OBJID_WINDOW.0 {
            return;
        }
        if event == EVENT_SYSTEM_FOREGROUND {
            LAST_FOREGROUND.store(hwnd.0 as isize, Ordering::Release);
        }
        let relevant = event == EVENT_SYSTEM_FOREGROUND
            || (event == EVENT_OBJECT_SHOW
                && hwnd.0 as isize == LAST_FOREGROUND.load(Ordering::Acquire))
            || (event == EVENT_OBJECT_LOCATIONCHANGE && current_foreground() == hwnd);
        if relevant {
            post_window_sync(hwnd, event);
        }
    }));
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
