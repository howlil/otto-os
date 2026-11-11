use std::mem::size_of;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::w;

#[must_use]
pub fn is_system_in_dark_mode() -> bool {
    let mut hkey = HKEY::default();
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let open_status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey, None, KEY_READ, &mut hkey) };
    if open_status.is_err() {
        return true;
    }
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    let query_status = unsafe {
        RegQueryValueExW(
            hkey,
            w!("AppsUseLightTheme"),
            None,
            None,
            Some((&raw mut data).cast()),
            Some(&mut size),
        )
    };
    let _ = unsafe { RegCloseKey(hkey) };
    if query_status.is_ok() {
        data == 0
    } else {
        true
    }
}

#[must_use]
pub fn is_transparency_enabled() -> bool {
    let mut hkey = HKEY::default();
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let open_status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey, None, KEY_READ, &mut hkey) };
    if open_status.is_err() {
        return true;
    }
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    let query_status = unsafe {
        RegQueryValueExW(
            hkey,
            w!("EnableTransparency"),
            None,
            None,
            Some((&raw mut data).cast()),
            Some(&mut size),
        )
    };
    let _ = unsafe { RegCloseKey(hkey) };
    if query_status.is_ok() {
        data != 0
    } else {
        true
    }
}

#[must_use]
pub fn is_high_contrast_active() -> bool {
    let mut hc = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            size_of::<HIGHCONTRASTW>() as u32,
            Some((&raw mut hc).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if ok.is_ok() {
        (hc.dwFlags.0 & HCF_HIGHCONTRASTON.0) != 0
    } else {
        false
    }
}
