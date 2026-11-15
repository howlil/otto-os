use std::ffi::OsStr;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;

use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW};
use windows::core::{PCWSTR, s, w};

pub const MIN_WINDOWS_11_BUILD: u32 = 22_621;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsBuild {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
}

#[must_use]
pub const fn is_supported_windows_build(major: u32, build: u32) -> bool {
    major > 10 || (major == 10 && build >= MIN_WINDOWS_11_BUILD)
}

#[must_use]
pub fn read_windows_build() -> Option<WindowsBuild> {
    type RtlGetVersionFn = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;

    unsafe {
        let ntdll = GetModuleHandleW(w!("ntdll.dll")).ok()?;
        let proc = GetProcAddress(ntdll, s!("RtlGetVersion"))?;
        let rtl_get_version: RtlGetVersionFn =
            std::mem::transmute::<unsafe extern "system" fn() -> isize, RtlGetVersionFn>(proc);
        let mut info = OSVERSIONINFOW {
            dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
            ..OSVERSIONINFOW::default()
        };
        if rtl_get_version(&raw mut info) != 0 {
            return None;
        }
        Some(WindowsBuild {
            major: info.dwMajorVersion,
            minor: info.dwMinorVersion,
            build: info.dwBuildNumber,
        })
    }
}

#[must_use]
pub fn is_supported_windows_version() -> bool {
    read_windows_build().is_none_or(|ver| is_supported_windows_build(ver.major, ver.build))
}

#[must_use]
pub fn current_windows_build_label() -> String {
    match read_windows_build() {
        Some(ver) => format!("{}.{}.{}", ver.major, ver.minor, ver.build),
        None => String::from("unknown"),
    }
}

pub fn show_error_dialog(title: &str, message: &str) {
    let title_wide = wide_string(title);
    let message_wide = wide_string(message);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

fn wide_string(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{is_supported_windows_build, read_windows_build};

    #[test]
    fn windows_10_and_windows_11_21h2_are_rejected() {
        assert!(!is_supported_windows_build(10, 17_763));
        assert!(!is_supported_windows_build(10, 19_045));
        assert!(!is_supported_windows_build(10, 22_000));
    }

    #[test]
    fn windows_11_22h2_and_newer_are_supported() {
        assert!(is_supported_windows_build(10, 22_621));
        assert!(is_supported_windows_build(10, 26_100));
        assert!(is_supported_windows_build(11, 100));
    }

    #[test]
    fn reads_current_host_windows_build() {
        let ver = read_windows_build().expect("RtlGetVersion returns build info");
        assert!(ver.major >= 10);
        assert!(ver.build > 0);
    }
}
