use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetSystemMetrics, HICON, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LoadIconW,
    LoadImageW, RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON, WM_CONTEXTMENU, WM_LBUTTONDBLCLK,
    WM_LBUTTONUP, WM_RBUTTONUP,
};
use windows::core::{PCWSTR, w};

pub const WM_APP_TRAY_ICON: u32 = 0x8000 + 32;

const TRAY_ICON_ID: u32 = 1;
const EMBEDDED_APP_ICON_ID: usize = 1;
const TRAY_TOOLTIP: &str = "Otto OS";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayIconEvent {
    LeftClick,
    RightClick,
}

#[must_use]
pub const fn classify_tray_icon_message(lparam: isize) -> Option<TrayIconEvent> {
    match (lparam as u32) & 0xFFFF {
        WM_LBUTTONUP | WM_LBUTTONDBLCLK => Some(TrayIconEvent::LeftClick),
        WM_RBUTTONUP | WM_CONTEXTMENU => Some(TrayIconEvent::RightClick),
        _ => None,
    }
}

fn fill_tooltip(tip: &mut [u16; 128], text: &str) {
    tip.fill(0);
    for (slot, unit) in tip.iter_mut().take(127).zip(text.encode_utf16()) {
        *slot = unit;
    }
}

fn load_tray_icon() -> (HICON, bool) {
    if let Ok(module) = unsafe { GetModuleHandleW(None) } {
        let width = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let height = unsafe { GetSystemMetrics(SM_CYSMICON) }.max(16);
        if let Ok(handle) = unsafe {
            LoadImageW(
                Some(module.into()),
                PCWSTR(EMBEDDED_APP_ICON_ID as *const u16),
                IMAGE_ICON,
                width,
                height,
                LR_DEFAULTCOLOR,
            )
        } && !handle.0.is_null()
        {
            return (HICON(handle.0), true);
        }
    }
    let fallback = unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default();
    (fallback, false)
}

fn notify_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        ..NOTIFYICONDATAW::default()
    }
}

fn add_tray_icon(hwnd: HWND, icon: HICON) -> bool {
    let mut data = notify_icon_data(hwnd);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_APP_TRAY_ICON;
    data.hIcon = icon;
    fill_tooltip(&mut data.szTip, TRAY_TOOLTIP);
    unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool()
}

#[derive(Debug)]
pub struct TrayIcon {
    hwnd: HWND,
    icon: HICON,
    owns_icon: bool,
    taskbar_created: Option<u32>,
}

impl TrayIcon {
    #[must_use]
    pub fn register(hwnd: isize) -> Option<Self> {
        if hwnd == 0 {
            return None;
        }
        let hwnd = HWND(hwnd as *mut c_void);
        let (icon, owns_icon) = load_tray_icon();
        if !add_tray_icon(hwnd, icon) {
            if owns_icon && !icon.0.is_null() {
                unsafe {
                    let _ = DestroyIcon(icon);
                }
            }
            return None;
        }
        let raw_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        Some(Self {
            hwnd,
            icon,
            owns_icon,
            taskbar_created: (raw_msg != 0).then_some(raw_msg),
        })
    }

    #[must_use]
    pub const fn taskbar_created_message(&self) -> Option<u32> {
        self.taskbar_created
    }

    #[must_use]
    pub fn re_add(&self) -> bool {
        add_tray_icon(self.hwnd, self.icon)
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let data = notify_icon_data(self.hwnd);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
        if self.owns_icon && !self.icon.0.is_null() {
            unsafe {
                let _ = DestroyIcon(self.icon);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TrayIconEvent, WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP,
        classify_tray_icon_message, fill_tooltip,
    };

    #[test]
    fn tray_messages_classify_left_and_right_interactions() {
        assert_eq!(
            classify_tray_icon_message(WM_LBUTTONUP as isize),
            Some(TrayIconEvent::LeftClick)
        );
        assert_eq!(
            classify_tray_icon_message(WM_LBUTTONDBLCLK as isize),
            Some(TrayIconEvent::LeftClick)
        );
        assert_eq!(
            classify_tray_icon_message(WM_RBUTTONUP as isize),
            Some(TrayIconEvent::RightClick)
        );
        assert_eq!(
            classify_tray_icon_message(WM_CONTEXTMENU as isize),
            Some(TrayIconEvent::RightClick)
        );
        assert_eq!(classify_tray_icon_message(0), None);
    }

    #[test]
    fn tooltip_encoding_is_null_terminated_and_bounded() {
        let mut tip = [1u16; 128];
        fill_tooltip(&mut tip, "Otto OS");
        assert_eq!(
            &tip[..8],
            &[
                u16::from(b'O'),
                u16::from(b't'),
                u16::from(b't'),
                u16::from(b'o'),
                u16::from(b' '),
                u16::from(b'O'),
                u16::from(b'S'),
                0
            ]
        );
        let long = "x".repeat(200);
        fill_tooltip(&mut tip, &long);
        assert_eq!(tip[126], u16::from(b'x'));
        assert_eq!(tip[127], 0);
    }
}
