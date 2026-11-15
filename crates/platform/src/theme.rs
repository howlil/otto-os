use std::mem::size_of;

use otto_core::{ContrastColors, Rgba, SystemAppearance};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW,
    COLOR_WINDOWTEXT, GetSysColor, SYS_COLOR_INDEX,
};
use windows::Win32::System::Power::{
    GetSystemPowerStatus, HPOWERNOTIFY, RegisterPowerSettingNotification, SYSTEM_POWER_STATUS,
    UnregisterPowerSettingNotification,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::System::SystemServices::GUID_POWER_SAVING_STATUS;
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_WINDOW_HANDLE, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
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

const BATTERY_SAVER_ON: u8 = 1;

pub(crate) const fn battery_saver_from_flag(flag: u8) -> bool {
    flag & BATTERY_SAVER_ON != 0
}

const AC_LINE_OFFLINE: u8 = 0;

const fn on_battery_from_line(line_status: u8) -> bool {
    line_status == AC_LINE_OFFLINE
}

#[must_use]
pub fn is_running_on_battery() -> bool {
    let mut power = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut power) }.is_ok() && on_battery_from_line(power.ACLineStatus)
}

#[must_use]
pub fn is_battery_saver_active() -> bool {
    let mut power = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut power) }.is_ok()
        && battery_saver_from_flag(power.SystemStatusFlag)
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

pub struct PowerNotificationGuard(HPOWERNOTIFY);

impl PowerNotificationGuard {
    #[must_use]
    pub fn register(hwnd: isize) -> Option<Self> {
        if hwnd == 0 {
            return None;
        }
        let handle = unsafe {
            RegisterPowerSettingNotification(
                HANDLE(hwnd as *mut _),
                &GUID_POWER_SAVING_STATUS,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }
        .ok()?;
        (!handle.is_invalid()).then_some(Self(handle))
    }
}

impl Drop for PowerNotificationGuard {
    fn drop(&mut self) {
        let _ = unsafe { UnregisterPowerSettingNotification(self.0) };
    }
}

#[must_use]
pub fn read_system_appearance() -> SystemAppearance {
    SystemAppearance {
        dark: is_system_in_dark_mode(),
        transparency: is_transparency_enabled() && !is_battery_saver_active(),
        contrast: system_contrast_colors(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PowerNotificationGuard, battery_saver_from_flag, is_battery_saver_active,
        on_battery_from_line, read_system_appearance,
    };

    #[test]
    fn only_an_offline_ac_line_counts_as_battery_power() {
        assert!(on_battery_from_line(0));
        assert!(!on_battery_from_line(1));
        assert!(!on_battery_from_line(255));
    }

    #[test]
    fn battery_saver_is_the_lowest_status_flag_bit() {
        assert!(battery_saver_from_flag(1));
        assert!(battery_saver_from_flag(0b11));
        assert!(!battery_saver_from_flag(0));
        assert!(!battery_saver_from_flag(0b10));
    }

    #[test]
    fn battery_saver_turns_transparency_off_in_the_system_appearance() {
        let appearance = read_system_appearance();
        if is_battery_saver_active() {
            assert!(!appearance.transparency);
        }
    }

    #[test]
    fn power_notification_registration_rejects_null_window() {
        assert!(PowerNotificationGuard::register(0).is_none());
    }
}
