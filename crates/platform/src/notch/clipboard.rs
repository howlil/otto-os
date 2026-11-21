use std::ffi::c_void;
use std::mem::{align_of, size_of};
use std::slice;

use otto_core::{
    ClipboardPayload, MAX_CLIPBOARD_FILES, MAX_CLIPBOARD_TEXT_BYTES, MAX_SHELF_PATH_LEN,
    is_valid_shelf_file_path,
};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, POINT};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, RemoveClipboardFormatListener, SetClipboardData,
};
use windows::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::{DROPFILES, DragQueryFileW, HDROP};
use windows::core::w;

pub const WM_CLIPBOARDUPDATE: u32 = 0x031D;

#[derive(Debug)]
pub struct ClipboardListenerGuard {
    hwnd: HWND,
}

impl ClipboardListenerGuard {
    #[must_use]
    pub fn register(hwnd: isize) -> Option<Self> {
        if hwnd == 0 {
            return None;
        }
        let hwnd = HWND(hwnd as *mut c_void);
        unsafe { AddClipboardFormatListener(hwnd) }
            .ok()
            .map(|()| Self { hwnd })
    }
}

impl Drop for ClipboardListenerGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveClipboardFormatListener(self.hwnd);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardRead {
    Captured { seq: u32, payload: ClipboardPayload },
    SensitiveOrEmpty { seq: u32 },
    Busy,
}

#[must_use]
pub fn clipboard_sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

#[must_use]
pub fn read_clipboard_payload() -> ClipboardRead {
    let seq = clipboard_sequence_number();
    let Some(_guard) = ClipboardGuard::open() else {
        return ClipboardRead::Busy;
    };
    if clipboard_is_sensitive() {
        return ClipboardRead::SensitiveOrEmpty { seq };
    }
    if is_format_available(u32::from(CF_HDROP.0))
        && let Ok(handle) = unsafe { GetClipboardData(u32::from(CF_HDROP.0)) }
    {
        let hdrop = HDROP(handle.0);
        let paths = extract_hdrop_paths(hdrop, MAX_CLIPBOARD_FILES);
        if let Some(payload) = ClipboardPayload::Files(paths).normalize() {
            return ClipboardRead::Captured { seq, payload };
        }
    }
    if is_format_available(u32::from(CF_UNICODETEXT.0))
        && let Ok(handle) = unsafe { GetClipboardData(u32::from(CF_UNICODETEXT.0)) }
    {
        let hglobal = HGLOBAL(handle.0);
        if !hglobal.is_invalid()
            && let Some(text) = read_utf16_hglobal(hglobal, MAX_CLIPBOARD_TEXT_BYTES + 2)
            && let Some(payload) = ClipboardPayload::Text(text).normalize()
        {
            return ClipboardRead::Captured { seq, payload };
        }
    }
    ClipboardRead::SensitiveOrEmpty { seq }
}

#[must_use]
pub fn copy_clipboard_payload(payload: &ClipboardPayload) -> Option<u32> {
    let normalized = payload.clone().normalize()?;
    let (format, hglobal) = match &normalized {
        ClipboardPayload::Files(paths) => {
            let valid: Vec<String> = paths
                .iter()
                .filter(|path| is_valid_shelf_file_path(path))
                .take(MAX_CLIPBOARD_FILES)
                .cloned()
                .collect();
            let hglobal = build_dropfiles_hglobal(&valid)?;
            (u32::from(CF_HDROP.0), hglobal)
        }
        ClipboardPayload::Text(text) | ClipboardPayload::Link(text) => {
            let hglobal = build_unicodetext_hglobal(text, MAX_CLIPBOARD_TEXT_BYTES)?;
            (u32::from(CF_UNICODETEXT.0), hglobal)
        }
    };
    write_clipboard_hglobal(format, hglobal)
}

fn sensitive_clipboard_formats() -> (u32, u32, u32) {
    static FORMATS: std::sync::OnceLock<(u32, u32, u32)> = std::sync::OnceLock::new();
    *FORMATS.get_or_init(|| unsafe {
        (
            RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
            RegisterClipboardFormatW(w!("Clipboard Viewer Ignore")),
            RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
        )
    })
}

fn clipboard_is_sensitive() -> bool {
    let (exclude_fmt, ignore_fmt, history_fmt) = sensitive_clipboard_formats();
    if exclude_fmt != 0 && is_format_available(exclude_fmt) {
        return true;
    }
    if ignore_fmt != 0 && is_format_available(ignore_fmt) {
        return true;
    }
    if history_fmt != 0 && is_format_available(history_fmt) {
        let Ok(handle) = (unsafe { GetClipboardData(history_fmt) }) else {
            return true;
        };
        let hglobal = HGLOBAL(handle.0);
        if hglobal.is_invalid() {
            return true;
        }
        let size = unsafe { GlobalSize(hglobal) };
        if size < size_of::<u32>() {
            return true;
        }
        let Some(lock) = GlobalLockGuard::acquire(hglobal) else {
            return true;
        };
        let bytes = unsafe { slice::from_raw_parts(lock.ptr().cast::<u8>(), size_of::<u32>()) };
        return !can_include_in_history_bytes(bytes);
    }
    false
}

fn can_include_in_history_bytes(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    if rest.len() < 3 {
        return false;
    }
    u32::from_le_bytes([first, rest[0], rest[1], rest[2]]) != 0
}

fn is_format_available(format: u32) -> bool {
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

pub(crate) fn write_clipboard_hglobal(format: u32, hglobal: OwnedHGlobal) -> Option<u32> {
    {
        let _clipboard = ClipboardGuard::open()?;
        unsafe { EmptyClipboard() }.ok()?;
        let raw = hglobal.as_handle();
        unsafe { SetClipboardData(format, Some(raw)) }.ok()?;
        hglobal.disarm_os_owned();
    }
    Some(clipboard_sequence_number())
}

pub(crate) struct OwnedHGlobal(pub(crate) Option<HGLOBAL>);

impl OwnedHGlobal {
    pub(crate) fn as_handle(&self) -> HANDLE {
        match self.0 {
            Some(handle) => HANDLE(handle.0),
            None => HANDLE::default(),
        }
    }

    pub(crate) fn into_raw(mut self) -> HGLOBAL {
        self.0.take().unwrap_or_default()
    }

    pub(crate) fn disarm_os_owned(mut self) {
        self.0 = None;
    }
}

impl Drop for OwnedHGlobal {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            unsafe {
                let _ = GlobalFree(Some(handle));
            }
        }
    }
}

pub(crate) struct GlobalLockGuard {
    handle: HGLOBAL,
    ptr: *mut c_void,
}

impl GlobalLockGuard {
    pub(crate) fn acquire(handle: HGLOBAL) -> Option<Self> {
        let ptr = unsafe { GlobalLock(handle) };
        (!ptr.is_null()).then_some(Self { handle, ptr })
    }

    pub(crate) const fn ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl Drop for GlobalLockGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = GlobalUnlock(self.handle);
        }
    }
}

pub(crate) struct ClipboardGuard;

impl ClipboardGuard {
    pub(crate) fn open() -> Option<Self> {
        unsafe { OpenClipboard(None) }.is_ok().then_some(Self)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

pub(crate) fn build_dropfiles_bytes(paths: &[String]) -> Option<Vec<u8>> {
    if paths.is_empty() {
        return None;
    }
    let header_len = size_of::<DROPFILES>();
    let wide_units = encode_dropfiles_wide_units(paths)?;
    let total_bytes = header_len.checked_add(wide_units.len().checked_mul(size_of::<u16>())?)?;
    let mut buf = vec![0u8; total_bytes];
    let header = DROPFILES {
        pFiles: u32::try_from(header_len).ok()?,
        pt: POINT::default(),
        fNC: false.into(),
        fWide: true.into(),
    };
    unsafe {
        std::ptr::copy_nonoverlapping(
            (&raw const header).cast::<u8>(),
            buf.as_mut_ptr(),
            header_len,
        );
    }
    for (idx, unit) in wide_units.into_iter().enumerate() {
        let offset = header_len + idx * 2;
        buf[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }
    Some(buf)
}

fn encode_dropfiles_wide_units(paths: &[String]) -> Option<Vec<u16>> {
    if paths.is_empty() {
        return None;
    }
    let mut units = Vec::new();
    for path in paths {
        if !is_valid_shelf_file_path(path) {
            return None;
        }
        units.extend(path.encode_utf16());
        units.push(0);
    }
    units.push(0);
    Some(units)
}

pub(crate) fn encode_unicodetext_bytes(text: &str, max_bytes: usize) -> Option<Vec<u8>> {
    if text.is_empty() {
        return None;
    }
    let clamped = truncate_utf8_slice(text, max_bytes);
    let mut units: Vec<u16> = clamped.encode_utf16().collect();
    units.push(0);
    let mut bytes = Vec::with_capacity(units.len().checked_mul(size_of::<u16>())?);
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    Some(bytes)
}

fn truncate_utf8_slice(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

pub(crate) fn build_dropfiles_hglobal(paths: &[String]) -> Option<OwnedHGlobal> {
    let bytes = build_dropfiles_bytes(paths)?;
    alloc_hglobal_bytes(&bytes)
}

pub(crate) fn build_unicodetext_hglobal(text: &str, max_bytes: usize) -> Option<OwnedHGlobal> {
    let bytes = encode_unicodetext_bytes(text, max_bytes)?;
    alloc_hglobal_bytes(&bytes)
}

pub(crate) fn alloc_hglobal_bytes(bytes: &[u8]) -> Option<OwnedHGlobal> {
    let handle = unsafe { GlobalAlloc(GHND, bytes.len()) }.ok()?;
    let owned = OwnedHGlobal(Some(handle));
    {
        let lock = GlobalLockGuard::acquire(handle)?;
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), lock.ptr().cast::<u8>(), bytes.len());
        }
    }
    Some(owned)
}

pub(crate) fn read_utf16_hglobal(hglobal: HGLOBAL, max_units: usize) -> Option<String> {
    let size_bytes = unsafe { GlobalSize(hglobal) };
    if size_bytes < size_of::<u16>() {
        return None;
    }
    let lock = GlobalLockGuard::acquire(hglobal)?;
    if !(lock.ptr() as usize).is_multiple_of(align_of::<u16>()) {
        return None;
    }
    let units_available = (size_bytes / size_of::<u16>()).min(max_units);
    let slice = unsafe { slice::from_raw_parts(lock.ptr().cast::<u16>(), units_available) };
    let end = slice.iter().position(|&ch| ch == 0).unwrap_or(slice.len());
    Some(String::from_utf16_lossy(&slice[..end]))
}

pub(crate) fn read_ansi_hglobal(hglobal: HGLOBAL, max_bytes: usize) -> Option<String> {
    let size_bytes = unsafe { GlobalSize(hglobal) };
    if size_bytes == 0 {
        return None;
    }
    let lock = GlobalLockGuard::acquire(hglobal)?;
    let clamped = size_bytes.min(max_bytes);
    let slice = unsafe { slice::from_raw_parts(lock.ptr().cast::<u8>(), clamped) };
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    Some(String::from_utf8_lossy(&slice[..end]).into_owned())
}

pub(crate) fn extract_hdrop_paths(hdrop: HDROP, max_items: usize) -> Vec<String> {
    let count = unsafe { DragQueryFileW(hdrop, 0xFFFF_FFFF, None) };
    let limit = usize::try_from(count).unwrap_or(0).min(max_items);
    let mut out = Vec::with_capacity(limit);
    for idx in 0..limit {
        let Ok(file_idx) = u32::try_from(idx) else {
            break;
        };
        let char_len = unsafe { DragQueryFileW(hdrop, file_idx, None) } as usize;
        if char_len == 0 || char_len > MAX_SHELF_PATH_LEN {
            continue;
        }
        let mut buf = vec![0u16; char_len + 1];
        let written = unsafe { DragQueryFileW(hdrop, file_idx, Some(&mut buf)) } as usize;
        let path = String::from_utf16_lossy(&buf[..written.min(char_len)]);
        if is_valid_shelf_file_path(&path) {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use otto_core::ClipboardPayload;
    use windows::Win32::UI::Shell::HDROP;

    use super::{
        ClipboardListenerGuard, build_dropfiles_hglobal, build_unicodetext_hglobal,
        can_include_in_history_bytes, copy_clipboard_payload, encode_unicodetext_bytes,
        extract_hdrop_paths, read_utf16_hglobal,
    };

    #[test]
    fn clipboard_history_flag_respects_dword_zero_and_nonzero() {
        assert!(!can_include_in_history_bytes(&[]));
        assert!(!can_include_in_history_bytes(&[1, 0, 0]));
        assert!(!can_include_in_history_bytes(&0u32.to_le_bytes()));
        assert!(can_include_in_history_bytes(&1u32.to_le_bytes()));
        assert!(can_include_in_history_bytes(&42u32.to_le_bytes()));
    }

    #[test]
    fn encode_unicodetext_bytes_clamps_on_utf8_boundary_with_nul() {
        assert!(encode_unicodetext_bytes("", 16).is_none());
        let bytes = encode_unicodetext_bytes("halo 世界", 7).expect("clamped utf16");
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(units.last().copied(), Some(0));
        let decoded = String::from_utf16_lossy(&units[..units.len() - 1]);
        assert_eq!(decoded, "halo ");
    }

    #[test]
    fn hglobal_round_trips_unicode_and_dropfiles_in_memory() {
        let unicode = build_unicodetext_hglobal("https://otto.dev", 64).expect("unicode hglobal");
        let handle = unicode.0.expect("raw hglobal");
        assert_eq!(
            read_utf16_hglobal(handle, 64).as_deref(),
            Some("https://otto.dev")
        );
        drop(unicode);

        let paths = vec![String::from(r"C:\Temp\a.txt"), String::from(r"D:\b.png")];
        let drop_hglobal = build_dropfiles_hglobal(&paths).expect("dropfiles hglobal");
        let hdrop = HDROP(drop_hglobal.0.expect("raw hdrop").0);
        assert_eq!(extract_hdrop_paths(hdrop, 6), paths);
        drop(drop_hglobal);
    }

    #[test]
    fn copy_clipboard_payload_rejects_invalid_payloads_before_clipboard() {
        assert_eq!(
            copy_clipboard_payload(&ClipboardPayload::Text(String::new())),
            None
        );
        assert_eq!(
            copy_clipboard_payload(&ClipboardPayload::Text(String::from("   \n\t  "))),
            None
        );
        assert_eq!(
            copy_clipboard_payload(&ClipboardPayload::Link(String::from("ftp://example.com"))),
            None
        );
        assert_eq!(
            copy_clipboard_payload(&ClipboardPayload::Files(Vec::new())),
            None
        );
        assert_eq!(
            copy_clipboard_payload(&ClipboardPayload::Files(vec![String::from("bad\0path")])),
            None
        );
        assert_eq!(
            copy_clipboard_payload(&ClipboardPayload::Files(vec![
                "a".repeat(super::MAX_SHELF_PATH_LEN + 1),
            ])),
            None
        );
    }

    #[test]
    fn clipboard_listener_guard_rejects_null_hwnd() {
        assert!(ClipboardListenerGuard::register(0).is_none());
    }
}
