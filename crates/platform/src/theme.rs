use std::mem::size_of;

use otto_core::{ContrastColors, Rgba, SystemAppearance};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW,
    COLOR_WINDOWTEXT, GetSysColor, SYS_COLOR_INDEX,
};
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

fn system_color(index: SYS_COLOR_INDEX) -> Rgba {
    let value = unsafe { GetSysColor(index) };
    let channel = |shift: u32| f32::from(((value >> shift) & 0xFF) as u8) / 255.0;
    Rgba::new(channel(0), channel(8), channel(16), 1.0)
}

#[must_use]
pub fn system_contrast_colors() -> Option<ContrastColors> {
    is_high_contrast_active().then(|| ContrastColors {
        window: system_color(COLOR_WINDOW),
        window_text: system_color(COLOR_WINDOWTEXT),
        highlight: system_color(COLOR_HIGHLIGHT),
        highlight_text: system_color(COLOR_HIGHLIGHTTEXT),
        gray_text: system_color(COLOR_GRAYTEXT),
        button_face: system_color(COLOR_BTNFACE),
    })
}

#[must_use]
pub fn read_system_appearance() -> SystemAppearance {
    SystemAppearance {
        dark: is_system_in_dark_mode(),
        transparency: is_transparency_enabled(),
        contrast: system_contrast_colors(),
    }
}
