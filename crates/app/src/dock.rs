use std::collections::HashMap;
use std::mem::size_of;
use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use otto_core::{
    BACKDROP_PADDING_PT, BarKey, CaptureRegion, ChangeGate, CommandOutcome, DockDropEffect,
    DockDropInput, DockEffect, DockFocus, DockFocusOutcome, DockInput, DockLayout, DockMetrics,
    DockPreferences, MotionMetrics, MotionPreference, Rect, ResolvedTheme, StableOrder,
    TickCadence, WindowCommand, WindowIdentity, advance_bounce_phase, animate_value, bounce_offset,
    build_dock_accessible_tree, capture_interval_for, capture_wanted, compute_dock_layout,
    dock_activation_effect, dock_capacity, dock_command_effect, dock_drop_effect, dock_entry_badge,
    dock_entry_progress, dock_rest_zone, dock_should_hide, scale, split_dock_overflow,
};
use otto_platform::release_capture_if_owned;
use otto_platform::{
    DockEntry, DockEntryOptions, DockItemKind, DockReservation, IconBitmap, PrimaryScreen,
    WM_APP_UIA_INVOKE, collect_dock_entries_with_options, current_process_id, cursor_position,
    empty_recycle_bin, enable_drag_accept_files, execute_window_command, extract_dock_icon,
    extract_dropped_files, extract_window_icon, handle_wm_getobject, hide_application,
    hide_other_applications, is_escape_down, is_running_on_battery, is_window_minimized,
    launch_new_instance, motion_preference, open_file_with_dock_entry, other_applications_visible,
    overflow_dock_entry, primary_screen, quit_application, read_downloads_active_progress,
    read_notification_app_counts, remove_pinned_app, reorder_pinned_apps, resume_recycle_bin_task,
    route_bar_keys, routes_keys_to, show_application, shutdown_recycle_bin_task,
    signal_command_failure, stop_routing_if_targets, toggle_pin_entry, watch_bottom_edge,
    window_change_generations,
};
use otto_render::{DockRenderItem, DockRenderer};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer,
    SetWindowPos, ShowWindow, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_GETOBJECT: u32 = 0x003D;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_DROPFILES: u32 = 0x0233;
const DOCK_POLL_TIMER: usize = 10;
const DOCK_ANIM_TIMER: usize = 11;
const DOCK_POLL_MS: u32 = 150;
const DOCK_ENTRIES_FALLBACK_TICKS: u32 = 14;
const DOCK_BACKDROP_FALLBACK_TICKS: u32 = 7;
const DOCK_IDLE_MS: u32 = 1000;
const DOCK_REVEAL_BAND: i32 = 6;
const WM_APP_DOCK_KEY: u32 = 0x8000 + 23;
const WM_APP_DOCK_BACKDROP: u32 = 0x8000 + 25;
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
    window_overlaps: bool,
    reservation: Option<DockReservation>,
    fullscreen_gate: ChangeGate,
    entries_gate: ChangeGate,
    cadence: TickCadence,
    focus: DockFocus,
    backdrop: Option<BackdropFeed>,
    backdrop_gate: ChangeGate,
    notification_counts: HashMap<String, u32>,
    downloads_progress: Option<f32>,
    badge_poll_tick: u32,
}

pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    resume_recycle_bin_task();
    enable_drag_accept_files(hwnd);
    let own_pid = current_process_id();
    let screen = primary_screen();
    let preferences = runtime.dock_preferences.get();
    let metrics = preferences.metrics();
    let motion = motion_preference();
    let motion_metrics = MotionMetrics::default();
    let mut order = StableOrder::default();
    let entries =
        collect_dock_entries_with_options(own_pid, &mut order, entry_options(preferences));
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
        notification_counts: read_notification_app_counts(),
        downloads_progress: read_downloads_active_progress(),
        badge_poll_tick: 0,
    };
    dock.refresh_cover();
    dock.sync_reservation();
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

    *runtime.dock.borrow_mut() = Some(dock);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    shutdown_recycle_bin_task();
    if let Ok(mut guard) = runtime.dock.try_borrow_mut()
        && let Some(dock) = guard.take()
    {
        unsafe {
            let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_POLL_TIMER);
            let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_ANIM_TIMER);
            let _ = ShowWindow(HWND(dock.hwnd as *mut _), SW_HIDE);
        }
    }
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if message == WM_MOUSEMOVE {
        wake_timer_only(runtime);
    } else if matches!(
        message,
        WM_LBUTTONDOWN | WM_RBUTTONUP | WM_APP_DOCK_KEY | WM_APP_UIA_INVOKE
    ) {
        wake(runtime);
    }
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_CLOSE => Some(LRESULT(0)),
        WM_GETOBJECT => runtime
            .dock
            .try_borrow()
            .ok()
            .and_then(|guard| {
                let dock = guard.as_ref()?;
                handle_wm_getobject(hwnd, wparam.0, lparam.0, WM_APP_UIA_INVOKE, || {
                    let entries = dock.combined_entries();
                    let labels: Vec<&str> =
                        entries.iter().map(|entry| entry.title.as_str()).collect();
                    build_dock_accessible_tree(&dock.layout, dock.window_origin(), &labels)
                })
            })
            .map(LRESULT),
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
            extract_dock_icon(entry)
        })
        .or_else(|| extract_window_icon(hwnd));
    (icon, dark_theme)
}

fn report(outcome: CommandOutcome) {
    if !matches!(
        outcome,
        CommandOutcome::Completed | CommandOutcome::Cancelled
    ) {
        signal_command_failure();
    }
}

fn show_window(runtime: &RuntimeContext, window: WindowIdentity, target: Option<Rect>) {
    if is_window_minimized(window.hwnd)
        && crate::genie::restore_window_to(runtime, window.hwnd, target)
    {
        return;
    }
    report(execute_window_command(
        window,
        WindowCommand::Focus,
        current_process_id(),
    ));
}

pub fn apply_preferences(runtime: &RuntimeContext, preferences: DockPreferences) {
    with_dock(runtime, |dock| {
        let entries_changed = dock.preferences.show_downloads != preferences.show_downloads
            || dock.preferences.show_trash != preferences.show_trash;
        dock.preferences = preferences;
        dock.metrics = preferences.metrics();
        dock.ensure_animating();
        if entries_changed {
            dock.refresh_entries(runtime);
        }
        dock.rebuild(runtime);
    });
}

pub fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_dock(runtime, |dock| {
        if enabled {
            if dock.backdrop.is_none() {
                dock.backdrop = Some(BackdropFeed::start(
                    dock.hwnd,
                    WM_APP_DOCK_BACKDROP,
                    vec![dock.hwnd],
                    runtime.capture_visible.get(),
                ));
            }
        } else {
            dock.backdrop = None;
            dock.renderer.clear_backdrop();
            dock.present(runtime);
        }
        dock.sync_backdrop(runtime, true);
    });
}

pub fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_dock(runtime, |dock| {
        if let Some(feed) = dock.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        dock.sync_backdrop(runtime, true);
        dock.present(runtime);
    });
}

pub fn activate_overflow_entry(runtime: &RuntimeContext, entry: &DockEntry) {
    activate_entry(runtime, entry, None);
}

fn activate_entry(runtime: &RuntimeContext, entry: &DockEntry, target: Option<Rect>) {
    let effect = dock_activation_effect(entry.app.as_ref());
    if !entry.active
        && matches!(effect, DockEffect::ShowWindow(window) if !is_window_minimized(window.hwnd))
        && let Some(app) = entry.app.as_ref()
    {
        report(show_application(app, current_process_id()));
        return;
    }
    execute_dock_effect(runtime, effect, entry, target);
}

pub fn execute_dock_menu_command(
    runtime: &RuntimeContext,
    cmd: usize,
    entry: &DockEntry,
    target: Option<Rect>,
) {
    if let Some(effect) = dock_command_effect(cmd, entry.app.as_ref()) {
        execute_dock_effect(runtime, effect, entry, target);
    }
}

fn execute_dock_effect(
    runtime: &RuntimeContext,
    effect: DockEffect,
    entry: &DockEntry,
    target: Option<Rect>,
) {
    let own_pid = current_process_id();
    match effect {
        DockEffect::Launch if matches!(entry.kind, DockItemKind::Apps) => {
            crate::spotlight::open_apps(runtime, target);
        }
        DockEffect::Launch => launch_new_instance(entry),
        DockEffect::ShowWindow(window) => show_window(runtime, window, target),
        DockEffect::TogglePin => {
            toggle_pin_entry(entry);
            with_dock(runtime, |dock| dock.refresh_entries(runtime));
        }
        DockEffect::EmptyTrash => empty_recycle_bin(),
        DockEffect::RevealInExplorer => entry.reveal_in_explorer(),
        DockEffect::ForceQuit => {
            report(entry.force_quit(own_pid));
            with_dock(runtime, |dock| dock.refresh_entries(runtime));
        }
        DockEffect::Unhide | DockEffect::Hide | DockEffect::HideOthers | DockEffect::Quit => {
            let Some(app) = entry.app.as_ref() else {
                return;
            };
            report(match effect {
                DockEffect::Unhide => show_application(app, own_pid),
                DockEffect::Hide => hide_application(app, own_pid),
                DockEffect::HideOthers => hide_other_applications(&app.identity, own_pid),
                _ => quit_application(app, own_pid),
            });
        }
        DockEffect::Fail(outcome) => report(outcome),
    }
}

fn count_entries(entries: &[DockEntry]) -> (usize, usize, bool) {
    let right_count = entries
        .iter()
        .filter(|e| matches!(e.kind, DockItemKind::Downloads | DockItemKind::Trash { .. }))
        .count();
    let has_trash = entries
        .last()
        .is_some_and(|e| matches!(e.kind, DockItemKind::Trash { .. }));
    let app_count = entries.len().saturating_sub(right_count);
    (app_count, right_count, has_trash)
}

fn sync_icons(renderer: &mut DockRenderer, entries: &[DockEntry]) {
    for entry in entries {
        if entry.apple_icon.is_none()
            && !renderer.has_icon(&entry.key)
            && let Some(bmp) = extract_dock_icon(entry)
        {
            let _ = renderer.set_icon(entry.key.clone(), bmp.width, bmp.height, &bmp.pixels);
        }
    }
    let alive: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    renderer.retain_icons(&alive);
}

const fn entry_options(preferences: DockPreferences) -> DockEntryOptions {
    DockEntryOptions {
        show_downloads: preferences.show_downloads,
        show_trash: preferences.show_trash,
    }
}

impl Dock {
    fn refresh_entries(&mut self, runtime: &RuntimeContext) {
        self.entries = collect_dock_entries_with_options(
            self.own_pid,
            &mut self.order,
            entry_options(self.preferences),
        );
        for entry in &self.entries {
            self.presence.entry(entry.key.clone()).or_insert(1.0);
        }
        let updated = self.combined_entries();
        sync_icons(&mut self.renderer, &updated);
        self.recompute_layout();
        self.present(runtime);
    }

    fn cancel_drag(&mut self) -> bool {
        if self.drag.start_idx.is_some() {
            let was_dragging = self.drag.is_dragging;
            self.drag = DragState::default();
            let _ = release_capture_if_owned(self.hwnd);
            if was_dragging {
                self.recompute_layout();
                return true;
            }
        }
        false
    }

    fn cancel_drag_on_escape(&mut self) -> bool {
        self.drag.start_idx.is_some() && is_escape_down() && self.cancel_drag()
    }

    fn track_hover_from_cursor(&mut self) {
        if !self.hovering {
            return;
        }
        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_y = gy - win_top;
        let still_inside = !self.anim.should_hide
            && gx >= self.layout.pill.x - 20
            && gx <= self.layout.pill.right() + 20
            && local_y >= self.layout.pill.y - 36;
        if still_inside {
            self.last_cursor_x = Some(gx as f32);
        } else {
            self.hovering = false;
        }
    }

    fn window_origin(&self) -> (i32, i32) {
        (0, self.screen.height - self.layout.window_height)
    }

    fn combined_entries(&self) -> Vec<DockEntry> {
        self.displayed_entries().0
    }

    fn base_displayed_entries(&self) -> (Vec<DockEntry>, Vec<DockEntry>) {
        let mut all = self.all_entries();
        let right_items = all
            .iter()
            .rev()
            .take_while(|entry| {
                matches!(
                    entry.kind,
                    DockItemKind::Downloads | DockItemKind::Trash { .. }
                )
            })
            .count();
        let app_count = all.len() - right_items;
        let capacity = dock_capacity(
            &self.metrics,
            self.screen.width,
            self.screen.dpi,
            right_items,
        );
        let split = split_dock_overflow(app_count, capacity);
        if split.hidden_apps == 0 {
            return (all, Vec::new());
        }
        let right = all.split_off(app_count);
        let hidden = all.split_off(split.visible_apps);
        all.push(overflow_dock_entry(hidden.len()));
        all.extend(right);
        (all, hidden)
    }

    fn displayed_entries(&self) -> (Vec<DockEntry>, Vec<DockEntry>) {
        let (mut visible, hidden) = self.base_displayed_entries();
        if self.drag.is_dragging
            && let (Some(from), Some(to)) = (self.drag.start_idx, self.drag.hover_idx)
        {
            let (app_count, _, _) = count_entries(&visible);
            if from < app_count && to < app_count && from != to {
                let order =
                    DockLayout::preview_reordered_indices(visible.len(), Some(from), Some(to));
                visible = order.into_iter().map(|i| visible[i].clone()).collect();
            }
        }
        (visible, hidden)
    }

    fn all_entries(&self) -> Vec<DockEntry> {
        if self.closing_entries.is_empty() {
            return self.entries.clone();
        }
        let mut list = Vec::with_capacity(self.entries.len() + self.closing_entries.len());
        let has_trash = self
            .entries
            .last()
            .is_some_and(|e| matches!(e.kind, DockItemKind::Trash { .. }));
        let app_slice = if has_trash {
            &self.entries[..self.entries.len() - 1]
        } else {
            &self.entries[..]
        };
        list.extend_from_slice(app_slice);
        for closing in &self.closing_entries {
            if !list.iter().any(|e| e.key == closing.key) {
                list.push(closing.clone());
            }
        }
        if has_trash && let Some(trash) = self.entries.last() {
            list.push(trash.clone());
        }
        list
    }

    fn place(&self) {
        let (ox, oy) = self.window_origin();
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                ox,
                oy,
                self.layout.window_width,
                self.layout.window_height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    fn refresh_cover(&mut self) {
        self.fullscreen_covered = DockEntry::is_fullscreen_on_primary(
            self.screen.width,
            self.screen.height,
            self.own_pid,
        );
        self.window_overlaps = self.preferences.autohide
            && dock_rest_zone(&self.layout, self.screen.height)
                .is_some_and(|zone| DockEntry::any_window_overlaps(zone, self.own_pid));
    }

    fn sync_reservation(&mut self) {
        if !(self.preferences.reserve_space && self.preferences.hide_taskbar) {
            self.reservation = None;
            return;
        }
        if self.reservation.is_none() {
            self.reservation = DockReservation::register(self.hwnd);
        }
        if let Some(reservation) = self.reservation.as_mut() {
            reservation.reserve(
                self.screen.width,
                self.screen.height,
                self.layout.rest_height,
            );
        }
    }

    fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.screen = primary_screen();
        self.recompute_layout();
        self.refresh_cover();
        self.sync_reservation();
        let all = self.combined_entries();
        if let Ok(mut renderer) = DockRenderer::new(
            self.layout.window_width,
            self.layout.window_height,
            self.screen.dpi,
        ) {
            sync_icons(&mut renderer, &all);
            self.renderer = renderer;
            if let Some(feed) = self.backdrop.as_mut() {
                feed.reset();
            }
        }
        self.sync_backdrop(runtime, true);
        self.place();
        self.present(runtime);
    }

    fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = capture_wanted(&appearance, self.backdrop.is_some());
        let hidden = self.anim.hide_progress >= DOCK_HIDDEN_PROGRESS;
        let (origin_x, origin_y) = self.window_origin();
        let window = Rect {
            x: origin_x,
            y: origin_y,
            width: self.layout.window_width,
            height: self.layout.window_height,
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let region =
            CaptureRegion::padded_to_screen(window, pad, self.screen.width, self.screen.height);
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        if !wanted {
            if feed.is_active() {
                feed.reset();
                self.renderer.clear_backdrop();
                self.present(runtime);
            }
            return;
        }
        if hidden {
            return;
        }
        if !force && self.anim.timer_running && self.renderer.has_backdrop() {
            return;
        }
        if let Some(region) = region {
            if !self.renderer.has_backdrop()
                && let Some(pixels) = otto_platform::capture_backdrop_now(
                    region.origin(),
                    region.width,
                    region.height,
                )
            {
                self.renderer
                    .set_backdrop(pixels, (region.x - origin_x, region.y - origin_y));
            }
            feed.request(
                region,
                capture_interval_for(
                    is_running_on_battery(),
                    self.hovering && feed.recently_changed(),
                ),
                force,
            );
        }
    }

    fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let (origin_x, origin_y) = self.window_origin();
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (update.origin.0 - origin_x, update.origin.1 - origin_y);
        self.renderer.set_backdrop(update.pixels, local);
        self.present(runtime);
    }

    fn refresh_entries_if_changed(&mut self, topology: Option<u64>) -> bool {
        if !self.entries_gate.should_refresh(topology) {
            return false;
        }
        let latest = collect_dock_entries_with_options(
            self.own_pid,
            &mut self.order,
            entry_options(self.preferences),
        );
        if latest == self.entries {
            return false;
        }
        let mut need_anim = false;
        for new_e in &latest {
            if !self.entries.iter().any(|old_e| old_e.key == new_e.key) {
                self.presence.insert(new_e.key.clone(), 0.08);
                need_anim = true;
            }
            if new_e.running
                && !matches!(new_e.kind, DockItemKind::Trash { .. })
                && !self
                    .entries
                    .iter()
                    .any(|old_e| old_e.key == new_e.key && old_e.running)
            {
                self.anim.bounce_key = Some(new_e.key.clone());
                self.anim.bounce_phase = 0.01;
                need_anim = true;
            }
        }

        for old_e in &self.entries {
            if !matches!(
                old_e.kind,
                DockItemKind::Finder
                    | DockItemKind::Apps
                    | DockItemKind::Downloads
                    | DockItemKind::Trash { .. }
            ) && !latest.iter().any(|new_e| new_e.key == old_e.key)
                && !self.closing_entries.iter().any(|c| c.key == old_e.key)
            {
                let mut ghost = old_e.clone();
                ghost.running = false;
                ghost.active = false;
                self.closing_entries.push(ghost);
                need_anim = true;
            }
        }
        if need_anim {
            self.ensure_animating();
        }

        self.closing_entries
            .retain(|c| !latest.iter().any(|n| n.key == c.key));
        self.entries = latest;
        let all = self.combined_entries();
        sync_icons(&mut self.renderer, &all);
        self.recompute_layout();
        true
    }

    fn refresh_motion(&mut self) {
        let next_motion = motion_preference();
        if next_motion != self.motion {
            self.motion = next_motion;
            self.ensure_animating();
        }
    }

    fn on_poll_timer(&mut self, runtime: &RuntimeContext) {
        let mut dirty = self.cancel_drag_on_escape();
        let generations = window_change_generations();
        if self.refresh_entries_if_changed(generations.map(|changes| changes.topology)) {
            dirty = true;
        }
        if self
            .fullscreen_gate
            .should_refresh(generations.map(|changes| changes.geometry))
        {
            self.refresh_cover();
        }
        self.sync_reservation();
        self.badge_poll_tick = self.badge_poll_tick.wrapping_add(1);
        if dirty
            || self
                .badge_poll_tick
                .is_multiple_of(DOCK_ENTRIES_FALLBACK_TICKS)
        {
            let next_counts = read_notification_app_counts();
            let next_progress = read_downloads_active_progress();
            if next_counts != self.notification_counts || next_progress != self.downloads_progress {
                self.notification_counts = next_counts;
                self.downloads_progress = next_progress;
                dirty = true;
            }
        }

        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;

        let should_auto_hide = dock_should_hide(
            self.preferences,
            self.fullscreen_covered,
            self.window_overlaps,
        );

        let at_bottom_edge = gy >= self.screen.height - DOCK_REVEAL_BAND;
        let in_interactive_zone = self.anim.hide_progress < 0.85
            && local_x >= self.layout.pill.x - 20
            && local_x <= self.layout.pill.right() + 20
            && local_y >= self.layout.pill.y - 36;

        let target_hide = should_auto_hide && !at_bottom_edge && !in_interactive_zone;
        if target_hide != self.anim.should_hide {
            self.anim.should_hide = target_hide;
            if self.anim.should_hide {
                self.hovering = false;
            }
            self.ensure_animating();
        }

        if self.hovering && !in_interactive_zone {
            self.hovering = false;
            self.ensure_animating();
        } else if !self.anim.should_hide && in_interactive_zone && self.anim.hide_progress < 0.25 {
            let new_cx = local_x as f32;
            if !self.hovering
                || self
                    .last_cursor_x
                    .is_none_or(|old| (old - new_cx).abs() > 0.5)
            {
                self.hovering = true;
                self.last_cursor_x = Some(new_cx);
                self.ensure_animating();
            }
        }

        if dirty && !self.anim.timer_running {
            self.present(runtime);
        }
        let hidden = self.anim.should_hide;
        watch_bottom_edge(
            hidden.then_some(DOCK_REVEAL_BAND),
            self.screen.width,
            self.screen.height,
        );
        let reveal_needs_polling = should_auto_hide && (!hidden || generations.is_none());
        self.sync_keyboard_focus(runtime);
        let geometry_changed = self
            .backdrop_gate
            .should_refresh(generations.map(|changes| changes.geometry));
        self.sync_backdrop(runtime, geometry_changed);
        let busy = dirty
            || self.focus.is_active()
            || reveal_needs_polling
            || self.hovering
            || self.anim.timer_running
            || self.drag.start_idx.is_some();
        if let Some(interval) = self.cadence.observe(busy) {
            self.set_poll_interval(interval);
        }
    }

    fn wake(&mut self, runtime: &RuntimeContext) {
        if let Some(interval) = self.cadence.wake() {
            self.set_poll_interval(interval);
            self.on_poll_timer(runtime);
        }
    }

    fn wake_timer_only(&mut self) {
        if let Some(interval) = self.cadence.wake() {
            self.set_poll_interval(interval);
        }
    }

    fn set_poll_interval(&self, interval: u32) {
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                DOCK_POLL_TIMER,
                interval,
                None,
            );
        }
    }

    fn on_anim_timer(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.anim.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.anim.last_tick = now;

        self.cancel_drag_on_escape();
        self.track_hover_from_cursor();

        let target_mag = if self.hovering && !self.anim.should_hide {
            1.0
        } else {
            0.0
        };
        let target_hide = if self.anim.should_hide { 1.0 } else { 0.0 };

        self.anim.mag_strength = animate_value(
            self.anim.mag_strength,
            target_mag,
            self.motion_metrics.dock_magnify_rate,
            dt,
            self.motion,
        );
        self.anim.hide_progress = animate_value(
            self.anim.hide_progress,
            target_hide,
            self.motion_metrics.dock_hide_rate,
            dt,
            self.motion,
        );

        if self.anim.bounce_phase > 0.0 {
            self.anim.bounce_phase = advance_bounce_phase(
                self.anim.bounce_phase,
                dt,
                self.motion_metrics.bounce_duration_seconds,
                self.motion,
            );
            if self.anim.bounce_phase >= 1.0 {
                self.anim.bounce_phase = 0.0;
                self.anim.bounce_key = None;
            }
        }

        let mut presence_animating = false;
        for entry in &self.entries {
            if let Some(cur) = self.presence.get_mut(&entry.key) {
                let next = animate_value(
                    *cur,
                    1.0,
                    self.motion_metrics.dock_presence_enter_rate,
                    dt,
                    self.motion,
                );
                if (next - 1.0).abs() > 0.002 {
                    *cur = next;
                    presence_animating = true;
                } else {
                    self.presence.remove(&entry.key);
                }
            }
        }
        if !self.closing_entries.is_empty() {
            let mut still_closing = Vec::new();
            for closing in self.closing_entries.drain(..) {
                let cur = self.presence.get(&closing.key).copied().unwrap_or(1.0);
                let next = animate_value(
                    cur,
                    0.0,
                    self.motion_metrics.dock_presence_exit_rate,
                    dt,
                    self.motion,
                );
                if next > 0.03 {
                    self.presence.insert(closing.key.clone(), next);
                    still_closing.push(closing);
                    presence_animating = true;
                } else {
                    self.presence.remove(&closing.key);
                }
            }
            self.closing_entries = still_closing;
        }

        self.recompute_layout();
        self.present(runtime);

        let mag_done = (self.anim.mag_strength - target_mag).abs() < 0.001;
        let hide_done = (self.anim.hide_progress - target_hide).abs() < 0.001;
        let bounce_done = self.anim.bounce_phase == 0.0;

        if mag_done && hide_done && bounce_done && !presence_animating {
            if !self.hovering {
                self.last_cursor_x = None;
            }
            let all = self.combined_entries();
            sync_icons(&mut self.renderer, &all);
            self.stop_animating();
        }
    }

    fn on_mouse_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if !self.tracking_mouse {
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: HWND(self.hwnd as *mut _),
                dwHoverTime: 0,
            };
            if unsafe { TrackMouseEvent(&mut track) }.is_ok() {
                self.tracking_mouse = true;
            }
        }

        if self.drag.start_idx.is_some() {
            self.drag.current_pos = (x, y);
            if !self.drag.is_dragging {
                let dx = x - self.drag.start_pos.0;
                let dy = y - self.drag.start_pos.1;
                if dx * dx + dy * dy > 36 {
                    self.drag.is_dragging = true;
                }
            }
            if self.drag.is_dragging {
                let next_hover = self.drag_preview_target(x, y);
                if next_hover != self.drag.hover_idx {
                    self.drag.hover_idx = next_hover;
                    self.recompute_layout();
                    self.present(runtime);
                }
            }
        }

        if self.anim.should_hide {
            return;
        }

        let near_pill = x >= self.layout.pill.x - 16
            && x <= self.layout.pill.right() + 16
            && y >= self.layout.pill.y - 36;

        if near_pill {
            self.hovering = true;
            self.last_cursor_x = Some(x as f32);
            self.ensure_animating();
        } else if self.hovering {
            self.hovering = false;
            self.ensure_animating();
        }
    }

    fn on_mouse_leave(&mut self) {
        self.tracking_mouse = false;
        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;
        let still_over_dock = !self.anim.should_hide
            && local_x >= self.layout.pill.x - 12
            && local_x <= self.layout.pill.right() + 12
            && local_y >= self.layout.pill.y - 28
            && local_y <= self.layout.pill.bottom();
        if !still_over_dock && self.hovering {
            self.hovering = false;
            self.ensure_animating();
        }
    }

    fn drag_preview_target(&self, x: i32, y: i32) -> Option<usize> {
        let start_idx = self.drag.start_idx?;
        let (base, _) = self.base_displayed_entries();
        let source = base.get(start_idx)?;
        let is_reorderable = |candidate: &DockEntry| {
            candidate.pinned && matches!(candidate.kind, DockItemKind::App { .. })
        };
        let is_pinnable = |candidate: &DockEntry| {
            !candidate.pinned && matches!(candidate.kind, DockItemKind::App { .. })
        };
        let source_reorderable = is_reorderable(source);
        let source_pinnable = is_pinnable(source);
        if !source_reorderable && !source_pinnable {
            return None;
        }
        let unpin_distance = scale(45, self.screen.dpi);
        if y < self.layout.pill.y - unpin_distance {
            return None;
        }
        let (app_count, _, _) = count_entries(&base);
        let first_pin = base[..app_count].iter().position(is_reorderable)?;
        let last_pin = base[..app_count].iter().rposition(is_reorderable)?;
        let Some(slot) = self.layout.reorder_slot_index(x, app_count) else {
            return self.drag.hover_idx;
        };
        if base.get(slot).is_some_and(is_reorderable) {
            Some(slot)
        } else if slot < first_pin {
            Some(first_pin)
        } else if source_reorderable && slot > last_pin {
            Some(last_pin)
        } else {
            None
        }
    }

    fn on_left_down(&mut self, x: i32, y: i32) {
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let idx = self.layout.hit_test(x, y, gap_pad);
        self.drag = DragState {
            start_idx: idx,
            hover_idx: None,
            start_pos: (x, y),
            current_pos: (x, y),
            is_dragging: false,
        };
        if idx.is_some() {
            unsafe {
                let _ = SetCapture(HWND(self.hwnd as *mut _));
            }
        }
    }

    fn on_left_up(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        let drag = std::mem::take(&mut self.drag);
        let Some(start_idx) = drag.start_idx else {
            return;
        };
        let _ = release_capture_if_owned(self.hwnd);
        if drag.is_dragging {
            self.on_drop(runtime, start_idx, drag.hover_idx, x, y);
        } else {
            self.on_left_click(runtime, drag.start_pos.0, drag.start_pos.1);
        }
    }

    fn on_drop(
        &mut self,
        runtime: &RuntimeContext,
        start_idx: usize,
        hover_idx: Option<usize>,
        x: i32,
        y: i32,
    ) {
        let all = self.combined_entries();
        let Some(entry) = all.get(start_idx).cloned() else {
            return;
        };
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;

        let unpin_distance = scale(45, self.screen.dpi);
        let reorderable = |candidate: &DockEntry| {
            candidate.pinned && matches!(candidate.kind, DockItemKind::App { .. })
        };
        let target = self.layout.hit_test(x, y, gap_pad).or(hover_idx);
        let target_entry = target.and_then(|index| all.get(index));
        let effect = dock_drop_effect(DockDropInput {
            source: start_idx,
            source_reorderable: reorderable(&entry),
            source_pinnable: !entry.pinned && matches!(entry.kind, DockItemKind::App { .. }),
            beyond_unpin_line: y < self.layout.pill.y - unpin_distance,
            target,
            target_reorderable: target_entry.is_some_and(reorderable),
        });
        let applied = match effect {
            DockDropEffect::Unpin => remove_pinned_app(&entry.key),
            DockDropEffect::Reorder { .. } => target_entry
                .is_some_and(|target_entry| reorder_pinned_apps(&entry.key, &target_entry.key)),
            DockDropEffect::PinAt { .. } => {
                target_entry.is_some_and(|target_entry| entry.pin_at_target(&target_entry.key))
            }
            DockDropEffect::Nothing => false,
        };
        if applied {
            self.refresh_entries(runtime);
        } else if hover_idx.is_some() {
            self.recompute_layout();
            self.present(runtime);
        }
    }

    fn on_drop_files(&mut self, runtime: &RuntimeContext, hdrop: usize) {
        let (pt, files) = extract_dropped_files(hdrop);
        if files.is_empty() {
            return;
        }
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(pt.0, pt.1, gap_pad) else {
            return;
        };
        let all = self.combined_entries();
        let Some(entry) = all.get(idx) else {
            return;
        };
        for file in &files {
            open_file_with_dock_entry(entry, file);
        }
        self.present(runtime);
    }

    fn on_left_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        crate::menu::close(runtime);
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(x, y, gap_pad) else {
            return;
        };
        self.activate_index(runtime, idx);
    }

    fn on_dock_key(&mut self, runtime: &RuntimeContext, key: BarKey) {
        let count = self.combined_entries().len();
        match self.focus.handle(key, count) {
            DockFocusOutcome::Moved(_) => self.present(runtime),
            DockFocusOutcome::Exited => self.leave_keyboard_focus(runtime),
            DockFocusOutcome::Activate(index) => {
                self.leave_keyboard_focus(runtime);
                self.activate_index(runtime, index);
            }
            DockFocusOutcome::ReturnToBar => {
                self.leave_keyboard_focus(runtime);
                crate::bar::enter_keyboard_focus_from_dock(runtime, key == BarKey::PreviousTab);
            }
            DockFocusOutcome::Unchanged => {}
        }
    }

    fn enter_keyboard_focus(&mut self, runtime: &RuntimeContext, from_end: bool) {
        let count = self.combined_entries().len();
        if self.focus.enter(count, from_end).is_some() && route_bar_keys(self.hwnd, WM_APP_DOCK_KEY)
        {
            self.present(runtime);
        } else {
            self.focus.exit();
        }
    }

    fn leave_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        stop_routing_if_targets(self.hwnd);
        self.focus.exit();
        self.present(runtime);
    }

    fn sync_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if !self.focus.is_active() {
            return;
        }
        if !routes_keys_to(self.hwnd) {
            self.focus.exit();
            self.present(runtime);
        } else if self.focus.revalidate(self.combined_entries().len()) {
            self.present(runtime);
        }
    }

    fn activate_index(&mut self, runtime: &RuntimeContext, idx: usize) {
        let all = self.combined_entries();
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        if matches!(
            entry.kind,
            DockItemKind::Downloads | DockItemKind::Overflow { .. }
        ) {
            let Some(slot) = self.layout.items.get(idx).copied() else {
                return;
            };
            let (origin_x, origin_y) = self.window_origin();
            let center_x = origin_x + (slot.x + slot.size * 0.5).round() as i32;
            let bottom_y = origin_y + slot.y.round() as i32 - scale(10, self.screen.dpi);
            if matches!(entry.kind, DockItemKind::Downloads) {
                crate::menu::open_downloads_stack_menu(runtime, center_x, bottom_y);
            } else {
                let hidden = self.displayed_entries().1;
                crate::menu::open_dock_overflow_menu(runtime, center_x, bottom_y, hidden);
            }
            return;
        }
        if self.motion.allows_motion()
            && !entry.running
            && matches!(entry.kind, DockItemKind::App { .. })
        {
            self.anim.bounce_key = Some(entry.key.clone());
            self.anim.bounce_phase = 0.01;
            self.ensure_animating();
        }
        let target = self.target_for_entry(&entry);
        activate_entry(runtime, &entry, target);
    }

    fn on_right_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(x, y, gap_pad) else {
            if self.layout.pill.contains(x, y) {
                let (origin_x, origin_y) = self.window_origin();
                let bottom_y = origin_y + self.layout.pill.y - scale(10, self.screen.dpi);
                crate::menu::open_dock_settings_menu(
                    runtime,
                    origin_x + x,
                    bottom_y,
                    self.preferences,
                );
            }
            return;
        };
        let all = self.combined_entries();
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        let Some(slot) = self.layout.items.get(idx).copied() else {
            return;
        };
        self.hovering = false;
        self.ensure_animating();

        let (origin_x, origin_y) = self.window_origin();
        let center_x = origin_x + (slot.x + slot.size * 0.5).round() as i32;
        let bottom_y = origin_y + slot.y.round() as i32 - scale(10, self.screen.dpi);
        let target = self.target_for_entry(&entry);
        let others_visible = other_applications_visible(&all, &entry.key);
        crate::menu::open_dock_menu(runtime, entry, others_visible, center_x, bottom_y, target);
    }

    fn ensure_animating(&mut self) {
        if !self.anim.timer_running {
            otto_platform::begin_fine_timing();
            self.anim.timer_running = true;
            self.anim.last_tick = Instant::now();
            unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    DOCK_ANIM_TIMER,
                    self.motion_metrics.animation_tick_ms,
                    None,
                );
            }
        }
    }

    fn stop_animating(&mut self) {
        if self.anim.timer_running {
            self.anim.timer_running = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), DOCK_ANIM_TIMER);
            }
            otto_platform::end_fine_timing();
        }
    }

    fn recompute_layout(&mut self) {
        let all = self.combined_entries();
        let (app_count, right_count, has_trash) = count_entries(&all);
        let bounce_offsets: Vec<f32> = all
            .iter()
            .map(|entry| {
                if self.anim.bounce_phase > 0.0
                    && self.anim.bounce_key.as_deref() == Some(entry.key.as_str())
                {
                    bounce_offset(self.anim.bounce_phase)
                } else {
                    0.0
                }
            })
            .collect();
        let item_presence: Vec<f32> = all
            .iter()
            .map(|entry| self.presence.get(&entry.key).copied().unwrap_or(1.0))
            .collect();

        let active_cursor = if self.hovering || self.anim.mag_strength > 0.001 {
            self.last_cursor_x
        } else {
            None
        };

        let mut layout = compute_dock_layout(
            &self.metrics,
            DockInput {
                screen_width: self.screen.width,
                dpi: self.screen.dpi,
                app_count,
                right_count,
                has_trash,
                cursor_x: active_cursor,
                mag_strength: self.anim.mag_strength,
                bounce_offsets: &bounce_offsets,
                item_presence: &item_presence,
                hide_progress: self.anim.hide_progress,
            },
        );
        if !self.hovering {
            layout.hovered_index = None;
        }
        self.layout = layout;
    }

    fn target_for_entry(&self, entry: &DockEntry) -> Option<Rect> {
        let index = self
            .combined_entries()
            .iter()
            .position(|candidate| candidate.key == entry.key)?;
        let item = self.layout.items.get(index)?;
        let (origin_x, origin_y) = self.window_origin();
        let size = item.size.round().max(1.0) as i32;
        Some(Rect {
            x: origin_x + item.x.round() as i32,
            y: origin_y + item.y.round() as i32,
            width: size,
            height: size,
        })
    }

    fn present(&mut self, runtime: &RuntimeContext) {
        let all = self.combined_entries();
        let render_items: Vec<DockRenderItem<'_>> = all
            .iter()
            .enumerate()
            .map(|(index, e)| {
                let notif_count = entry_notification_count(&self.notification_counts, e);
                let badge_count = dock_entry_badge(&e.window_titles, notif_count);
                let progress = if matches!(e.kind, DockItemKind::Downloads) {
                    self.downloads_progress
                } else {
                    dock_entry_progress(&e.window_titles, None)
                };
                DockRenderItem {
                    key: &e.key,
                    title: &e.title,
                    apple_icon: e.apple_icon,
                    running: e.running,
                    active: e.active,
                    hidden: e.hidden,
                    focused: self.focus.current() == Some(index),
                    badge_count,
                    progress,
                }
            })
            .collect();
        let appearance = crate::theme_state::appearance(runtime);
        crate::host::log_present_error(
            "dock",
            self.renderer.present(
                self.hwnd,
                self.window_origin(),
                &self.layout,
                &render_items,
                appearance,
            ),
        );
    }
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
mod tests {
    use super::{
        LPARAM, LRESULT, RuntimeContext, WM_CLOSE, WM_MOUSEMOVE, WPARAM, handle_message,
        wake_timer_only,
    };

    #[test]
    fn wm_close_is_absorbed_without_destroying_surface() {
        let runtime = RuntimeContext::new(false);
        assert_eq!(
            handle_message(&runtime, 0, WM_CLOSE, WPARAM(0), LPARAM(0)),
            Some(LRESULT(0))
        );
    }

    #[test]
    fn mouse_move_and_wake_timer_only_handle_unmounted_runtime() {
        let runtime = RuntimeContext::new(false);
        wake_timer_only(&runtime);
        assert_eq!(
            handle_message(&runtime, 0, WM_MOUSEMOVE, WPARAM(0), LPARAM(0)),
            Some(LRESULT(0))
        );
    }
}
