use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;

use otto_core::{AppleIconKind, PixelLimits, bgra_layout};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDIBits,
    GetObjectW, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, REG_VALUE_TYPE, RegCloseKey,
    RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_SYSICONINDEX, SHGetFileInfoW, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GCLP_HICON, GetClassLongPtrW, GetIconInfo, HICON, ICON_BIG, ICONINFO, SMTO_ABORTIFHUNG,
    SendMessageTimeoutW, WM_GETICON,
};
use windows::core::PCWSTR;

use crate::gdi::ScreenDc;

use super::{DockEntry, DockItemKind, IconBitmap, to_wide};

use super::pins::USER_MODEL_KEY_PREFIX;

#[must_use]
pub fn extract_dock_icon(entry: &DockEntry) -> Option<IconBitmap> {
    if entry.apple_icon.is_some() {
        return None;
    }
    let DockItemKind::App { exe_path } = &entry.kind else {
        return None;
    };
    let shell_icon = entry
        .key
        .starts_with(USER_MODEL_KEY_PREFIX)
        .then(|| crate::notch::shelf::extract_shell_icon(exe_path, 256))
        .flatten()
        .map(|icon| IconBitmap {
            width: icon.width,
            height: icon.height,
            pixels: icon.bgra,
        });
    let raw = shell_icon
        .or_else(|| file_icon_bitmap(exe_path))
        .or_else(|| entry.hwnds.first().copied().and_then(window_icon_bitmap))?;
    Some(normalize_to_macos_squircle(&raw))
}

#[must_use]
pub fn extract_window_icon(hwnd_val: isize) -> Option<IconBitmap> {
    window_icon_bitmap(hwnd_val).map(|raw| normalize_to_macos_squircle(&raw))
}

#[must_use]
pub fn extract_app_icon_bitmap(icon_source: &str, exe_hint: Option<&str>) -> Option<IconBitmap> {
    let from_shell = |path: &str| {
        crate::notch::shelf::extract_shell_icon(path, 64).map(|icon| IconBitmap {
            width: icon.width,
            height: icon.height,
            pixels: icon.bgra,
        })
    };
    from_shell(icon_source)
        .or_else(|| exe_hint.and_then(from_shell))
        .or_else(|| exe_hint.and_then(file_icon_bitmap))
        .or_else(|| {
            (!icon_source.starts_with("shell:")
                && !icon_source.starts_with("aumid:")
                && icon_source.contains(['\\', '/', ':']))
            .then(|| file_icon_bitmap(icon_source))
            .flatten()
        })
        .or_else(|| {
            (!icon_source.contains(['\\', '/', ':']))
                .then(|| extract_bare_stem_icon(icon_source))
                .flatten()
        })
}

fn extract_bare_stem_icon(raw_stem: &str) -> Option<IconBitmap> {
    let stem = otto_core::normalize_app_key(raw_stem);
    if stem.is_empty() {
        return None;
    }
    let from_shell = |path: &str| {
        crate::notch::shelf::extract_shell_icon(path, 64).map(|icon| IconBitmap {
            width: icon.width,
            height: icon.height,
            pixels: icon.bgra,
        })
    };
    for win in crate::enumerate_windows(0) {
        if (otto_core::normalize_app_key(&win.executable) == stem
            || otto_core::normalize_app_key(&win.app_name) == stem)
            && let Some(bmp) = (!win.executable.is_empty())
                .then(|| from_shell(&win.executable).or_else(|| file_icon_bitmap(&win.executable)))
                .flatten()
                .or_else(|| window_icon_bitmap(win.identity.hwnd))
        {
            return Some(bmp);
        }
    }
    let exe_path = resolve_exe_from_app_paths_or_path(&stem)?;
    from_shell(&exe_path).or_else(|| file_icon_bitmap(&exe_path))
}

fn resolve_exe_from_app_paths_or_path(stem: &str) -> Option<String> {
    let exe_name = format!("{stem}.exe");
    let subkey = to_wide(&format!(
        r"Software\Microsoft\Windows\CurrentVersion\App Paths\{exe_name}"
    ));
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Some(path) = query_app_path_registry(root, &subkey)
            && Path::new(&path).is_file()
        {
            return Some(path);
        }
    }
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(&exe_name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn query_app_path_registry(root: HKEY, subkey: &[u16]) -> Option<String> {
    let mut hkey = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            root,
            PCWSTR(subkey.as_ptr()),
            None,
            KEY_QUERY_VALUE,
            &raw mut hkey,
        )
    };
    if status.is_err() {
        return None;
    }
    let mut reg_type = REG_VALUE_TYPE::default();
    let mut bytes = 0u32;
    let query_len = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR::null(),
            None,
            Some(&raw mut reg_type),
            None,
            Some(&raw mut bytes),
        )
    };
    if query_len.is_err() || !(2..=4096).contains(&bytes) {
        let _ = unsafe { RegCloseKey(hkey) };
        return None;
    }
    let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
    let mut actual_bytes = (buf.len() * 2) as u32;
    let query_val = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR::null(),
            None,
            Some(&raw mut reg_type),
            Some(buf.as_mut_ptr().cast()),
            Some(&raw mut actual_bytes),
        )
    };
    let _ = unsafe { RegCloseKey(hkey) };
    if query_val.is_err() {
        return None;
    }
    let units = (actual_bytes as usize / 2).min(buf.len());
    let len = buf[..units]
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units);
    let raw = String::from_utf16_lossy(&buf[..len]);
    let trimmed = raw.trim().trim_matches('"').trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(super) fn file_icon_bitmap(path: &str) -> Option<IconBitmap> {
    let wide = to_wide(path);
    let mut info = SHFILEINFOW::default();
    let res = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&raw mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX,
        )
    };
    if res == 0 {
        return None;
    }
    image_list_icon_bitmap(info.iIcon)
}

pub(super) fn image_list_icon_bitmap(index: i32) -> Option<IconBitmap> {
    unsafe {
        if let Ok(jumbo) = SHGetImageList::<IImageList>(SHIL_JUMBO as i32)
            && let Ok(hicon) = jumbo.GetIcon(index, ILD_TRANSPARENT.0)
        {
            let bmp = hicon_to_bitmap(hicon);
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(hicon);
            if let Some(b) = bmp
                && !is_sparse_jumbo_icon(&b)
            {
                return Some(b);
            }
        }
        let extra = SHGetImageList::<IImageList>(SHIL_EXTRALARGE as i32).ok()?;
        let hicon = extra.GetIcon(index, ILD_TRANSPARENT.0).ok()?;
        let bmp = hicon_to_bitmap(hicon);
        let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(hicon);
        bmp
    }
}

pub(super) fn is_sparse_jumbo_icon(bmp: &IconBitmap) -> bool {
    if bmp.width < 128 || bmp.height < 128 {
        return false;
    }
    let stride = bmp.width as usize * 4;
    for y in 72..bmp.height as usize {
        let row = &bmp.pixels[y * stride..(y + 1) * stride];
        if row.chunks_exact(4).any(|px| px[3] > 16) {
            return false;
        }
    }
    true
}

pub(super) fn window_icon_bitmap(hwnd_val: isize) -> Option<IconBitmap> {
    let hwnd = HWND(hwnd_val as *mut c_void);
    let mut result = 0usize;
    let handle = unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(ICON_BIG as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            50,
            Some(&raw mut result),
        );
        if result == 0 {
            GetClassLongPtrW(hwnd, GCLP_HICON)
        } else {
            result
        }
    };
    if handle == 0 {
        return None;
    }
    let hicon = HICON(handle as *mut c_void);
    hicon_to_bitmap(hicon)
}

pub(super) fn hicon_to_bitmap(hicon: HICON) -> Option<IconBitmap> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(hicon, &raw mut info).ok()?;
        let image = read_color_bitmap(&info);
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        image
    }
}

pub(super) unsafe fn read_color_bitmap(info: &ICONINFO) -> Option<IconBitmap> {
    if info.hbmColor.0.is_null() {
        return None;
    }
    unsafe {
        let mut bitmap = BITMAP::default();
        GetObjectW(
            HGDIOBJ(info.hbmColor.0),
            size_of::<BITMAP>() as i32,
            Some((&raw mut bitmap).cast()),
        );
        let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
        let layout = bgra_layout(
            u32::try_from(width).ok()?,
            u32::try_from(height).ok()?,
            PixelLimits::ICON,
        )
        .ok()?;
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut pixels = vec![0u8; layout.byte_len];
        let screen = ScreenDc::acquire()?;
        let rows = GetDIBits(
            screen.handle(),
            info.hbmColor,
            0,
            layout.height,
            Some(pixels.as_mut_ptr().cast()),
            &raw mut header,
            DIB_RGB_COLORS,
        );
        if rows == 0 {
            return None;
        }
        premultiply(&mut pixels);
        Some(IconBitmap {
            width: width as u32,
            height: height as u32,
            pixels,
        })
    }
}

pub(super) fn premultiply(pixels: &mut [u8]) {
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
    for pixel in pixels.chunks_exact_mut(4) {
        if !has_alpha {
            pixel[3] = 255;
            continue;
        }
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
}

pub(super) fn normalize_to_macos_squircle(src: &IconBitmap) -> IconBitmap {
    const OUT: usize = 256;
    const TILE_MIN: f32 = 18.0;
    const TILE_MAX: f32 = 238.0;
    const TILE_SIZE: f32 = TILE_MAX - TILE_MIN;
    const RADIUS: f32 = 49.0;

    let (min_x, min_y, max_x, max_y, opaque_ratio) = trim_bounds(src);
    let trim_w = (max_x - min_x + 1).max(1) as f32;
    let trim_h = (max_y - min_y + 1).max(1) as f32;
    let aspect = (trim_w / trim_h).clamp(0.5, 2.0);
    let is_full_bleed = opaque_ratio > 0.87 && (0.88..=1.14).contains(&aspect);

    let (art_min, art_size) = if is_full_bleed {
        (TILE_MIN, TILE_SIZE)
    } else {
        let inset = 34.0;
        (TILE_MIN + inset, TILE_SIZE - inset * 2.0)
    };

    let mut out = vec![0u8; OUT * OUT * 4];
    for y in 0..OUT {
        let fy = y as f32 + 0.5;
        let t_grad = ((fy - TILE_MIN) / TILE_SIZE).clamp(0.0, 1.0);
        for x in 0..OUT {
            let fx = x as f32 + 0.5;
            let idx = (y * OUT + x) * 4;

            let d = rounded_box_sdf(fx, fy, TILE_MIN, TILE_MAX, RADIUS);
            let tile_cov = (0.75 - d).clamp(0.0, 1.0);

            let mut dst = [0.0_f32; 4];
            if is_full_bleed || tile_cov < 1.0 {
                let shadow_d =
                    rounded_box_sdf(fx, fy - 4.0, TILE_MIN + 2.0, TILE_MAX - 2.0, RADIUS);
                if shadow_d < 8.0 {
                    let sa = ((8.0 - shadow_d) / 10.0).clamp(0.0, 1.0).powi(2) * 0.34;
                    dst = [0.0, 0.0, 0.0, sa];
                }
            }

            if tile_cov > 0.0 {
                let mut px = if is_full_bleed {
                    let u = ((fx - art_min) / art_size).clamp(0.0, 1.0);
                    let v = ((fy - art_min) / art_size).clamp(0.0, 1.0);
                    sample_bilinear(
                        src,
                        min_x as f32 + u * (trim_w - 1.0),
                        min_y as f32 + v * (trim_h - 1.0),
                    )
                } else {
                    let top_c = [0.97_f32, 0.96, 0.95];
                    let bot_c = [0.85_f32, 0.84, 0.82];
                    let mut plate = [
                        bot_c[0].mul_add(t_grad, top_c[0] * (1.0 - t_grad)),
                        bot_c[1].mul_add(t_grad, top_c[1] * (1.0 - t_grad)),
                        bot_c[2].mul_add(t_grad, top_c[2] * (1.0 - t_grad)),
                        1.0,
                    ];

                    if fx >= art_min
                        && fx <= art_min + art_size
                        && fy >= art_min
                        && fy <= art_min + art_size
                    {
                        let u = (fx - art_min) / art_size;
                        let v = (fy - art_min) / art_size;
                        let art = sample_bilinear(
                            src,
                            min_x as f32 + u * (trim_w - 1.0),
                            min_y as f32 + v * (trim_h - 1.0),
                        );
                        plate = blend_over(plate, art);
                    }
                    plate
                };

                let rim = (1.35 - d.abs()).clamp(0.0, 1.0);
                if rim > 0.0 && d <= 0.0 {
                    let highlight = (1.0 - t_grad * 0.7) * 0.28 * rim;
                    px[0] = (px[0] + highlight).min(1.0);
                    px[1] = (px[1] + highlight).min(1.0);
                    px[2] = (px[2] + highlight).min(1.0);
                }

                px[0] *= tile_cov;
                px[1] *= tile_cov;
                px[2] *= tile_cov;
                px[3] *= tile_cov;
                dst = blend_over(dst, px);
            }

            out[idx] = (dst[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out[idx + 1] = (dst[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out[idx + 2] = (dst[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out[idx + 3] = (dst[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }

    IconBitmap {
        width: OUT as u32,
        height: OUT as u32,
        pixels: out,
    }
}

pub(super) fn rounded_box_sdf(px: f32, py: f32, min_v: f32, max_v: f32, r: f32) -> f32 {
    let cx = (min_v + max_v) * 0.5;
    let half = (max_v - min_v) * 0.5 - r;
    let dx = (px - cx).abs() - half;
    let dy = (py - cx).abs() - half;
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    let inside = dx.max(dy).min(0.0);
    outside + inside - r
}

pub(super) fn trim_bounds(bmp: &IconBitmap) -> (u32, u32, u32, u32, f32) {
    let (w, h) = (bmp.width, bmp.height);
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut opaque_count = 0u32;

    for y in 0..h {
        for x in 0..w {
            let a = bmp.pixels[((y * w + x) * 4 + 3) as usize];
            if a > 20 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                if a > 220 {
                    opaque_count += 1;
                }
            }
        }
    }

    if min_x > max_x || min_y > max_y {
        return (0, 0, w.saturating_sub(1), h.saturating_sub(1), 1.0);
    }
    let box_area = ((max_x - min_x + 1) * (max_y - min_y + 1)).max(1);
    let ratio = opaque_count as f32 / box_area as f32;
    (min_x, min_y, max_x, max_y, ratio)
}

pub(super) fn sample_bilinear(bmp: &IconBitmap, sx: f32, sy: f32) -> [f32; 4] {
    let max_x = bmp.width.saturating_sub(1) as f32;
    let max_y = bmp.height.saturating_sub(1) as f32;
    let x = sx.clamp(0.0, max_x);
    let y = sy.clamp(0.0, max_y);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(bmp.width.saturating_sub(1));
    let y1 = (y0 + 1).min(bmp.height.saturating_sub(1));
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = read_px(bmp, x0, y0);
    let p10 = read_px(bmp, x1, y0);
    let p01 = read_px(bmp, x0, y1);
    let p11 = read_px(bmp, x1, y1);

    let mut out = [0.0_f32; 4];
    for i in 0..4 {
        let top = p00[i] * (1.0 - fx) + p10[i] * fx;
        let bot = p01[i] * (1.0 - fx) + p11[i] * fx;
        out[i] = top * (1.0 - fy) + bot * fy;
    }
    out
}

pub(super) fn read_px(bmp: &IconBitmap, x: u32, y: u32) -> [f32; 4] {
    const INV_255: f32 = 1.0 / 255.0;
    let i = ((y * bmp.width + x) * 4) as usize;
    [
        f32::from(bmp.pixels[i]) * INV_255,
        f32::from(bmp.pixels[i + 1]) * INV_255,
        f32::from(bmp.pixels[i + 2]) * INV_255,
        f32::from(bmp.pixels[i + 3]) * INV_255,
    ]
}

pub(super) fn blend_over(dst: [f32; 4], src: [f32; 4]) -> [f32; 4] {
    let inv = 1.0 - src[3];
    [
        src[0] + dst[0] * inv,
        src[1] + dst[1] * inv,
        src[2] + dst[2] * inv,
        src[3] + dst[3] * inv,
    ]
}

pub(super) fn classify_apple_icon(exe_path: &str) -> Option<AppleIconKind> {
    let stem = Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())?
        .to_ascii_lowercase();
    match stem.as_str() {
        "explorer" => Some(AppleIconKind::Finder),
        "windowsterminal" | "wt" | "powershell" | "pwsh" | "cmd" | "alacritty" | "wezterm-gui" => {
            Some(AppleIconKind::Terminal)
        }
        "systemsettings" | "control" => Some(AppleIconKind::Settings),
        "msedge" | "chrome" | "brave" | "firefox" | "arc" | "opera" => Some(AppleIconKind::Browser),
        "notepad" | "notespace" | "obsidian" => Some(AppleIconKind::Notes),
        "code" | "code - insiders" | "cursor" | "windsurf" | "zed" | "devenv" => {
            Some(AppleIconKind::Code)
        }
        "spotify" | "music" | "applemusic" | "itunes" | "youtube music" | "youtubemusic"
        | "amazon music" | "amazonmusic" | "tidal" | "deezer" | "qobuz" | "plexamp"
        | "wmplayer" | "foobar2000" | "musicbee" | "mediamonkey" | "winamp" | "aimp" | "vlc" => {
            Some(AppleIconKind::Music)
        }
        _ => None,
    }
}
