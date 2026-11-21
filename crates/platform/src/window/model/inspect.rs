use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;

use otto_core::{
    AppIdentity, WindowIdentity, WindowInfo, WindowPresence, WindowRole, WindowTraits,
};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_NOCLOSE, EnumChildWindows, EnumWindows, GCL_STYLE, GW_OWNER, GWL_EXSTYLE, GWL_STYLE,
    GetClassLongPtrW, GetMenuState, GetSystemMenu, GetWindow, GetWindowLongW, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindowVisible, IsZoomed,
    MF_BYCOMMAND, MF_DISABLED, MF_GRAYED, SC_CLOSE, WS_CAPTION, WS_DISABLED, WS_EX_APPWINDOW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_THICKFRAME,
};
use windows::core::BOOL;

use crate::dock::{friendly_app_name, is_explorer_path};
use crate::window::caption::{effective_caption_style, has_dwm_caption_buttons};
use crate::window::hidden::{self, Concealment};
use crate::window::ops::{
    is_own_settings_utf16, is_shell_or_desktop_utf16, utf16_eq_ascii, visible_window_rect,
    window_class_is, window_class_utf16,
};

use super::process::DWM_CLOAKED_SHELL;
use super::process::FRAME_HOST_CLASS;
use super::process::FULL_SCREEN_SHORTCUT_APPS;
use super::process::HOSTED_CORE_CLASS;
use super::process::ProcessCache;
use super::process::ProcessFacts;
use super::process::cached_process_facts;

pub(crate) fn owner_ids(hwnd: HWND) -> (u32, u32) {
    let mut process_id = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process_id)) };
    (process_id, thread_id)
}

pub(super) fn hosted_process_id(frame: HWND, frame_process_id: u32) -> Option<u32> {
    struct Search {
        frame_process_id: u32,
        hosted: Option<u32>,
        fallback: Option<u32>,
    }

    unsafe extern "system" fn visit(child: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let (process_id, _) = owner_ids(child);
        if process_id != 0 && process_id != search.frame_process_id {
            if window_class_is(child, HOSTED_CORE_CLASS) {
                search.hosted = Some(process_id);
                return BOOL(0);
            }
            if search.fallback.is_none() {
                search.fallback = Some(process_id);
            }
        }
        BOOL(1)
    }

    let mut search = Search {
        frame_process_id,
        hosted: None,
        fallback: None,
    };
    unsafe {
        let _ = EnumChildWindows(Some(frame), Some(visit), LPARAM(&raw mut search as isize));
    }
    search.hosted.or(search.fallback)
}

pub(super) fn window_title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    Some(String::from_utf16_lossy(&buffer[..copied.max(0) as usize]))
}

pub(super) fn is_shell_cloaked(hwnd: HWND) -> bool {
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

pub(super) fn monitor_rects(hwnd: HWND) -> Option<(RECT, RECT)> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    unsafe { GetMonitorInfoW(monitor, &raw mut info) }
        .as_bool()
        .then_some((info.rcMonitor, info.rcWork))
}

pub(crate) fn covers_monitor(hwnd: HWND) -> bool {
    let Some((monitor, _)) = monitor_rects(hwnd) else {
        return false;
    };
    let frame = visible_window_rect(hwnd);
    frame.left <= monitor.left
        && frame.top <= monitor.top
        && frame.right >= monitor.right
        && frame.bottom >= monitor.bottom
}

pub(super) const fn window_role(owner_visible: bool, app_window: bool) -> WindowRole {
    if owner_visible && !app_window {
        WindowRole::OwnedDialog
    } else {
        WindowRole::Primary
    }
}

pub(super) const fn is_full_screen(covers_monitor: bool, zoomed: bool, style: u32) -> bool {
    let captioned = style & WS_CAPTION.0 == WS_CAPTION.0;
    covers_monitor && !(zoomed && captioned)
}

pub(super) fn close_is_enabled(hwnd: HWND) -> bool {
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

pub(super) fn uses_full_screen_shortcut(executable: &str) -> bool {
    Path::new(executable)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| {
            FULL_SCREEN_SHORTCUT_APPS
                .iter()
                .any(|known| stem.eq_ignore_ascii_case(known))
        })
}

pub(super) fn read_traits(
    hwnd: HWND,
    style: u32,
    facts: &ProcessFacts,
    full_screen: bool,
) -> WindowTraits {
    let effective = effective_caption_style(style, has_dwm_caption_buttons(hwnd), full_screen);
    WindowTraits {
        closable: close_is_enabled(hwnd),
        minimizable: effective.can_minimize,
        maximizable: effective.can_maximize,
        resizable: style & WS_THICKFRAME.0 != 0,
        blocked_by_modal: style & WS_DISABLED.0 != 0,
        elevated: facts.elevated,
        responding: !unsafe { IsHungAppWindow(hwnd) }.as_bool(),
        full_screen_shortcut: uses_full_screen_shortcut(&facts.executable),
        title_bar: effective.has_caption && effective.has_system_menu,
    }
}

pub(super) fn is_recorded_hidden(recorded: &[WindowIdentity], candidate: &WindowIdentity) -> bool {
    recorded.iter().any(|known| known.is_same_window(candidate))
}

pub(super) struct Inspection<'a> {
    pub(super) own_process_id: u32,
    pub(super) allow_own_settings: bool,
    pub(super) foreground: isize,
    pub(super) hidden: &'a [WindowIdentity],
    pub(super) cache: &'a mut ProcessCache,
}

pub(super) fn inspect(hwnd: HWND, context: &mut Inspection<'_>) -> Option<WindowInfo> {
    let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
    if !visible && context.hidden.is_empty() {
        return None;
    }
    let (process_id, thread_id) = owner_ids(hwnd);
    if process_id == 0 {
        return None;
    }
    let mut class_buf = [0u16; 128];
    let class_len = window_class_utf16(hwnd, &mut class_buf);
    let class = &class_buf[..class_len];
    let own_settings = process_id == context.own_process_id
        && context.allow_own_settings
        && is_own_settings_utf16(class);
    if process_id == context.own_process_id && !own_settings {
        return None;
    }
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    let app_window = ex_style & WS_EX_APPWINDOW.0 != 0;
    if ex_style & WS_EX_NOACTIVATE.0 != 0 || (ex_style & WS_EX_TOOLWINDOW.0 != 0 && !app_window) {
        return None;
    }
    if is_shell_or_desktop_utf16(class) {
        return None;
    }
    let title = window_title(hwnd)?;
    let handle = hwnd.0 as isize;
    if is_shell_cloaked(hwnd) {
        return None;
    }
    let owner_facts = cached_process_facts(context.cache, process_id)?;
    let identity = WindowIdentity {
        hwnd: handle,
        process_id,
        thread_id,
        generation: owner_facts.generation,
    };
    if !visible && !is_recorded_hidden(context.hidden, &identity) {
        return None;
    }
    let app_facts = if utf16_eq_ascii(class, FRAME_HOST_CLASS) {
        hosted_process_id(hwnd, process_id)
            .and_then(|hosted| cached_process_facts(context.cache, hosted))
            .unwrap_or_else(|| owner_facts.clone())
    } else {
        owner_facts.clone()
    };
    if is_explorer_path(&app_facts.executable)
        && !utf16_eq_ascii(class, "CabinetWClass")
        && !utf16_eq_ascii(class, "ExploreWClass")
    {
        return None;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let owner_visible = unsafe { GetWindow(hwnd, GW_OWNER) }
        .is_ok_and(|owner| !owner.0.is_null() && unsafe { IsWindowVisible(owner) }.as_bool());
    let minimized = unsafe { IsIconic(hwnd) }.as_bool();
    let zoomed = unsafe { IsZoomed(hwnd) }.as_bool();
    let full_screen = !minimized && visible && is_full_screen(covers_monitor(hwnd), zoomed, style);
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
    let app_name = if own_settings {
        String::from("Otto Settings")
    } else {
        friendly_app_name(&app_facts.executable)
    };
    Some(WindowInfo {
        identity,
        app,
        app_name,
        traits: read_traits(hwnd, style, &app_facts, full_screen),
        executable: app_facts.executable,
        title,
        role: window_role(owner_visible, app_window),
        presence,
        zoomed,
        full_screen,
        active: context.foreground == handle,
    })
}

pub(super) fn foreground_root() -> isize {
    let foreground = crate::os_events::current_foreground();
    if foreground.0.is_null() {
        return 0;
    }
    crate::window::ops::visible_root_owner(foreground).0 as isize
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

    let hidden = hidden::recorded(Concealment::Hidden);
    let mut cache = ProcessCache::new();
    let mut state = State {
        context: Inspection {
            own_process_id,
            allow_own_settings: false,
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
            hidden::forget(known.hwnd, Concealment::Hidden);
        }
    }
    windows
}

#[must_use]
pub fn inspect_window(hwnd: isize, own_process_id: u32) -> Option<WindowInfo> {
    let hidden = hidden::recorded(Concealment::Hidden);
    let mut cache = ProcessCache::new();
    let mut context = Inspection {
        own_process_id,
        allow_own_settings: true,
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
    let mut class_buf = [0u16; 128];
    let class_len = window_class_utf16(HWND(root as *mut c_void), &mut class_buf);
    let class = &class_buf[..class_len];
    if utf16_eq_ascii(class, "Progman") || utf16_eq_ascii(class, "WorkerW") {
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
