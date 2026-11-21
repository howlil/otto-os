use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{BOOL, s, w};

const DWMWA_SYSTEMBACKDROP_TYPE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(38);
const DWMSBT_NONE: u32 = 1;
const DWMSBT_TRANSIENTWINDOW: u32 = 3;

const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackdropMaterial {
    None,
    Acrylic,
}

#[must_use]
pub(crate) fn apply_backdrop_material(hwnd: isize, material: BackdropMaterial) -> bool {
    let handle = HWND(hwnd as *mut c_void);
    if !crate::theme::is_transparency_enabled() && material != BackdropMaterial::None {
        let none_val = DWMSBT_NONE;
        unsafe {
            let _ = DwmSetWindowAttribute(
                handle,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&raw const none_val).cast(),
                size_of::<u32>() as u32,
            );
        }
        return false;
    }

    let dwm_type = match material {
        BackdropMaterial::None => DWMSBT_NONE,
        BackdropMaterial::Acrylic => DWMSBT_TRANSIENTWINDOW,
    };

    let dwm_res = unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&raw const dwm_type).cast(),
            size_of::<u32>() as u32,
        )
    };

    if dwm_res.is_ok() {
        return true;
    }

    let accent_state = match material {
        BackdropMaterial::None => 0,
        BackdropMaterial::Acrylic => ACCENT_ENABLE_ACRYLICBLURBEHIND,
    };

    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else {
            return false;
        };
        let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else {
            return false;
        };
        let set: SetWindowCompositionAttribute = std::mem::transmute(proc);
        let mut policy = AccentPolicy {
            state: accent_state,
            flags: 0,
            gradient_color: 0,
            animation_id: 0,
        };
        let mut data = CompositionAttribute {
            attribute: WCA_ACCENT_POLICY,
            data: (&raw mut policy).cast(),
            size: size_of::<AccentPolicy>(),
        };
        set(handle, &raw mut data).as_bool()
    }
}
