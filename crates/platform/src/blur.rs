use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{BOOL, s, w};

const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct CompositionAttribute {
    attribute: u32,
    data: *mut c_void,
    size: usize,
}

type SetWindowCompositionAttribute =
    unsafe extern "system" fn(HWND, *mut CompositionAttribute) -> BOOL;

#[must_use]
pub fn enable_backdrop_blur(hwnd: isize) -> bool {
    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else {
            return false;
        };
        let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else {
            return false;
        };
        let set: SetWindowCompositionAttribute = std::mem::transmute(proc);
        let mut policy = AccentPolicy {
            state: ACCENT_ENABLE_BLURBEHIND,
            flags: 0,
            gradient_color: 0,
            animation_id: 0,
        };
        let mut data = CompositionAttribute {
            attribute: WCA_ACCENT_POLICY,
            data: (&raw mut policy).cast(),
            size: size_of::<AccentPolicy>(),
        };
        set(HWND(hwnd as *mut c_void), &raw mut data).as_bool()
    }
}
