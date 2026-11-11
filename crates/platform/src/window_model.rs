use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;
use std::sync::OnceLock;

use otto_core::{
    AppIdentity, AppInfo, Capability, CommandOutcome, Rect, WindowCommand, WindowIdentity,
    WindowInfo, WindowPresence, WindowRole, WindowTraits,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, FILETIME, HANDLE, HWND, LPARAM, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_F11;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_NOCLOSE, EnumChildWindows, EnumWindows, GCL_STYLE, GW_OWNER, GWL_EXSTYLE, GWL_STYLE,
    GetClassLongPtrW, GetMenuState, GetSystemMenu, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsHungAppWindow, IsIconic,
    IsWindow, IsWindowVisible, IsZoomed, MF_BYCOMMAND, MF_DISABLED, MF_GRAYED, PostMessageW,
    SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, SMTO_ABORTIFHUNG, SW_HIDE, SW_SHOWNA,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos,
    ShowWindow, WM_CLOSE, WM_SYSCOMMAND, WS_CAPTION, WS_DISABLED, WS_EX_APPWINDOW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
};
use windows::core::{BOOL, PWSTR};

use crate::dock::{
    focus_window_hwnd, friendly_app_name, is_explorer_path, is_shell_or_desktop_class,
    visible_window_rect, window_class_name,
};
use crate::hidden_windows::{self, Concealment};

const DWM_CLOAKED_SHELL: u32 = 2;
const FRAME_HOST_CLASS: &str = "ApplicationFrameWindow";
const HOSTED_CORE_CLASS: &str = "Windows.UI.Core.CoreWindow";
const RESTORE_TIMEOUT_MS: u32 = 80;
const FULL_SCREEN_SHORTCUT_APPS: &[&str] = &[
    "chrome",
    "msedge",
    "firefox",
    "brave",
    "opera",
    "vivaldi",
    "explorer",
    "windowsterminal",
    "code",
];

#[derive(Debug, Clone)]
struct ProcessFacts {
    generation: u64,
    executable: String,
    user_model_id: Option<String>,
    elevated: bool,
}

struct ProcessHandle(HANDLE);

impl ProcessHandle {
    fn open(process_id: u32) -> Option<Self> {
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
            .ok()
            .map(Self)
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn filetime_value(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

fn process_generation(process: &ProcessHandle) -> Option<u64> {
    let mut created = FILETIME::default();
    let mut ignored = [FILETIME::default(); 3];
    unsafe {
        GetProcessTimes(
            process.0,
            &raw mut created,
            &raw mut ignored[0],
            &raw mut ignored[1],
            &raw mut ignored[2],
        )
    }
    .ok()?;
    Some(filetime_value(created))
}

fn process_executable(process: &ProcessHandle) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut length,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

fn process_user_model_id(process: &ProcessHandle) -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut length = buffer.len() as u32;
    let status = unsafe {
        GetApplicationUserModelId(process.0, &raw mut length, Some(PWSTR(buffer.as_mut_ptr())))
    };
    if status.0 != 0 || length <= 1 {
        return None;
    }
    let id = String::from_utf16_lossy(&buffer[..length as usize - 1]);
    (!id.trim().is_empty()).then_some(id)
}

fn token_is_elevated(process: HANDLE) -> Option<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }.ok()?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&raw mut elevation).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &raw mut returned,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    read.ok()?;
    Some(elevation.TokenIsElevated != 0)
}

fn own_process_is_elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(|| token_is_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false))
}

fn read_process_facts(process_id: u32) -> Option<ProcessFacts> {
    let process = ProcessHandle::open(process_id)?;
    let target_elevated = token_is_elevated(process.0).unwrap_or(true);
    Some(ProcessFacts {
        generation: process_generation(&process)?,
        executable: process_executable(&process)?,
        user_model_id: process_user_model_id(&process),
        elevated: target_elevated && !own_process_is_elevated(),
    })
}

type ProcessCache = HashMap<u32, Option<ProcessFacts>>;

fn cached_process_facts(cache: &mut ProcessCache, process_id: u32) -> Option<ProcessFacts> {
    cache
        .entry(process_id)
        .or_insert_with(|| read_process_facts(process_id))
        .clone()
}

fn owner_ids(hwnd: HWND) -> (u32, u32) {
    let mut process_id = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process_id)) };
    (process_id, thread_id)
}

fn hosted_process_id(frame: HWND, frame_process_id: u32) -> Option<u32> {
    struct Search {
        frame_process_id: u32,
        hosted: Option<u32>,
    }

    unsafe extern "system" fn visit(child: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let (process_id, _) = owner_ids(child);
        if process_id != 0
            && process_id != search.frame_process_id
            && window_class_name(child) == HOSTED_CORE_CLASS
        {
            search.hosted = Some(process_id);
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut search = Search {
        frame_process_id,
        hosted: None,
    };
    unsafe {
        let _ = EnumChildWindows(Some(frame), Some(visit), LPARAM(&raw mut search as isize));
    }
    search.hosted
}

fn window_title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    Some(String::from_utf16_lossy(&buffer[..copied.max(0) as usize]))
}

fn is_shell_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    unsafe {
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
    }
    cloaked & DWM_CLOAKED_SHELL != 0
}

fn monitor_rects(hwnd: HWND) -> Option<(RECT, RECT)> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    unsafe { GetMonitorInfoW(monitor, &raw mut info) }
        .as_bool()
        .then_some((info.rcMonitor, info.rcWork))
}

fn covers_monitor(hwnd: HWND) -> bool {
    let Some((monitor, _)) = monitor_rects(hwnd) else {
        return false;
    };
    let frame = visible_window_rect(hwnd);
    frame.left <= monitor.left
        && frame.top <= monitor.top
        && frame.right >= monitor.right
        && frame.bottom >= monitor.bottom
}

const fn window_role(owner_visible: bool, app_window: bool) -> WindowRole {
    if owner_visible && !app_window {
        WindowRole::OwnedDialog
    } else {
        WindowRole::Primary
    }
}

const fn is_full_screen(covers_monitor: bool, zoomed: bool, style: u32) -> bool {
    let captioned = style & WS_CAPTION.0 == WS_CAPTION.0;
    covers_monitor && !(zoomed && captioned)
}

fn close_is_enabled(hwnd: HWND) -> bool {
    let class_style = unsafe { GetClassLongPtrW(hwnd, GCL_STYLE) } as u32;
    if class_style & CS_NOCLOSE.0 != 0 {
        return false;
    }
    let menu = unsafe { GetSystemMenu(hwnd, false) };
    if menu.0.is_null() {
        return true;
    }
    let state = unsafe { GetMenuState(menu, SC_CLOSE, MF_BYCOMMAND) };
    state == u32::MAX || state & (MF_GRAYED.0 | MF_DISABLED.0) == 0
}

fn uses_full_screen_shortcut(executable: &str) -> bool {
    Path::new(executable)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| {
            FULL_SCREEN_SHORTCUT_APPS
                .iter()
                .any(|known| stem.eq_ignore_ascii_case(known))
        })
}

fn read_traits(hwnd: HWND, style: u32, facts: &ProcessFacts) -> WindowTraits {
    WindowTraits {
        closable: close_is_enabled(hwnd),
        minimizable: style & WS_MINIMIZEBOX.0 != 0,
        maximizable: style & WS_MAXIMIZEBOX.0 != 0,
        resizable: style & WS_THICKFRAME.0 != 0,
        blocked_by_modal: style & WS_DISABLED.0 != 0,
        elevated: facts.elevated,
        responding: !unsafe { IsHungAppWindow(hwnd) }.as_bool(),
        full_screen_shortcut: uses_full_screen_shortcut(&facts.executable),
        title_bar: style & WS_CAPTION.0 == WS_CAPTION.0,
    }
}

struct Inspection<'a> {
    own_process_id: u32,
    foreground: isize,
    hidden: &'a [WindowIdentity],
    cache: &'a mut ProcessCache,
}

fn inspect(hwnd: HWND, context: &mut Inspection<'_>) -> Option<WindowInfo> {
    let (process_id, thread_id) = owner_ids(hwnd);
    if process_id == 0 || process_id == context.own_process_id {
        return None;
    }
    let class = window_class_name(hwnd);
    if is_shell_or_desktop_class(&class) {
        return None;
    }
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    let app_window = ex_style & WS_EX_APPWINDOW.0 != 0;
    if ex_style & WS_EX_NOACTIVATE.0 != 0 || (ex_style & WS_EX_TOOLWINDOW.0 != 0 && !app_window) {
        return None;
    }
    let title = window_title(hwnd)?;
    let handle = hwnd.0 as isize;
    let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
    let recorded_hidden = context.hidden.iter().any(|known| known.hwnd == handle);
    if (!visible && !recorded_hidden) || is_shell_cloaked(hwnd) {
        return None;
    }
    let owner_facts = cached_process_facts(context.cache, process_id)?;
    let app_facts = if class == FRAME_HOST_CLASS {
        let hosted = hosted_process_id(hwnd, process_id)?;
        cached_process_facts(context.cache, hosted)?
    } else {
        owner_facts.clone()
    };
    if is_explorer_path(&app_facts.executable)
        && class != "CabinetWClass"
        && class != "ExploreWClass"
    {
        return None;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let owner_visible = unsafe { GetWindow(hwnd, GW_OWNER) }
        .is_ok_and(|owner| !owner.0.is_null() && unsafe { IsWindowVisible(owner) }.as_bool());
    let minimized = unsafe { IsIconic(hwnd) }.as_bool();
    let zoomed = unsafe { IsZoomed(hwnd) }.as_bool();
    let presence = if !visible {
        WindowPresence::Hidden
    } else if minimized {
        WindowPresence::Minimized
    } else {
        WindowPresence::Visible
    };
    let app = app_facts.user_model_id.as_deref().map_or_else(
        || AppIdentity::executable(&app_facts.executable),
        AppIdentity::user_model_id,
    );
    Some(WindowInfo {
        identity: WindowIdentity {
            hwnd: handle,
            process_id,
            thread_id,
            generation: owner_facts.generation,
        },
        app,
        app_name: friendly_app_name(&app_facts.executable),
        traits: read_traits(hwnd, style, &app_facts),
        executable: app_facts.executable,
        title,
        role: window_role(owner_visible, app_window),
        presence,
        zoomed,
        full_screen: !minimized && visible && is_full_screen(covers_monitor(hwnd), zoomed, style),
        active: context.foreground == handle,
    })
}

fn foreground_root() -> isize {
    let foreground = crate::os_events::current_foreground();
    if foreground.0.is_null() {
        return 0;
    }
    crate::dock::visible_root_owner(foreground).0 as isize
}

#[must_use]
pub fn enumerate_windows(own_process_id: u32) -> Vec<WindowInfo> {
    struct State<'a> {
        context: Inspection<'a>,
        windows: Vec<WindowInfo>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut State<'_>) };
        if let Some(window) = inspect(hwnd, &mut state.context) {
            state.windows.push(window);
        }
        BOOL(1)
    }

    let hidden = hidden_windows::recorded(Concealment::Hidden);
    let mut cache = ProcessCache::new();
    let mut state = State {
        context: Inspection {
            own_process_id,
            foreground: foreground_root(),
            hidden: &hidden,
            cache: &mut cache,
        },
        windows: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut state as isize));
    }
    let windows = state.windows;
    for known in &hidden {
        if !windows
            .iter()
            .any(|window| window.identity.is_same_window(known))
        {
            hidden_windows::forget(known.hwnd, Concealment::Hidden);
        }
    }
    windows
}

#[must_use]
pub fn inspect_window(hwnd: isize, own_process_id: u32) -> Option<WindowInfo> {
    let hidden = hidden_windows::recorded(Concealment::Hidden);
    let mut cache = ProcessCache::new();
    let mut context = Inspection {
        own_process_id,
        foreground: foreground_root(),
        hidden: &hidden,
        cache: &mut cache,
    };
    inspect(HWND(hwnd as *mut c_void), &mut context)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundApplication {
    pub name: String,
    pub window: Option<WindowInfo>,
}

#[must_use]
pub fn foreground_application(own_process_id: u32) -> Option<ForegroundApplication> {
    let root = foreground_root();
    if root == 0 {
        return None;
    }
    let class = window_class_name(HWND(root as *mut c_void));
    if matches!(class.as_str(), "Progman" | "WorkerW") {
        return Some(ForegroundApplication {
            name: String::from("Finder"),
            window: None,
        });
    }
    let window = inspect_window(root, own_process_id)?;
    Some(ForegroundApplication {
        name: window.app_name.clone(),
        window: Some(window),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationContext {
    pub app: AppInfo,
    pub other_apps_visible: bool,
}

#[must_use]
pub fn application_of(target: WindowIdentity, own_process_id: u32) -> Option<ApplicationContext> {
    let windows = enumerate_windows(own_process_id);
    let identity = windows
        .iter()
        .find(|window| window.identity.is_same_window(&target))?
        .app
        .clone();
    let apps = otto_core::group_windows(windows);
    let other_apps_visible = apps.iter().any(|app| {
        app.identity != identity
            && app
                .primary_windows()
                .any(|window| window.presence == WindowPresence::Visible)
    });
    let app = apps.into_iter().find(|app| app.identity == identity)?;
    Some(ApplicationContext {
        app,
        other_apps_visible,
    })
}

fn live_window(target: WindowIdentity, own_process_id: u32) -> Option<WindowInfo> {
    let hwnd = HWND(target.hwnd as *mut c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return None;
    }
    inspect_window(target.hwnd, own_process_id)
        .filter(|window| window.identity.is_same_window(&target))
}

fn post_outcome(posted: windows::core::Result<()>) -> CommandOutcome {
    match posted {
        Ok(()) => CommandOutcome::Completed,
        Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => CommandOutcome::Denied,
        Err(_) => CommandOutcome::TargetGone,
    }
}

fn post_system_command(hwnd: HWND, command: u32) -> CommandOutcome {
    post_outcome(unsafe {
        PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM(command as usize),
            LPARAM(0),
        )
    })
}

fn unhide(window: &WindowInfo) {
    if window.presence == WindowPresence::Hidden {
        unsafe {
            let _ = ShowWindow(
                HWND(window.identity.hwnd as *mut std::ffi::c_void),
                SW_SHOWNA,
            );
        }
        hidden_windows::forget(window.identity.hwnd, Concealment::Hidden);
    }
}

fn tile(hwnd: HWND, left_half: bool) -> CommandOutcome {
    let Some((_, work)) = monitor_rects(hwnd) else {
        return CommandOutcome::TargetGone;
    };
    if unsafe { IsZoomed(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
        unsafe {
            let _ = SendMessageTimeoutW(
                hwnd,
                WM_SYSCOMMAND,
                WPARAM(SC_RESTORE as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                RESTORE_TIMEOUT_MS,
                None,
            );
        }
    }
    let mut outer = RECT::default();
    if unsafe { GetWindowRect(hwnd, &raw mut outer) }.is_err() {
        return CommandOutcome::TargetGone;
    }
    let frame = visible_window_rect(hwnd);
    let half = tile_rect(work, left_half);
    let target = Rect {
        x: half.x - (frame.left - outer.left),
        y: half.y - (frame.top - outer.top),
        width: half.width + (frame.left - outer.left) + (outer.right - frame.right),
        height: half.height + (frame.top - outer.top) + (outer.bottom - frame.bottom),
    };
    post_outcome(unsafe {
        SetWindowPos(
            hwnd,
            None,
            target.x,
            target.y,
            target.width,
            target.height,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        )
    })
}

const fn tile_rect(work: RECT, left_half: bool) -> Rect {
    let width = work.right - work.left;
    let half = width / 2;
    Rect {
        x: if left_half {
            work.left
        } else {
            work.left + half
        },
        y: work.top,
        width: if left_half { half } else { width - half },
        height: work.bottom - work.top,
    }
}

fn toggle_full_screen(hwnd: HWND) -> CommandOutcome {
    focus_window_hwnd(hwnd.0 as isize);
    if foreground_root() != hwnd.0 as isize {
        return CommandOutcome::TimedOut;
    }
    crate::actions::send_key_chord(&[VK_F11]);
    CommandOutcome::Completed
}

#[must_use]
pub fn window_command_capability(
    target: WindowIdentity,
    command: WindowCommand,
    own_process_id: u32,
) -> Option<Capability> {
    live_window(target, own_process_id).map(|window| window.capability(command))
}

#[must_use]
pub fn execute_window_command(
    target: WindowIdentity,
    command: WindowCommand,
    own_process_id: u32,
) -> CommandOutcome {
    let Some(window) = live_window(target, own_process_id) else {
        return CommandOutcome::TargetGone;
    };
    if let Some(blocked) = CommandOutcome::from_capability(window.capability(command)) {
        return blocked;
    }
    let hwnd = HWND(target.hwnd as *mut c_void);
    match command {
        WindowCommand::Focus => {
            unhide(&window);
            focus_window_hwnd(target.hwnd);
            CommandOutcome::Completed
        }
        WindowCommand::Close => {
            post_outcome(unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) })
        }
        WindowCommand::Minimize => post_system_command(hwnd, SC_MINIMIZE),
        WindowCommand::Zoom => post_system_command(
            hwnd,
            if window.zoomed {
                SC_RESTORE
            } else {
                SC_MAXIMIZE
            },
        ),
        WindowCommand::ToggleFullScreen => toggle_full_screen(hwnd),
        WindowCommand::TileLeft => tile(hwnd, true),
        WindowCommand::TileRight => tile(hwnd, false),
    }
}

fn summarize(completed: usize, denied: usize) -> CommandOutcome {
    if completed > 0 {
        CommandOutcome::Completed
    } else if denied > 0 {
        CommandOutcome::Denied
    } else {
        CommandOutcome::TargetGone
    }
}

fn hide_window(window: &WindowInfo) -> CommandOutcome {
    if window.traits.elevated {
        return CommandOutcome::Denied;
    }
    let hwnd = HWND(window.identity.hwnd as *mut std::ffi::c_void);
    if !hidden_windows::record(window.identity, Concealment::Hidden) {
        return CommandOutcome::TemporarilyUnavailable;
    }
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        hidden_windows::forget(window.identity.hwnd, Concealment::Hidden);
        return CommandOutcome::Denied;
    }
    CommandOutcome::Completed
}

fn hide_windows<'a>(
    windows: impl Iterator<Item = &'a WindowInfo>,
    own_process_id: u32,
) -> CommandOutcome {
    let (mut completed, mut denied) = (0, 0);
    for stale in windows.filter(|window| window.presence != WindowPresence::Hidden) {
        let Some(live) = live_window(stale.identity, own_process_id) else {
            continue;
        };
        match hide_window(&live) {
            CommandOutcome::Completed => completed += 1,
            CommandOutcome::Denied => denied += 1,
            _ => {}
        }
    }
    summarize(completed, denied)
}

#[must_use]
pub fn hide_application(app: &AppInfo, own_process_id: u32) -> CommandOutcome {
    hide_windows(app.windows.iter(), own_process_id)
}

#[must_use]
pub fn hide_other_applications(keep: &AppIdentity, own_process_id: u32) -> CommandOutcome {
    let windows = enumerate_windows(own_process_id);
    hide_windows(
        windows.iter().filter(|window| &window.app != keep),
        own_process_id,
    )
}

#[must_use]
pub fn show_application(app: &AppInfo, own_process_id: u32) -> CommandOutcome {
    let mut shown = 0;
    let mut first = None;
    for stale in &app.windows {
        if let Some(live) = live_window(stale.identity, own_process_id) {
            unhide(&live);
            first.get_or_insert(live.identity.hwnd);
            shown += 1;
        }
    }
    if let Some(hwnd) = first {
        focus_window_hwnd(hwnd);
    }
    summarize(shown, 0)
}

#[must_use]
pub fn quit_application(app: &AppInfo, own_process_id: u32) -> CommandOutcome {
    let (mut completed, mut denied) = (0, 0);
    for stale in app.primary_windows() {
        let Some(live) = live_window(stale.identity, own_process_id) else {
            continue;
        };
        if !live.capability(WindowCommand::Close).is_available() {
            denied += usize::from(live.traits.elevated);
            continue;
        }
        unhide(&live);
        let hwnd = HWND(live.identity.hwnd as *mut c_void);
        match post_outcome(unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }) {
            CommandOutcome::Completed => completed += 1,
            CommandOutcome::Denied => denied += 1,
            _ => {}
        }
    }
    summarize(completed, denied)
}

fn identity_is_live(identity: WindowIdentity) -> bool {
    let hwnd = HWND(identity.hwnd as *mut c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return false;
    }
    let (process_id, thread_id) = owner_ids(hwnd);
    process_id == identity.process_id
        && thread_id == identity.thread_id
        && ProcessHandle::open(process_id)
            .and_then(|process| process_generation(&process))
            .is_some_and(|generation| generation == identity.generation)
}

pub(crate) fn current_identity(hwnd: isize) -> Option<WindowIdentity> {
    let handle = HWND(hwnd as *mut c_void);
    if !unsafe { IsWindow(Some(handle)) }.as_bool() {
        return None;
    }
    let (process_id, thread_id) = owner_ids(handle);
    let generation =
        ProcessHandle::open(process_id).and_then(|process| process_generation(&process))?;
    Some(WindowIdentity {
        hwnd,
        process_id,
        thread_id,
        generation,
    })
}

#[must_use]
pub fn window_identity(hwnd: isize) -> Option<WindowIdentity> {
    current_identity(hwnd)
}

#[must_use]
pub fn restore_hidden_windows() -> usize {
    let mut restored = 0;
    for entry in hidden_windows::take_all() {
        let identity = entry.identity;
        if !identity_is_live(identity) {
            continue;
        }
        let hwnd = HWND(identity.hwnd as *mut c_void);
        match entry.kind {
            Concealment::Hidden => {
                if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOWNA);
                    }
                    restored += 1;
                }
            }
            Concealment::TransitionsDisabled => {
                if crate::dock::set_dwm_transition_policy(hwnd, true) {
                    restored += 1;
                }
            }
        }
    }
    restored
}

#[cfg(test)]
mod fixture_tests {
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use otto_core::{
        AppCommand, AppInfo, Capability, CommandOutcome, WindowCommand, WindowInfo, WindowPresence,
        group_windows,
    };

    use super::{
        enumerate_windows, execute_window_command, hide_application, quit_application,
        restore_hidden_windows, show_application,
    };
    use crate::hidden_windows::{self, Concealment};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const STEP_TIMEOUT: Duration = Duration::from_secs(15);

    struct Fixture {
        child: Child,
        marker: String,
        data_dir: std::path::PathBuf,
    }

    impl Fixture {
        fn launch() -> Self {
            let marker = format!("OttoFixture{}", std::process::id());
            let data_dir = std::env::temp_dir().join(format!("{marker}-data"));
            std::fs::create_dir_all(&data_dir).expect("fixture data directory");
            unsafe {
                std::env::set_var(crate::file::DATA_DIR_OVERRIDE, &data_dir);
            }
            let script = format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 $script:open = 2; \
                 $closed = {{ $script:open--; if ($script:open -le 0) {{ [Windows.Forms.Application]::Exit() }} }}; \
                 foreach ($name in 'A','B') {{ $form = New-Object Windows.Forms.Form; \
                 $form.Text = '{marker}-' + $name; $form.Width = 420; $form.Height = 260; \
                 $form.add_FormClosed($closed); $form.Show() }}; \
                 [Windows.Forms.Application]::Run()"
            );
            let child = Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .expect("fixture process");
            Self {
                child,
                marker,
                data_dir,
            }
        }

        fn windows(&self) -> Vec<WindowInfo> {
            enumerate_windows(std::process::id())
                .into_iter()
                .filter(|window| window.title.starts_with(&self.marker))
                .collect()
        }

        fn app(&self) -> Option<AppInfo> {
            group_windows(self.windows()).into_iter().next()
        }

        fn window(&self, name: &str) -> Option<WindowInfo> {
            let title = format!("{}-{name}", self.marker);
            self.windows()
                .into_iter()
                .find(|window| window.title == title)
        }

        fn wait_for(&self, description: &str, mut condition: impl FnMut(&Self) -> bool) {
            let deadline = Instant::now() + STEP_TIMEOUT;
            while Instant::now() < deadline {
                if condition(self) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            panic!("fixture did not reach: {description}");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = restore_hidden_windows();
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn window_and_application_commands_stay_distinct_on_a_real_application() {
        let own = std::process::id();
        let mut fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let first = fixture.window("A").expect("window A");
        let second = fixture.window("B").expect("window B");
        assert_eq!(first.app, second.app);
        assert_eq!(first.capability(WindowCommand::Zoom), Capability::Available);

        assert_eq!(
            execute_window_command(first.identity, WindowCommand::ToggleFullScreen, own),
            CommandOutcome::Unsupported
        );
        assert_eq!(
            execute_window_command(first.identity, WindowCommand::Zoom, own),
            CommandOutcome::Completed
        );
        fixture.wait_for("window A zoomed", |f| {
            f.window("A").is_some_and(|window| window.zoomed)
        });
        assert!(!fixture.window("A").expect("window A").full_screen);

        assert_eq!(
            execute_window_command(second.identity, WindowCommand::Minimize, own),
            CommandOutcome::Completed
        );
        fixture.wait_for("window B minimized", |f| {
            f.window("B")
                .is_some_and(|window| window.presence == WindowPresence::Minimized)
        });
        let app = fixture.app().expect("fixture application");
        assert!(!app.is_hidden());
        assert_eq!(
            app.capability(AppCommand::Hide, true),
            Capability::Available
        );

        assert_eq!(hide_application(&app, own), CommandOutcome::Completed);
        fixture.wait_for("application hidden", |f| {
            f.app().is_some_and(|app| app.is_hidden())
        });
        assert_eq!(hidden_windows::recorded(Concealment::Hidden).len(), 2);
        assert!(fixture.data_dir.join("hidden_windows.txt").exists());

        let hidden = fixture.app().expect("hidden application");
        assert_eq!(show_application(&hidden, own), CommandOutcome::Completed);
        fixture.wait_for("application shown again", |f| {
            f.app().is_some_and(|app| !app.is_hidden())
        });
        assert!(hidden_windows::recorded(Concealment::Hidden).is_empty());

        assert_eq!(
            execute_window_command(second.identity, WindowCommand::Close, own),
            CommandOutcome::Completed
        );
        fixture.wait_for("only window A left", |f| f.windows().len() == 1);
        assert!(fixture.child.try_wait().expect("fixture status").is_none());
        assert_eq!(
            execute_window_command(second.identity, WindowCommand::Close, own),
            CommandOutcome::TargetGone
        );

        let remaining = fixture.app().expect("remaining application");
        assert_eq!(quit_application(&remaining, own), CommandOutcome::Completed);
        let deadline = Instant::now() + STEP_TIMEOUT;
        while fixture.child.try_wait().expect("fixture status").is_none() {
            assert!(
                Instant::now() < deadline,
                "quit did not end the application"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(fixture.windows().is_empty());
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn suppressed_transitions_are_recorded_first_and_recovered_after_a_crash() {
        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let window = fixture.window("A").expect("window A");
        let hwnd = window.identity.hwnd;
        assert!(crate::dock::set_window_transitions_enabled(hwnd, false));
        assert_eq!(
            hidden_windows::recorded(Concealment::TransitionsDisabled),
            vec![window.identity]
        );
        assert!(fixture.data_dir.join("hidden_windows.txt").exists());
        assert_eq!(restore_hidden_windows(), 1);
        assert!(hidden_windows::recorded(Concealment::TransitionsDisabled).is_empty());
        assert!(crate::dock::set_window_transitions_enabled(hwnd, false));
        assert!(crate::dock::set_window_transitions_enabled(hwnd, true));
        assert!(hidden_windows::recorded(Concealment::TransitionsDisabled).is_empty());
        assert!(!fixture.data_dir.join("hidden_windows.txt").exists());
    }

    #[test]
    #[ignore = "native fixture: minimizes and restores a real window"]
    fn quiet_minimize_and_restore_reach_the_native_state_and_release_overrides() {
        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let hwnd = fixture.window("B").expect("window B").identity.hwnd;
        assert!(crate::dock::minimize_window_native_quiet(hwnd));
        fixture.wait_for("window B minimized", |f| {
            f.window("B")
                .is_some_and(|window| window.presence == WindowPresence::Minimized)
        });
        assert!(crate::dock::set_window_transitions_enabled(hwnd, true));
        assert!(crate::dock::restore_window_native_quiet(hwnd));
        fixture.wait_for("window B restored", |f| {
            f.window("B")
                .is_some_and(|window| window.presence == WindowPresence::Visible)
        });
        assert!(crate::dock::set_window_transitions_enabled(hwnd, true));
        assert!(hidden_windows::recorded(Concealment::TransitionsDisabled).is_empty());
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn windows_hidden_before_a_crash_are_shown_again_from_the_ledger() {
        let own = std::process::id();
        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let app = fixture.app().expect("fixture application");
        assert_eq!(hide_application(&app, own), CommandOutcome::Completed);
        fixture.wait_for("application hidden", |f| {
            f.app().is_some_and(|app| app.is_hidden())
        });
        assert_eq!(restore_hidden_windows(), 2);
        fixture.wait_for("windows visible after recovery", |f| {
            f.windows()
                .iter()
                .all(|window| window.presence == WindowPresence::Visible)
        });
        assert!(!fixture.data_dir.join("hidden_windows.txt").exists());
        assert_eq!(restore_hidden_windows(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        filetime_value, is_full_screen, summarize, tile_rect, uses_full_screen_shortcut,
        window_role,
    };
    use otto_core::{CommandOutcome, Rect};
    use windows::Win32::Foundation::{FILETIME, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{WS_CAPTION, WS_POPUP};

    #[test]
    fn windows_owned_by_a_hidden_framework_window_are_application_windows() {
        use otto_core::WindowRole;
        assert_eq!(window_role(false, false), WindowRole::Primary);
        assert_eq!(window_role(true, false), WindowRole::OwnedDialog);
        assert_eq!(window_role(true, true), WindowRole::Primary);
    }

    #[test]
    fn maximized_window_filling_the_monitor_is_not_full_screen() {
        assert!(!is_full_screen(true, true, WS_CAPTION.0));
        assert!(is_full_screen(true, false, WS_POPUP.0));
        assert!(is_full_screen(true, true, WS_POPUP.0));
        assert!(!is_full_screen(false, false, WS_POPUP.0));
    }

    #[test]
    fn generation_combines_both_halves_of_the_creation_time() {
        let time = FILETIME {
            dwLowDateTime: 0x89AB_CDEF,
            dwHighDateTime: 0x0123_4567,
        };
        assert_eq!(filetime_value(time), 0x0123_4567_89AB_CDEF);
    }

    #[test]
    fn tiling_splits_the_work_area_without_overlap_or_gap() {
        let work = RECT {
            left: -1921,
            top: 32,
            right: 0,
            bottom: 1080,
        };
        let left = tile_rect(work, true);
        let right = tile_rect(work, false);
        assert_eq!(
            left,
            Rect {
                x: -1921,
                y: 32,
                width: 960,
                height: 1048
            }
        );
        assert_eq!(left.right(), right.x);
        assert_eq!(right.right(), 0);
    }

    #[test]
    fn full_screen_shortcut_is_limited_to_known_applications() {
        assert!(uses_full_screen_shortcut("C:\\Apps\\Chrome.exe"));
        assert!(uses_full_screen_shortcut("C:\\Windows\\explorer.exe"));
        assert!(!uses_full_screen_shortcut("C:\\Windows\\notepad.exe"));
        assert!(!uses_full_screen_shortcut(""));
    }

    #[test]
    fn batch_outcome_prefers_success_then_denial_then_absence() {
        assert_eq!(summarize(1, 3), CommandOutcome::Completed);
        assert_eq!(summarize(0, 2), CommandOutcome::Denied);
        assert_eq!(summarize(0, 0), CommandOutcome::TargetGone);
    }
}
