use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
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
