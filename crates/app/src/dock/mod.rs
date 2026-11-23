use std::collections::HashMap;
use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::{
    DOCK_ANIM_TIMER, DOCK_POLL_TIMER, WM_APP_DOCK_BACKDROP, WM_APP_DOCK_KEY, WM_DROPFILES,
    WM_GETOBJECT, WM_LBUTTONUP, WM_MOUSELEAVE,
};
use otto_core::{
    BarKey, ChangeGate, DockEntry, DockFocus, DockInput, DockItemKind, DockLayout, DockMetrics,
    DockPort, DockPreferences, IconBitmap, MotionMetrics, MotionPreference, Rect, ResolvedTheme,
    StableOrder, TickCadence, build_dock_accessible_tree, compute_dock_layout, scale,
};
use otto_platform::{
    DockReservation, PrimaryScreen, WM_APP_UIA_INVOKE, WindowsDockPort, current_process_id,
    enable_drag_accept_files, handle_wm_getobject, motion_preference, primary_screen,
    resume_recycle_bin_task, shutdown_recycle_bin_task,
};
use otto_render::DockRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, MA_NOACTIVATE, SW_HIDE, SetTimer, ShowWindow, WM_CAPTURECHANGED, WM_CLOSE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP,
    WM_SETTINGCHANGE, WM_TIMER,
};
use windows::core::Result;

mod actions;
mod entries;
mod keyboard;
mod pointer;
mod presentation;
mod timing;
mod visibility;

pub use self::actions::activate_overflow_entry;
pub use self::actions::apply_preferences;
use self::actions::count_entries;
use self::actions::entry_options;
pub use self::actions::execute_dock_menu_command;
pub use self::actions::set_capture_visible;
pub use self::actions::set_glass_enabled;
use self::actions::sync_icons;

const DOCK_POLL_MS: u32 = 150;
const DOCK_ENTRIES_FALLBACK_TICKS: u32 = 14;
const DOCK_BACKDROP_FALLBACK_TICKS: u32 = 7;
const DOCK_IDLE_MS: u32 = 1000;
const DOCK_REVEAL_BAND: i32 = 6;
const DOCK_HIDDEN_PROGRESS: f32 = 0.99;
const DOCK_IDLE_AFTER_TICKS: u32 = 14;

struct AnimState {
    mag_strength: f32,
    hide_progress: f32,
    should_hide: bool,
    bounce_key: Option<String>,
    bounce_phase: f32,
    timer_running: bool,
    last_tick: Instant,
}

impl Default for AnimState {
    fn default() -> Self {
        Self {
            mag_strength: 0.0,
            hide_progress: 0.0,
            should_hide: false,
            bounce_key: None,
            bounce_phase: 0.0,
            timer_running: false,
            last_tick: Instant::now(),
        }
    }
}

#[derive(Default)]
struct DragState {
    start_idx: Option<usize>,
    hover_idx: Option<usize>,
    start_pos: (i32, i32),
    current_pos: (i32, i32),
    is_dragging: bool,
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Dock {
    hwnd: isize,
    own_pid: u32,
    renderer: DockRenderer,
    screen: PrimaryScreen,
    metrics: DockMetrics,
    order: StableOrder,
    entries: Vec<DockEntry>,
    closing_entries: Vec<DockEntry>,
    presence: HashMap<String, f32>,
    layout: DockLayout,
    hovering: bool,
    last_cursor_x: Option<f32>,
    tracking_mouse: bool,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    anim: AnimState,
    drag: DragState,
    preferences: DockPreferences,
    fullscreen_covered: bool,
    shell_full_screen: bool,
    window_overlaps: bool,
    reservation: Option<DockReservation>,
    fullscreen_gate: ChangeGate,
    entries_gate: ChangeGate,
    cadence: TickCadence,
    focus: DockFocus,
    backdrop: Option<BackdropFeed>,
    backdrop_gate: ChangeGate,
    port: WindowsDockPort,
    notification_counts: HashMap<String, u32>,
    downloads_progress: Option<f32>,
    badge_poll_tick: u32,
    last_focus_badge_suppressed: bool,
}

pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let preferences = runtime.dock_preferences.get();
    if preferences.show_trash {
        resume_recycle_bin_task();
    } else {
        shutdown_recycle_bin_task();
    }
    enable_drag_accept_files(hwnd);
    let own_pid = current_process_id();
    let screen = primary_screen();
    let metrics = preferences.metrics();
    let motion = motion_preference();
    let motion_metrics = MotionMetrics::default();
    let mut order = StableOrder::default();
    let entries = WindowsDockPort.collect_entries(own_pid, &mut order, entry_options(preferences));
    let mut presence = HashMap::new();
    for entry in &entries {
        presence.insert(entry.key.clone(), 1.0);
    }
    let (app_count, right_count, has_trash) = count_entries(&entries);
    let layout = compute_dock_layout(
        &metrics,
        DockInput {
            screen_width: screen.width,
            dpi: screen.dpi,
            app_count,
            right_count,
            has_trash,
            cursor_x: None,
            mag_strength: 0.0,
            bounce_offsets: &[],
            item_presence: &[],
            hide_progress: 0.0,
        },
    );
    let mut renderer = DockRenderer::new(layout.window_width, layout.window_height, screen.dpi)?;
    sync_icons(&mut renderer, &entries);

    let mut dock = Dock {
        hwnd,
        own_pid,
        renderer,
        screen,
        metrics,
        order,
        entries,
        closing_entries: Vec::new(),
        presence,
        layout,
        hovering: false,
        last_cursor_x: None,
        tracking_mouse: false,
        motion,
        motion_metrics,
        anim: AnimState::default(),
        drag: DragState::default(),
        preferences,
        fullscreen_covered: false,
        shell_full_screen: false,
        window_overlaps: false,
        reservation: None,
        fullscreen_gate: ChangeGate::new(DOCK_BACKDROP_FALLBACK_TICKS),
        entries_gate: ChangeGate::new(DOCK_ENTRIES_FALLBACK_TICKS),
        cadence: TickCadence::new(DOCK_POLL_MS, DOCK_IDLE_MS, DOCK_IDLE_AFTER_TICKS),
        focus: DockFocus::default(),
        backdrop: runtime.glass_enabled.get().then(|| {
            BackdropFeed::start(
                hwnd,
                WM_APP_DOCK_BACKDROP,
                vec![hwnd],
                runtime.capture_visible.get(),
            )
        }),
        backdrop_gate: ChangeGate::new(DOCK_BACKDROP_FALLBACK_TICKS),
        port: WindowsDockPort,
        notification_counts: WindowsDockPort.notification_counts(),
        downloads_progress: preferences
            .show_downloads
            .then(|| WindowsDockPort.downloads_progress())
            .flatten(),
        badge_poll_tick: 0,
        last_focus_badge_suppressed: false,
    };
    dock.refresh_cover();
    let _ = dock.sync_reservation();
    dock.sync_backdrop(runtime, true);
    dock.place();
    dock.present(runtime);

    unsafe {
        SetTimer(
            Some(HWND(hwnd as *mut _)),
            DOCK_POLL_TIMER,
            DOCK_POLL_MS,
            None,
        );
    }

    let mut slot = runtime
        .dock
        .try_borrow_mut()
        .map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    *slot = Some(dock);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    shutdown_recycle_bin_task();
    if let Ok(mut guard) = runtime.dock.try_borrow_mut()
        && let Some(mut dock) = guard.take()
    {
        dock.stop_animating();
        otto_platform::watch_bottom_edge(None, dock.screen.width, dock.screen.height);
        otto_platform::stop_routing_if_targets(dock.hwnd);
        unsafe {
            let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_POLL_TIMER);
            let _ = ShowWindow(HWND(dock.hwnd as *mut _), SW_HIDE);
        }
    }
}

fn wake_for_message(runtime: &RuntimeContext, message: u32) {
    if message == WM_MOUSEMOVE {
        wake_timer_only(runtime);
    } else if matches!(
        message,
        WM_LBUTTONDOWN
            | WM_RBUTTONUP
            | WM_APP_DOCK_KEY
            | WM_APP_UIA_INVOKE
            | otto_platform::DOCK_APPBAR_CALLBACK_MESSAGE
    ) {
        wake(runtime);
    }
}

fn handle_accessible_object(
    runtime: &RuntimeContext,
    hwnd: isize,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    runtime
        .dock
        .try_borrow()
        .ok()
        .and_then(|guard| {
            let dock = guard.as_ref()?;
            handle_wm_getobject(hwnd, wparam.0, lparam.0, WM_APP_UIA_INVOKE, || {
                let entries = dock.combined_entries();
                let labels: Vec<&str> = entries.iter().map(|entry| entry.title.as_str()).collect();
                build_dock_accessible_tree(&dock.layout, dock.window_origin(), &labels)
            })
        })
        .map(LRESULT)
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    wake_for_message(runtime, message);
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_CLOSE => Some(LRESULT(0)),
        WM_GETOBJECT => handle_accessible_object(runtime, hwnd, wparam, lparam),
        WM_APP_UIA_INVOKE => {
            with_dock(runtime, |dock| dock.activate_index(runtime, wparam.0));
            Some(LRESULT(0))
        }
        WM_APP_DOCK_KEY => {
            if let Some(key) = BarKey::from_code(wparam.0) {
                with_dock(runtime, |dock| dock.on_dock_key(runtime, key));
            }
            Some(LRESULT(0))
        }
        WM_APP_DOCK_BACKDROP => {
            with_dock(runtime, |dock| dock.on_backdrop(runtime));
            Some(LRESULT(0))
        }
        otto_platform::DOCK_APPBAR_CALLBACK_MESSAGE => {
            if let Some(notification) =
                otto_platform::classify_shell_notification(wparam.0, lparam.0)
            {
                with_dock(runtime, |dock| {
                    dock.on_shell_notification(runtime, notification);
                });
            }
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == DOCK_POLL_TIMER => {
            with_dock(runtime, |dock| dock.on_poll_timer(runtime));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == DOCK_ANIM_TIMER => {
            with_dock(runtime, |dock| dock.on_anim_timer(runtime));
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| dock.on_mouse_move(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_dock(runtime, Dock::on_mouse_leave);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| dock.on_left_down(x, y));
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| dock.on_left_up(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| dock.on_right_click(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_dock(runtime, |dock| {
                let was_dragging = dock.drag.is_dragging;
                dock.drag = DragState::default();
                if was_dragging {
                    dock.recompute_layout();
                    dock.present(runtime);
                }
            });
            Some(LRESULT(0))
        }
        WM_DROPFILES => {
            let hdrop = wparam.0;
            with_dock(runtime, |dock| dock.on_drop_files(runtime, hdrop));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_dock(runtime, |dock| dock.rebuild(runtime));
            Some(LRESULT(0))
        }
        WM_SETTINGCHANGE => {
            with_dock(runtime, Dock::refresh_motion);
            None
        }
        _ => None,
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

pub fn keyboard_entry_count(runtime: &RuntimeContext) -> usize {
    runtime
        .dock
        .try_borrow()
        .ok()
        .and_then(|guard| guard.as_ref().map(|dock| dock.combined_entries().len()))
        .unwrap_or(0)
}

pub fn enter_keyboard_focus(runtime: &RuntimeContext, from_end: bool) {
    with_dock(runtime, |dock| dock.enter_keyboard_focus(runtime, from_end));
}

pub fn wake(runtime: &RuntimeContext) {
    with_dock(runtime, |dock| dock.wake(runtime));
}

pub fn wake_timer_only(runtime: &RuntimeContext) {
    with_dock(runtime, Dock::wake_timer_only);
}

fn with_dock(runtime: &RuntimeContext, action: impl FnOnce(&mut Dock)) {
    if let Ok(mut guard) = runtime.dock.try_borrow_mut()
        && let Some(dock) = guard.as_mut()
    {
        action(dock);
    }
}

pub fn repaint(runtime: &RuntimeContext) {
    with_dock(runtime, |dock| {
        dock.sync_backdrop(runtime, true);
        dock.present(runtime);
    });
}

pub fn on_taskbar_created(runtime: &RuntimeContext) {
    with_dock(runtime, |dock| dock.on_taskbar_created(runtime));
}

fn item_screen_rect(dock: &Dock, index: usize) -> Option<Rect> {
    let item = dock.layout.items.get(index)?;
    let (origin_x, origin_y) = dock.window_origin();
    let size = item.size.round().max(1.0) as i32;
    Some(Rect {
        x: origin_x + item.x.round() as i32,
        y: (origin_y + item.y.round() as i32).min(dock.screen.height - 4),
        width: size,
        height: size,
    })
}

fn dock_target_in(dock: &Dock, hwnd: isize) -> Option<Rect> {
    let (visible, hidden) = dock.displayed_entries();
    if let Some(index) = visible.iter().position(|entry| entry.hwnds.contains(&hwnd)) {
        return item_screen_rect(dock, index);
    }
    if hidden.iter().any(|entry| entry.hwnds.contains(&hwnd))
        && let Some(overflow_index) = visible
            .iter()
            .position(|entry| matches!(entry.kind, DockItemKind::Overflow { .. }))
    {
        return item_screen_rect(dock, overflow_index);
    }
    None
}

#[must_use]
pub fn dock_target_for_window(runtime: &RuntimeContext, hwnd: isize) -> Option<Rect> {
    let guard = runtime.dock.try_borrow().ok()?;
    let dock = guard.as_ref()?;
    dock_target_in(dock, hwnd)
}

#[must_use]
pub fn fallback_target_for_window(runtime: &RuntimeContext, hwnd: isize) -> Rect {
    if let Ok(guard) = runtime.dock.try_borrow()
        && let Some(dock) = guard.as_ref()
    {
        if let Some(target) = dock_target_in(dock, hwnd) {
            return target;
        }
        let size = scale(dock.metrics.base_icon_size, dock.screen.dpi).max(24);
        let (origin_x, origin_y) = dock.window_origin();
        let x = origin_x + dock.layout.pill.x + (dock.layout.pill.width - size) / 2;
        let y = (origin_y + dock.layout.pill.y + (dock.layout.pill.height - size) / 2)
            .min(dock.screen.height - 4);
        return Rect {
            x,
            y,
            width: size,
            height: size,
        };
    }
    let screen = primary_screen();
    let size = scale(48, screen.dpi).max(24);
    Rect {
        x: (screen.width - size) / 2,
        y: screen.height - size - 4,
        width: size,
        height: size,
    }
}

#[must_use]
pub fn fallback_card_context(runtime: &RuntimeContext, hwnd: isize) -> (Option<IconBitmap>, bool) {
    let dark_theme = crate::theme_state::appearance(runtime).theme == ResolvedTheme::Dark;
    let icon = runtime
        .dock
        .try_borrow()
        .ok()
        .and_then(|guard| {
            let dock = guard.as_ref()?;
            let entry = dock
                .entries
                .iter()
                .find(|entry| entry.hwnds.contains(&hwnd))?;
            WindowsDockPort.icon_for_entry(entry)
        })
        .or_else(|| WindowsDockPort.icon_for_window(hwnd));
    (icon, dark_theme)
}

fn entry_notification_count(counts: &HashMap<String, u32>, entry: &DockEntry) -> Option<u32> {
    if counts.is_empty() {
        return None;
    }
    if let Some(app) = entry.app.as_ref() {
        let name_key = app.name.trim().to_ascii_lowercase();
        if let Some(&count) = counts.get(&name_key) {
            return Some(count);
        }
        let exe_key = app.executable.trim().to_ascii_lowercase();
        if let Some(&count) = counts.get(&exe_key) {
            return Some(count);
        }
        if let Some(stem) = std::path::Path::new(&app.executable)
            .file_stem()
            .and_then(|s| s.to_str())
            && let Some(&count) = counts.get(&stem.to_ascii_lowercase())
        {
            return Some(count);
        }
    }
    let title_key = entry.title.trim().to_ascii_lowercase();
    if let Some(&count) = counts.get(&title_key) {
        return Some(count);
    }
    if let DockItemKind::App { exe_path } = &entry.kind
        && let Some(stem) = std::path::Path::new(exe_path)
            .file_stem()
            .and_then(|s| s.to_str())
        && let Some(&count) = counts.get(&stem.to_ascii_lowercase())
    {
        return Some(count);
    }
    None
}

#[cfg(test)]
mod tests;
