use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, DragQueryPoint, HDROP, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
use windows::core::{PCWSTR, w};

use super::{DockEntry, DockItemKind, to_wide};

const MAX_DROPPED_FILES: u32 = 64;
const MAX_DROPPED_PATH_LEN: usize = 4096;

pub fn enable_drag_accept_files(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        DragAcceptFiles(hwnd, true);
    }
}

#[must_use]
pub fn extract_dropped_files(hdrop_val: usize) -> ((i32, i32), Vec<String>) {
    if hdrop_val == 0 {
        return ((0, 0), Vec::new());
    }
    let hdrop = HDROP(hdrop_val as *mut c_void);
    let mut pt = POINT::default();
    unsafe {
        let _ = DragQueryPoint(hdrop, &raw mut pt);
        let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None).min(MAX_DROPPED_FILES);
        let mut files = Vec::with_capacity(count as usize);
        let mut buf: Vec<u16> = Vec::new();
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            if len == 0 || len > MAX_DROPPED_PATH_LEN {
                continue;
            }
            buf.resize(len + 1, 0);
            let copied = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
            let s = String::from_utf16_lossy(&buf[..copied.min(len)]);
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

#[cfg(test)]
mod tests {
    use super::extract_dropped_files;

    #[test]
    fn null_hdrop_returns_empty_without_touching_shell() {
        assert_eq!(extract_dropped_files(0), ((0, 0), Vec::new()));
    }
}
