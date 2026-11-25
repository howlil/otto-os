use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_LSHIFT, VK_LWIN, VK_TAB, VK_VOLUME_DOWN, VK_VOLUME_UP,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
use windows::core::{PCWSTR, w};

use super::hotkeys::vk_char;
use super::keys::send_key_chord;

pub fn open_start_menu() {
    send_key_chord(&[VK_LWIN]);
}

pub fn open_calendar_notifications() {
    send_key_chord(&[VK_LWIN, vk_char(b'N')]);
}

pub fn open_task_view() {
    send_key_chord(&[VK_LWIN, VK_TAB]);
}

pub fn adjust_volume(wheel_delta: i16) {
    if crate::bar::audio::step_system_volume(wheel_delta).is_some() {
        return;
    }
    if wheel_delta > 0 {
        send_key_chord(&[VK_VOLUME_UP]);
    } else if wheel_delta < 0 {
        send_key_chord(&[VK_VOLUME_DOWN]);
    }
}

pub fn open_downloads_folder() {
    shell_exec(w!("shell:Downloads"), PCWSTR::null());
}

pub fn open_path(path: &Path) {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    shell_exec(PCWSTR(wide.as_ptr()), PCWSTR::null());
}

pub(super) fn shell_exec(file: PCWSTR, params: PCWSTR) {
    let _ = shell_exec_ok(file, params);
}

pub(crate) fn shell_exec_ok(file: PCWSTR, params: PCWSTR) -> bool {
    let instance =
        unsafe { ShellExecuteW(None, w!("open"), file, params, PCWSTR::null(), SW_SHOW) };
    instance.0 as isize > 32
}

pub(crate) fn open_external_url(url: &str) -> bool {
    let wide = windows::core::HSTRING::from(url);
    shell_exec_ok(PCWSTR(wide.as_ptr()), PCWSTR::null())
}

pub(crate) fn open_night_light_settings() {
    shell_exec(w!("ms-settings:nightlight"), PCWSTR::null());
}

pub(crate) fn open_power_settings() {
    shell_exec(w!("ms-settings:powersleep"), PCWSTR::null());
}

pub(crate) fn lock_screen() {
    send_key_chord(&[VK_LWIN, vk_char(b'L')]);
}

pub(crate) fn trigger_screenshot() {
    send_key_chord(&[VK_LWIN, VK_LSHIFT, vk_char(b'S')]);
}
