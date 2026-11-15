use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;
use std::slice;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use otto_core::{
    MAX_SHELF_ITEMS, MAX_SHELF_PATH_LEN, MAX_SHELF_TEXT_BYTES, PixelLimits, SHELF_THUMBNAIL_MAX_PX,
    ShelfPayload, ShelfThumbnail, bgra_layout, decode_dropped_text, is_openable_shelf_url,
    is_unc_shelf_path, is_valid_shelf_file_path,
};
use windows::Win32::Foundation::{DRAGDROP_S_DROP, HWND, S_OK, SIZE};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDIBits,
    GetObjectW, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, DVASPECT_CONTENT, FORMATETC,
    IDataObject, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Ole::{
    CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE,
    DoDragDrop, IDropSource, IDropTarget, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_PATHMUSTEXIST, FileOpenDialog, HDROP,
    IFileOpenDialog, ILFree, IShellItemImageFactory, SHCreateItemFromParsingName,
    SHOpenFolderAndSelectItems, SHParseDisplayName, SIGDN_FILESYSPATH, SIIGBF_BIGGERSIZEOK,
    SIIGBF_RESIZETOFIT, SIIGBF_THUMBNAILONLY,
};
use windows::Win32::UI::WindowsAndMessaging::WM_APP;
use windows::core::{PCWSTR, PWSTR, w};

use crate::actions::shell_exec_ok;
use crate::clipboard::{
    alloc_hglobal_bytes, build_dropfiles_bytes, build_dropfiles_hglobal, build_unicodetext_hglobal,
    encode_unicodetext_bytes as encode_clipboard_unicodetext_bytes, extract_hdrop_paths,
    read_ansi_hglobal, read_utf16_hglobal, write_clipboard_hglobal,
};
use crate::com::ComGuard;
use crate::gdi::ScreenDc;

pub const WM_APP_SHELF_EVENT: u32 = WM_APP + 26;

const MAX_BATCH_DROP_ITEMS: usize = MAX_SHELF_ITEMS * 4;
const MAX_URL_UTF16_UNITS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfDropEvent {
    DragEnter,
    DragLeave,
    Drop(Vec<ShelfPayload>),
}

#[derive(Debug, Clone, Default)]
pub struct ShelfDropQueue {
    inner: Arc<Mutex<VecDeque<ShelfDropEvent>>>,
}

impl ShelfDropQueue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, event: ShelfDropEvent) {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.push_back(event);
    }

    #[must_use]
    pub fn drain(&self) -> Vec<ShelfDropEvent> {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.drain(..).collect()
    }
}

#[derive(Debug, Default)]
pub struct ShelfDropRegion {
    rect: Mutex<Option<(i32, i32, i32, i32)>>,
}

impl ShelfDropRegion {
    #[must_use]
    pub fn full_window() -> Arc<Self> {
        Arc::new(Self {
            rect: Mutex::new(None),
        })
    }

    #[must_use]
    pub fn restricted(left: i32, top: i32, right: i32, bottom: i32) -> Arc<Self> {
        Arc::new(Self {
            rect: Mutex::new(Some((left, top, right, bottom))),
        })
    }

    pub fn set_rect(&self, left: i32, top: i32, right: i32, bottom: i32) {
        let mut guard = self
            .rect
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some((left, top, right, bottom));
    }

    #[must_use]
    pub fn contains_client_point(&self, x: i32, y: i32) -> bool {
        let guard = self
            .rect
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *guard {
            None => true,
            Some((left, top, right, bottom)) => x >= left && x < right && y >= top && y < bottom,
        }
    }
}

pub struct ShelfDropTargetGuard {
    hwnd: HWND,
}

impl Drop for ShelfDropTargetGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = RevokeDragDrop(self.hwnd);
        }
    }
}

#[must_use]
pub fn register_shelf_drop_target(
    hwnd: isize,
    queue: ShelfDropQueue,
    region: Arc<ShelfDropRegion>,
    local_drag: Arc<AtomicBool>,
) -> Option<ShelfDropTargetGuard> {
    if hwnd == 0 {
        return None;
    }
    let hwnd = HWND(hwnd as *mut c_void);
    let target: IDropTarget =
        com_impl::ShelfDropTarget::new(hwnd, queue, region, local_drag).into();
    unsafe { RegisterDragDrop(hwnd, &target) }
        .is_ok()
        .then_some(ShelfDropTargetGuard { hwnd })
}

#[must_use]
pub fn pick_shelf_files() -> Vec<String> {
    let _com = ComGuard::single_threaded();
    let Ok(dialog) = (unsafe {
        CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
    }) else {
        return Vec::new();
    };
    let Ok(options) = (unsafe { dialog.GetOptions() }) else {
        return Vec::new();
    };
    let desired = options | FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST;
    if unsafe { dialog.SetOptions(desired) }.is_err() {
        return Vec::new();
    }
    if unsafe { dialog.Show(None) }.is_err() {
        return Vec::new();
    }
    let Ok(items) = (unsafe { dialog.GetResults() }) else {
        return Vec::new();
    };
    let Ok(count) = (unsafe { items.GetCount() }) else {
        return Vec::new();
    };
    let limit = usize::try_from(count)
        .unwrap_or(0)
        .min(MAX_BATCH_DROP_ITEMS);
    let mut paths = Vec::with_capacity(limit);
    for idx in 0..limit {
        let Ok(item_idx) = u32::try_from(idx) else {
            break;
        };
        let Ok(item) = (unsafe { items.GetItemAt(item_idx) }) else {
            continue;
        };
        let Ok(raw_name) = (unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }) else {
            continue;
        };
        let owned = CoTaskStr(raw_name);
        if let Some(path) = owned.to_string_bounded(MAX_SHELF_PATH_LEN)
            && is_valid_shelf_file_path(&path)
        {
            paths.push(path);
        }
    }
    paths
}

#[must_use]
pub fn inspect_shelf_file(path: &str, max_px: u32) -> (bool, Option<ShelfThumbnail>) {
    extract_shell_thumbnail(path, max_px, true)
}

#[must_use]
pub(crate) fn extract_shell_thumbnail(
    path: &str,
    max_px: u32,
    thumbnail_only: bool,
) -> (bool, Option<ShelfThumbnail>) {
    if !is_valid_shelf_file_path(path) || is_unc_shelf_path(path) {
        return (false, None);
    }
    let fs_path = Path::new(path);
    let Ok(meta) = std::fs::metadata(fs_path) else {
        return (false, None);
    };
    let is_dir = meta.is_dir();
    let _com = ComGuard::single_threaded();
    let wide = to_wide_nul(path);
    let Ok(factory) = (unsafe {
        SHCreateItemFromParsingName::<_, _, IShellItemImageFactory>(PCWSTR(wide.as_ptr()), None)
    }) else {
        return (is_dir, None);
    };
    let target_px = i32::try_from(max_px.clamp(16, SHELF_THUMBNAIL_MAX_PX)).unwrap_or(64);
    let mut flags = SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK;
    if thumbnail_only {
        flags |= SIIGBF_THUMBNAILONLY;
    }
    let Ok(bitmap) = (unsafe {
        factory.GetImage(
            SIZE {
                cx: target_px,
                cy: target_px,
            },
            flags,
        )
    }) else {
        return (is_dir, None);
    };
    let owned = OwnedHBitmap(bitmap);
    let thumb = hbitmap_to_thumbnail(owned.0);
    (is_dir, thumb)
}

pub fn open_shelf_payload(payload: &ShelfPayload) -> bool {
    match payload {
        ShelfPayload::File(path) => {
            if !is_valid_shelf_file_path(path) {
                return false;
            }
            let wide = to_wide_nul(path);
            shell_exec_ok(PCWSTR(wide.as_ptr()), PCWSTR::null())
        }
        ShelfPayload::Url(url) => {
            if !is_openable_shelf_url(url) {
                return false;
            }
            let wide = to_wide_nul(url);
            shell_exec_ok(PCWSTR(wide.as_ptr()), PCWSTR::null())
        }
        ShelfPayload::Text(_) => copy_shelf_payload(payload),
    }
}

pub fn reveal_shelf_payload(payload: &ShelfPayload) -> bool {
    let ShelfPayload::File(path) = payload else {
        return false;
    };
    if !is_valid_shelf_file_path(path) {
        return false;
    }
    let _com = ComGuard::single_threaded();
    let wide = to_wide_nul(path);
    let mut raw_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &raw mut raw_pidl, 0, None) }
        .is_err()
    {
        return false;
    }
    let pidl = OwnedPidl(raw_pidl);
    if pidl.0.is_null() {
        return false;
    }
    unsafe { SHOpenFolderAndSelectItems(pidl.0.cast_const(), None, 0) }.is_ok()
}

pub fn copy_shelf_payload(payload: &ShelfPayload) -> bool {
    let (format, hglobal) = match payload {
        ShelfPayload::File(path) => {
            if !is_valid_shelf_file_path(path) {
                return false;
            }
            let Some(hglobal) = build_dropfiles_hglobal(slice::from_ref(path)) else {
                return false;
            };
            (u32::from(CF_HDROP.0), hglobal)
        }
        ShelfPayload::Text(text) | ShelfPayload::Url(text) => {
            if text.is_empty() {
                return false;
            }
            let Some(hglobal) = build_unicodetext_hglobal(text, MAX_SHELF_TEXT_BYTES) else {
                return false;
            };
            (u32::from(CF_UNICODETEXT.0), hglobal)
        }
    };
    write_clipboard_hglobal(format, hglobal).is_some()
}

pub fn start_shelf_drag(_hwnd: isize, payload: &ShelfPayload) -> bool {
    let Some(data_object) = com_impl::ShelfDataObject::from_payload(payload) else {
        return false;
    };
    let data_object: IDataObject = data_object.into();
    let drop_source: IDropSource = com_impl::ShelfDropSource.into();
    let allowed = match payload {
        ShelfPayload::File(_) => DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK,
        ShelfPayload::Text(_) | ShelfPayload::Url(_) => DROPEFFECT_COPY,
    };
    let mut effect = DROPEFFECT_NONE;
    let hr = unsafe { DoDragDrop(&data_object, &drop_source, allowed, &raw mut effect) };
    hr == DRAGDROP_S_DROP && effect != DROPEFFECT_NONE
}

fn encode_unicodetext_bytes(text: &str) -> Option<Vec<u8>> {
    encode_clipboard_unicodetext_bytes(text, MAX_SHELF_TEXT_BYTES)
}

fn encode_ansi_url_bytes(text: &str) -> Option<Vec<u8>> {
    if !is_openable_shelf_url(text) {
        return None;
    }
    let mut bytes: Vec<u8> = text
        .as_bytes()
        .iter()
        .copied()
        .take(MAX_SHELF_PATH_LEN)
        .collect();
    bytes.push(0);
    Some(bytes)
}

fn hbitmap_to_thumbnail(hbitmap: HBITMAP) -> Option<ShelfThumbnail> {
    if hbitmap.0.is_null() {
        return None;
    }
    let mut bmp = BITMAP::default();
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(hbitmap.0),
            i32::try_from(size_of::<BITMAP>()).ok()?,
            Some((&raw mut bmp).cast()),
        )
    };
    if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
        return None;
    }
    let width = u32::try_from(bmp.bmWidth).ok()?;
    let height = u32::try_from(bmp.bmHeight).ok()?;
    let layout = bgra_layout(width, height, PixelLimits::ICON).ok()?;
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).ok()?,
            biWidth: bmp.bmWidth,
            biHeight: -bmp.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    let mut pixels = vec![0u8; layout.byte_len];
    let screen = ScreenDc::acquire()?;
    let rows = unsafe {
        GetDIBits(
            screen.handle(),
            hbitmap,
            0,
            layout.height,
            Some(pixels.as_mut_ptr().cast()),
            &raw mut header,
            DIB_RGB_COLORS,
        )
    };
    if rows == 0 {
        return None;
    }
    premultiply_bgra(&mut pixels);
    Some(ShelfThumbnail {
        width: layout.width,
        height: layout.height,
        bgra: pixels,
    })
}

fn premultiply_bgra(pixels: &mut [u8]) {
    let has_alpha = pixels.chunks_exact(4).any(|px| px[3] != 0);
    for px in pixels.chunks_exact_mut(4) {
        if !has_alpha {
            px[3] = 255;
            continue;
        }
        let alpha = u32::from(px[3]);
        for channel in &mut px[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
}

fn to_wide_nul(text: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    wide
}

fn formatetc_hglobal(cf_format: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: cf_format,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1),
    }
}

fn url_wide_clipboard_format() -> u16 {
    let id = unsafe { RegisterClipboardFormatW(w!("UniformResourceLocatorW")) };
    u16::try_from(id).unwrap_or(0)
}

fn url_ansi_clipboard_format() -> u16 {
    let id = unsafe { RegisterClipboardFormatW(w!("UniformResourceLocator")) };
    u16::try_from(id).unwrap_or(0)
}

fn extract_hdrop_payloads(data_obj: &IDataObject) -> Vec<ShelfPayload> {
    let fmt = formatetc_hglobal(CF_HDROP.0);
    let Ok(medium) = (unsafe { data_obj.GetData(&raw const fmt) }) else {
        return Vec::new();
    };
    let medium = OwnedStgMedium(medium);
    if medium.0.tymed != u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1) {
        return Vec::new();
    }
    let hglobal = unsafe { medium.0.u.hGlobal };
    if hglobal.is_invalid() {
        return Vec::new();
    }
    let hdrop = HDROP(hglobal.0);
    extract_hdrop_paths(hdrop, MAX_BATCH_DROP_ITEMS)
        .into_iter()
        .map(ShelfPayload::File)
        .collect()
}

fn extract_text_or_url_payloads(data_obj: &IDataObject) -> Vec<ShelfPayload> {
    let url_wide_cf = url_wide_clipboard_format();
    if url_wide_cf != 0 {
        let fmt = formatetc_hglobal(url_wide_cf);
        if let Ok(medium) = unsafe { data_obj.GetData(&raw const fmt) } {
            let medium = OwnedStgMedium(medium);
            if medium.0.tymed == u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1) {
                let hglobal = unsafe { medium.0.u.hGlobal };
                if !hglobal.is_invalid()
                    && let Some(text) = read_utf16_hglobal(hglobal, MAX_URL_UTF16_UNITS)
                    && let Some(payload) = decode_dropped_text(&text)
                {
                    return vec![payload];
                }
            }
        }
    }

    let url_ansi_cf = url_ansi_clipboard_format();
    if url_ansi_cf != 0 {
        let fmt = formatetc_hglobal(url_ansi_cf);
        if let Ok(medium) = unsafe { data_obj.GetData(&raw const fmt) } {
            let medium = OwnedStgMedium(medium);
            if medium.0.tymed == u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1) {
                let hglobal = unsafe { medium.0.u.hGlobal };
                if !hglobal.is_invalid()
                    && let Some(text) = read_ansi_hglobal(hglobal, MAX_SHELF_PATH_LEN)
                    && let Some(payload) = decode_dropped_text(&text)
                {
                    return vec![payload];
                }
            }
        }
    }

    let fmt = formatetc_hglobal(CF_UNICODETEXT.0);
    if let Ok(medium) = unsafe { data_obj.GetData(&raw const fmt) } {
        let medium = OwnedStgMedium(medium);
        if medium.0.tymed == u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1) {
            let hglobal = unsafe { medium.0.u.hGlobal };
            if !hglobal.is_invalid()
                && let Some(text) = read_utf16_hglobal(hglobal, MAX_SHELF_TEXT_BYTES + 2)
                && let Some(payload) = decode_dropped_text(&text)
            {
                return vec![payload];
            }
        }
    }
    Vec::new()
}

fn data_object_has_supported_format(data_obj: &IDataObject) -> bool {
    let mut formats = vec![
        formatetc_hglobal(CF_HDROP.0),
        formatetc_hglobal(CF_UNICODETEXT.0),
    ];
    let url_wide_cf = url_wide_clipboard_format();
    if url_wide_cf != 0 {
        formats.push(formatetc_hglobal(url_wide_cf));
    }
    let url_ansi_cf = url_ansi_clipboard_format();
    if url_ansi_cf != 0 {
        formats.push(formatetc_hglobal(url_ansi_cf));
    }
    formats
        .iter()
        .any(|fmt| unsafe { data_obj.QueryGetData(fmt) } == S_OK)
}

fn extract_drop_payloads(data_obj: &IDataObject) -> Vec<ShelfPayload> {
    let files = extract_hdrop_payloads(data_obj);
    if !files.is_empty() {
        return files;
    }
    extract_text_or_url_payloads(data_obj)
}

struct OwnedStgMedium(STGMEDIUM);

impl Drop for OwnedStgMedium {
    fn drop(&mut self) {
        unsafe {
            ReleaseStgMedium(&raw mut self.0);
        }
    }
}

struct OwnedHBitmap(HBITMAP);

impl Drop for OwnedHBitmap {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0.0));
            }
        }
    }
}

struct OwnedPidl(*mut ITEMIDLIST);

impl Drop for OwnedPidl {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                ILFree(Some(self.0.cast_const()));
            }
        }
    }
}

struct CoTaskStr(PWSTR);

impl CoTaskStr {
    fn to_string_bounded(&self, max_chars: usize) -> Option<String> {
        if self.0.0.is_null() {
            return None;
        }
        let mut len = 0usize;
        while len < max_chars {
            let ch = unsafe { *self.0.0.add(len) };
            if ch == 0 {
                break;
            }
            len += 1;
        }
        if len == 0 {
            return None;
        }
        let slice = unsafe { slice::from_raw_parts(self.0.0, len) };
        Some(String::from_utf16_lossy(slice))
    }
}

impl Drop for CoTaskStr {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe {
                CoTaskMemFree(Some(self.0.0.cast_const().cast()));
            }
        }
    }
}

#[allow(clippy::inline_always, clippy::ref_as_ptr)]
mod com_impl {
    use std::cell::Cell;
    use std::mem::ManuallyDrop;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use otto_core::ShelfPayload;
    use windows::Win32::Foundation::{
        DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
        DV_E_FORMATETC, E_NOTIMPL, HWND, LPARAM, POINT, POINTL, S_OK, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::System::Com::{
        DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC,
        IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Ole::{
        CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropSource,
        IDropSource_Impl, IDropTarget, IDropTarget_Impl,
    };
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    use windows::core::{BOOL, HRESULT, Ref, Result, implement};

    use super::{
        ShelfDropEvent, ShelfDropQueue, ShelfDropRegion, WM_APP_SHELF_EVENT, alloc_hglobal_bytes,
        data_object_has_supported_format, encode_ansi_url_bytes, encode_unicodetext_bytes,
        extract_drop_payloads, formatetc_hglobal, url_ansi_clipboard_format,
        url_wide_clipboard_format,
    };

    #[implement(IDropTarget)]
    pub(super) struct ShelfDropTarget {
        hwnd: HWND,
        queue: ShelfDropQueue,
        region: Arc<ShelfDropRegion>,
        local_drag: Arc<AtomicBool>,
        has_supported: Cell<bool>,
        active_hover: Cell<bool>,
    }

    impl ShelfDropTarget {
        pub(super) fn new(
            hwnd: HWND,
            queue: ShelfDropQueue,
            region: Arc<ShelfDropRegion>,
            local_drag: Arc<AtomicBool>,
        ) -> Self {
            Self {
                hwnd,
                queue,
                region,
                local_drag,
                has_supported: Cell::new(false),
                active_hover: Cell::new(false),
            }
        }

        fn point_in_region(&self, pt: &POINTL) -> bool {
            let mut client = POINT { x: pt.x, y: pt.y };
            unsafe {
                let _ = ScreenToClient(self.hwnd, &raw mut client);
            }
            self.region.contains_client_point(client.x, client.y)
        }

        fn push_and_wake(&self, event: ShelfDropEvent) {
            self.queue.push(event);
            unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_APP_SHELF_EVENT, WPARAM(0), LPARAM(0));
            }
        }

        fn set_effect(pdweffect: *mut DROPEFFECT, effect: DROPEFFECT) {
            if !pdweffect.is_null() {
                unsafe {
                    *pdweffect = effect;
                }
            }
        }
    }

    impl IDropTarget_Impl for ShelfDropTarget_Impl {
        fn DragEnter(
            &self,
            pdataobj: Ref<IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> Result<()> {
            let supported = !self.local_drag.load(Ordering::Acquire)
                && pdataobj
                    .as_ref()
                    .is_some_and(data_object_has_supported_format);
            self.has_supported.set(supported);
            let inside = supported && self.point_in_region(pt);
            self.active_hover.set(inside);
            if inside {
                self.push_and_wake(ShelfDropEvent::DragEnter);
                ShelfDropTarget::set_effect(pdweffect, DROPEFFECT_COPY);
            } else {
                ShelfDropTarget::set_effect(pdweffect, DROPEFFECT_NONE);
            }
            Ok(())
        }

        fn DragOver(
            &self,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> Result<()> {
            let inside = !self.local_drag.load(Ordering::Acquire)
                && self.has_supported.get()
                && self.point_in_region(pt);
            let was_active = self.active_hover.replace(inside);
            match (was_active, inside) {
                (false, true) => self.push_and_wake(ShelfDropEvent::DragEnter),
                (true, false) => self.push_and_wake(ShelfDropEvent::DragLeave),
                _ => {}
            }
            ShelfDropTarget::set_effect(
                pdweffect,
                if inside {
                    DROPEFFECT_COPY
                } else {
                    DROPEFFECT_NONE
                },
            );
            Ok(())
        }

        fn DragLeave(&self) -> Result<()> {
            self.has_supported.set(false);
            if self.active_hover.replace(false) {
                self.push_and_wake(ShelfDropEvent::DragLeave);
            }
            Ok(())
        }

        fn Drop(
            &self,
            pdataobj: Ref<IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> Result<()> {
            let was_active = self.active_hover.replace(false);
            let supported =
                !self.local_drag.load(Ordering::Acquire) && self.has_supported.replace(false);
            let inside = supported && self.point_in_region(pt);
            if !inside {
                if was_active {
                    self.push_and_wake(ShelfDropEvent::DragLeave);
                }
                ShelfDropTarget::set_effect(pdweffect, DROPEFFECT_NONE);
                return Ok(());
            }
            let Some(data_obj) = pdataobj.as_ref() else {
                self.push_and_wake(ShelfDropEvent::DragLeave);
                ShelfDropTarget::set_effect(pdweffect, DROPEFFECT_NONE);
                return Ok(());
            };
            let payloads = extract_drop_payloads(data_obj);
            if payloads.is_empty() {
                self.push_and_wake(ShelfDropEvent::DragLeave);
                ShelfDropTarget::set_effect(pdweffect, DROPEFFECT_NONE);
            } else {
                self.push_and_wake(ShelfDropEvent::Drop(payloads));
                ShelfDropTarget::set_effect(pdweffect, DROPEFFECT_COPY);
            }
            Ok(())
        }
    }

    #[implement(IDropSource)]
    pub(super) struct ShelfDropSource;

    impl IDropSource_Impl for ShelfDropSource_Impl {
        fn QueryContinueDrag(
            &self,
            fescapepressed: BOOL,
            grfkeystate: MODIFIERKEYS_FLAGS,
        ) -> HRESULT {
            if fescapepressed.as_bool() {
                DRAGDROP_S_CANCEL
            } else if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
                DRAGDROP_S_DROP
            } else {
                S_OK
            }
        }

        fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
            DRAGDROP_S_USEDEFAULTCURSORS
        }
    }

    #[derive(Clone)]
    struct FormatBytes {
        format: u16,
        bytes: Vec<u8>,
    }

    #[implement(IDataObject)]
    pub(super) struct ShelfDataObject {
        entries: Vec<FormatBytes>,
    }

    impl ShelfDataObject {
        pub(super) fn from_payload(payload: &ShelfPayload) -> Option<Self> {
            let mut entries = Vec::new();
            match payload {
                ShelfPayload::File(path) => {
                    let bytes = super::build_dropfiles_bytes(std::slice::from_ref(path))?;
                    entries.push(FormatBytes {
                        format: CF_HDROP.0,
                        bytes,
                    });
                }
                ShelfPayload::Text(text) => {
                    let bytes = encode_unicodetext_bytes(text)?;
                    entries.push(FormatBytes {
                        format: CF_UNICODETEXT.0,
                        bytes,
                    });
                }
                ShelfPayload::Url(url) => {
                    let unicode_bytes = encode_unicodetext_bytes(url)?;
                    entries.push(FormatBytes {
                        format: CF_UNICODETEXT.0,
                        bytes: unicode_bytes.clone(),
                    });
                    let url_wide_cf = url_wide_clipboard_format();
                    if url_wide_cf != 0 {
                        entries.push(FormatBytes {
                            format: url_wide_cf,
                            bytes: unicode_bytes,
                        });
                    }
                    let url_ansi_cf = url_ansi_clipboard_format();
                    if url_ansi_cf != 0
                        && let Some(ansi_bytes) = encode_ansi_url_bytes(url)
                    {
                        entries.push(FormatBytes {
                            format: url_ansi_cf,
                            bytes: ansi_bytes,
                        });
                    }
                }
            }
            (!entries.is_empty()).then_some(Self { entries })
        }

        fn match_entry(&self, fmt: *const FORMATETC) -> Option<&FormatBytes> {
            let fmt = unsafe { fmt.as_ref() }?;
            let tymed_hglobal = u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1);
            if fmt.dwAspect != DVASPECT_CONTENT.0 || (fmt.tymed & tymed_hglobal) == 0 {
                return None;
            }
            self.entries
                .iter()
                .find(|entry| entry.format == fmt.cfFormat)
        }
    }

    impl IDataObject_Impl for ShelfDataObject_Impl {
        fn GetData(&self, pformatetcin: *const FORMATETC) -> Result<STGMEDIUM> {
            let Some(entry) = self.match_entry(pformatetcin) else {
                return Err(DV_E_FORMATETC.into());
            };
            let hglobal = alloc_hglobal_bytes(&entry.bytes).ok_or(DV_E_FORMATETC)?;
            Ok(STGMEDIUM {
                tymed: u32::try_from(TYMED_HGLOBAL.0).unwrap_or(1),
                u: STGMEDIUM_0 {
                    hGlobal: hglobal.into_raw(),
                },
                pUnkForRelease: ManuallyDrop::new(None),
            })
        }

        fn GetDataHere(
            &self,
            _pformatetc: *const FORMATETC,
            _pmedium: *mut STGMEDIUM,
        ) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
            if self.match_entry(pformatetc).is_some() {
                S_OK
            } else {
                DV_E_FORMATETC
            }
        }

        fn GetCanonicalFormatEtc(
            &self,
            pformatectin: *const FORMATETC,
            pformatetcout: *mut FORMATETC,
        ) -> HRESULT {
            if let (Some(in_fmt), Some(out_fmt)) = (unsafe { pformatectin.as_ref() }, unsafe {
                pformatetcout.as_mut()
            }) {
                *out_fmt = *in_fmt;
                out_fmt.ptd = std::ptr::null_mut();
            }
            DATA_S_SAMEFORMATETC
        }

        fn SetData(
            &self,
            _pformatetc: *const FORMATETC,
            _pmedium: *const STGMEDIUM,
            _frelease: BOOL,
        ) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn EnumFormatEtc(&self, dwdirection: u32) -> Result<IEnumFORMATETC> {
            if dwdirection != 1 {
                return Err(E_NOTIMPL.into());
            }
            let formats: Vec<FORMATETC> = self
                .entries
                .iter()
                .map(|entry| formatetc_hglobal(entry.format))
                .collect();
            unsafe { SHCreateStdEnumFmtEtc(&formats) }
        }

        fn DAdvise(
            &self,
            _pformatetc: *const FORMATETC,
            _advf: u32,
            _padvsink: Ref<IAdviseSink>,
        ) -> Result<u32> {
            Err(E_NOTIMPL.into())
        }

        fn DUnadvise(&self, _dwconnection: u32) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
            Err(E_NOTIMPL.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use otto_core::ShelfPayload;
    use windows::Win32::Foundation::POINTL;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::CF_HDROP;
    use windows::Win32::UI::Shell::DROPFILES;

    use super::{
        ShelfDropEvent, ShelfDropQueue, ShelfDropRegion, build_dropfiles_bytes, com_impl,
        data_object_has_supported_format, extract_drop_payloads, inspect_shelf_file,
    };

    #[test]
    fn build_dropfiles_bytes_writes_wide_header_and_double_nul_terminator() {
        let paths = vec![String::from(r"C:\Temp\a.txt"), String::from(r"D:\b.png")];
        let bytes = build_dropfiles_bytes(&paths).expect("valid dropfiles bytes");
        let header_len = size_of::<DROPFILES>();
        assert!(bytes.len() > header_len + 4);
        assert_eq!(
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            header_len as u32
        );
        let wide: Vec<u16> = bytes[header_len..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let expected: Vec<u16> = "C:\\Temp\\a.txt\0D:\\b.png\0\0".encode_utf16().collect();
        assert_eq!(wide, expected);
    }

    #[test]
    fn drop_queue_drains_in_fifo_order() {
        let queue = ShelfDropQueue::new();
        queue.push(ShelfDropEvent::DragEnter);
        queue.push(ShelfDropEvent::Drop(vec![ShelfPayload::Text(
            "hello".into(),
        )]));
        queue.push(ShelfDropEvent::DragLeave);
        assert_eq!(
            queue.drain(),
            vec![
                ShelfDropEvent::DragEnter,
                ShelfDropEvent::Drop(vec![ShelfPayload::Text("hello".into())]),
                ShelfDropEvent::DragLeave,
            ]
        );
        assert!(queue.drain().is_empty());
    }

    #[test]
    fn drop_region_checks_bounds_and_full_window() {
        let full = ShelfDropRegion::full_window();
        assert!(full.contains_client_point(-10, -10));
        let restricted = ShelfDropRegion::restricted(100, 0, 300, 40);
        assert!(restricted.contains_client_point(100, 0));
        assert!(restricted.contains_client_point(299, 39));
        assert!(!restricted.contains_client_point(300, 10));
        assert!(!restricted.contains_client_point(50, 10));
        restricted.set_rect(200, 0, 400, 44);
        assert!(!restricted.contains_client_point(150, 10));
        assert!(restricted.contains_client_point(350, 40));
    }

    #[test]
    fn shelf_data_object_round_trips_file_and_url_payloads() {
        let file_obj: IDataObject = com_impl::ShelfDataObject::from_payload(&ShelfPayload::File(
            String::from(r"C:\Temp\demo.pdf"),
        ))
        .expect("file data object")
        .into();
        assert!(data_object_has_supported_format(&file_obj));
        assert_eq!(
            extract_drop_payloads(&file_obj),
            vec![ShelfPayload::File(String::from(r"C:\Temp\demo.pdf"))]
        );

        let url_obj: IDataObject = com_impl::ShelfDataObject::from_payload(&ShelfPayload::Url(
            String::from("https://example.com/path"),
        ))
        .expect("url data object")
        .into();
        assert!(data_object_has_supported_format(&url_obj));
        assert_eq!(
            extract_drop_payloads(&url_obj),
            vec![ShelfPayload::Url(String::from("https://example.com/path"))]
        );
    }

    #[test]
    fn encode_rejects_empty_and_overlong_paths() {
        assert!(build_dropfiles_bytes(&[]).is_none());
        assert!(build_dropfiles_bytes(&[String::new()]).is_none());
        assert_eq!(CF_HDROP.0, 15);
        let _ = POINTL { x: 0, y: 0 };
    }

    #[test]
    fn inspect_shelf_file_skips_unc_paths_without_blocking() {
        let started = std::time::Instant::now();
        assert_eq!(
            inspect_shelf_file(r"\\192.0.2.1\share\doc.pdf", 64),
            (false, None)
        );
        assert_eq!(
            inspect_shelf_file(r"\\?\UNC\192.0.2.1\share\doc.pdf", 64),
            (false, None)
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}
