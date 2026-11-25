use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::{
    DOCK_ANIM_TIMER, WM_APP_DOCK_BACKDROP, WM_DROPFILES, WM_GETOBJECT, WM_LBUTTONUP, WM_MOUSELEAVE,
};
use otto_core::{
    DockDropEffect, DockDropInput, DockEntry, DockItemKind, DockPort, advance_bounce_phase,
    animate_value, build_dock_accessible_tree, dock_drop_effect, other_applications_visible, scale,
};
use otto_platform::{
    ShellNotification, WM_APP_UIA_INVOKE, capture_mouse, cursor_position, extract_dropped_files,
    handle_wm_getobject, release_capture_if_owned, track_mouse_leave,
};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MA_NOACTIVATE, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER,
};

use super::actions::{activate_entry, count_entries, sync_icons};
use super::visibility::should_assert_topmost;
use super::{Dock, DragState, unpack_point, with_dock};

fn handle_secondary_pointer_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| {
                dock.on_secondary_mouse_move(runtime, hwnd, x, y);
            });
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_dock(runtime, |dock| dock.on_secondary_mouse_leave(hwnd));
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| dock.on_secondary_left_down(hwnd, x, y));
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| {
                dock.on_secondary_left_up(runtime, hwnd, x, y);
            });
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_dock(runtime, |dock| {
                dock.on_secondary_right_click(runtime, hwnd, x, y);
            });
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_dock(runtime, |dock| {
                if let Some(index) = dock.secondary_index(hwnd) {
                    let was_dragging = dock.secondary_docks[index].drag.is_dragging;
                    dock.secondary_docks[index].drag = DragState::default();
                    if was_dragging {
                        dock.recompute_secondary_dock_layout(index);
                        dock.present_secondary(runtime, index);
                    }
                }
            });
            Some(LRESULT(0))
        }
        _ => None,
    }
}

pub(super) fn handle_secondary_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if let Some(result) = handle_secondary_pointer_message(runtime, hwnd, message, lparam) {
        return Some(result);
    }
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_CLOSE => Some(LRESULT(0)),
        WM_GETOBJECT => handle_secondary_accessible_object(runtime, hwnd, wparam, lparam),
        WM_APP_UIA_INVOKE => {
            with_dock(runtime, |dock| {
                dock.activate_secondary_index(runtime, hwnd, wparam.0);
            });
            Some(LRESULT(0))
        }
        WM_APP_DOCK_BACKDROP => {
            with_dock(runtime, |dock| dock.on_secondary_backdrop(runtime, hwnd));
            Some(LRESULT(0))
        }
        otto_platform::DOCK_APPBAR_CALLBACK_MESSAGE => {
            if let Some(notification) =
                otto_platform::classify_shell_notification(wparam.0, lparam.0)
            {
                with_dock(runtime, |dock| {
                    dock.on_secondary_shell_notification(runtime, hwnd, notification);
                });
            }
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == DOCK_ANIM_TIMER => {
            with_dock(runtime, |dock| {
                if let Some(index) = dock.secondary_index(hwnd) {
                    dock.step_secondary_anim(runtime, index);
                }
            });
            Some(LRESULT(0))
        }
        WM_DROPFILES => {
            let hdrop = wparam.0;
            with_dock(runtime, |dock| {
                dock.on_secondary_drop_files(runtime, hwnd, hdrop);
            });
            Some(LRESULT(0))
        }
        WM_DPICHANGED => {
            with_dock(runtime, |dock| dock.sync_secondary_docks(runtime));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE => Some(LRESULT(0)),
        _ => None,
    }
}

fn handle_secondary_accessible_object(
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
            let index = dock.secondary_index(hwnd)?;
            let sec = &dock.secondary_docks[index];
            handle_wm_getobject(hwnd, wparam.0, lparam.0, WM_APP_UIA_INVOKE, || {
                let entries = sec.combined_entries(dock);
                let labels: Vec<&str> = entries.iter().map(|entry| entry.title.as_str()).collect();
                build_dock_accessible_tree(&sec.layout, sec.window_origin(), &labels)
            })
        })
        .map(LRESULT)
}

impl Dock {
    fn on_secondary_backdrop(&mut self, runtime: &RuntimeContext, hwnd: isize) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let (origin_x, origin_y) = self.secondary_docks[index].window_origin();
        let Some(update) = self.secondary_docks[index]
            .backdrop
            .as_mut()
            .and_then(BackdropFeed::poll)
        else {
            return;
        };
        let local = (update.origin.0 - origin_x, update.origin.1 - origin_y);
        self.secondary_docks[index]
            .renderer
            .set_backdrop(update.pixels, local);
        self.present_secondary(runtime, index);
    }

    fn on_secondary_shell_notification(
        &mut self,
        runtime: &RuntimeContext,
        hwnd: isize,
        notification: ShellNotification,
    ) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        match notification {
            ShellNotification::PositionChanged => {
                let bounds = self.secondary_docks[index].monitor.bounds;
                let rest_height = self.secondary_docks[index].layout.rest_height;
                if let Some(reservation) = self.secondary_docks[index].reservation.as_mut() {
                    let _ = reservation.reapply_in_bounds(bounds, rest_height);
                }
                if should_assert_topmost(
                    self.secondary_docks[index].anim.should_hide,
                    Self::higher_overlay_open(runtime),
                ) {
                    self.secondary_docks[index].place();
                }
            }
            ShellNotification::FullScreenApp(active) => {
                self.secondary_docks[index].cover.shell_full_screen = active;
                self.refresh_secondary_cover(index);
                self.sync_secondary_hide_from_pointer(runtime, index);
                if !active {
                    self.secondary_docks[index].ensure_topmost(runtime);
                }
            }
        }
    }

    fn secondary_drag_preview_target(&self, index: usize, x: i32, y: i32) -> Option<usize> {
        let sec = &self.secondary_docks[index];
        let start_idx = sec.drag.start_idx?;
        let (base, _) = sec.base_displayed_entries(self);
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
        let unpin_distance = scale(45, sec.monitor.dpi);
        if y < sec.layout.pill.y - unpin_distance {
            return None;
        }
        let (app_count, _, _) = count_entries(&base);
        let first_pin = base[..app_count].iter().position(is_reorderable)?;
        let last_pin = base[..app_count].iter().rposition(is_reorderable)?;
        let Some(slot) = sec.layout.reorder_slot_index(x, app_count) else {
            return sec.drag.hover_idx;
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

    fn on_secondary_mouse_move(&mut self, runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        if !self.secondary_docks[index].tracking_mouse && track_mouse_leave(hwnd) {
            self.secondary_docks[index].tracking_mouse = true;
        }
        if self.secondary_docks[index].drag.start_idx.is_some() {
            self.secondary_docks[index].drag.current_pos = (x, y);
            if !self.secondary_docks[index].drag.is_dragging {
                let start = self.secondary_docks[index].drag.start_pos;
                let dx = x - start.0;
                let dy = y - start.1;
                if dx * dx + dy * dy > 36 {
                    self.secondary_docks[index].drag.is_dragging = true;
                }
            }
            if self.secondary_docks[index].drag.is_dragging {
                let next_hover = self.secondary_drag_preview_target(index, x, y);
                if next_hover != self.secondary_docks[index].drag.hover_idx {
                    self.secondary_docks[index].drag.hover_idx = next_hover;
                    self.recompute_secondary_dock_layout(index);
                    self.present_secondary(runtime, index);
                }
            }
        }
        if self.secondary_docks[index].anim.should_hide {
            return;
        }
        let pill = self.secondary_docks[index].layout.pill;
        let near_pill = x >= pill.x - 16 && x <= pill.right() + 16 && y >= pill.y - 36;
        let tick_ms = self.motion_metrics.animation_tick_ms;
        if near_pill {
            let started_hover = !self.secondary_docks[index].hovering;
            self.secondary_docks[index].hovering = true;
            self.secondary_docks[index].last_cursor_x = Some(x as f32);
            if started_hover {
                self.secondary_docks[index].ensure_topmost(runtime);
            }
            self.secondary_docks[index].ensure_animating(tick_ms);
        } else if self.secondary_docks[index].hovering {
            self.secondary_docks[index].hovering = false;
            self.secondary_docks[index].ensure_animating(tick_ms);
        }
    }

    fn on_secondary_mouse_leave(&mut self, hwnd: isize) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let tick_ms = self.motion_metrics.animation_tick_ms;
        let sec = &mut self.secondary_docks[index];
        sec.tracking_mouse = false;
        let (gx, gy) = cursor_position();
        let (win_left, win_top) = sec.window_origin();
        let local_x = gx - win_left;
        let local_y = gy - win_top;
        let still_over = sec.monitor.bounds.contains(gx, gy)
            && !sec.anim.should_hide
            && local_x >= sec.layout.pill.x - 12
            && local_x <= sec.layout.pill.right() + 12
            && local_y >= sec.layout.pill.y - 28
            && local_y <= sec.layout.pill.bottom();
        if !still_over && sec.hovering {
            sec.hovering = false;
            sec.ensure_animating(tick_ms);
        }
    }

    fn on_secondary_left_down(&mut self, hwnd: isize, x: i32, y: i32) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let sec = &mut self.secondary_docks[index];
        let gap_pad = scale(self.metrics.icon_gap, sec.monitor.dpi) as f32 * 0.5;
        let idx = sec.layout.hit_test(x, y, gap_pad);
        sec.drag = DragState {
            start_idx: idx,
            hover_idx: None,
            start_pos: (x, y),
            current_pos: (x, y),
            is_dragging: false,
        };
        if idx.is_some() {
            capture_mouse(hwnd);
        }
    }

    fn on_secondary_left_up(&mut self, runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let drag = std::mem::take(&mut self.secondary_docks[index].drag);
        let Some(start_idx) = drag.start_idx else {
            return;
        };
        let _ = release_capture_if_owned(hwnd);
        if drag.is_dragging {
            self.on_secondary_drop(runtime, index, start_idx, drag.hover_idx, x, y);
        } else {
            crate::menu::close(runtime);
            let dpi = self.secondary_docks[index].monitor.dpi;
            let gap_pad = scale(self.metrics.icon_gap, dpi) as f32 * 0.5;
            if let Some(hit) = self.secondary_docks[index].layout.hit_test(
                drag.start_pos.0,
                drag.start_pos.1,
                gap_pad,
            ) {
                self.activate_secondary_index(runtime, hwnd, hit);
            }
        }
    }

    fn on_secondary_drop(
        &mut self,
        runtime: &RuntimeContext,
        index: usize,
        start_idx: usize,
        hover_idx: Option<usize>,
        x: i32,
        y: i32,
    ) {
        let all = self.secondary_docks[index].combined_entries(self);
        let Some(entry) = all.get(start_idx).cloned() else {
            return;
        };
        let dpi = self.secondary_docks[index].monitor.dpi;
        let gap_pad = scale(self.metrics.icon_gap, dpi) as f32 * 0.5;
        let unpin_distance = scale(45, dpi);
        let reorderable = |candidate: &DockEntry| {
            candidate.pinned && matches!(candidate.kind, DockItemKind::App { .. })
        };
        let target = self.secondary_docks[index]
            .layout
            .hit_test(x, y, gap_pad)
            .or(hover_idx);
        let target_entry = target.and_then(|i| all.get(i));
        let effect = dock_drop_effect(DockDropInput {
            source: start_idx,
            source_reorderable: reorderable(&entry),
            source_pinnable: !entry.pinned && matches!(entry.kind, DockItemKind::App { .. }),
            beyond_unpin_line: y < self.secondary_docks[index].layout.pill.y - unpin_distance,
            target,
            target_reorderable: target_entry.is_some_and(reorderable),
        });
        let applied = match effect {
            DockDropEffect::Unpin => self.port.remove_pin(&entry.key),
            DockDropEffect::Reorder { .. } => target_entry
                .is_some_and(|target_entry| self.port.reorder_pins(&entry.key, &target_entry.key)),
            DockDropEffect::PinAt { .. } => target_entry
                .is_some_and(|target_entry| self.port.pin_at_target(&entry, &target_entry.key)),
            DockDropEffect::Nothing => false,
        };
        if applied {
            self.refresh_entries(runtime);
        } else if hover_idx.is_some() {
            self.recompute_secondary_dock_layout(index);
            self.present_secondary(runtime, index);
        }
    }

    fn activate_secondary_index(&mut self, runtime: &RuntimeContext, hwnd: isize, idx: usize) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let (all, hidden) = self.secondary_docks[index].displayed_entries(self);
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        if matches!(
            entry.kind,
            DockItemKind::Downloads | DockItemKind::Overflow { .. }
        ) {
            let Some(slot) = self.secondary_docks[index].layout.items.get(idx).copied() else {
                return;
            };
            let dpi = self.secondary_docks[index].monitor.dpi;
            let (origin_x, origin_y) = self.secondary_docks[index].window_origin();
            let center_x = origin_x + (slot.x + slot.size * 0.5).round() as i32;
            let bottom_y = origin_y + slot.y.round() as i32 - scale(10, dpi);
            if matches!(entry.kind, DockItemKind::Downloads) {
                crate::menu::open_downloads_stack_menu(runtime, center_x, bottom_y);
            } else {
                crate::menu::open_dock_overflow_menu(runtime, center_x, bottom_y, hidden);
            }
            return;
        }
        let tick_ms = self.motion_metrics.animation_tick_ms;
        if self.motion.allows_motion()
            && !entry.running
            && matches!(entry.kind, DockItemKind::App { .. })
        {
            self.secondary_docks[index].anim.bounce_key = Some(entry.key.clone());
            self.secondary_docks[index].anim.bounce_phase = 0.01;
            self.secondary_docks[index].ensure_animating(tick_ms);
        }
        let target = self.secondary_docks[index].item_screen_rect(idx);
        activate_entry(runtime, &entry, target);
        self.secondary_docks[index].ensure_topmost(runtime);
    }

    fn on_secondary_right_click(&mut self, runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let dpi = self.secondary_docks[index].monitor.dpi;
        let gap_pad = scale(self.metrics.icon_gap, dpi) as f32 * 0.5;
        let Some(idx) = self.secondary_docks[index].layout.hit_test(x, y, gap_pad) else {
            if self.secondary_docks[index].layout.pill.contains(x, y) {
                let (origin_x, origin_y) = self.secondary_docks[index].window_origin();
                let bottom_y =
                    origin_y + self.secondary_docks[index].layout.pill.y - scale(10, dpi);
                crate::menu::open_dock_settings_menu(
                    runtime,
                    origin_x + x,
                    bottom_y,
                    self.preferences,
                );
            }
            return;
        };
        let all = self.secondary_docks[index].combined_entries(self);
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        let Some(slot) = self.secondary_docks[index].layout.items.get(idx).copied() else {
            return;
        };
        let tick_ms = self.motion_metrics.animation_tick_ms;
        self.secondary_docks[index].hovering = false;
        self.secondary_docks[index].ensure_animating(tick_ms);
        let (origin_x, origin_y) = self.secondary_docks[index].window_origin();
        let center_x = origin_x + (slot.x + slot.size * 0.5).round() as i32;
        let bottom_y = origin_y + slot.y.round() as i32 - scale(10, dpi);
        let target = self.secondary_docks[index].item_screen_rect(idx);
        let others_visible = other_applications_visible(&all, &entry.key);
        crate::menu::open_dock_menu(runtime, entry, others_visible, center_x, bottom_y, target);
    }

    fn on_secondary_drop_files(&mut self, runtime: &RuntimeContext, hwnd: isize, hdrop: usize) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let (pt, files) = extract_dropped_files(hdrop);
        if files.is_empty() {
            return;
        }
        let dpi = self.secondary_docks[index].monitor.dpi;
        let gap_pad = scale(self.metrics.icon_gap, dpi) as f32 * 0.5;
        let Some(idx) = self.secondary_docks[index]
            .layout
            .hit_test(pt.0, pt.1, gap_pad)
        else {
            return;
        };
        let all = self.secondary_docks[index].combined_entries(self);
        let Some(entry) = all.get(idx) else {
            return;
        };
        for file in &files {
            self.port.open_file_with(entry, file);
        }
        self.present_secondary(runtime, index);
    }

    pub(super) fn step_secondary_anim(&mut self, runtime: &RuntimeContext, index: usize) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.secondary_docks[index].anim.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.secondary_docks[index].anim.last_tick = now;

        if self.secondary_docks[index].hovering {
            let (gx, gy) = cursor_position();
            let sec = &mut self.secondary_docks[index];
            let (win_left, win_top) = sec.window_origin();
            let local_x = gx - win_left;
            let local_y = gy - win_top;
            let still_inside = sec.monitor.bounds.contains(gx, gy)
                && !sec.anim.should_hide
                && local_x >= sec.layout.pill.x - 20
                && local_x <= sec.layout.pill.right() + 20
                && local_y >= sec.layout.pill.y - 36;
            if still_inside {
                sec.last_cursor_x = Some(local_x as f32);
            } else {
                sec.hovering = false;
            }
        }

        let motion = self.motion;
        let metrics = self.motion_metrics;
        let sec = &mut self.secondary_docks[index];
        let target_mag = if sec.hovering && !sec.anim.should_hide {
            1.0
        } else {
            0.0
        };
        let target_hide = if sec.anim.should_hide { 1.0 } else { 0.0 };
        sec.anim.mag_strength = animate_value(
            sec.anim.mag_strength,
            target_mag,
            metrics.dock_magnify_rate,
            dt,
            motion,
        );
        sec.anim.hide_progress = animate_value(
            sec.anim.hide_progress,
            target_hide,
            metrics.dock_hide_rate,
            dt,
            motion,
        );
        if sec.anim.bounce_phase > 0.0 {
            sec.anim.bounce_phase = advance_bounce_phase(
                sec.anim.bounce_phase,
                dt,
                metrics.bounce_duration_seconds,
                motion,
            );
            if sec.anim.bounce_phase >= 1.0 {
                sec.anim.bounce_phase = 0.0;
                sec.anim.bounce_key = None;
            }
        }
        let mag_done = (sec.anim.mag_strength - target_mag).abs() < 0.001;
        let hide_done = (sec.anim.hide_progress - target_hide).abs() < 0.001;
        let bounce_done = sec.anim.bounce_phase == 0.0 && self.anim.bounce_phase == 0.0;
        let presence_done = !self.anim.timer_running && self.closing_entries.is_empty();

        self.recompute_secondary_dock_layout(index);
        self.present_secondary(runtime, index);

        if mag_done && hide_done && bounce_done && presence_done {
            if !self.secondary_docks[index].hovering {
                self.secondary_docks[index].last_cursor_x = None;
            }
            let all = self.secondary_docks[index].combined_entries(self);
            sync_icons(&mut self.secondary_docks[index].renderer, &all);
            self.secondary_docks[index].stop_animating();
        }
    }
}
