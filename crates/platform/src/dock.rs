use std::collections::HashSet;
use std::ffi::c_void;
use std::fs;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use otto_core::{
    AppleIconKind, CMD_DOCK_EMPTY_TRASH, CMD_DOCK_NEW_WINDOW, CMD_DOCK_PIN, CMD_DOCK_QUIT,
    CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, MenuRow, Rect, TrafficLightAction, WindowControlAction,
    is_blank_frame, select_dock_click_window,
};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAK, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_TRANSITIONS_FORCEDISABLED,
    DwmFlush, DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, GetPixel, HGDIOBJ,
    ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_SYSICONINDEX, SHGetFileInfoW, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO,
    ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, EnumWindows, GA_ROOTOWNER, GCLP_HICON, GW_OWNER,
    GWL_EXSTYLE, GetAncestor, GetClassLongPtrW, GetClassNameW, GetCursorPos, GetForegroundWindow,
    GetIconInfo, GetWindow, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, HICON, ICON_BIG, ICONINFO, IsIconic, IsWindowVisible, IsZoomed,
    MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, PostMessageW, SC_MAXIMIZE, SC_MINIMIZE,
    SC_RESTORE, SMTO_ABORTIFHUNG, SW_SHOW, SendMessageTimeoutW, SetForegroundWindow, ShowWindow,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_CLOSE, WM_GETICON,
    WM_SYSCOMMAND, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PCWSTR, PWSTR, w};

use crate::trash::is_recycle_bin_full;

#[derive(Debug, Clone)]
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockItemKind {
    Finder,
    Launchpad,
    App { exe_path: String },
    Downloads,
    Trash { is_full: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockEntry {
    pub key: String,
    pub title: String,
    pub kind: DockItemKind,
    pub apple_icon: Option<AppleIconKind>,
    pub hwnds: Vec<isize>,
    pub window_titles: Vec<String>,
    pub running: bool,
    pub active: bool,
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockMenuAction {
    TogglePin,
    ToggleWindow,
    QuitApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWindowTarget {
    pub hwnd: isize,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub maximized: bool,
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
        found: bool,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = unsafe { &mut *(lparam.0 as *mut Probe) };
        if is_normal_visible_window(hwnd, probe.own_pid) {
            if unsafe { IsZoomed(hwnd) }.as_bool() {
                probe.found = true;
                return BOOL(0);
            }
            let rect = visible_window_rect(hwnd);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width >= probe.screen_width - 12
                && height >= probe.screen_height - 56
                && rect.bottom >= probe.screen_height - 8
            {
                probe.found = true;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    let mut probe = Probe {
        screen_width,
        screen_height,
        own_pid,
        found: false,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.found
}

#[must_use]
pub fn foreground_app_snapshot(own_pid: u32) -> Option<(String, isize)> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return None;
    }
    let root = unsafe { GetAncestor(fg, GA_ROOTOWNER) };
    let target = if root.0.is_null() { fg } else { root };
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(target, Some(&raw mut pid));
    }
    if pid == own_pid || pid == 0 {
        return None;
    }
    let class = window_class_name(target);
    if matches!(class.as_str(), "Progman" | "WorkerW") {
        return Some((String::from("Finder"), 0));
    }
    if matches!(class.as_str(), "CabinetWClass" | "ExploreWClass") {
        return Some((String::from("Finder"), target.0 as isize));
    }
    if is_shell_or_desktop_class(&class) {
        return None;
    }
    let exe_path = process_image_path(pid)?;
    Some((friendly_app_name(&exe_path), target.0 as isize))
}

#[must_use]
pub fn active_window_target(own_pid: u32) -> Option<ActiveWindowTarget> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return None;
    }
    let root = unsafe { GetAncestor(fg, GA_ROOTOWNER) };
    let target = if root.0.is_null() { fg } else { root };
    if !is_normal_visible_window(target, own_pid) {
        return None;
    }
    let rect = visible_window_rect(target);
    if rect.right - rect.left < 200 || rect.bottom - rect.top < 80 {
        return None;
    }
    let maximized = unsafe { IsZoomed(target).as_bool() };

    Some(ActiveWindowTarget {
        hwnd: target.0 as isize,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        maximized,
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

#[must_use]
pub fn capture_window(hwnd_val: isize) -> Option<WindowSnapshot> {
    let hwnd = HWND(hwnd_val as *mut c_void);
    let rect = window_bounds(hwnd_val)?;
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.0.is_null() {
            return None;
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if memory_dc.0.is_null() {
            ReleaseDC(None, screen_dc);
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: rect.width,
                biHeight: -rect.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) =
            CreateDIBSection(Some(screen_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, screen_dc);
            return None;
        };
        let previous = SelectObject(memory_dc, HGDIOBJ(bitmap.0));

        let mut captured = false;
        let mut pixels = vec![0u8; rect.width as usize * rect.height as usize * 4];
        let mut header = info;

        if PrintWindow(hwnd, memory_dc, PRINT_WINDOW_FLAGS(2)).as_bool() {
            let rows = GetDIBits(
                memory_dc,
                bitmap,
                0,
                rect.height as u32,
                Some(pixels.as_mut_ptr().cast()),
                &raw mut header,
                DIB_RGB_COLORS,
            );
            if rows > 0 && !is_blank_frame(rect.width as u32, rect.height as u32, &pixels) {
                captured = true;
            }
        }

        if !captured && GetForegroundWindow() == hwnd {
            let blt_ok = BitBlt(
                memory_dc,
                0,
                0,
                rect.width,
                rect.height,
                Some(screen_dc),
                rect.x,
                rect.y,
                SRCCOPY,
            )
            .is_ok();
            if blt_ok {
                let rows = GetDIBits(
                    memory_dc,
                    bitmap,
                    0,
                    rect.height as u32,
                    Some(pixels.as_mut_ptr().cast()),
                    &raw mut header,
                    DIB_RGB_COLORS,
                );
                if rows > 0 && !is_blank_frame(rect.width as u32, rect.height as u32, &pixels) {
                    captured = true;
                }
            }
        }

        let _ = SelectObject(memory_dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);

        if !captured {
            return None;
        }

        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        Some(WindowSnapshot {
            rect,
            width: rect.width as u32,
            height: rect.height as u32,
            pixels,
        })
    }
}

#[must_use]
pub fn set_window_cloaked(hwnd_val: isize, cloaked: bool) -> bool {
    let hwnd = HWND(hwnd_val as *mut c_void);
    let value = u32::from(cloaked);
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            (&raw const value).cast(),
            size_of::<u32>() as u32,
        )
        .is_ok()
    }
}

pub fn flush_compositor() {
    unsafe {
        let _ = DwmFlush();
    }
}

pub fn set_window_transitions_enabled(hwnd_val: isize, enabled: bool) {
    set_dwm_transition_policy(HWND(hwnd_val as *mut c_void), enabled);
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

pub fn minimize_window_native_quiet(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    set_dwm_transition_policy(hwnd, false);
    post_minimize(hwnd);
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

pub fn restore_window_native_quiet(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    set_dwm_transition_policy(hwnd, false);
    send_restore(hwnd);
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
pub fn sample_titlebar_rgb(target: ActiveWindowTarget, plate_width: i32) -> (u8, u8, u8) {
    let base_x = (target.right - plate_width - 4).max(target.left + 20);
    let sample_y = target.top + 5;
    let c0 = sample_screen_rgb(base_x, sample_y);
    let c1 = sample_screen_rgb((base_x - 8).max(target.left + 12), sample_y);
    let c2 = sample_screen_rgb((base_x - 16).max(target.left + 8), sample_y);

    let d01 = color_dist(c0, c1);
    let d12 = color_dist(c1, c2);
    let d02 = color_dist(c0, c2);
    if d01 <= d12 && d01 <= d02 {
        c0
    } else if d12 <= d02 {
        c1
    } else {
        c2
    }
}

fn color_dist(a: (u8, u8, u8), b: (u8, u8, u8)) -> i32 {
    (i32::from(a.0) - i32::from(b.0)).abs()
        + (i32::from(a.1) - i32::from(b.1)).abs()
        + (i32::from(a.2) - i32::from(b.2)).abs()
}

pub fn execute_window_control(hwnd_val: isize, action: WindowControlAction) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    enable_dwm_transitions(hwnd);
    unsafe {
        match action {
            WindowControlAction::Close => {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            WindowControlAction::Minimize => {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_SYSCOMMAND,
                    WPARAM(SC_MINIMIZE as usize),
                    LPARAM(0),
                );
            }
            WindowControlAction::ToggleMaximize => {
                let cmd = if IsZoomed(hwnd).as_bool() {
                    SC_RESTORE
                } else {
                    SC_MAXIMIZE
                };
                let _ = PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(cmd as usize), LPARAM(0));
            }
        }
    }
}

pub fn execute_traffic_light(hwnd_val: isize, action: TrafficLightAction) {
    execute_window_control(hwnd_val, action);
}

#[must_use]
pub fn current_process_id() -> u32 {
    unsafe { GetCurrentProcessId() }
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
        hwnds: Vec::new(),
        window_titles: Vec::new(),
        running: false,
        active: false,
        pinned: true,
    }
}

#[must_use]
pub fn collect_dock_entries(own_pid: u32) -> Vec<DockEntry> {
    let pinned_list = load_pinned_apps();
    let pinned_set: HashSet<String> = pinned_list.iter().map(|s| s.to_lowercase()).collect();
    let fg_root = foreground_root_hwnd();
    let raw_windows = enumerate_app_windows(own_pid);

    let mut finder_hwnds = Vec::new();
    let mut finder_titles = Vec::new();
    for win in &raw_windows {
        if is_explorer_path(&win.exe_path) {
            finder_hwnds.push(win.hwnd);
            finder_titles.push(win.title.clone());
        }
    }

    let mut entries = vec![
        DockEntry {
            key: String::from("__finder__"),
            title: String::from("Finder"),
            kind: DockItemKind::Finder,
            apple_icon: Some(AppleIconKind::Finder),
            running: !finder_hwnds.is_empty(),
            active: fg_root.is_some_and(|fg| finder_hwnds.contains(&fg)),
            pinned: true,
            hwnds: finder_hwnds,
            window_titles: finder_titles,
        },
        fixed_dock_entry(
            "__launchpad__",
            "Launchpad",
            DockItemKind::Launchpad,
            AppleIconKind::Launchpad,
        ),
    ];

    for exe in &pinned_list {
        if is_explorer_path(exe) {
            continue;
        }
        let key = exe.to_lowercase();
        if entries.iter().any(|e| e.key == key) {
            continue;
        }
        entries.push(DockEntry {
            key,
            title: friendly_app_name(exe),
            apple_icon: classify_apple_icon(exe),
            kind: DockItemKind::App {
                exe_path: exe.clone(),
            },
            hwnds: Vec::new(),
            window_titles: Vec::new(),
            running: false,
            active: false,
            pinned: true,
        });
    }

    for win in raw_windows {
        if !is_explorer_path(&win.exe_path) {
            merge_running_window(&mut entries, win, &pinned_set, fg_root);
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

fn merge_running_window(
    entries: &mut Vec<DockEntry>,
    win: RawWindow,
    pinned_set: &HashSet<String>,
    fg_root: Option<isize>,
) {
    let key = win.exe_path.to_lowercase();
    if let Some(existing) = entries.iter_mut().find(|e| e.key == key) {
        existing.hwnds.push(win.hwnd);
        existing.window_titles.push(win.title);
        existing.running = true;
        if fg_root == Some(win.hwnd) {
            existing.active = true;
        }
        return;
    }
    let pinned = pinned_set.contains(&key);
    let title = friendly_app_name(&win.exe_path);
    let apple_icon = classify_apple_icon(&win.exe_path);
    entries.push(DockEntry {
        key,
        title: if title.is_empty() {
            win.title.clone()
        } else {
            title
        },
        apple_icon,
        kind: DockItemKind::App {
            exe_path: win.exe_path,
        },
        hwnds: vec![win.hwnd],
        window_titles: vec![win.title],
        running: true,
        active: fg_root == Some(win.hwnd),
        pinned,
    });
}

pub fn activate_or_launch(entry: &DockEntry) {
    match &entry.kind {
        DockItemKind::Finder => {
            if entry.hwnds.len() > 1 {
                if let Some(hwnd) = select_dock_click_window(&entry.hwnds, foreground_root_hwnd()) {
                    focus_window_hwnd(hwnd);
                }
            } else if let Some(&hwnd) = entry.hwnds.first() {
                toggle_window_hwnd(hwnd);
            } else {
                shell_open(w!("explorer.exe"));
            }
        }
        DockItemKind::Launchpad => {
            crate::actions::open_start_menu();
        }
        DockItemKind::Downloads => {
            shell_open(w!("shell:Downloads"));
        }
        DockItemKind::Trash { .. } => {
            shell_open(w!("shell:RecycleBinFolder"));
        }
        DockItemKind::App { exe_path } => {
            if entry.hwnds.len() > 1 {
                if let Some(hwnd) = select_dock_click_window(&entry.hwnds, foreground_root_hwnd()) {
                    focus_window_hwnd(hwnd);
                }
            } else if let Some(&hwnd) = entry.hwnds.first() {
                toggle_window_hwnd(hwnd);
            } else {
                let wide = to_wide(exe_path);
                shell_open(PCWSTR(wide.as_ptr()));
            }
        }
    }
}

pub fn launch_new_instance(entry: &DockEntry) {
    match &entry.kind {
        DockItemKind::Finder => shell_open(w!("explorer.exe")),
        DockItemKind::Launchpad => crate::actions::open_start_menu(),
        DockItemKind::Downloads => shell_open(w!("shell:Downloads")),
        DockItemKind::Trash { .. } => shell_open(w!("shell:RecycleBinFolder")),
        DockItemKind::App { exe_path } => {
            let wide = to_wide(exe_path);
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

#[must_use]
pub fn build_dock_context_menu_rows(entry: &DockEntry) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    let fg = foreground_root_hwnd();
    for (idx, (&hwnd, title)) in entry
        .hwnds
        .iter()
        .zip(&entry.window_titles)
        .enumerate()
        .take(8)
    {
        rows.push(MenuRow::check_item(
            CMD_DOCK_WINDOW_BASE + idx,
            truncate_menu_label(title, 34),
            fg == Some(hwnd),
            true,
        ));
    }
    if !rows.is_empty() {
        rows.push(MenuRow::Separator);
    }

    match &entry.kind {
        DockItemKind::Finder => {
            rows.push(MenuRow::item(
                CMD_DOCK_NEW_WINDOW,
                "New Finder Window",
                "⌘N",
            ));
            rows.push(MenuRow::item(
                CMD_DOCK_TOGGLE,
                if entry.running {
                    "Show / Minimize"
                } else {
                    "Open"
                },
                "",
            ));
        }
        DockItemKind::Launchpad => {
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, "Open Launchpad", ""));
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
            rows.push(MenuRow::item(CMD_DOCK_NEW_WINDOW, "New Window", ""));
            rows.push(MenuRow::item(
                CMD_DOCK_TOGGLE,
                if entry.running {
                    "Show / Minimize"
                } else {
                    "Open"
                },
                "",
            ));
            if entry.running {
                rows.push(MenuRow::Separator);
                rows.push(MenuRow::item(CMD_DOCK_QUIT, "Quit", "⌘Q"));
            }
        }
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
    let mut list = load_pinned_apps();
    let key = exe_path.to_lowercase();
    if let Some(pos) = list.iter().position(|p| p.to_lowercase() == key) {
        list.remove(pos);
    } else {
        list.push(exe_path.clone());
    }
    save_pinned_apps(&list);
}

pub fn close_entry_windows(entry: &DockEntry) {
    for &hwnd_val in &entry.hwnds {
        let hwnd = HWND(hwnd_val as *mut c_void);
        enable_dwm_transitions(hwnd);
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

#[must_use]
pub fn show_dock_context_menu(hwnd: isize, entry: &DockEntry) -> Option<DockMenuAction> {
    const CMD_PIN: usize = 1;
    const CMD_TOGGLE: usize = 2;
    const CMD_QUIT_APP: usize = 3;

    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        if matches!(entry.kind, DockItemKind::App { .. }) {
            let pin_flags = if entry.pinned {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING | MF_UNCHECKED
            };
            let _ = AppendMenuW(menu, pin_flags, CMD_PIN, w!("Keep in Dock"));
        }
        if entry.running && !entry.hwnds.is_empty() {
            let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE, w!("Show / Minimize"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT_APP, w!("Quit"));
        } else if matches!(
            entry.kind,
            DockItemKind::Finder
                | DockItemKind::Launchpad
                | DockItemKind::Downloads
                | DockItemKind::Trash { .. }
        ) {
            let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE, w!("Open"));
        }
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let _ = SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        match selected.0 as usize {
            CMD_PIN => Some(DockMenuAction::TogglePin),
            CMD_TOGGLE => Some(DockMenuAction::ToggleWindow),
            CMD_QUIT_APP => Some(DockMenuAction::QuitApp),
            _ => None,
        }
    }
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

struct RawWindow {
    hwnd: isize,
    title: String,
    exe_path: String,
}

fn enumerate_app_windows(own_pid: u32) -> Vec<RawWindow> {
    struct State {
        own_pid: u32,
        list: Vec<RawWindow>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut State) };
        if let Some(win) = inspect_window(hwnd, state.own_pid) {
            state.list.push(win);
        }
        BOOL(1)
    }

    let mut state = State {
        own_pid,
        list: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut state as isize));
    }
    state.list
}

fn enable_dwm_transitions(hwnd: HWND) {
    set_dwm_transition_policy(hwnd, true);
}

fn set_dwm_transition_policy(hwnd: HWND, enabled: bool) {
    let disable = u32::from(!enabled);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&raw const disable).cast(),
            size_of::<u32>() as u32,
        );
    }
}

fn sample_screen_rgb(px: i32, py: i32) -> (u8, u8, u8) {
    unsafe {
        let dc = GetDC(None);
        if dc.0.is_null() {
            return (32, 33, 36);
        }
        let color = GetPixel(dc, px, py).0;
        ReleaseDC(None, dc);
        if color == u32::MAX {
            return (32, 33, 36);
        }
        (
            (color & 0xFF) as u8,
            ((color >> 8) & 0xFF) as u8,
            ((color >> 16) & 0xFF) as u8,
        )
    }
}

fn window_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

fn is_shell_or_desktop_class(class: &str) -> bool {
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

fn visible_window_rect(hwnd: HWND) -> RECT {
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

fn inspect_window(hwnd: HWND, own_pid: u32) -> Option<RawWindow> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        if pid == own_pid || pid == 0 {
            return None;
        }
        let class = window_class_name(hwnd);
        if is_shell_or_desktop_class(&class) {
            return None;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let app_window = ex_style & WS_EX_APPWINDOW.0 != 0;
        let tool_window = ex_style & WS_EX_TOOLWINDOW.0 != 0;
        let no_activate = ex_style & WS_EX_NOACTIVATE.0 != 0;
        let has_owner = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.0.is_null());
        if no_activate || (!app_window && (tool_window || has_owner)) {
            return None;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return None;
        }
        let title_len = GetWindowTextLengthW(hwnd);
        if title_len <= 0 {
            return None;
        }
        let exe_path = process_image_path(pid)?;
        if is_explorer_path(&exe_path) && class != "CabinetWClass" && class != "ExploreWClass" {
            return None;
        }
        let mut buf = vec![0u16; title_len as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        let title = String::from_utf16_lossy(&buf[..copied.max(0) as usize]);
        Some(RawWindow {
            hwnd: hwnd.0 as isize,
            title,
            exe_path,
        })
    }
}

fn process_image_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut len,
        )
        .is_ok();
        let _ = CloseHandle(process);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn foreground_root_hwnd() -> Option<isize> {
    let root = unsafe { GetAncestor(GetForegroundWindow(), GA_ROOTOWNER) };
    (!root.0.is_null()).then_some(root.0 as isize)
}

fn toggle_window_hwnd(hwnd_val: isize) {
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
            let _ = SetForegroundWindow(hwnd);
        } else if foreground_root_hwnd() == Some(hwnd_val) {
            let _ = PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM(SC_MINIMIZE as usize),
                LPARAM(0),
            );
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
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
    let (handle, owned) = unsafe {
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
            (GetClassLongPtrW(hwnd, GCLP_HICON), false)
        } else {
            (result, true)
        }
    };
    if handle == 0 {
        return None;
    }
    let hicon = HICON(handle as *mut c_void);
    let bmp = hicon_to_bitmap(hicon);
    if owned {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(hicon);
        }
    }
    bmp
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
        if width <= 0 || height <= 0 {
            return None;
        }
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
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let screen = GetDC(None);
        let rows = GetDIBits(
            screen,
            info.hbmColor,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &raw mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, screen);
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

fn is_explorer_path(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("explorer.exe"))
}

fn friendly_app_name(exe_path: &str) -> String {
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

fn pinned_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("otto-os").join("pinned.txt"))
}

fn load_pinned_apps() -> Vec<String> {
    let Some(path) = pinned_config_path() else {
        return Vec::new();
    };
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && Path::new(line).exists())
        .map(String::from)
        .collect()
}

fn save_pinned_apps(apps: &[String]) {
    let Some(path) = pinned_config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = apps.join("\n");
    let _ = fs::write(path, body);
}
