use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};

const DWMWA_SYSTEMBACKDROP_TYPE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(38);
const DWMSBT_NONE: u32 = 1;
const DWMSBT_MAINWINDOW: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackdropMaterial {
    None,
    Mica,
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
        BackdropMaterial::Mica => DWMSBT_MAINWINDOW,
    };

    unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&raw const dwm_type).cast(),
            size_of::<u32>() as u32,
        )
        .is_ok()
    }
}
