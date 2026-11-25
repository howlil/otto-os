use crate::host::RuntimeContext;
use otto_core::{
    DockDropEffect, DockDropInput, DockEntry, DockItemKind, DockPort, dock_drop_effect,
    other_applications_visible, scale,
};
use otto_platform::release_capture_if_owned;
use otto_platform::{
    capture_mouse, cursor_position, extract_dropped_files, is_escape_down, track_mouse_leave,
};

use super::Dock;
use super::DragState;
use super::actions::count_entries;

impl Dock {
    pub(super) fn cancel_drag(&mut self) -> bool {
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

    pub(super) fn cancel_drag_on_escape(&mut self) -> bool {
        self.drag.start_idx.is_some() && is_escape_down() && self.cancel_drag()
    }

    pub(super) fn track_hover_from_cursor(&mut self) {
        if !self.hovering {
            return;
        }
        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_y = gy - win_top;
        let on_primary = gx >= 0 && gx < self.screen.width && gy >= 0 && gy < self.screen.height;
        let still_inside = on_primary
            && !self.anim.should_hide
            && gx >= self.layout.pill.x - 20
            && gx <= self.layout.pill.right() + 20
            && local_y >= self.layout.pill.y - 36;
        if still_inside {
            self.last_cursor_x = Some(gx as f32);
        } else {
            self.hovering = false;
        }
    }

    pub(super) fn window_origin(&self) -> (i32, i32) {
        (0, self.screen.height - self.layout.window_height)
    }

    pub(super) fn on_mouse_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if !self.tracking_mouse && track_mouse_leave(self.hwnd) {
            self.tracking_mouse = true;
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
            let started_hover = !self.hovering;
            self.hovering = true;
            self.last_cursor_x = Some(x as f32);
            if started_hover {
                self.ensure_topmost(runtime);
            }
            self.ensure_animating();
        } else if self.hovering {
            self.hovering = false;
            self.ensure_animating();
        }
    }

    pub(super) fn on_mouse_leave(&mut self) {
        self.tracking_mouse = false;
        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;
        let on_primary = gx >= 0 && gx < self.screen.width && gy >= 0 && gy < self.screen.height;
        let still_over_dock = on_primary
            && !self.anim.should_hide
            && local_x >= self.layout.pill.x - 12
            && local_x <= self.layout.pill.right() + 12
            && local_y >= self.layout.pill.y - 28
            && local_y <= self.layout.pill.bottom();
        if !still_over_dock && self.hovering {
            self.hovering = false;
            self.ensure_animating();
        }
    }

    pub(super) fn drag_preview_target(&self, x: i32, y: i32) -> Option<usize> {
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

    pub(super) fn on_left_down(&mut self, x: i32, y: i32) {
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
            capture_mouse(self.hwnd);
        }
    }

    pub(super) fn on_left_up(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
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

    pub(super) fn on_drop(
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
            self.recompute_layout();
            self.present(runtime);
        }
    }

    pub(super) fn on_drop_files(&mut self, runtime: &RuntimeContext, hdrop: usize) {
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
            self.port.open_file_with(entry, file);
        }
        self.present(runtime);
    }

    pub(super) fn on_left_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        crate::menu::close(runtime);
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(x, y, gap_pad) else {
            return;
        };
        self.activate_index(runtime, idx);
    }

    pub(super) fn on_right_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
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
}
