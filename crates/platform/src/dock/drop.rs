use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, DragQueryPoint, HDROP, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
use windows::core::{PCWSTR, w};

use super::{DockEntry, DockItemKind, to_wide};

pub fn enable_drag_accept_files(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        DragAcceptFiles(hwnd, true);
    }
}

#[must_use]
pub fn extract_dropped_files(hdrop_val: usize) -> ((i32, i32), Vec<String>) {
    let hdrop = HDROP(hdrop_val as *mut c_void);
    let mut pt = POINT::default();
    unsafe {
        let _ = DragQueryPoint(hdrop, &raw mut pt);
        let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
        let mut files = Vec::with_capacity(count as usize);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            let s = String::from_utf16_lossy(&buf[..len]);
            if !s.is_empty() {
                files.push(s);
            }
        }
        DragFinish(hdrop);
        ((pt.x, pt.y), files)
    }
}

pub fn open_file_with_dock_entry(entry: &DockEntry, file_path: &str) {
    match &entry.kind {
        DockItemKind::App { exe_path } => {
            let wide_exe = to_wide(exe_path);
            let wide_file = to_wide(file_path);
            unsafe {
                let _ = ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(wide_exe.as_ptr()),
                    PCWSTR(wide_file.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOW,
                );
            }
        }
        DockItemKind::Trash { .. } => {
            let _ = crate::dock::trash::recycle_path(file_path);
        }
        DockItemKind::Finder | DockItemKind::Downloads => {
            let wide_file = to_wide(&format!("/select,\"{file_path}\""));
            unsafe {
                let _ = ShellExecuteW(
                    None,
                    w!("open"),
                    w!("explorer.exe"),
                    PCWSTR(wide_file.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOW,
                );
            }
        }
        DockItemKind::Apps | DockItemKind::Overflow { .. } => {}
    }
}
