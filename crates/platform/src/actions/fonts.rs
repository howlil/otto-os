use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{AddFontResourceW, RemoveFontResourceW};
use windows::Win32::UI::WindowsAndMessaging::{HWND_BROADCAST, SendNotifyMessageW, WM_FONTCHANGE};
use windows::core::PCWSTR;

pub(super) static LOADED_APPLE_FONTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(super) fn font_path_wide_z(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub(super) fn broadcast_font_change() {
    unsafe {
        let _ = SendNotifyMessageW(HWND_BROADCAST, WM_FONTCHANGE, WPARAM(0), LPARAM(0));
    }
}

pub(super) fn record_loaded_font(registry: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    if registry.contains(&path) {
        return false;
    }
    registry.push(path);
    true
}

pub fn load_apple_fonts() {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let font_dir = PathBuf::from(local_app_data).join("Microsoft\\Windows\\Fonts");
    let Ok(mut loaded_registry) = LOADED_APPLE_FONTS.lock() else {
        return;
    };
    let mut loaded = false;
    for file in [
        "SF-Pro-Text-Regular.otf",
        "SF-Pro-Text-Medium.otf",
        "SF-Pro-Text-Semibold.otf",
    ] {
        let path = font_dir.join(file);
        if loaded_registry.contains(&path) || !path.exists() {
            continue;
        }
        let wide = font_path_wide_z(&path);
        unsafe {
            if AddFontResourceW(PCWSTR(wide.as_ptr())) > 0
                && record_loaded_font(&mut loaded_registry, path)
            {
                loaded = true;
            }
        }
    }
    drop(loaded_registry);
    if loaded {
        broadcast_font_change();
    }
}

pub fn remove_apple_fonts() {
    let Ok(mut loaded_registry) = LOADED_APPLE_FONTS.lock() else {
        return;
    };
    if loaded_registry.is_empty() {
        return;
    }
    let paths = std::mem::take(&mut *loaded_registry);
    drop(loaded_registry);
    let mut removed = false;
    for path in paths {
        let wide = font_path_wide_z(&path);
        unsafe {
            if RemoveFontResourceW(PCWSTR(wide.as_ptr())).as_bool() {
                removed = true;
            }
        }
    }
    if removed {
        broadcast_font_change();
    }
}
