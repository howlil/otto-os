use std::collections::HashMap;
use std::mem::size_of;
use std::time::Instant;

use crate::host::RuntimeContext;
use otto_core::{
    ChangeGate, CommandOutcome, DockDropEffect, DockDropInput, DockEffect, DockInput, DockLayout,
    DockMetrics, MotionMetrics, MotionPreference, Rect, StableOrder, WindowCommand, WindowIdentity,
    advance_bounce_phase, animate_value, bounce_offset, compute_dock_layout,
    dock_activation_effect, dock_capacity, dock_command_effect, dock_drop_effect, scale,
    split_dock_overflow,
};
use otto_platform::{
    DockEntry, DockItemKind, PrimaryScreen, collect_dock_entries, current_process_id,
    cursor_position, empty_recycle_bin, enable_drag_accept_files, execute_window_command,
    extract_dock_icon, extract_dropped_files, has_maximized_or_fullscreen_window, hide_application,
    is_escape_down, is_window_minimized, launch_new_instance, motion_preference,
    open_file_with_dock_entry, other_applications_visible, overflow_dock_entry, primary_screen,
    quit_application, remove_pinned_app, reorder_pinned_apps, show_application,
    shutdown_recycle_bin_task, signal_command_failure, toggle_pin_entry, window_change_generations,
};
use otto_render::{DockRenderItem, DockRenderer};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer, SetWindowPos,
    WM_CAPTURECHANGED, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_DROPFILES: u32 = 0x0233;
const DOCK_POLL_TIMER: usize = 10;
const DOCK_ANIM_TIMER: usize = 11;
const DOCK_POLL_MS: u32 = 150;
const DOCK_ENTRIES_FALLBACK_TICKS: u32 = 14;
const DOCK_COVER_FALLBACK_TICKS: u32 = 7;

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
    start_pos: (i32, i32),
    current_pos: (i32, i32),
    is_dragging: bool,
}

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
    entries_gate: ChangeGate,
    cover_gate: ChangeGate,
    covered: bool,
}

pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    enable_drag_accept_files(hwnd);
    let own_pid = current_process_id();
    let screen = primary_screen();
    let metrics = DockMetrics::default();
    let motion = motion_preference();
    let motion_metrics = MotionMetrics::default();
    let mut order = StableOrder::default();
    let entries = collect_dock_entries(own_pid, &mut order);
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

    let dock = Dock {
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
        entries_gate: ChangeGate::new(DOCK_ENTRIES_FALLBACK_TICKS),
        cover_gate: ChangeGate::new(DOCK_COVER_FALLBACK_TICKS),
        covered: false,
    };
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
    if let Some(dock) = runtime.dock.borrow_mut().take() {
        unsafe {
            let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_POLL_TIMER);
            let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_ANIM_TIMER);
        }
    }
}

pub fn handle_message(
    runtime: &RuntimeContext,
    _hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
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
            with_dock(runtime, |dock| dock.on_mouse_move(x, y));
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
            with_dock(runtime, |dock| dock.drag = DragState::default());
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
        _ => None,
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn with_dock(runtime: &RuntimeContext, action: impl FnOnce(&mut Dock)) {
    if let Ok(mut guard) = runtime.dock.try_borrow_mut()
        && let Some(dock) = guard.as_mut()
    {
        action(dock);
    }
}

pub fn repaint(runtime: &RuntimeContext) {
    with_dock(runtime, |dock| dock.present(runtime));
}

#[must_use]
pub fn dock_target_for_window(runtime: &RuntimeContext, hwnd: isize) -> Option<Rect> {
    let guard = runtime.dock.try_borrow().ok()?;
    let dock = guard.as_ref()?;
    let entries = dock.combined_entries();
    let index = entries
        .iter()
        .position(|entry| entry.hwnds.contains(&hwnd))?;
    let item = dock.layout.items.get(index)?;
    let (origin_x, origin_y) = dock.window_origin();
    let size = item.size.round().max(1.0) as i32;
    Some(Rect {
        x: origin_x + item.x.round() as i32,
        y: origin_y + item.y.round() as i32,
        width: size,
        height: size,
    })
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

pub fn activate_overflow_entry(runtime: &RuntimeContext, entry: &DockEntry) {
    activate_entry(runtime, entry, None);
}

fn activate_entry(runtime: &RuntimeContext, entry: &DockEntry, target: Option<Rect>) {
    execute_dock_effect(
        runtime,
        dock_activation_effect(entry.app.as_ref()),
        entry,
        target,
    );
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
        DockEffect::Launch => launch_new_instance(entry),
        DockEffect::ShowWindow(window) => show_window(runtime, window, target),
        DockEffect::TogglePin => {
            toggle_pin_entry(entry);
            with_dock(runtime, |dock| dock.refresh_entries(runtime));
        }
        DockEffect::EmptyTrash => empty_recycle_bin(),
        DockEffect::Unhide | DockEffect::Hide | DockEffect::Quit => {
            let Some(app) = entry.app.as_ref() else {
                return;
            };
            report(match effect {
                DockEffect::Unhide => show_application(app, own_pid),
                DockEffect::Hide => hide_application(app, own_pid),
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

impl Dock {
    fn refresh_entries(&mut self, runtime: &RuntimeContext) {
        self.entries = collect_dock_entries(self.own_pid, &mut self.order);
        let updated = self.combined_entries();
        sync_icons(&mut self.renderer, &updated);
        self.recompute_layout();
        self.present(runtime);
    }

    fn cancel_drag(&mut self) {
        if self.drag.start_idx.is_some() {
            self.drag = DragState::default();
            unsafe {
                let _ = ReleaseCapture();
            }
        }
    }

    fn cancel_drag_on_escape(&mut self) {
        if self.drag.start_idx.is_some() && is_escape_down() {
            self.cancel_drag();
        }
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

    fn displayed_entries(&self) -> (Vec<DockEntry>, Vec<DockEntry>) {
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

    fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.screen = primary_screen();
        self.cover_gate.invalidate();
        self.recompute_layout();
        let all = self.combined_entries();
        if let Ok(mut renderer) = DockRenderer::new(
            self.layout.window_width,
            self.layout.window_height,
            self.screen.dpi,
        ) {
            sync_icons(&mut renderer, &all);
            self.renderer = renderer;
        }
        self.place();
        self.present(runtime);
    }

    fn refresh_entries_if_changed(&mut self, topology: Option<u64>) -> bool {
        if !self.entries_gate.should_refresh(topology) {
            return false;
        }
        let latest = collect_dock_entries(self.own_pid, &mut self.order);
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
                    | DockItemKind::Launchpad
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

    fn on_poll_timer(&mut self, runtime: &RuntimeContext) {
        let mut dirty = false;
        let next_motion = motion_preference();
        if next_motion != self.motion {
            self.motion = next_motion;
            self.ensure_animating();
        }
        self.cancel_drag_on_escape();
        let generations = window_change_generations();
        if self.refresh_entries_if_changed(generations.map(|changes| changes.topology)) {
            dirty = true;
        }

        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;

        if self
            .cover_gate
            .should_refresh(generations.map(|changes| changes.geometry))
        {
            self.covered = has_maximized_or_fullscreen_window(
                self.screen.width,
                self.screen.height,
                self.own_pid,
            );
        }
        let fullscreen_or_max = self.covered;

        let at_bottom_edge = gy >= self.screen.height - 6;
        let in_interactive_zone = self.anim.hide_progress < 0.85
            && local_x >= self.layout.pill.x - 20
            && local_x <= self.layout.pill.right() + 20
            && local_y >= self.layout.pill.y - 36;

        let target_hide = fullscreen_or_max && !at_bottom_edge && !in_interactive_zone;
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
            let cur = self.presence.get(&entry.key).copied().unwrap_or(1.0);
            let next = animate_value(
                cur,
                1.0,
                self.motion_metrics.dock_presence_enter_rate,
                dt,
                self.motion,
            );
            self.presence.insert(entry.key.clone(), next);
            if (next - 1.0).abs() > 0.002 {
                presence_animating = true;
            }
        }
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

    fn on_mouse_move(&mut self, x: i32, y: i32) {
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

    fn on_left_down(&mut self, x: i32, y: i32) {
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let idx = self.layout.hit_test(x, y, gap_pad);
        self.drag = DragState {
            start_idx: idx,
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
        unsafe {
            let _ = ReleaseCapture();
        }
        if drag.is_dragging {
            self.on_drop(runtime, start_idx, x, y);
        } else {
            self.on_left_click(runtime, drag.start_pos.0, drag.start_pos.1);
        }
    }

    fn on_drop(&mut self, runtime: &RuntimeContext, start_idx: usize, x: i32, y: i32) {
        let all = self.combined_entries();
        let Some(entry) = all.get(start_idx).cloned() else {
            return;
        };
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;

        let unpin_distance = scale(45, self.screen.dpi);
        let reorderable = |candidate: &DockEntry| {
            candidate.pinned && matches!(candidate.kind, DockItemKind::App { .. })
        };
        let target = self.layout.hit_test(x, y, gap_pad);
        let target_entry = target.and_then(|index| all.get(index));
        let effect = dock_drop_effect(DockDropInput {
            source: start_idx,
            source_reorderable: reorderable(&entry),
            beyond_unpin_line: y < self.layout.pill.y - unpin_distance,
            target,
            target_reorderable: target_entry.is_some_and(reorderable),
        });
        let applied = match effect {
            DockDropEffect::Unpin => remove_pinned_app(&entry.key),
            DockDropEffect::Reorder { .. } => target_entry
                .is_some_and(|target_entry| reorder_pinned_apps(&entry.key, &target_entry.key)),
            DockDropEffect::Nothing => false,
        };
        if applied {
            self.refresh_entries(runtime);
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
            && (!entry.running
                || matches!(
                    entry.kind,
                    DockItemKind::Finder
                        | DockItemKind::Launchpad
                        | DockItemKind::Downloads
                        | DockItemKind::Trash { .. }
                ))
        {
            self.anim.bounce_key = Some(entry.key.clone());
            self.anim.bounce_phase = 0.01;
            self.ensure_animating();
        } else if self.motion.allows_motion() {
            self.anim.bounce_key = Some(entry.key.clone());
            self.anim.bounce_phase = 0.45;
            self.ensure_animating();
        }
        let target = self.target_for_entry(&entry);
        activate_entry(runtime, &entry, target);
    }

    fn on_right_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(x, y, gap_pad) else {
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

    fn present(&self, runtime: &RuntimeContext) {
        let all = self.combined_entries();
        let render_items: Vec<DockRenderItem<'_>> = all
            .iter()
            .map(|e| DockRenderItem {
                key: &e.key,
                title: &e.title,
                apple_icon: e.apple_icon,
                running: e.running,
                active: e.active,
                hidden: e.hidden,
            })
            .collect();
        let appearance = crate::theme_state::appearance(runtime);
        let _ = self.renderer.present(
            self.hwnd,
            self.window_origin(),
            &self.layout,
            &render_items,
            appearance,
        );
    }
}
