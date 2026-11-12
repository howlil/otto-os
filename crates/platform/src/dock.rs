use std::collections::HashSet;
use std::ffi::c_void;
use std::fs;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use otto_core::{
    AppCommand, AppInfo, AppleIconKind, CMD_DOCK_EMPTY_TRASH, CMD_DOCK_HIDE, CMD_DOCK_NEW_WINDOW,
    CMD_DOCK_PIN, CMD_DOCK_QUIT, CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, Capability, CropRegion,
    MenuRow, PixelLimits, Rect, StableOrder, WindowPresence, bgra_layout, crop_bgra, group_windows,
    is_blank_frame, resize_bgra, scale_dimension,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_TRANSITIONS_FORCEDISABLED, DwmFlush,
    DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BitBlt, DIB_RGB_COLORS, DeleteObject, GetDIBits,
    GetObjectW, HGDIOBJ, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MonitorFromPoint, MonitorFromWindow, SRCCOPY,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, DragQueryPoint, HDROP, SHFILEINFOW,
    SHGFI_SYSICONINDEX, SHGetFileInfoW, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GCLP_HICON, GW_OWNER, GWL_EXSTYLE, GetAncestor, GetClassLongPtrW,
    GetClassNameW, GetCursorPos, GetForegroundWindow, GetIconInfo, GetWindow, GetWindowLongW,
    GetWindowRect, GetWindowTextLengthW, GetWindowThreadProcessId, HICON, ICON_BIG, ICONINFO,
    IsHungAppWindow, IsIconic, IsWindowVisible, IsZoomed, PostMessageW, SC_MINIMIZE, SC_RESTORE,
    SMTO_ABORTIFHUNG, SW_SHOW, SendMessageTimeoutW, SetForegroundWindow, ShowWindow, WM_GETICON,
    WM_NULL, WM_SYSCOMMAND, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::gdi::{CaptureCanvas, ScreenDc};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};

const MAX_OWNER_DEPTH: usize = 16;
use crate::trash::is_recycle_bin_full;

#[derive(Debug, Clone)]
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockItemKind {
    Finder,
    Launchpad,
    App { exe_path: String },
    Downloads,
    Trash { is_full: bool },
    Overflow { hidden: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct DockEntry {
    pub key: String,
    pub title: String,
    pub kind: DockItemKind,
    pub apple_icon: Option<AppleIconKind>,
    pub app: Option<AppInfo>,
    pub hwnds: Vec<isize>,
    pub window_titles: Vec<String>,
    pub running: bool,
    pub active: bool,
    pub pinned: bool,
    pub hidden: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWindowTarget {
    pub hwnd: isize,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub maximized: bool,
    pub dpi: u32,
    pub on_primary: bool,
}

#[derive(Debug, Clone)]
pub struct WindowSnapshot {
    pub rect: Rect,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[must_use]
pub fn cursor_position() -> (i32, i32) {
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
    }
    (pt.x, pt.y)
}

const COVER_WIDTH_SLACK: i32 = 12;
const COVER_HEIGHT_SLACK: i32 = 56;
const COVER_BOTTOM_SLACK: i32 = 8;

fn window_covers_dock(
    on_primary: bool,
    zoomed: bool,
    rect: RECT,
    screen_width: i32,
    screen_height: i32,
) -> bool {
    if !on_primary {
        return false;
    }
    zoomed
        || (rect.right - rect.left >= screen_width - COVER_WIDTH_SLACK
            && rect.bottom - rect.top >= screen_height - COVER_HEIGHT_SLACK
            && rect.bottom >= screen_height - COVER_BOTTOM_SLACK)
}

#[must_use]
pub fn has_maximized_or_fullscreen_window(
    screen_width: i32,
    screen_height: i32,
    own_pid: u32,
) -> bool {
    struct Probe {
        screen_width: i32,
        screen_height: i32,
        own_pid: u32,
        primary_monitor: HMONITOR,
        found: bool,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = unsafe { &mut *(lparam.0 as *mut Probe) };
        if is_normal_visible_window(hwnd, probe.own_pid) {
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            let on_primary = monitor == probe.primary_monitor;
            let zoomed = on_primary && unsafe { IsZoomed(hwnd) }.as_bool();
            let rect = if on_primary {
                visible_window_rect(hwnd)
            } else {
                RECT::default()
            };
            if window_covers_dock(
                on_primary,
                zoomed,
                rect,
                probe.screen_width,
                probe.screen_height,
            ) {
                probe.found = true;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    let primary_monitor =
        unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };

    let mut probe = Probe {
        screen_width,
        screen_height,
        own_pid,
        primary_monitor,
        found: false,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.found
}

#[must_use]
pub fn active_window_target(own_pid: u32) -> Option<ActiveWindowTarget> {
    let fg = crate::os_events::current_foreground();
    if fg.0.is_null() {
        return None;
    }
    let target = visible_root_owner(fg);
    if !is_normal_visible_window(target, own_pid) {
        return None;
    }
    let rect = visible_window_rect(target);
    if rect.right - rect.left < 200 || rect.bottom - rect.top < 80 {
        return None;
    }
    let maximized = unsafe { IsZoomed(target).as_bool() };

    let (_, monitor_dpi) = window_and_monitor_dpi(target);
    let monitor = unsafe { MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST) };
    let primary = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    Some(ActiveWindowTarget {
        hwnd: target.0 as isize,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        maximized,
        dpi: monitor_dpi,
        on_primary: monitor == primary,
    })
}

#[must_use]
pub fn window_bounds(hwnd_val: isize) -> Option<Rect> {
    let rect = visible_window_rect(HWND(hwnd_val as *mut c_void));
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    (width > 0 && height > 0).then_some(Rect {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

const CAPTURE_RESPONSIVENESS_PROBE_MS: u32 = 50;

#[must_use]
pub fn capture_window(hwnd_val: isize) -> Option<WindowSnapshot> {
    let hwnd = HWND(hwnd_val as *mut c_void);
    let frame = window_bounds(hwnd_val)?;
    let outer = outer_window_rect(hwnd).unwrap_or(frame);
    let frame_layout = bgra_layout(
        u32::try_from(frame.width).ok()?,
        u32::try_from(frame.height).ok()?,
        PixelLimits::WINDOW_SNAPSHOT,
    )
    .ok()?;
    let canvas_layout = bgra_layout(
        u32::try_from(outer.width.max(frame.width)).ok()?,
        u32::try_from(outer.height.max(frame.height)).ok()?,
        PixelLimits::WINDOW_SNAPSHOT,
    )
    .ok()?;
    if !is_window_responsive(hwnd) {
        return None;
    }
    let screen = ScreenDc::acquire()?;
    let canvas = CaptureCanvas::create(&screen, canvas_layout)?;

    let (window_dpi, monitor_dpi) = window_and_monitor_dpi(hwnd);
    let painted_length =
        |length: i32| scale_dimension(u32::try_from(length).unwrap_or(0), window_dpi, monitor_dpi);
    let printed_region = CropRegion {
        x: painted_length(frame.x - outer.x).min(canvas_layout.width - 1),
        y: painted_length(frame.y - outer.y).min(canvas_layout.height - 1),
        width: painted_length(frame.width),
        height: painted_length(frame.height),
    };
    let printed_layout = bgra_layout(
        printed_region.width,
        printed_region.height,
        PixelLimits::WINDOW_SNAPSHOT,
    )
    .ok()?;
    let printed = unsafe { PrintWindow(hwnd, canvas.dc(), PRINT_WINDOW_FLAGS(2)) }
        .as_bool()
        .then(|| crop_bgra(&canvas.copy_pixels(), canvas.layout(), printed_region))
        .flatten()
        .filter(|pixels| !is_blank_frame(printed_layout.width, printed_layout.height, pixels))
        .and_then(|pixels| resize_bgra(&pixels, printed_layout, frame_layout));

    let pixels = printed.or_else(|| copy_foreground_window(hwnd, &screen, &canvas, frame))?;
    Some(WindowSnapshot {
        rect: frame,
        width: frame_layout.width,
        height: frame_layout.height,
        pixels: opaque(pixels),
    })
}

fn window_and_monitor_dpi(hwnd: HWND) -> (u32, u32) {
    let window_dpi = unsafe { GetDpiForWindow(hwnd) };
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let (mut dpi_x, mut dpi_y) = (0u32, 0u32);
    let monitor_dpi = if unsafe {
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
    }
    .is_ok()
    {
        dpi_x
    } else {
        window_dpi
    };
    if window_dpi == 0 || monitor_dpi == 0 {
        (96, 96)
    } else {
        (window_dpi, monitor_dpi)
    }
}

fn copy_foreground_window(
    hwnd: HWND,
    screen: &ScreenDc,
    canvas: &CaptureCanvas,
    frame: Rect,
) -> Option<Vec<u8>> {
    if unsafe { GetForegroundWindow() } != hwnd {
        return None;
    }
    unsafe {
        BitBlt(
            canvas.dc(),
            0,
            0,
            frame.width,
            frame.height,
            Some(screen.handle()),
            frame.x,
            frame.y,
            SRCCOPY,
        )
    }
    .ok()?;
    let region = CropRegion {
        x: 0,
        y: 0,
        width: u32::try_from(frame.width).ok()?,
        height: u32::try_from(frame.height).ok()?,
    };
    crop_bgra(&canvas.copy_pixels(), canvas.layout(), region)
        .filter(|pixels| !is_blank_frame(region.width, region.height, pixels))
}

fn opaque(mut pixels: Vec<u8>) -> Vec<u8> {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    pixels
}

fn is_window_responsive(hwnd: HWND) -> bool {
    let mut reply = 0usize;
    unsafe {
        !IsHungAppWindow(hwnd).as_bool()
            && SendMessageTimeoutW(
                hwnd,
                WM_NULL,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                CAPTURE_RESPONSIVENESS_PROBE_MS,
                Some(&raw mut reply),
            )
            .0 != 0
    }
}

fn outer_window_rect(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    (width > 0 && height > 0).then_some(Rect {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

#[must_use]
pub fn is_window_cloaked(hwnd_val: isize) -> bool {
    let mut cloaked = 0u32;
    let read = unsafe {
        DwmGetWindowAttribute(
            HWND(hwnd_val as *mut c_void),
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    read.is_ok() && cloaked != 0
}

fn suppress_transitions(hwnd_val: isize) -> bool {
    use crate::hidden_windows::{Concealment, forget, record};
    let Some(identity) = crate::window_model::current_identity(hwnd_val) else {
        return false;
    };
    if !record(identity, Concealment::TransitionsDisabled) {
        return false;
    }
    let applied = set_dwm_transition_policy(HWND(hwnd_val as *mut c_void), false);
    if !applied {
        forget(hwnd_val, Concealment::TransitionsDisabled);
    }
    applied
}

pub(crate) fn release_transitions(hwnd_val: isize) -> bool {
    let applied = set_dwm_transition_policy(HWND(hwnd_val as *mut c_void), true);
    if applied {
        crate::hidden_windows::forget(
            hwnd_val,
            crate::hidden_windows::Concealment::TransitionsDisabled,
        );
    }
    applied
}

pub fn flush_compositor() {
    unsafe {
        let _ = DwmFlush();
    }
}

#[must_use]
pub fn set_window_transitions_enabled(hwnd_val: isize, enabled: bool) -> bool {
    if enabled {
        release_transitions(hwnd_val)
    } else {
        suppress_transitions(hwnd_val)
    }
}

#[must_use]
pub fn is_window_minimized(hwnd_val: isize) -> bool {
    unsafe { IsIconic(HWND(hwnd_val as *mut c_void)).as_bool() }
}

#[must_use]
pub fn is_window_foreground(hwnd_val: isize) -> bool {
    foreground_root_hwnd() == Some(hwnd_val)
}

pub fn minimize_window_native(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    enable_dwm_transitions(hwnd);
    post_minimize(hwnd);
}

#[must_use]
pub fn minimize_window_native_quiet(hwnd_val: isize) -> bool {
    let quiet = suppress_transitions(hwnd_val);
    post_minimize(HWND(hwnd_val as *mut c_void));
    quiet
}

fn post_minimize(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM(SC_MINIMIZE as usize),
            LPARAM(0),
        );
    }
}

pub fn restore_window_native(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    enable_dwm_transitions(hwnd);
    send_restore(hwnd);
}

#[must_use]
pub fn restore_window_native_quiet(hwnd_val: isize) -> bool {
    let quiet = suppress_transitions(hwnd_val);
    send_restore(HWND(hwnd_val as *mut c_void));
    quiet
}

fn send_restore(hwnd: HWND) {
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_SYSCOMMAND,
            WPARAM(SC_RESTORE as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            80,
            None,
        );
        let _ = SetForegroundWindow(hwnd);
    }
}

#[must_use]
pub fn current_process_id() -> u32 {
    unsafe { GetCurrentProcessId() }
}

pub const OVERFLOW_ENTRY_KEY: &str = "__overflow__";

#[must_use]
pub fn overflow_dock_entry(hidden: usize) -> DockEntry {
    fixed_dock_entry(
        OVERFLOW_ENTRY_KEY,
        &format!("{hidden} More Apps"),
        DockItemKind::Overflow { hidden },
        AppleIconKind::Folder,
    )
}

fn fixed_dock_entry(
    key: &str,
    title: &str,
    kind: DockItemKind,
    apple_icon: AppleIconKind,
) -> DockEntry {
    DockEntry {
        key: String::from(key),
        title: String::from(title),
        kind,
        apple_icon: Some(apple_icon),
        app: None,
        hwnds: Vec::new(),
        window_titles: Vec::new(),
        running: false,
        active: false,
        pinned: true,
        hidden: false,
        minimized: false,
    }
}

fn with_application(mut entry: DockEntry, app: AppInfo) -> DockEntry {
    entry.hwnds = app
        .primary_windows()
        .map(|window| window.identity.hwnd)
        .collect();
    entry.window_titles = app
        .primary_windows()
        .map(|window| window.title.clone())
        .collect();
    entry.running = true;
    entry.active = app.is_active();
    entry.hidden = app.is_hidden();
    entry.minimized = app.is_minimized();
    entry.app = Some(app);
    entry
}

fn application_entry(key: String, title: String, executable: String, pinned: bool) -> DockEntry {
    DockEntry {
        key,
        title,
        apple_icon: classify_apple_icon(&executable),
        kind: DockItemKind::App {
            exe_path: executable,
        },
        app: None,
        hwnds: Vec::new(),
        window_titles: Vec::new(),
        running: false,
        active: false,
        pinned,
        hidden: false,
        minimized: false,
    }
}

fn merge_finder_windows(apps: &mut Vec<AppInfo>) -> Option<AppInfo> {
    let mut finder: Option<AppInfo> = None;
    let mut index = 0;
    while index < apps.len() {
        if is_explorer_path(&apps[index].executable) {
            let app = apps.remove(index);
            match finder.as_mut() {
                Some(existing) => existing.windows.extend(app.windows),
                None => finder = Some(app),
            }
        } else {
            index += 1;
        }
    }
    finder
}

#[must_use]
pub fn collect_dock_entries(own_pid: u32, order: &mut StableOrder) -> Vec<DockEntry> {
    let pinned = available_pinned_apps();
    let mut apps: Vec<AppInfo> = group_windows(crate::window_model::enumerate_windows(own_pid))
        .into_iter()
        .filter(AppInfo::has_primary_window)
        .collect();

    let finder = fixed_dock_entry(
        "__finder__",
        "Finder",
        DockItemKind::Finder,
        AppleIconKind::Finder,
    );
    let mut entries = vec![
        match merge_finder_windows(&mut apps) {
            Some(app) => with_application(finder, app),
            None => finder,
        },
        fixed_dock_entry(
            "__launchpad__",
            "Launchpad",
            DockItemKind::Launchpad,
            AppleIconKind::Launchpad,
        ),
    ];

    for pin in pinned {
        if is_explorer_path(&pin.executable) || entries.iter().any(|entry| entry.key == pin.key) {
            continue;
        }
        let entry = application_entry(
            pin.key.clone(),
            friendly_app_name(&pin.executable),
            pin.executable,
            true,
        );
        let running = apps
            .iter()
            .position(|app| app.identity.key() == pin.key)
            .map(|index| apps.remove(index));
        entries.push(match running {
            Some(app) => with_application(entry, app),
            None => entry,
        });
    }

    let keys: Vec<String> = apps.iter().map(|app| app.identity.key()).collect();
    let mut unpinned: Vec<Option<AppInfo>> = apps.into_iter().map(Some).collect();
    for index in order.arrange(&keys) {
        if let Some(app) = unpinned.get_mut(index).and_then(Option::take) {
            let title = if app.name.is_empty() {
                app.windows
                    .first()
                    .map(|window| window.title.clone())
                    .unwrap_or_default()
            } else {
                app.name.clone()
            };
            let entry = application_entry(app.identity.key(), title, app.executable.clone(), false);
            entries.push(with_application(entry, app));
        }
    }

    entries.push(fixed_dock_entry(
        "__downloads__",
        "Downloads",
        DockItemKind::Downloads,
        AppleIconKind::Folder,
    ));
    let is_full = is_recycle_bin_full();
    entries.push(fixed_dock_entry(
        "__trash__",
        "Trash",
        DockItemKind::Trash { is_full },
        if is_full {
            AppleIconKind::TrashFull
        } else {
            AppleIconKind::Trash
        },
    ));
    entries
}

#[must_use]
pub fn other_applications_visible(entries: &[DockEntry], key: &str) -> bool {
    entries.iter().any(|entry| {
        entry.key != key
            && entry.app.as_ref().is_some_and(|app| {
                app.primary_windows()
                    .any(|window| window.presence == WindowPresence::Visible)
            })
    })
}

pub fn launch_new_instance(entry: &DockEntry) {
    match &entry.kind {
        DockItemKind::Finder => shell_open(w!("explorer.exe")),
        DockItemKind::Launchpad => crate::actions::open_start_menu(),
        DockItemKind::Overflow { .. } => {}
        DockItemKind::Downloads => shell_open(w!("shell:Downloads")),
        DockItemKind::Trash { .. } => shell_open(w!("shell:RecycleBinFolder")),
        DockItemKind::App { exe_path } => {
            let target = entry
                .key
                .strip_prefix(USER_MODEL_KEY_PREFIX)
                .map_or_else(|| exe_path.clone(), |id| format!("shell:AppsFolder\\{id}"));
            let wide = to_wide(&target);
            shell_open(PCWSTR(wide.as_ptr()));
        }
    }
}

pub fn focus_window_hwnd(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    enable_dwm_transitions(hwnd);
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = SendMessageTimeoutW(
                hwnd,
                WM_SYSCOMMAND,
                WPARAM(SC_RESTORE as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                80,
                None,
            );
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

fn push_capable(rows: &mut Vec<MenuRow>, capability: Capability, id: usize, label: &str) {
    match capability {
        Capability::Available => rows.push(MenuRow::item(id, label, "")),
        Capability::Unsupported => {}
        Capability::TemporarilyUnavailable | Capability::Denied => {
            rows.push(MenuRow::item_with_enabled(id, label, "", false));
        }
    }
}

#[must_use]
pub fn build_dock_context_menu_rows(entry: &DockEntry, other_apps_visible: bool) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    if let Some(app) = &entry.app {
        for (index, window) in app.primary_windows().enumerate() {
            rows.push(MenuRow::check_item(
                CMD_DOCK_WINDOW_BASE + index,
                truncate_menu_label(&window.title, 34),
                window.active,
                true,
            ));
        }
    }
    if !rows.is_empty() {
        rows.push(MenuRow::Separator);
    }
    let open_label = if entry.running { "Show" } else { "Open" };
    match &entry.kind {
        DockItemKind::Finder => {
            rows.push(MenuRow::item(CMD_DOCK_NEW_WINDOW, "New Finder Window", ""));
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, open_label, ""));
        }
        DockItemKind::Overflow { .. } => {}
        DockItemKind::Launchpad => {
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, "Open Start", ""));
        }
        DockItemKind::Downloads => {
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, "Open Downloads", ""));
        }
        DockItemKind::Trash { is_full } => {
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, "Open", ""));
            rows.push(MenuRow::Separator);
            rows.push(MenuRow::check_item(
                CMD_DOCK_EMPTY_TRASH,
                "Empty Trash...",
                false,
                *is_full,
            ));
        }
        DockItemKind::App { .. } => {
            rows.push(MenuRow::check_item(
                CMD_DOCK_PIN,
                "Keep in Dock",
                entry.pinned,
                true,
            ));
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, open_label, ""));
            if let Some(app) = &entry.app {
                rows.push(MenuRow::item(CMD_DOCK_NEW_WINDOW, "Open New Instance", ""));
                push_capable(
                    &mut rows,
                    app.capability(AppCommand::Hide, other_apps_visible),
                    CMD_DOCK_HIDE,
                    "Hide",
                );
                rows.push(MenuRow::Separator);
                push_capable(
                    &mut rows,
                    app.capability(AppCommand::Quit, other_apps_visible),
                    CMD_DOCK_QUIT,
                    "Quit",
                );
            }
        }
    }
    while matches!(rows.last(), Some(MenuRow::Separator)) {
        rows.pop();
    }
    rows
}

fn truncate_menu_label(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return String::from(trimmed);
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn toggle_pin_entry(entry: &DockEntry) {
    let DockItemKind::App { exe_path } = &entry.kind else {
        return;
    };
    let Some(pins) = read_pinned_file() else {
        return;
    };
    save_pinned_apps(&toggled_pins(pins, &entry.key, exe_path));
}

#[must_use]
pub fn extract_dock_icon(entry: &DockEntry) -> Option<IconBitmap> {
    if entry.apple_icon.is_some() {
        return None;
    }
    let DockItemKind::App { exe_path } = &entry.kind else {
        return None;
    };
    let raw = file_icon_bitmap(exe_path)
        .or_else(|| entry.hwnds.first().copied().and_then(window_icon_bitmap))?;
    Some(normalize_to_macos_squircle(&raw))
}

fn enable_dwm_transitions(hwnd: HWND) {
    let _ = release_transitions(hwnd.0 as isize);
}

pub(crate) fn set_dwm_transition_policy(hwnd: HWND, enabled: bool) -> bool {
    let disable = u32::from(!enabled);
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&raw const disable).cast(),
            size_of::<u32>() as u32,
        )
        .is_ok()
    }
}

pub(crate) fn window_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

pub(crate) fn is_shell_or_desktop_class(class: &str) -> bool {
    matches!(
        class,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "Windows.UI.Core.CoreWindow"
            | "XamlExplorerHostIslandWindow"
            | "ForegroundStaging"
            | "MultitaskingViewFrame"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
    )
}

pub(crate) fn visible_window_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut rect).cast(),
            size_of::<RECT>() as u32,
        )
    };
    if hr.is_err() || rect.right <= rect.left || rect.bottom <= rect.top {
        unsafe {
            let _ = GetWindowRect(hwnd, &mut rect);
        }
    }
    rect
}

fn is_normal_visible_window(hwnd: HWND, own_pid: u32) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        if pid == own_pid || pid == 0 {
            return false;
        }
        let class = window_class_name(hwnd);
        if is_shell_or_desktop_class(&class) {
            return false;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 && ex_style & WS_EX_APPWINDOW.0 == 0 {
            return false;
        }
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return false;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        cloaked == 0 && GetWindowTextLengthW(hwnd) > 0
    }
}

pub(crate) fn visible_root_owner(window: HWND) -> HWND {
    let mut current = unsafe { GetAncestor(window, GA_ROOT) };
    if current.0.is_null() {
        current = window;
    }
    for _ in 0..MAX_OWNER_DEPTH {
        let owner = unsafe { GetWindow(current, GW_OWNER) }.unwrap_or_default();
        if owner.0.is_null() || !unsafe { IsWindowVisible(owner) }.as_bool() {
            break;
        }
        current = owner;
    }
    current
}

fn foreground_root_hwnd() -> Option<isize> {
    let foreground = crate::os_events::current_foreground();
    (!foreground.0.is_null()).then(|| visible_root_owner(foreground).0 as isize)
}

fn shell_open(target: PCWSTR) {
    unsafe {
        let _ = ShellExecuteW(
            None,
            w!("open"),
            target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOW,
        );
    }
}

fn file_icon_bitmap(path: &str) -> Option<IconBitmap> {
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

fn image_list_icon_bitmap(index: i32) -> Option<IconBitmap> {
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

fn is_sparse_jumbo_icon(bmp: &IconBitmap) -> bool {
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

fn window_icon_bitmap(hwnd_val: isize) -> Option<IconBitmap> {
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

fn hicon_to_bitmap(hicon: HICON) -> Option<IconBitmap> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(hicon, &raw mut info).ok()?;
        let image = read_color_bitmap(&info);
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        image
    }
}

unsafe fn read_color_bitmap(info: &ICONINFO) -> Option<IconBitmap> {
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

fn premultiply(pixels: &mut [u8]) {
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

fn normalize_to_macos_squircle(src: &IconBitmap) -> IconBitmap {
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

            let shadow_d = rounded_box_sdf(fx, fy - 4.0, TILE_MIN + 2.0, TILE_MAX - 2.0, RADIUS);
            let mut dst = [0.0_f32; 4];
            if shadow_d < 8.0 {
                let sa = ((8.0 - shadow_d) / 10.0).clamp(0.0, 1.0).powi(2) * 0.34;
                dst = [0.0, 0.0, 0.0, sa];
            }

            let d = rounded_box_sdf(fx, fy, TILE_MIN, TILE_MAX, RADIUS);
            let tile_cov = (0.75 - d).clamp(0.0, 1.0);

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

fn rounded_box_sdf(px: f32, py: f32, min_v: f32, max_v: f32, r: f32) -> f32 {
    let cx = (min_v + max_v) * 0.5;
    let half = (max_v - min_v) * 0.5 - r;
    let dx = (px - cx).abs() - half;
    let dy = (py - cx).abs() - half;
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    let inside = dx.max(dy).min(0.0);
    outside + inside - r
}

fn trim_bounds(bmp: &IconBitmap) -> (u32, u32, u32, u32, f32) {
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

fn sample_bilinear(bmp: &IconBitmap, sx: f32, sy: f32) -> [f32; 4] {
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

fn read_px(bmp: &IconBitmap, x: u32, y: u32) -> [f32; 4] {
    let i = ((y * bmp.width + x) * 4) as usize;
    [
        f32::from(bmp.pixels[i]) / 255.0,
        f32::from(bmp.pixels[i + 1]) / 255.0,
        f32::from(bmp.pixels[i + 2]) / 255.0,
        f32::from(bmp.pixels[i + 3]) / 255.0,
    ]
}

fn blend_over(dst: [f32; 4], src: [f32; 4]) -> [f32; 4] {
    let inv = 1.0 - src[3];
    [
        src[0] + dst[0] * inv,
        src[1] + dst[1] * inv,
        src[2] + dst[2] * inv,
        src[3] + dst[3] * inv,
    ]
}

fn classify_apple_icon(exe_path: &str) -> Option<AppleIconKind> {
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
        "spotify" | "music" | "itunes" | "wmplayer" | "foobar2000" | "vlc" => {
            Some(AppleIconKind::Music)
        }
        _ => None,
    }
}

pub(crate) fn is_explorer_path(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("explorer.exe"))
}

pub(crate) fn friendly_app_name(exe_path: &str) -> String {
    let stem = Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("App");
    let lower = stem.to_ascii_lowercase();
    match lower.as_str() {
        "explorer" => return String::from("Finder"),
        "windowsterminal" | "wt" | "powershell" | "pwsh" | "cmd" => {
            return String::from("Terminal");
        }
        "systemsettings" | "control" => return String::from("Settings"),
        "msedge" => return String::from("Edge"),
        "chrome" => return String::from("Chrome"),
        "firefox" => return String::from("Firefox"),
        "brave" => return String::from("Brave"),
        "code" | "code - insiders" => return String::from("Code"),
        "cursor" => return String::from("Cursor"),
        "windsurf" => return String::from("Windsurf"),
        "spotify" | "music" | "itunes" | "wmplayer" => return String::from("Music"),
        "notepad" => return String::from("Notes"),
        _ => {}
    }
    let mut out = String::with_capacity(stem.len());
    for (i, ch) in stem.chars().take(16).enumerate() {
        if ch == '_' || ch == '-' {
            out.push(' ');
        } else if i == 0 {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

const USER_MODEL_KEY_PREFIX: &str = "aumid:";
const MAX_PINNED_APPS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedApp {
    pub key: String,
    pub executable: String,
}

fn parse_pinned_line(line: &str) -> Option<PinnedApp> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if line.starts_with(USER_MODEL_KEY_PREFIX) {
        let (key, executable) = line.split_once('|')?;
        let key = key.trim();
        let executable = executable.trim();
        let has_identity = key.len() > USER_MODEL_KEY_PREFIX.len();
        return (has_identity && !executable.is_empty()).then(|| PinnedApp {
            key: String::from(key),
            executable: String::from(executable),
        });
    }
    Some(PinnedApp {
        key: line.to_lowercase(),
        executable: String::from(line),
    })
}

fn parse_pinned(content: &str) -> Vec<PinnedApp> {
    let mut seen = HashSet::new();
    content
        .lines()
        .filter_map(parse_pinned_line)
        .filter(|pin| seen.insert(pin.key.clone()))
        .collect()
}

fn toggled_pins(mut pins: Vec<PinnedApp>, key: &str, executable: &str) -> Vec<PinnedApp> {
    if let Some(position) = pins.iter().position(|pin| pin.key == key) {
        pins.remove(position);
    } else if pins.len() < MAX_PINNED_APPS {
        pins.push(PinnedApp {
            key: String::from(key),
            executable: String::from(executable),
        });
    }
    pins
}

fn format_pinned(pins: &[PinnedApp]) -> String {
    let mut body = String::new();
    for pin in pins {
        if pin.key.starts_with(USER_MODEL_KEY_PREFIX) {
            body.push_str(&pin.key);
            body.push('|');
        }
        body.push_str(&pin.executable);
        body.push('\n');
    }
    body
}

fn move_pin(pins: &mut Vec<PinnedApp>, moved_key: &str, target_key: &str) -> bool {
    let Some(from) = pins.iter().position(|pin| pin.key == moved_key) else {
        return false;
    };
    let Some(to) = pins.iter().position(|pin| pin.key == target_key) else {
        return false;
    };
    if from == to {
        return false;
    }
    let pin = pins.remove(from);
    pins.insert(to, pin);
    true
}

fn pinned_config_path() -> Option<PathBuf> {
    crate::file::data_dir().map(|dir| dir.join("pinned.txt"))
}

fn pinned_from_read(read: io::Result<String>) -> Option<Vec<PinnedApp>> {
    match read {
        Ok(content) => Some(parse_pinned(&content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

fn read_pinned_file() -> Option<Vec<PinnedApp>> {
    let path = pinned_config_path()?;
    pinned_from_read(fs::read_to_string(path))
}

fn available_pinned_apps() -> Vec<PinnedApp> {
    read_pinned_file()
        .unwrap_or_default()
        .into_iter()
        .filter(|pin| Path::new(&pin.executable).exists())
        .collect()
}

#[must_use]
pub fn remove_pinned_app(key: &str) -> bool {
    let Some(mut list) = read_pinned_file() else {
        return false;
    };
    let before = list.len();
    list.retain(|pin| pin.key != key);
    list.len() != before && save_pinned_apps(&list)
}

#[must_use]
pub fn reorder_pinned_apps(moved_key: &str, target_key: &str) -> bool {
    let Some(mut list) = read_pinned_file() else {
        return false;
    };
    move_pin(&mut list, moved_key, target_key) && save_pinned_apps(&list)
}

#[must_use]
pub fn read_downloads_stack_items(max_items: usize) -> Vec<StackItem> {
    let Some(base) = std::env::var_os("USERPROFILE") else {
        return Vec::new();
    };
    let downloads_dir = PathBuf::from(base).join("Downloads");
    let Ok(entries) = fs::read_dir(downloads_dir) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        items.push((modified, StackItem { path, name, is_dir }));
    }
    items.sort_by_key(|b| std::cmp::Reverse(b.0));
    items
        .into_iter()
        .take(max_items)
        .map(|(_, item)| item)
        .collect()
}

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
            let _ = crate::trash::recycle_path(file_path);
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
        DockItemKind::Launchpad | DockItemKind::Overflow { .. } => {}
    }
}

fn save_pinned_apps(apps: &[PinnedApp]) -> bool {
    let Some(path) = pinned_config_path() else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    crate::file::replace_file_atomically(&path, format_pinned(apps).as_bytes()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{
        DockEntry, DockItemKind, MAX_PINNED_APPS, PinnedApp, build_dock_context_menu_rows,
        format_pinned, move_pin, parse_pinned, pinned_from_read, toggled_pins, window_covers_dock,
    };
    use otto_core::{
        AppIdentity, CMD_DOCK_WINDOW_BASE, MenuRow, WindowIdentity, WindowInfo, WindowPresence,
        WindowRole, WindowTraits, group_windows,
    };
    use windows::Win32::Foundation::RECT;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn windows_on_another_monitor_never_hide_the_dock() {
        let full = rect(0, 0, 1920, 1080);
        assert!(!window_covers_dock(false, true, full, 1920, 1080));
        assert!(!window_covers_dock(false, false, full, 1920, 1080));
        assert!(window_covers_dock(
            true,
            true,
            rect(0, 0, 10, 10),
            1920,
            1080
        ));
        assert!(window_covers_dock(true, false, full, 1920, 1080));
        assert!(!window_covers_dock(
            true,
            false,
            rect(0, 0, 1200, 800),
            1920,
            1080
        ));
    }

    fn window(hwnd: isize) -> WindowInfo {
        WindowInfo {
            identity: WindowIdentity {
                hwnd,
                process_id: 7,
                thread_id: 8,
                generation: 9,
            },
            app: AppIdentity::executable("editor.exe"),
            app_name: String::from("Editor"),
            executable: String::from("editor.exe"),
            title: format!("Document {hwnd}"),
            role: WindowRole::Primary,
            presence: WindowPresence::Visible,
            zoomed: false,
            full_screen: false,
            active: false,
            traits: WindowTraits::STANDARD,
        }
    }

    #[test]
    fn every_window_of_a_busy_app_is_listed_in_the_dock_menu() {
        let app = group_windows((1..=12).map(window).collect()).remove(0);
        let entry = DockEntry {
            key: String::from("editor.exe"),
            title: String::from("Editor"),
            kind: DockItemKind::App {
                exe_path: String::from("editor.exe"),
            },
            apple_icon: None,
            app: Some(app),
            hwnds: (1..=12).collect(),
            window_titles: Vec::new(),
            running: true,
            active: false,
            pinned: false,
            hidden: false,
            minimized: false,
        };
        let rows = build_dock_context_menu_rows(&entry, true);
        let window_ids: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                MenuRow::Item { id, .. } if *id >= CMD_DOCK_WINDOW_BASE => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(window_ids.len(), 12);
        assert_eq!(window_ids[8], CMD_DOCK_WINDOW_BASE + 8);
        assert_eq!(window_ids[11], CMD_DOCK_WINDOW_BASE + 11);
    }

    fn pin(key: &str, executable: &str) -> PinnedApp {
        PinnedApp {
            key: String::from(key),
            executable: String::from(executable),
        }
    }

    #[test]
    fn pinned_file_round_trips_executables_and_packaged_identities() {
        let pins = vec![
            pin("c:\\tools\\app.exe", "C:\\Tools\\App.exe"),
            pin(
                "aumid:Vendor.App_abc!Main",
                "C:\\Program Files\\WindowsApps\\app.exe",
            ),
        ];
        assert_eq!(parse_pinned(&format_pinned(&pins)), pins);
    }

    #[test]
    fn legacy_plain_path_lines_are_still_accepted() {
        let pins = parse_pinned("C:\\Tools\\App.exe\n\n  C:\\Other\\Tool.exe  \n");
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].key, "c:\\tools\\app.exe");
        assert_eq!(pins[1].executable, "C:\\Other\\Tool.exe");
    }

    #[test]
    fn duplicate_and_malformed_pins_are_dropped() {
        let pins = parse_pinned("C:\\a.exe\nc:\\A.EXE\naumid:\naumid:Only.Id\naumid:X|\n");
        assert_eq!(pins, vec![pin("c:\\a.exe", "C:\\a.exe")]);
    }

    #[test]
    fn blank_identity_pins_are_dropped_and_identities_are_trimmed() {
        let pins = parse_pinned("aumid:   |C:\\a.exe\naumid:Vendor.App!Main  |  C:\\b.exe  \n");
        assert_eq!(
            pins,
            vec![PinnedApp {
                key: String::from("aumid:Vendor.App!Main"),
                executable: String::from("C:\\b.exe"),
            }]
        );
    }

    #[test]
    fn unreadable_pin_file_is_never_treated_as_an_empty_list() {
        assert_eq!(
            pinned_from_read(Err(std::io::Error::from(std::io::ErrorKind::NotFound))),
            Some(Vec::new())
        );
        assert_eq!(
            pinned_from_read(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "not utf-8",
            ))),
            None
        );
        assert_eq!(
            pinned_from_read(Ok(String::from("C:\\a.exe\n"))).map(|pins| pins.len()),
            Some(1)
        );
    }

    #[test]
    fn long_pin_lists_survive_a_rewrite_and_only_new_pins_are_capped() {
        let lines: Vec<String> = (0..MAX_PINNED_APPS + 2)
            .map(|index| format!("C:\\Apps\\app{index}.exe"))
            .collect();
        let text = lines.join("\n");
        let pins = parse_pinned(&text);
        assert_eq!(pins.len(), MAX_PINNED_APPS + 2);
        assert_eq!(parse_pinned(&format_pinned(&pins)), pins);
        let added = toggled_pins(pins.clone(), "c:\\new\\tool.exe", "C:\\new\\tool.exe");
        assert_eq!(added.len(), MAX_PINNED_APPS + 2);
        let first = pins[0].key.clone();
        let removed = toggled_pins(pins, &first, "");
        assert_eq!(removed.len(), MAX_PINNED_APPS + 1);
    }

    #[test]
    fn toggling_a_pin_adds_it_then_removes_it() {
        let added = toggled_pins(Vec::new(), "c:\\a.exe", "C:\\a.exe");
        assert_eq!(added, vec![pin("c:\\a.exe", "C:\\a.exe")]);
        assert!(toggled_pins(added, "c:\\a.exe", "C:\\a.exe").is_empty());
    }

    #[test]
    fn reordering_uses_keys_so_unavailable_pins_keep_their_place() {
        let mut pins = vec![pin("a", "a"), pin("missing", "missing"), pin("b", "b")];
        assert!(move_pin(&mut pins, "b", "a"));
        let keys: Vec<&str> = pins.iter().map(|pin| pin.key.as_str()).collect();
        assert_eq!(keys, ["b", "a", "missing"]);
        assert!(!move_pin(&mut pins, "b", "b"));
        assert!(!move_pin(&mut pins, "b", "unknown"));
        assert_eq!(pins.len(), 3);
    }
}
