mod drop;
mod icons;
mod pins;
mod port;
mod stack;
pub(crate) mod trash;

pub use self::port::WindowsDockPort;

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use otto_core::{
    AppCommand, AppInfo, AppleIconKind, CMD_DOCK_EMPTY_TRASH, CMD_DOCK_HIDE, CMD_DOCK_NEW_WINDOW,
    CMD_DOCK_PIN, CMD_DOCK_QUIT, CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, Capability, CommandOutcome,
    DockEffect, DockEntry, DockEntryOptions, DockItemKind, IconBitmap, MenuRow, StableOrder,
    StackItem, group_windows,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint,
    MonitorFromWindow,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, IsIconic, IsZoomed, SC_RESTORE, SMTO_ABORTIFHUNG, SW_SHOW, SendMessageTimeoutW,
    SetForegroundWindow, ShowWindow, WM_SYSCOMMAND,
};
use windows::core::{BOOL, PCWSTR, w};

pub use self::drop::enable_drag_accept_files;
pub use self::drop::extract_dropped_files;
pub use self::drop::open_file_with_dock_entry;
use self::icons::classify_apple_icon;
pub use self::icons::extract_dock_icon;
pub use self::icons::extract_window_icon;
pub use self::pins::PinnedApp;
use self::pins::USER_MODEL_KEY_PREFIX;
use self::pins::available_pinned_apps;
use self::pins::insert_pin_at;
use self::pins::read_pinned_file;
pub use self::pins::remove_pinned_app;
pub use self::pins::reorder_pinned_apps;
use self::pins::save_pinned_apps;
use self::pins::toggled_pins;
pub use self::stack::read_downloads_active_progress;
pub use self::stack::read_downloads_stack_items;
pub use self::stack::read_downloads_stack_thumbnails;
use crate::dock::trash::is_recycle_bin_full;
use crate::window::ops::enable_dwm_transitions;
use crate::window::ops::is_normal_visible_window;
use crate::window::ops::rects_overlap;
use crate::window::ops::visible_window_rect;
use crate::window::ops::window_is_fullscreen_on_primary;

#[must_use]
pub fn window_overlaps_zone(zone: otto_core::Rect, own_pid: u32) -> bool {
    struct Probe {
        zone: RECT,
        own_pid: u32,
        found: bool,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = unsafe { &mut *(lparam.0 as *mut Probe) };
        if is_normal_visible_window(hwnd, probe.own_pid)
            && rects_overlap(visible_window_rect(hwnd), probe.zone)
        {
            probe.found = true;
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut probe = Probe {
        zone: RECT {
            left: zone.x,
            top: zone.y,
            right: zone.right(),
            bottom: zone.bottom(),
        },
        own_pid,
        found: false,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.found
}

#[must_use]
pub fn fullscreen_on_primary(screen_width: i32, screen_height: i32, own_pid: u32) -> bool {
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
            let rect = if on_primary && !zoomed {
                visible_window_rect(hwnd)
            } else {
                RECT::default()
            };
            if window_is_fullscreen_on_primary(
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
pub fn pin_entry_at_target(entry: &DockEntry, target_key: &str) -> bool {
    let DockItemKind::App { exe_path } = &entry.kind else {
        return false;
    };
    let Some(pins) = read_pinned_file() else {
        return false;
    };
    let Some(updated) = insert_pin_at(pins, &entry.key, exe_path, target_key) else {
        return false;
    };
    save_pinned_apps(&updated)
}

pub fn reveal_entry_in_explorer(entry: &DockEntry) {
    match &entry.kind {
        DockItemKind::App { exe_path } if !exe_path.is_empty() => {
            let wide_args = to_wide(&format!("/select,\"{exe_path}\""));
            unsafe {
                let _ = ShellExecuteW(
                    None,
                    w!("open"),
                    w!("explorer.exe"),
                    PCWSTR(wide_args.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOW,
                );
            }
        }
        DockItemKind::Downloads => shell_open(w!("shell:Downloads")),
        DockItemKind::Finder => shell_open(w!("explorer.exe")),
        DockItemKind::Trash { .. } => shell_open(w!("shell:RecycleBinFolder")),
        DockItemKind::App { .. } | DockItemKind::Apps | DockItemKind::Overflow { .. } => {}
    }
}

#[must_use]
pub fn force_quit_entry(entry: &DockEntry, own_pid: u32) -> CommandOutcome {
    let Some(app) = entry.app.as_ref() else {
        return CommandOutcome::TargetGone;
    };
    let pids = app.process_ids();
    if pids.is_empty() {
        return CommandOutcome::TargetGone;
    }
    let mut any_completed = false;
    let mut any_denied = false;
    for pid in pids {
        if pid == 0 || pid == own_pid {
            continue;
        }
        if app
            .windows
            .iter()
            .any(|window| window.identity.process_id == pid && window.traits.elevated)
        {
            any_denied = true;
            continue;
        }
        match unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) } {
            Ok(handle) => {
                let terminated = unsafe { TerminateProcess(handle, 1) }.is_ok();
                unsafe {
                    let _ = CloseHandle(handle);
                }
                if terminated {
                    any_completed = true;
                } else {
                    any_denied = true;
                }
            }
            Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                any_denied = true;
            }
            Err(_) => {}
        }
    }
    if any_completed {
        CommandOutcome::Completed
    } else if any_denied {
        CommandOutcome::Denied
    } else {
        CommandOutcome::TargetGone
    }
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

pub fn collect_dock_entries(own_pid: u32, order: &mut StableOrder) -> Vec<DockEntry> {
    collect_dock_entries_with_options(own_pid, order, DockEntryOptions::default())
}

pub fn collect_dock_entries_with_options(
    own_pid: u32,
    order: &mut StableOrder,
    options: DockEntryOptions,
) -> Vec<DockEntry> {
    let pinned = available_pinned_apps();
    let mut apps: Vec<AppInfo> = group_windows(crate::window::model::enumerate_windows(own_pid))
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
            "__apps__",
            "Apps",
            DockItemKind::Apps,
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

    if options.show_downloads {
        entries.push(fixed_dock_entry(
            "__downloads__",
            "Downloads",
            DockItemKind::Downloads,
            AppleIconKind::Folder,
        ));
    }
    if options.show_trash {
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
    }
    entries
}

pub fn launch_new_instance(entry: &DockEntry) {
    match &entry.kind {
        DockItemKind::Finder => shell_open(w!("explorer.exe")),
        DockItemKind::Apps | DockItemKind::Overflow { .. } => {}
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
    let alt_down = (unsafe { GetAsyncKeyState(0x12) } as u16 & 0x8000) != 0;
    build_dock_context_menu_rows_with_alt(entry, other_apps_visible, alt_down)
}

fn build_dock_context_menu_rows_with_alt(
    entry: &DockEntry,
    other_apps_visible: bool,
    alt_down: bool,
) -> Vec<MenuRow> {
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
        DockItemKind::Apps => {
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, "Open Apps", ""));
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
            rows.push(MenuRow::item(DockEffect::CMD_REVEAL, "Show in Finder", ""));
            rows.push(MenuRow::item(CMD_DOCK_TOGGLE, open_label, ""));
            if let Some(app) = &entry.app {
                rows.push(MenuRow::item(CMD_DOCK_NEW_WINDOW, "Open New Instance", ""));
                if alt_down {
                    push_capable(
                        &mut rows,
                        app.capability(AppCommand::HideOthers, other_apps_visible),
                        DockEffect::CMD_HIDE_OTHERS,
                        "Hide Others",
                    );
                } else {
                    push_capable(
                        &mut rows,
                        app.capability(AppCommand::Hide, other_apps_visible),
                        CMD_DOCK_HIDE,
                        "Hide",
                    );
                }
                rows.push(MenuRow::Separator);
                if alt_down {
                    let elevated = app.windows.iter().any(|window| window.traits.elevated);
                    let force_quit_cap = if !app.has_primary_window() {
                        Capability::Unsupported
                    } else if elevated {
                        Capability::Denied
                    } else {
                        Capability::Available
                    };
                    push_capable(
                        &mut rows,
                        force_quit_cap,
                        DockEffect::CMD_FORCE_QUIT,
                        "Force Quit",
                    );
                } else {
                    push_capable(
                        &mut rows,
                        app.capability(AppCommand::Quit, other_apps_visible),
                        CMD_DOCK_QUIT,
                        "Quit",
                    );
                }
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

pub(super) fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pins::{
        MAX_PINNED_APPS, build_default_pinned_apps, format_pinned, insert_pin_at, move_pin,
        parse_pinned, pinned_from_read, toggled_pins,
    };
    use super::{
        AppleIconKind, DockEntry, DockItemKind, PinnedApp, build_dock_context_menu_rows_with_alt,
        classify_apple_icon, rects_overlap, window_is_fullscreen_on_primary, window_overlaps_zone,
    };
    use crate::window::ops::window_covers_dock;
    use otto_core::{
        AppIdentity, CMD_DOCK_HIDE, CMD_DOCK_QUIT, CMD_DOCK_WINDOW_BASE, DockEffect, MenuRow,
        WindowIdentity, WindowInfo, WindowPresence, WindowRole, WindowTraits, group_windows,
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
    fn popular_music_players_get_the_music_tile_and_video_or_unknown_apps_do_not() {
        for exe in [
            r"C:\Apps\Spotify.exe",
            r"C:\Apps\AppleMusic.exe",
            r"C:\Apps\YouTube Music.exe",
            r"C:\Apps\Amazon Music.exe",
            r"C:\Apps\TIDAL.exe",
            r"C:\Apps\Deezer.exe",
            r"C:\Apps\Qobuz.exe",
            r"C:\Apps\Plexamp.exe",
            r"C:\Apps\foobar2000.exe",
            r"C:\Apps\MusicBee.exe",
            r"C:\Apps\AIMP.exe",
            r"C:\Apps\winamp.exe",
        ] {
            assert_eq!(
                classify_apple_icon(exe),
                Some(AppleIconKind::Music),
                "{exe}"
            );
        }
        for exe in [
            r"C:\Apps\Plex.exe",
            r"C:\Apps\Kodi.exe",
            r"C:\Apps\Netflix.exe",
            r"C:\Apps\SomethingElse.exe",
        ] {
            assert_eq!(classify_apple_icon(exe), None, "{exe}");
        }
    }

    #[test]
    fn overlap_needs_real_intersection_and_touching_edges_do_not_count() {
        let zone = rect(400, 1000, 1500, 1080);
        assert!(rects_overlap(rect(0, 0, 1920, 1080), zone));
        assert!(rects_overlap(rect(1400, 900, 1700, 1010), zone));
        assert!(!rects_overlap(rect(0, 0, 1920, 1000), zone));
        assert!(!rects_overlap(rect(0, 1000, 400, 1080), zone));
        assert!(!rects_overlap(rect(1500, 1000, 1920, 1080), zone));
    }

    #[test]
    #[ignore = "creates a real window far off screen"]
    fn a_real_window_inside_the_zone_is_detected_and_one_outside_is_not() {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SW_SHOWNOACTIVATE, ShowWindow, WS_EX_APPWINDOW,
            WS_POPUP,
        };
        use windows::core::w;

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW,
                w!("STATIC"),
                w!("otto-dock-overlap-test"),
                WS_POPUP,
                30_000,
                30_000,
                400,
                300,
                None,
                None,
                None,
                None,
            )
        }
        .expect("create probe window");
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        let bounds = super::visible_window_rect(hwnd);
        let inside = otto_core::Rect {
            x: bounds.left + 20,
            y: bounds.bottom - 40,
            width: 200,
            height: 100,
        };
        let outside = otto_core::Rect {
            x: bounds.right + 5_000,
            y: bounds.bottom + 5_000,
            width: 200,
            height: 100,
        };
        let own_other = std::process::id() + 1;
        let detected = window_overlaps_zone(inside, own_other);
        let ignored_as_own = window_overlaps_zone(inside, std::process::id());
        let elsewhere = window_overlaps_zone(outside, own_other);
        let _ = unsafe { DestroyWindow(hwnd) };
        assert!(detected);
        assert!(!ignored_as_own);
        assert!(!elsewhere);
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
        let rows = build_dock_context_menu_rows_with_alt(&entry, true, false);
        let window_ids: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                MenuRow::Item { id, .. }
                    if *id >= CMD_DOCK_WINDOW_BASE && *id < DockEffect::CMD_REVEAL =>
                {
                    Some(*id)
                }
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

    #[test]
    fn default_pinned_apps_seed_only_existing_unique_candidates() {
        let candidates = vec![
            std::path::PathBuf::from("C:\\Tools\\Edge.exe"),
            std::path::PathBuf::from("c:\\tools\\edge.exe"),
            std::path::PathBuf::from("C:\\Tools\\Missing.exe"),
            std::path::PathBuf::from("C:\\Windows\\System32\\notepad.exe"),
        ];
        let seeded = build_default_pinned_apps(&candidates, |path| {
            !path.to_string_lossy().contains("Missing")
        });
        assert_eq!(
            seeded,
            vec![
                pin("c:\\tools\\edge.exe", "C:\\Tools\\Edge.exe"),
                pin(
                    "c:\\windows\\system32\\notepad.exe",
                    "C:\\Windows\\System32\\notepad.exe"
                ),
            ]
        );
    }

    #[test]
    fn active_download_temp_names_filter_cleanly() {
        assert!(super::stack::is_active_download_temp_name(
            "Unconfirmed 123.crdownload"
        ));
        assert!(super::stack::is_active_download_temp_name(
            "archive.zip.part"
        ));
        assert!(super::stack::is_active_download_temp_name("setup.TMP"));
        assert!(!super::stack::is_active_download_temp_name("notes.pdf"));
        assert!(!super::stack::is_active_download_temp_name("photo.png"));
    }

    #[test]
    fn fullscreen_detection_excludes_maximized_and_secondary_monitor_windows() {
        let full = rect(0, 0, 1920, 1080);
        assert!(window_is_fullscreen_on_primary(
            true, false, full, 1920, 1080
        ));
        assert!(!window_is_fullscreen_on_primary(
            true, true, full, 1920, 1080
        ));
        assert!(!window_is_fullscreen_on_primary(
            false, false, full, 1920, 1080
        ));
        assert!(!window_is_fullscreen_on_primary(
            true,
            false,
            rect(0, 0, 1920, 1040),
            1920,
            1080
        ));
    }

    #[test]
    fn insert_pin_at_places_new_pin_at_target_position() {
        let pins = vec![pin("a", "A.exe"), pin("b", "B.exe")];
        let updated = insert_pin_at(pins.clone(), "c", "C.exe", "b").expect("inserted");
        let keys: Vec<&str> = updated.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["a", "c", "b"]);
        assert!(insert_pin_at(pins.clone(), "a", "A.exe", "b").is_none());
        assert!(insert_pin_at(pins, "c", "C.exe", "missing").is_none());
    }

    #[test]
    fn context_menu_includes_show_in_finder_and_swaps_alt_actions() {
        let app = group_windows(vec![window(1)]).remove(0);
        let entry = DockEntry {
            key: String::from("editor.exe"),
            title: String::from("Editor"),
            kind: DockItemKind::App {
                exe_path: String::from("C:\\Tools\\editor.exe"),
            },
            apple_icon: None,
            app: Some(app),
            hwnds: vec![1],
            window_titles: Vec::new(),
            running: true,
            active: false,
            pinned: false,
            hidden: false,
            minimized: false,
        };
        let normal = build_dock_context_menu_rows_with_alt(&entry, true, false);
        assert!(normal.iter().any(|row| {
            matches!(row, MenuRow::Item { id, label, .. } if *id == DockEffect::CMD_REVEAL && label == "Show in Finder")
        }));
        assert!(normal.iter().any(|row| {
            matches!(row, MenuRow::Item { id, label, .. } if *id == CMD_DOCK_HIDE && label == "Hide")
        }));
        assert!(normal.iter().any(|row| {
            matches!(row, MenuRow::Item { id, label, .. } if *id == CMD_DOCK_QUIT && label == "Quit")
        }));

        let alt = build_dock_context_menu_rows_with_alt(&entry, true, true);
        assert!(alt.iter().any(|row| {
            matches!(row, MenuRow::Item { id, label, .. } if *id == DockEffect::CMD_HIDE_OTHERS && label == "Hide Others")
        }));
        assert!(alt.iter().any(|row| {
            matches!(row, MenuRow::Item { id, label, .. } if *id == DockEffect::CMD_FORCE_QUIT && label == "Force Quit")
        }));
    }
}
