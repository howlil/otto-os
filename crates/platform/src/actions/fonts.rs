use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::Win32::Graphics::Gdi::{
    AddFontResourceExW, FONT_RESOURCE_CHARACTERISTICS, FR_PRIVATE, RemoveFontResourceExW,
};
use windows::core::PCWSTR;

pub(super) static LOADED_APPLE_FONTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
pub(super) const PRIVATE_FONT_FLAGS: FONT_RESOURCE_CHARACTERISTICS = FR_PRIVATE;

pub(super) fn font_path_wide_z(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
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
            if AddFontResourceExW(PCWSTR(wide.as_ptr()), PRIVATE_FONT_FLAGS, None) > 0 {
                let _ = record_loaded_font(&mut loaded_registry, path);
            }
        }
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
    for path in paths {
        let wide = font_path_wide_z(&path);
        unsafe {
            let _ = RemoveFontResourceExW(PCWSTR(wide.as_ptr()), PRIVATE_FONT_FLAGS.0, None);
        }
    }
}
