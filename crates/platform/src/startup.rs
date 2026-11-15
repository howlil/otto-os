use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::core::{PCWSTR, w};

const RUN_SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const STARTUP_VALUE_NAME: PCWSTR = w!("OttoOS");

#[must_use]
pub fn format_startup_command(exe_path: &Path) -> String {
    format!("\"{}\"", exe_path.display())
}

#[must_use]
pub fn is_startup_entry_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|raw| !raw.trim().is_empty())
}

#[must_use]
pub fn read_launch_at_login() -> bool {
    let mut hkey = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_SUBKEY,
            None,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
    };
    if status.is_err() {
        return false;
    }
    let mut reg_type = REG_VALUE_TYPE::default();
    let mut bytes = 0u32;
    let query_len = unsafe {
        RegQueryValueExW(
            hkey,
            STARTUP_VALUE_NAME,
            None,
            Some(&raw mut reg_type),
            None,
            Some(&raw mut bytes),
        )
    };
    if query_len.is_err() || bytes < 2 {
        let _ = unsafe { RegCloseKey(hkey) };
        return false;
    }
    let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
    let mut actual_bytes = (buf.len() * 2) as u32;
    let query_val = unsafe {
        RegQueryValueExW(
            hkey,
            STARTUP_VALUE_NAME,
            None,
            Some(&raw mut reg_type),
            Some(buf.as_mut_ptr().cast()),
            Some(&raw mut actual_bytes),
        )
    };
    let _ = unsafe { RegCloseKey(hkey) };
    if query_val.is_err() {
        return false;
    }
    let units = (actual_bytes as usize / 2).min(buf.len());
    let len = buf[..units]
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units);
    let value = String::from_utf16_lossy(&buf[..len]);
    is_startup_entry_enabled(Some(&value))
}

#[must_use]
pub fn set_launch_at_login(enabled: bool) -> bool {
    if enabled {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let command = format_startup_command(&exe);
        let wide: Vec<u16> = std::ffi::OsStr::new(&command)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let byte_len = wide.len() * 2;
        let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), byte_len) };
        let mut hkey = HKEY::default();
        let open_status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN_SUBKEY,
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &raw mut hkey,
                None,
            )
        };
        if open_status.is_err() {
            crate::diagnostics::log_error(
                "startup",
                "failed to open Windows startup Run registry key",
            );
            return false;
        }
        let set_status =
            unsafe { RegSetValueExW(hkey, STARTUP_VALUE_NAME, None, REG_SZ, Some(bytes)) };
        let _ = unsafe { RegCloseKey(hkey) };
        if set_status.is_err() {
            crate::diagnostics::log_error(
                "startup",
                "failed to write Windows startup Run registry entry",
            );
            return false;
        }
        true
    } else {
        let mut hkey = HKEY::default();
        let open_status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                RUN_SUBKEY,
                None,
                KEY_SET_VALUE,
                &mut hkey,
            )
        };
        if open_status == ERROR_FILE_NOT_FOUND {
            return true;
        }
        if open_status.is_err() {
            crate::diagnostics::log_error(
                "startup",
                "failed to open Windows startup Run registry key",
            );
            return false;
        }
        let del_status = unsafe { RegDeleteValueW(hkey, STARTUP_VALUE_NAME) };
        let _ = unsafe { RegCloseKey(hkey) };
        let ok = del_status.is_ok() || del_status == ERROR_FILE_NOT_FOUND;
        if !ok {
            crate::diagnostics::log_error(
                "startup",
                "failed to remove Windows startup Run registry entry",
            );
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{format_startup_command, is_startup_entry_enabled};

    #[test]
    fn startup_command_quotes_executable_path() {
        let formatted = format_startup_command(Path::new(
            "C:\\Users\\Demo\\AppData\\Local\\Otto OS\\otto-os.exe",
        ));
        assert_eq!(
            formatted,
            "\"C:\\Users\\Demo\\AppData\\Local\\Otto OS\\otto-os.exe\""
        );
    }

    #[test]
    fn startup_entry_requires_non_blank_value() {
        assert!(!is_startup_entry_enabled(None));
        assert!(!is_startup_entry_enabled(Some("")));
        assert!(!is_startup_entry_enabled(Some("   ")));
        assert!(is_startup_entry_enabled(Some("\"C:\\Otto\\otto-os.exe\"")));
    }
}
