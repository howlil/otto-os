use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::Shell::{
    ABE_TOP, ABM_GETSTATE, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABM_SETSTATE,
    ABN_FULLSCREENAPP, ABN_POSCHANGED, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SHOW_WINDOW_CMD, SPI_GETWORKAREA, SPI_SETWORKAREA, SPIF_SENDCHANGE, SW_HIDE,
    SW_SHOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, ShowWindow, SystemParametersInfoW,
};
use windows::core::{PCWSTR, w};

use crate::file::replace_file_atomically;
use crate::screen::primary_screen;

type WorkArea = [i32; 4];
type ScreenSize = (i32, i32);

pub const APPBAR_CALLBACK_MESSAGE: u32 = 0x8000 + 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellNotification {
    PositionChanged,
    FullScreenApp(bool),
}

#[must_use]
pub fn classify_shell_notification(
    notification: usize,
    lparam: isize,
) -> Option<ShellNotification> {
    match u32::try_from(notification).ok()? {
        ABN_POSCHANGED => Some(ShellNotification::PositionChanged),
        ABN_FULLSCREENAPP => Some(ShellNotification::FullScreenApp(lparam != 0)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reservation {
    Shell,
    WorkArea,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShellRecovery {
    work_area: WorkArea,
    taskbar_state: usize,
    taskbar_hidden: bool,
    screen: Option<ScreenSize>,
    work_area_written: bool,
}

impl ShellRecovery {
    fn parse(content: &str) -> Option<Self> {
        let parts: Vec<&str> = content.trim().split(',').map(str::trim).collect();
        if parts.len() < 6 {
            return None;
        }
        let mut work_area = [0i32; 4];
        for (slot, part) in work_area.iter_mut().zip(&parts) {
            *slot = part.parse().ok()?;
        }
        let screen = match parts.get(6..8) {
            Some([width, height]) => Some((width.parse().ok()?, height.parse().ok()?)),
            _ => None,
        };
        let work_area_written = match parts.get(8) {
            Some(flag) => flag.parse::<i32>().ok()? != 0,
            None => true,
        };
        Some(Self {
            work_area,
            taskbar_state: parts[4].parse().ok()?,
            taskbar_hidden: parts[5].parse::<i32>().ok()? != 0,
            screen,
            work_area_written,
        })
    }

    fn to_line(self) -> String {
        let [left, top, right, bottom] = self.work_area;
        let mut line = format!(
            "{left},{top},{right},{bottom},{},{}",
            self.taskbar_state,
            i32::from(self.taskbar_hidden)
        );
        if let Some((width, height)) = self.screen {
            use std::fmt::Write;
            let _ = write!(
                line,
                ",{width},{height},{}",
                i32::from(self.work_area_written)
            );
        }
        line.push('\n');
        line
    }
}

fn rebase_work_area(
    original: WorkArea,
    old_screen: ScreenSize,
    new_screen: ScreenSize,
) -> WorkArea {
    let [left, top, right, bottom] = original;
    [
        left,
        top,
        (right + new_screen.0 - old_screen.0).max(left + 1),
        (bottom + new_screen.1 - old_screen.1).max(top + 1),
    ]
}

fn is_plausible_work_area(area: WorkArea, screen: ScreenSize) -> bool {
    let [left, top, right, bottom] = area;
    left >= 0 && top >= 0 && right > left && bottom > top && right <= screen.0 && bottom <= screen.1
}

fn recovered_original(recovery: ShellRecovery, screen: ScreenSize) -> Option<(WorkArea, usize)> {
    let area = match recovery.screen {
        Some(old) if old != screen => rebase_work_area(recovery.work_area, old, screen),
        _ => recovery.work_area,
    };
    is_plausible_work_area(area, screen).then_some((area, recovery.taskbar_state))
}

fn choose_original(
    recovery: Option<ShellRecovery>,
    live_work_area: WorkArea,
    live_state: usize,
    screen: ScreenSize,
) -> (WorkArea, usize) {
    recovery
        .and_then(|recovery| recovered_original(recovery, screen))
        .unwrap_or((live_work_area, live_state))
}

const fn rect_to_area(rect: RECT) -> WorkArea {
    [rect.left, rect.top, rect.right, rect.bottom]
}

const fn area_to_rect(area: WorkArea) -> RECT {
    RECT {
        left: area[0],
        top: area[1],
        right: area[2],
        bottom: area[3],
    }
}

#[derive(Debug)]
pub struct AppBar {
    hwnd: HWND,
    original_state: usize,
    original_work_area: RECT,
    screen: ScreenSize,
    taskbar_hidden: bool,
    reservation: Reservation,
    work_area_written: bool,
}

impl AppBar {
    #[must_use]
    pub fn register(hwnd: isize) -> Self {
        let hwnd = HWND(hwnd as *mut _);
        let mut data = Self::data(hwnd);
        let live_state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) };
        let live_work_area = rect_to_area(Self::get_work_area());
        let primary = primary_screen();
        let screen = (primary.width, primary.height);
        let (original_area, original_state) =
            choose_original(read_recovery(), live_work_area, live_state, screen);
        let mut registration = Self::data(hwnd);
        registration.uCallbackMessage = APPBAR_CALLBACK_MESSAGE;
        let reservation = if unsafe { SHAppBarMessage(ABM_NEW, &mut registration) } != 0 {
            Reservation::Shell
        } else {
            Reservation::WorkArea
        };
        let appbar = Self {
            hwnd,
            original_state,
            original_work_area: area_to_rect(original_area),
            screen,
            taskbar_hidden: false,
            reservation,
            work_area_written: false,
        };
        write_recovery(appbar.recovery());
        appbar
    }

    #[must_use]
    pub const fn is_shell_owned(&self) -> bool {
        matches!(self.reservation, Reservation::Shell)
    }

    pub fn reserve_top(&mut self, screen_width: i32, screen_height: i32, bar_height: i32) {
        match self.reservation {
            Reservation::Shell => self.reserve_shell_edge(screen_width, bar_height),
            Reservation::WorkArea => {
                self.write_work_area(screen_width, screen_height, bar_height);
            }
        }
    }

    fn reserve_shell_edge(&self, screen_width: i32, bar_height: i32) {
        let mut data = Self::data(self.hwnd);
        data.uEdge = ABE_TOP;
        data.rc = RECT {
            left: 0,
            top: 0,
            right: screen_width,
            bottom: bar_height,
        };
        unsafe {
            SHAppBarMessage(ABM_QUERYPOS, &mut data);
            data.rc.bottom = data.rc.top + bar_height;
            SHAppBarMessage(ABM_SETPOS, &mut data);
        }
    }

    fn write_work_area(&mut self, screen_width: i32, screen_height: i32, bar_height: i32) {
        let desired = RECT {
            left: self.original_work_area.left,
            top: self.original_work_area.top.max(bar_height),
            right: self.original_work_area.right.min(screen_width),
            bottom: self.original_work_area.bottom.min(screen_height),
        };
        if Self::get_work_area() == desired {
            return;
        }
        if !self.work_area_written {
            self.work_area_written = true;
            write_recovery(self.recovery());
        }
        Self::set_work_area(desired);
    }

    pub fn rebase(&mut self, screen_width: i32, screen_height: i32) {
        let next = (screen_width, screen_height);
        if next == self.screen {
            return;
        }
        let area = rebase_work_area(rect_to_area(self.original_work_area), self.screen, next);
        self.original_work_area = area_to_rect(area);
        self.screen = next;
        write_recovery(self.recovery());
    }

    pub fn hide_system_taskbar(&mut self) {
        self.set_taskbar_state(self.original_state | ABS_AUTOHIDE as usize);
        Self::set_tray_visibility(SW_HIDE);
        self.taskbar_hidden = true;
        write_recovery(self.recovery());
    }

    pub fn maintain(&mut self, screen_width: i32, screen_height: i32, bar_height: i32) {
        if self.taskbar_hidden {
            Self::set_tray_visibility(SW_HIDE);
        }
        if self.reservation == Reservation::WorkArea {
            self.reserve_top(screen_width, screen_height, bar_height);
        }
    }

    fn recovery(&self) -> ShellRecovery {
        ShellRecovery {
            work_area: rect_to_area(self.original_work_area),
            taskbar_state: self.original_state,
            taskbar_hidden: self.taskbar_hidden,
            screen: Some(self.screen),
            work_area_written: self.work_area_written,
        }
    }

    fn get_work_area() -> RECT {
        let mut rect = RECT::default();
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some((&raw mut rect).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            );
        }
        rect
    }

    fn set_work_area(mut rect: RECT) {
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_SETWORKAREA,
                0,
                Some((&raw mut rect).cast()),
                SPIF_SENDCHANGE,
            );
        }
    }

    fn set_taskbar_state(&self, state: usize) {
        let mut data = Self::data(self.hwnd);
        data.lParam = LPARAM(state as isize);
        unsafe {
            SHAppBarMessage(ABM_SETSTATE, &mut data);
        }
    }

    fn set_tray_visibility(cmd: SHOW_WINDOW_CMD) {
        unsafe {
            if let Ok(tray) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null())
                && !tray.0.is_null()
            {
                let _ = ShowWindow(tray, cmd);
            }
        }
    }

    fn data(hwnd: HWND) -> APPBARDATA {
        APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..APPBARDATA::default()
        }
    }
}

fn recovery_file_path() -> Option<std::path::PathBuf> {
    crate::file::data_dir().map(|dir| dir.join("shell_recovery.json"))
}

fn read_recovery() -> Option<ShellRecovery> {
    let content = std::fs::read_to_string(recovery_file_path()?).ok()?;
    ShellRecovery::parse(&content)
}

fn write_recovery(recovery: ShellRecovery) {
    let Some(path) = recovery_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = replace_file_atomically(&path, recovery.to_line().as_bytes());
}

fn clear_recovery_file() {
    if let Some(path) = recovery_file_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[must_use]
pub fn restore_shell_from_recovery() -> bool {
    let Some(path) = recovery_file_path() else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let _ = std::fs::remove_file(&path);
    let Some(recovery) = ShellRecovery::parse(&content) else {
        return false;
    };
    let primary = primary_screen();
    if recovery.work_area_written
        && let Some((area, _)) = recovered_original(recovery, (primary.width, primary.height))
    {
        AppBar::set_work_area(area_to_rect(area));
    }
    if recovery.taskbar_hidden {
        AppBar::set_tray_visibility(SW_SHOW);
        let mut data = APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: HWND::default(),
            lParam: LPARAM(recovery.taskbar_state as isize),
            ..APPBARDATA::default()
        };
        unsafe {
            SHAppBarMessage(ABM_SETSTATE, &mut data);
        }
    }
    true
}

impl Drop for AppBar {
    fn drop(&mut self) {
        if self.reservation == Reservation::Shell {
            let mut data = Self::data(self.hwnd);
            unsafe {
                SHAppBarMessage(ABM_REMOVE, &mut data);
            }
        }
        if self.work_area_written {
            Self::set_work_area(self.original_work_area);
        }
        if self.taskbar_hidden {
            Self::set_tray_visibility(SW_SHOW);
            self.set_taskbar_state(self.original_state);
        }
        clear_recovery_file();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ShellNotification, ShellRecovery, choose_original, classify_shell_notification,
        is_plausible_work_area, rebase_work_area, recovered_original,
    };

    const FHD: (i32, i32) = (1920, 1080);
    const QHD: (i32, i32) = (2560, 1440);

    fn recovery(screen: Option<(i32, i32)>) -> ShellRecovery {
        ShellRecovery {
            work_area: [0, 0, 1920, 1032],
            taskbar_state: 2,
            taskbar_hidden: true,
            screen,
            work_area_written: true,
        }
    }

    #[test]
    fn shell_notifications_are_classified_by_their_documented_codes() {
        assert_eq!(
            classify_shell_notification(1, 0),
            Some(ShellNotification::PositionChanged)
        );
        assert_eq!(
            classify_shell_notification(2, 1),
            Some(ShellNotification::FullScreenApp(true))
        );
        assert_eq!(
            classify_shell_notification(2, 0),
            Some(ShellNotification::FullScreenApp(false))
        );
        assert_eq!(classify_shell_notification(0, 0), None);
        assert_eq!(classify_shell_notification(3, 1), None);
    }

    #[test]
    fn planned_but_unapplied_work_area_change_is_not_marked_for_recovery() {
        let planned = ShellRecovery {
            work_area_written: false,
            taskbar_hidden: false,
            ..recovery(Some(FHD))
        };
        let parsed = ShellRecovery::parse(&planned.to_line()).unwrap();
        assert!(!parsed.work_area_written);
        assert!(!parsed.taskbar_hidden);
        assert!(
            ShellRecovery::parse("0,0,1920,1032,2,1,1920,1080")
                .unwrap()
                .work_area_written
        );
        assert!(ShellRecovery::parse("0,0,1920,1032,2,1,1920,1080,x").is_none());
    }

    #[test]
    fn parse_reads_legacy_six_field_line() {
        let parsed = ShellRecovery::parse("0,0,1920,1032,2,1\n").unwrap();
        assert_eq!(parsed, recovery(None));
    }

    #[test]
    fn parse_reads_screen_fields_when_present() {
        let parsed = ShellRecovery::parse("0,0,1920,1032,2,1,1920,1080").unwrap();
        assert_eq!(parsed, recovery(Some(FHD)));
    }

    #[test]
    fn parse_rejects_short_or_non_numeric_lines() {
        assert!(ShellRecovery::parse("0,0,1920,1032,2").is_none());
        assert!(ShellRecovery::parse("0,0,abc,1032,2,1").is_none());
        assert!(ShellRecovery::parse("0,0,1920,1032,2,1,1920,x").is_none());
        assert!(ShellRecovery::parse("").is_none());
    }

    #[test]
    fn line_round_trips() {
        let original = recovery(Some(FHD));
        assert_eq!(ShellRecovery::parse(&original.to_line()), Some(original));
        let legacy = recovery(None);
        assert_eq!(ShellRecovery::parse(&legacy.to_line()), Some(legacy));
    }

    #[test]
    fn rebase_follows_screen_growth_and_never_collapses() {
        assert_eq!(
            rebase_work_area([0, 0, 1920, 1032], FHD, QHD),
            [0, 0, 2560, 1392]
        );
        assert_eq!(rebase_work_area([0, 0, 10, 10], QHD, (1, 1)), [0, 0, 1, 1]);
    }

    #[test]
    fn plausibility_rejects_empty_negative_and_oversized_areas() {
        assert!(is_plausible_work_area([0, 0, 1920, 1032], FHD));
        assert!(!is_plausible_work_area([0, 0, 0, 1032], FHD));
        assert!(!is_plausible_work_area([-5, 0, 1920, 1032], FHD));
        assert!(!is_plausible_work_area([0, 0, 2560, 1032], FHD));
    }

    #[test]
    fn recovered_original_rebases_when_screen_changed_since_crash() {
        let (area, state) = recovered_original(recovery(Some(FHD)), QHD).unwrap();
        assert_eq!(area, [0, 0, 2560, 1392]);
        assert_eq!(state, 2);
    }

    #[test]
    fn recovered_original_rejects_implausible_legacy_area() {
        assert!(recovered_original(recovery(None), (1280, 720)).is_none());
    }

    #[test]
    fn choose_original_prefers_valid_recovery_over_live_state() {
        let live_area = [0, 40, 1920, 1080];
        let (area, state) = choose_original(Some(recovery(Some(FHD))), live_area, 3, FHD);
        assert_eq!(area, [0, 0, 1920, 1032]);
        assert_eq!(state, 2);
    }

    #[test]
    fn choose_original_falls_back_to_live_state() {
        let live_area = [0, 0, 1920, 1032];
        assert_eq!(choose_original(None, live_area, 0, FHD), (live_area, 0));
        let stale = recovery(None);
        assert_eq!(
            choose_original(Some(stale), live_area, 0, (1280, 720)),
            (live_area, 0)
        );
    }
}
