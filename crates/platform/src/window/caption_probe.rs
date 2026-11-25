use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::scale;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_ButtonControlTypeId,
    UIA_ControlTypePropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GWL_STYLE, GetLayeredWindowAttributes, GetWindowLongW, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, LAYERED_WINDOW_ATTRIBUTES_FLAGS, LWA_ALPHA,
    SMTO_ABORTIFHUNG, SendMessageTimeoutW, TITLEBARINFOEX, WM_GETTITLEBARINFOEX, WS_CAPTION,
    WS_EX_LAYERED, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
};
use windows::core::PWSTR;

use crate::com::ComGuard;
use crate::window::caption::dwm_rect;
use crate::window::ops::window_class_utf16;
use crate::worker::join_within;

const CACHE_LIMIT: usize = 64;
const QUEUE_LIMIT: usize = 24;
const MAX_UIA_BUTTON_PROBES: i32 = 48;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const TITLEBAR_INFO_TIMEOUT_MS: u32 = 25;
const MIN_OPAQUE_LAYERED_ALPHA: u8 = 220;
const STATE_SYSTEM_INVISIBLE_FLAG: u32 = 0x0000_8000;
const STATE_SYSTEM_OFFSCREEN_FLAG: u32 = 0x0001_0000;

const MIN_CAPTION_HEIGHT_DIP: i32 = 20;
const MAX_CAPTION_HEIGHT_DIP: i32 = 64;
const MAX_CAPTION_SPAN_DIP: i32 = 280;
const MIN_BUTTON_WIDTH_DIP: i32 = 16;
const MAX_BUTTON_WIDTH_DIP: i32 = 84;
const MAX_RIGHT_EDGE_GAP_DIP: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProbeKey {
    hwnd: isize,
    width: i32,
    height: i32,
    dpi: u32,
    maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProbeRequest {
    key: ProbeKey,
    custom_chrome: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MeasuredCaption {
    pub(crate) top_offset: i32,
    pub(crate) height: i32,
    pub(crate) width: i32,
    pub(crate) slots: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeEntry {
    Pending,
    Measured(MeasuredCaption),
    NoButtons,
    Inconclusive,
}

#[derive(Default)]
struct ProbeQueue {
    items: Vec<ProbeRequest>,
}

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static CACHE: OnceLock<Mutex<HashMap<ProbeKey, ProbeEntry>>> = OnceLock::new();
static QUEUE: OnceLock<(Mutex<ProbeQueue>, Condvar)> = OnceLock::new();
static WORKER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

fn cache() -> &'static Mutex<HashMap<ProbeKey, ProbeEntry>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn queue_pair() -> &'static (Mutex<ProbeQueue>, Condvar) {
    QUEUE.get_or_init(|| (Mutex::new(ProbeQueue::default()), Condvar::new()))
}

pub(crate) fn resume_caption_probe_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

pub(crate) fn shutdown_caption_probe_task() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    let (_, condvar) = queue_pair();
    condvar.notify_all();
    let handle = WORKER.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(handle) = handle {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

fn ensure_worker_started() {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    let mut guard = WORKER.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.as_ref().is_some_and(|handle| !handle.is_finished()) {
        return;
    }
    if let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-caption-probe"))
        .spawn(worker_loop)
    {
        *guard = Some(handle);
    }
}

pub(crate) fn is_ghost_overlay_window(hwnd: HWND, raw_style: u32, ex_style: u32) -> bool {
    if ex_style & WS_EX_LAYERED.0 == 0 {
        return false;
    }
    let has_caption = raw_style & WS_CAPTION.0 == WS_CAPTION.0;
    let mut alpha = 255_u8;
    let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
    let has_attrs = unsafe {
        GetLayeredWindowAttributes(hwnd, None, Some(&raw mut alpha), Some(&raw mut flags))
    }
    .is_ok();
    is_layered_ghost(has_caption, has_attrs, flags.0, alpha)
}

const fn is_layered_ghost(has_caption: bool, has_attrs: bool, flags: u32, alpha: u8) -> bool {
    if !has_attrs {
        return !has_caption;
    }
    if flags & LWA_ALPHA.0 != 0 && alpha < MIN_OPAQUE_LAYERED_ALPHA {
        return true;
    }
    false
}

pub(crate) fn window_class_string(hwnd: HWND) -> String {
    let mut buf = [0_u16; 128];
    let len = window_class_utf16(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len])
}

pub(crate) fn is_known_custom_chrome_class(class: &str) -> bool {
    matches!(
        class,
        "Chrome_WidgetWin_1"
            | "MozillaWindowClass"
            | "CASCADIA_HOSTING_WINDOW_CLASS"
            | "ApplicationFrameWindow"
            | "WinUIDesktopWin32WindowClass"
            | "CabinetWClass"
            | "ExploreWClass"
    ) || class.starts_with("HwndWrapper[")
        || class.starts_with("Qt5QWindow")
        || class.starts_with("Qt6")
}

pub(crate) fn custom_chrome_eligible(
    class: &str,
    raw_style: u32,
    has_dwm_buttons: bool,
    probe: Option<ProbeEntry>,
) -> bool {
    if matches!(probe, Some(ProbeEntry::NoButtons)) {
        return false;
    }
    if matches!(probe, Some(ProbeEntry::Measured(_))) {
        return true;
    }
    let frame_flags = raw_style & (WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) != 0;
    if !frame_flags {
        return false;
    }
    has_dwm_buttons || is_known_custom_chrome_class(class)
}

fn process_exe_stem(hwnd: HWND) -> Option<String> {
    let mut pid = 0_u32;
    let mut buf = [0_u16; 512];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        if pid == 0 {
            return None;
        }
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return None;
        };
        let queried = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut len,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        queried
    };
    if !ok || len == 0 {
        return None;
    }
    let full = String::from_utf16_lossy(&buf[..len as usize]);
    Path::new(&full)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_ascii_lowercase)
}

pub(crate) fn framework_caption_height_dip(
    class: &str,
    exe_stem: Option<&str>,
    maximized: bool,
) -> i32 {
    match class {
        "CASCADIA_HOSTING_WINDOW_CLASS" | "MozillaWindowClass" => {
            if maximized {
                32
            } else {
                40
            }
        }
        "CabinetWClass" | "ExploreWClass" => 44,
        "Chrome_WidgetWin_1" => match exe_stem {
            Some("chrome" | "msedge" | "brave" | "vivaldi" | "opera" | "arc") => {
                if maximized {
                    32
                } else {
                    40
                }
            }
            Some("discord") => 22,
            _ => 35,
        },
        _ => 32,
    }
}

pub(crate) fn fallback_caption_height_px(
    hwnd: HWND,
    class: &str,
    dpi: u32,
    maximized: bool,
) -> i32 {
    let stem = if class == "Chrome_WidgetWin_1" {
        process_exe_stem(hwnd)
    } else {
        None
    };
    scale(
        framework_caption_height_dip(class, stem.as_deref(), maximized),
        dpi,
    )
}

pub(crate) fn lookup_or_queue_probe(
    hwnd: HWND,
    frame: RECT,
    dpi: u32,
    maximized: bool,
    custom_chrome: bool,
) -> Option<ProbeEntry> {
    let width = frame.right - frame.left;
    let height = frame.bottom - frame.top;
    if width <= 0 || height <= 0 {
        return None;
    }
    let key = ProbeKey {
        hwnd: hwnd.0 as isize,
        width,
        height,
        dpi,
        maximized,
    };
    {
        let mut map = cache().lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&entry) = map.get(&key) {
            return Some(entry);
        }
        if map.len() >= CACHE_LIMIT {
            map.retain(|existing, _| existing.hwnd != key.hwnd);
            if map.len() >= CACHE_LIMIT
                && let Some(&evict) = map.keys().next()
            {
                map.remove(&evict);
            }
        }
        map.insert(key, ProbeEntry::Pending);
    }
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return None;
    }
    ensure_worker_started();
    let (mutex, condvar) = queue_pair();
    {
        let mut queue = mutex.lock().unwrap_or_else(PoisonError::into_inner);
        queue.items.retain(|item| item.key.hwnd != key.hwnd);
        if queue.items.len() >= QUEUE_LIMIT {
            queue.items.remove(0);
        }
        queue.items.push(ProbeRequest { key, custom_chrome });
    }
    condvar.notify_one();
    None
}

pub(crate) fn cached_probe_entry(
    hwnd: HWND,
    frame: RECT,
    dpi: u32,
    maximized: bool,
) -> Option<ProbeEntry> {
    let key = ProbeKey {
        hwnd: hwnd.0 as isize,
        width: frame.right - frame.left,
        height: frame.bottom - frame.top,
        dpi,
        maximized,
    };
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .copied()
}

fn worker_loop() {
    let _com = ComGuard::multithreaded();
    let automation: Option<IUIAutomation> =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok();
    let (mutex, condvar) = queue_pair();
    loop {
        let request = {
            let mut queue = mutex.lock().unwrap_or_else(PoisonError::into_inner);
            while !SHUTTING_DOWN.load(Ordering::Acquire) && queue.items.is_empty() {
                queue = condvar.wait(queue).unwrap_or_else(PoisonError::into_inner);
            }
            if SHUTTING_DOWN.load(Ordering::Acquire) {
                return;
            }
            queue.items.pop()
        };
        let Some(request) = request else {
            continue;
        };
        let outcome = run_probe(automation.as_ref(), request);
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            return;
        }
        {
            let mut map = cache().lock().unwrap_or_else(PoisonError::into_inner);
            map.insert(request.key, outcome);
        }
        if matches!(outcome, ProbeEntry::Measured(_) | ProbeEntry::NoButtons) {
            crate::os_events::notify_caption_probe_ready();
        }
    }
}

fn run_probe(automation: Option<&IUIAutomation>, request: ProbeRequest) -> ProbeEntry {
    let hwnd = HWND(request.key.hwnd as *mut c_void);
    let alive = unsafe {
        IsWindow(Some(hwnd)).as_bool()
            && IsWindowVisible(hwnd).as_bool()
            && !IsIconic(hwnd).as_bool()
    };
    if !alive {
        return ProbeEntry::Inconclusive;
    }
    let Some(live_frame) = dwm_rect(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS) else {
        return ProbeEntry::Inconclusive;
    };
    let dpi = request.key.dpi.max(96);
    if let Some(measured) = probe_via_titlebar_info(hwnd, live_frame, dpi) {
        return ProbeEntry::Measured(measured);
    }
    let Some(uia) = automation else {
        return ProbeEntry::Inconclusive;
    };
    probe_via_uia(uia, hwnd, live_frame, dpi, request.custom_chrome)
}

fn probe_via_titlebar_info(hwnd: HWND, frame: RECT, dpi: u32) -> Option<MeasuredCaption> {
    let mut info = TITLEBARINFOEX {
        cbSize: size_of::<TITLEBARINFOEX>() as u32,
        ..TITLEBARINFOEX::default()
    };
    let mut reply = 0_usize;
    let sent = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETTITLEBARINFOEX,
            WPARAM(0),
            LPARAM((&raw mut info) as isize),
            SMTO_ABORTIFHUNG,
            TITLEBAR_INFO_TIMEOUT_MS,
            Some(&raw mut reply),
        )
    };
    if sent.0 == 0 {
        return None;
    }
    let mut rects = Vec::with_capacity(3);
    for index in [2_usize, 3, 5] {
        let state = info.rgstate[index];
        if state & (STATE_SYSTEM_INVISIBLE_FLAG | STATE_SYSTEM_OFFSCREEN_FLAG) != 0 {
            continue;
        }
        let rect = info.rgrect[index];
        if is_valid_caption_button_rect(rect, frame, dpi) {
            rects.push(rect);
        }
    }
    combine_button_rects(&rects, frame, dpi)
}

fn probe_via_uia(
    automation: &IUIAutomation,
    hwnd: HWND,
    frame: RECT,
    dpi: u32,
    custom_chrome: bool,
) -> ProbeEntry {
    let mut button_rects = Vec::with_capacity(4);
    let mut scanned_buttons = 0_i32;
    let (probed, style, ex_style) = unsafe {
        let Ok(root) = automation.ElementFromHandle(hwnd) else {
            return ProbeEntry::Inconclusive;
        };
        let Ok(cond) = automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_ButtonControlTypeId.0),
        ) else {
            return ProbeEntry::Inconclusive;
        };
        let Ok(elements) = root.FindAll(TreeScope_Descendants, &cond) else {
            return ProbeEntry::Inconclusive;
        };
        let total = elements.Length().unwrap_or(0);
        let limit = total.min(MAX_UIA_BUTTON_PROBES);
        for index in 0..limit {
            let Ok(element) = elements.GetElement(index) else {
                continue;
            };
            scanned_buttons += 1;
            let Ok(rect) = element.CurrentBoundingRectangle() else {
                continue;
            };
            if !is_valid_caption_button_rect(rect, frame, dpi) {
                continue;
            }
            let name = element
                .CurrentName()
                .ok()
                .map(|bstr| bstr.to_string())
                .unwrap_or_default();
            let auto_id = element
                .CurrentAutomationId()
                .ok()
                .map(|bstr| bstr.to_string())
                .unwrap_or_default();
            if is_caption_button_identity(&name, &auto_id) {
                button_rects.push(rect);
            }
        }
        (
            total,
            GetWindowLongW(hwnd, GWL_STYLE) as u32,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
        )
    };
    if let Some(measured) = combine_button_rects(&button_rects, frame, dpi) {
        return ProbeEntry::Measured(measured);
    }
    if custom_chrome
        && probed > 0
        && scanned_buttons > 0
        && style & WS_CAPTION.0 != WS_CAPTION.0
        && (ex_style & WS_EX_LAYERED.0 != 0 || style & WS_MAXIMIZEBOX.0 == 0)
    {
        return ProbeEntry::NoButtons;
    }
    ProbeEntry::Inconclusive
}

fn is_caption_button_identity(name: &str, automation_id: &str) -> bool {
    let matches_token = |raw: &str| {
        let lower = raw.trim().to_ascii_lowercase();
        matches!(
            lower.as_str(),
            "close"
                | "minimize"
                | "maximize"
                | "restore"
                | "restore down"
                | "tutup"
                | "minimalkan"
                | "maksimalkan"
                | "pulihkan"
                | "closebutton"
                | "minimizebutton"
                | "maximizebutton"
                | "maximize-restore"
                | "minimize-restore"
                | "titlebarclosebutton"
                | "titlebarminimizebutton"
                | "titlebarmaximizebutton"
        ) || lower.starts_with("close ")
            || lower.starts_with("minimize ")
            || lower.starts_with("maximize ")
            || lower.starts_with("restore ")
    };
    matches_token(name) || matches_token(automation_id)
}

fn is_valid_caption_button_rect(rect: RECT, frame: RECT, dpi: u32) -> bool {
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let min_w = scale(MIN_BUTTON_WIDTH_DIP, dpi);
    let max_w = scale(MAX_BUTTON_WIDTH_DIP, dpi);
    let min_h = scale(MIN_CAPTION_HEIGHT_DIP, dpi);
    let max_h = scale(MAX_CAPTION_HEIGHT_DIP, dpi);
    let max_span = scale(MAX_CAPTION_SPAN_DIP, dpi);
    w >= min_w
        && w <= max_w
        && h >= min_h
        && h <= max_h
        && rect.left >= frame.right - max_span
        && rect.right <= frame.right + 4
        && rect.top >= frame.top - 4
        && rect.bottom <= frame.top + max_h + 4
}

fn combine_button_rects(rects: &[RECT], frame: RECT, dpi: u32) -> Option<MeasuredCaption> {
    if rects.is_empty() {
        return None;
    }
    let mut min_left = i32::MAX;
    let mut max_right = i32::MIN;
    let mut min_top = i32::MAX;
    let mut max_bottom = i32::MIN;
    for rect in rects {
        min_left = min_left.min(rect.left);
        max_right = max_right.max(rect.right);
        min_top = min_top.min(rect.top);
        max_bottom = max_bottom.max(rect.bottom);
    }
    let right_gap = frame.right - max_right;
    if right_gap < -4 || right_gap > scale(MAX_RIGHT_EDGE_GAP_DIP, dpi) {
        return None;
    }
    let visible_top = min_top.max(frame.top);
    let height = max_bottom - frame.top;
    let min_h = scale(MIN_CAPTION_HEIGHT_DIP, dpi);
    let max_h = scale(MAX_CAPTION_HEIGHT_DIP, dpi);
    if height < min_h || height > max_h {
        return None;
    }
    let width = frame.right - min_left;
    if width <= 0 || width > scale(MAX_CAPTION_SPAN_DIP, dpi) {
        return None;
    }
    let top_offset = (visible_top - frame.top).clamp(0, (height - 1).max(0));
    let slots = if rects.len() >= 2 { 3 } else { 1 };
    Some(MeasuredCaption {
        top_offset,
        height,
        width,
        slots,
    })
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        LWA_ALPHA, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };

    use super::{
        MeasuredCaption, ProbeEntry, combine_button_rects, custom_chrome_eligible,
        framework_caption_height_dip, is_caption_button_identity, is_known_custom_chrome_class,
        is_layered_ghost,
    };

    #[test]
    fn layered_transparent_or_per_pixel_alpha_non_caption_windows_are_rejected_as_ghosts() {
        assert!(is_layered_ghost(false, false, 0, 255));
        assert!(!is_layered_ghost(true, false, 0, 255));
        assert!(is_layered_ghost(false, true, LWA_ALPHA.0, 0));
        assert!(is_layered_ghost(true, true, LWA_ALPHA.0, 128));
        assert!(!is_layered_ghost(false, true, LWA_ALPHA.0, 255));
    }

    #[test]
    fn custom_chrome_rejects_arbitrary_non_caption_windows_and_accepts_known_shells() {
        assert!(!custom_chrome_eligible(
            "RecordlyOverlay",
            WS_SYSMENU.0,
            false,
            None
        ));
        assert!(!custom_chrome_eligible(
            "RecordlyOverlay",
            WS_THICKFRAME.0 | WS_MINIMIZEBOX.0,
            false,
            None
        ));
        assert!(custom_chrome_eligible(
            "Chrome_WidgetWin_1",
            WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0,
            false,
            None
        ));
        assert!(!custom_chrome_eligible(
            "Chrome_WidgetWin_1",
            WS_THICKFRAME.0 | WS_MINIMIZEBOX.0,
            false,
            Some(ProbeEntry::NoButtons)
        ));
        assert!(is_known_custom_chrome_class("WinUIDesktopWin32WindowClass"));
        assert!(is_known_custom_chrome_class("HwndWrapper[App;;1234]"));
    }

    #[test]
    fn framework_defaults_cover_electron_browsers_terminal_and_win32() {
        assert_eq!(
            framework_caption_height_dip("Chrome_WidgetWin_1", Some("antigravity"), false),
            35
        );
        assert_eq!(
            framework_caption_height_dip("Chrome_WidgetWin_1", Some("chrome"), false),
            40
        );
        assert_eq!(
            framework_caption_height_dip("Chrome_WidgetWin_1", Some("chrome"), true),
            32
        );
        assert_eq!(
            framework_caption_height_dip("Chrome_WidgetWin_1", Some("discord"), false),
            22
        );
        assert_eq!(
            framework_caption_height_dip("CASCADIA_HOSTING_WINDOW_CLASS", None, false),
            40
        );
        assert_eq!(
            framework_caption_height_dip("CabinetWClass", None, false),
            44
        );
        assert_eq!(framework_caption_height_dip("Notepad", None, false), 32);
    }

    #[test]
    fn uia_button_union_measures_exact_offset_height_and_width() {
        let frame = RECT {
            left: 100,
            top: 50,
            right: 1100,
            bottom: 800,
        };
        let rects = [
            RECT {
                left: 962,
                top: 54,
                right: 1008,
                bottom: 88,
            },
            RECT {
                left: 1008,
                top: 54,
                right: 1054,
                bottom: 88,
            },
            RECT {
                left: 1054,
                top: 54,
                right: 1100,
                bottom: 88,
            },
        ];
        let measured = combine_button_rects(&rects, frame, 96).expect("measured");
        assert_eq!(
            measured,
            MeasuredCaption {
                top_offset: 4,
                height: 38,
                width: 138,
                slots: 3,
            }
        );
        assert!(is_caption_button_identity("Close", ""));
        assert!(is_caption_button_identity("", "MaximizeButton"));
        assert!(!is_caption_button_identity("Search", "SearchBox"));
    }
}
