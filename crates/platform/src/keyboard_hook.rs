use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
};

use crate::audio::{is_available, step_system_volume, toggle_system_mute};
use crate::worker::join_within;

const READY_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
const INJECTED_FLAG: u32 = 0x10;
const WHEEL_UP: i16 = 120;
const WHEEL_DOWN: i16 = -120;

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);

struct HookThread {
    thread_id: u32,
    handle: JoinHandle<()>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VolumeKey {
    Up,
    Down,
    Mute,
}

fn volume_key(virtual_key: u32, flags: u32) -> Option<VolumeKey> {
    if flags & INJECTED_FLAG != 0 {
        return None;
    }
    match virtual_key {
        key if key == u32::from(VK_VOLUME_UP.0) => Some(VolumeKey::Up),
        key if key == u32::from(VK_VOLUME_DOWN.0) => Some(VolumeKey::Down),
        key if key == u32::from(VK_VOLUME_MUTE.0) => Some(VolumeKey::Mute),
        _ => None,
    }
}

fn is_key_down(message: usize) -> bool {
    message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize
}

fn apply_volume_key(key: VolumeKey) -> bool {
    match key {
        VolumeKey::Up => step_system_volume(WHEEL_UP).is_some(),
        VolumeKey::Down => step_system_volume(WHEEL_DOWN).is_some(),
        VolumeKey::Mute => toggle_system_mute().is_some(),
    }
}

fn swallow(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 {
        return false;
    }
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let Some(key) = volume_key(event.vkCode, event.flags.0) else {
        return false;
    };
    if is_key_down(wparam.0) {
        apply_volume_key(key)
    } else {
        is_available()
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled = catch_unwind(AssertUnwindSafe(|| swallow(code, wparam, lparam)));
    if matches!(handled, Ok(true)) {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn hook_thread_main(ready: &SyncSender<Option<u32>>) {
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0) };
    let Ok(hook) = hook else {
        let _ = ready.send(None);
        return;
    };
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        let _ = ready.send(Some(GetCurrentThreadId()));
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

pub fn install_keyboard_hook() -> bool {
    let Ok(mut guard) = HOOK_THREAD.lock() else {
        return false;
    };
    if guard.is_some() {
        return true;
    }
    let (ready_sender, ready_receiver) = sync_channel(1);
    let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-keyboard-hook"))
        .spawn(move || hook_thread_main(&ready_sender))
    else {
        return false;
    };
    match ready_receiver.recv_timeout(READY_TIMEOUT) {
        Ok(Some(thread_id)) => {
            *guard = Some(HookThread { thread_id, handle });
            true
        }
        _ => false,
    }
}

pub fn uninstall_keyboard_hook() {
    let Some(thread) = HOOK_THREAD.lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    unsafe {
        let _ = PostThreadMessageW(thread.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
    let _ = join_within(thread.handle, SHUTDOWN_TIMEOUT);
}

#[cfg(test)]
mod tests {
    use super::{
        HOOK_THREAD, INJECTED_FLAG, VolumeKey, install_keyboard_hook, is_key_down,
        uninstall_keyboard_hook, volume_key,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_A, VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
    };
    use windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN};

    #[test]
    fn maps_hardware_volume_keys() {
        assert_eq!(
            volume_key(u32::from(VK_VOLUME_UP.0), 0),
            Some(VolumeKey::Up)
        );
        assert_eq!(
            volume_key(u32::from(VK_VOLUME_DOWN.0), 0),
            Some(VolumeKey::Down)
        );
        assert_eq!(
            volume_key(u32::from(VK_VOLUME_MUTE.0), 0),
            Some(VolumeKey::Mute)
        );
    }

    #[test]
    fn ignores_injected_and_unrelated_keys() {
        assert_eq!(volume_key(u32::from(VK_VOLUME_UP.0), INJECTED_FLAG), None);
        assert_eq!(volume_key(u32::from(VK_A.0), 0), None);
    }

    #[test]
    fn distinguishes_key_down_messages() {
        assert!(is_key_down(WM_KEYDOWN as usize));
        assert!(is_key_down(WM_SYSKEYDOWN as usize));
        assert!(!is_key_down(WM_KEYUP as usize));
    }

    #[test]
    fn hook_thread_state_is_released_after_uninstall() {
        let installed = install_keyboard_hook();
        assert_eq!(HOOK_THREAD.lock().unwrap().is_some(), installed);
        uninstall_keyboard_hook();
        assert!(HOOK_THREAD.lock().unwrap().is_none());
    }
}
