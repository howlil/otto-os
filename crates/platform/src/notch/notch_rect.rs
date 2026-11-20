use std::ffi::c_void;
use std::sync::OnceLock;

use otto_core::{NOTCH_RECT_PROPERTY, Rect, pack_notch_rect};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::UI::WindowsAndMessaging::{RemovePropW, SetPropW};
use windows::core::PCWSTR;

fn property_name() -> PCWSTR {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    PCWSTR(
        NAME.get_or_init(|| NOTCH_RECT_PROPERTY.encode_utf16().chain(Some(0)).collect())
            .as_ptr(),
    )
}

pub fn publish_notch_rect(hwnd: isize, rect: Rect) {
    let Some(packed) = pack_notch_rect(rect) else {
        withdraw_notch_rect(hwnd);
        return;
    };
    unsafe {
        let _ = SetPropW(
            HWND(hwnd as *mut c_void),
            property_name(),
            Some(HANDLE(packed as usize as *mut c_void)),
        );
    }
}

pub fn withdraw_notch_rect(hwnd: isize) {
    unsafe {
        let _ = RemovePropW(HWND(hwnd as *mut c_void), property_name());
    }
}
