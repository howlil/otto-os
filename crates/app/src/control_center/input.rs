use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_CONTROL_CENTER_KEY;
use otto_core::{
    BarKey, CONTROL_GRID_COLUMNS, ControlCenterCommand, ControlCenterFocusOutcome,
    ControlCenterHitTarget, ControlKind, control_center_action, control_center_focus_targets,
    scale, scroll_offset_for_item, step_slider_percent,
};
use otto_platform::{
    capture_mouse, release_capture_if_owned, route_bar_keys, save_control_center_grid,
    stop_routing_if_targets,
};

use super::{ControlCenterSurface, DragMode, PostClickAction, WHEEL_SCROLL_STEP};

impl ControlCenterSurface {
    pub(super) fn toggle_edit_mode(&mut self, runtime: &RuntimeContext) {
        if self.visible && self.snapshot.page == otto_core::ControlCenterPage::Main {
            self.snapshot.editing = !self.snapshot.editing;
            self.drag = None;
            self.hovered = None;
            self.pressed = None;
            self.relayout();
            self.present(runtime);
        }
    }

    pub(super) fn enter_keyboard_focus(&mut self) -> bool {
        let targets = control_center_focus_targets(&self.layout, &self.snapshot);
        if self.focus.enter(&targets).is_some()
            && route_bar_keys(self.hwnd, WM_APP_CONTROL_CENTER_KEY)
        {
            self.ensure_focused_visible();
            true
        } else {
            self.focus.exit();
            false
        }
    }

    pub(super) fn leave_keyboard_focus(&mut self) -> bool {
        stop_routing_if_targets(self.hwnd);
        self.focus.exit()
    }

    pub(super) fn ensure_focused_visible(&mut self) {
        let Some(target) = self.focus.current() else {
            return;
        };
        if let Some(next_scroll) = scroll_offset_for_item(&self.layout, target) {
            self.scroll_offset = next_scroll;
            self.relayout();
        }
    }

    pub(super) fn begin_press(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if self.begin_drag(runtime, x, y) {
            return;
        }
        let Some(target) = self
            .layout
            .hit_test_with_editing(x, y, self.snapshot.editing)
        else {
            return;
        };
        if control_center_action(&self.snapshot, target).is_none() {
            return;
        }
        let _ = self.leave_keyboard_focus();
        self.pressed = Some(target);
        self.hovered = Some(target);
        capture_mouse(self.hwnd);
        self.present(runtime);
    }

    pub(super) fn release_press(
        &mut self,
        runtime: &RuntimeContext,
        x: i32,
        y: i32,
    ) -> PostClickAction {
        if let Some(drag) = self.drag.take() {
            let _ = release_capture_if_owned(self.hwnd);
            return self.finish_drag(runtime, drag);
        }
        let pressed = self.pressed.take();
        let _ = release_capture_if_owned(self.hwnd);
        let current = self
            .layout
            .hit_test_with_editing(x, y, self.snapshot.editing);
        if pressed.is_none() || pressed == current {
            let action = self.on_click(runtime, x, y);
            if matches!(action, PostClickAction::None) && pressed.is_some() {
                self.present(runtime);
            }
            action
        } else {
            self.hovered = current;
            self.present(runtime);
            PostClickAction::None
        }
    }

    fn finish_drag(&mut self, runtime: &RuntimeContext, drag: DragMode) -> PostClickAction {
        match drag {
            DragMode::Slider { .. } => PostClickAction::None,
            DragMode::ResizeTile {
                tile_id,
                moved,
                preview_cols,
                preview_rows,
                ..
            } => {
                if moved {
                    if self
                        .snapshot
                        .grid
                        .resize_tile(tile_id, preview_cols, preview_rows)
                    {
                        let _ = save_control_center_grid(&self.snapshot.grid);
                    }
                    self.relayout();
                    self.present(runtime);
                    PostClickAction::None
                } else {
                    PostClickAction::Command(ControlCenterCommand::CycleTileSize(tile_id), false)
                }
            }
            DragMode::MoveTile {
                tile_id,
                moved,
                preview_col,
                preview_row,
                ..
            } => {
                if moved {
                    if self
                        .snapshot
                        .grid
                        .move_tile(tile_id, preview_col, preview_row)
                    {
                        let _ = save_control_center_grid(&self.snapshot.grid);
                    }
                    self.relayout();
                    self.present(runtime);
                }
                PostClickAction::None
            }
        }
    }

    pub(super) fn begin_drag(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> bool {
        let Some(target) = self
            .layout
            .hit_test_with_editing(x, y, self.snapshot.editing)
        else {
            return false;
        };
        match target {
            ControlCenterHitTarget::TileResize(tile_id) => {
                let Some(&tile) = self.snapshot.grid.tile(tile_id) else {
                    return false;
                };
                let _ = self.leave_keyboard_focus();
                self.drag = Some(DragMode::ResizeTile {
                    tile_id,
                    start_pos: (x, y),
                    moved: false,
                    preview_cols: tile.cols,
                    preview_rows: tile.rows,
                });
                capture_mouse(self.hwnd);
                true
            }
            ControlCenterHitTarget::TileMove(tile_id) => {
                let Some(&tile) = self.snapshot.grid.tile(tile_id) else {
                    return false;
                };
                let _ = self.leave_keyboard_focus();
                self.drag = Some(DragMode::MoveTile {
                    tile_id,
                    start_pos: (x, y),
                    moved: false,
                    preview_col: tile.col,
                    preview_row: tile.row,
                });
                capture_mouse(self.hwnd);
                true
            }
            _ => {
                let is_slider = matches!(
                    target,
                    ControlCenterHitTarget::DisplaySlider
                        if self.snapshot.status.brightness.is_some()
                ) || target == ControlCenterHitTarget::SoundSlider
                    || matches!(target, ControlCenterHitTarget::TileSlider(id) if self.snapshot.grid.tile(id).is_some());
                if !is_slider {
                    return false;
                }
                let _ = self.leave_keyboard_focus();
                self.drag = Some(DragMode::Slider { target });
                capture_mouse(self.hwnd);
                self.apply_slider_point(runtime, target, x, y);
                true
            }
        }
    }

    pub(super) fn commit_slider_value(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        value: u8,
    ) {
        let resolved = match target {
            ControlCenterHitTarget::TileSlider(id) => {
                match self.snapshot.grid.tile(id).map(|t| t.kind) {
                    Some(ControlKind::Brightness) => ControlCenterHitTarget::DisplaySlider,
                    Some(ControlKind::Volume) => ControlCenterHitTarget::SoundSlider,
                    _ => return,
                }
            }
            other => other,
        };
        match resolved {
            ControlCenterHitTarget::DisplaySlider => {
                if let Some(pct) = self.port.set_brightness(value) {
                    self.snapshot.status.brightness = Some(pct);
                }
            }
            ControlCenterHitTarget::SoundSlider => {
                if let Some((vol, muted)) = self.port.set_volume(value) {
                    self.snapshot.status.volume = vol;
                    self.snapshot.status.muted = muted;
                }
            }
            _ => return,
        }
        self.present(runtime);
    }

    pub(super) fn apply_slider_point(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        x: i32,
        y: i32,
    ) {
        let Some(value) = self.layout.slider_value_at_point(target, x, y) else {
            return;
        };
        self.commit_slider_value(runtime, target, value);
    }

    pub(super) fn step_slider(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        delta: i8,
    ) {
        let current = match target {
            ControlCenterHitTarget::DisplaySlider => match self.snapshot.status.brightness {
                Some(pct) => pct,
                None => return,
            },
            ControlCenterHitTarget::SoundSlider => {
                if self.snapshot.status.muted {
                    0
                } else {
                    self.snapshot.status.volume
                }
            }
            _ => return,
        };
        let next = step_slider_percent(current, delta);
        self.commit_slider_value(runtime, target, next);
    }

    pub(super) fn on_mouse_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if let Some(drag) = self.drag {
            match drag {
                DragMode::Slider { target } => {
                    self.apply_slider_point(runtime, target, x, y);
                }
                DragMode::ResizeTile {
                    tile_id,
                    start_pos,
                    mut moved,
                    mut preview_cols,
                    mut preview_rows,
                } => {
                    if !moved && ((x - start_pos.0).abs() > 4 || (y - start_pos.1).abs() > 4) {
                        moved = true;
                    }
                    if let Some((cols, rows)) = self.layout.grid_resize_span(tile_id, x, y) {
                        preview_cols = cols;
                        preview_rows = rows;
                    }
                    self.drag = Some(DragMode::ResizeTile {
                        tile_id,
                        start_pos,
                        moved,
                        preview_cols,
                        preview_rows,
                    });
                    self.present(runtime);
                }
                DragMode::MoveTile {
                    tile_id,
                    start_pos,
                    mut moved,
                    ..
                } => {
                    if !moved && ((x - start_pos.0).abs() > 6 || (y - start_pos.1).abs() > 6) {
                        moved = true;
                    }
                    let (raw_col, raw_row) = self.layout.grid_cell_at(x, y);
                    let cols = self.snapshot.grid.tile(tile_id).map_or(1, |t| t.cols);
                    let preview_col = raw_col.min(CONTROL_GRID_COLUMNS.saturating_sub(cols));
                    self.drag = Some(DragMode::MoveTile {
                        tile_id,
                        start_pos,
                        moved,
                        preview_col,
                        preview_row: raw_row,
                    });
                    self.present(runtime);
                }
            }
            return;
        }
        let next = self
            .layout
            .hit_test_with_editing(x, y, self.snapshot.editing);
        let cleared_focus = next.is_some() && self.leave_keyboard_focus();
        let pressed_active_changed = self
            .pressed
            .is_some_and(|target| (self.hovered == Some(target)) != (next == Some(target)));
        if next != self.hovered || cleared_focus || pressed_active_changed {
            self.hovered = next;
            self.present(runtime);
        }
    }

    pub(super) fn on_wheel(&mut self, runtime: &RuntimeContext, delta: i16) {
        if !self.visible || self.layout.max_scroll <= 0 {
            return;
        }
        let cleared_focus = self.leave_keyboard_focus();
        let step = if delta > 0 {
            -scale(WHEEL_SCROLL_STEP, self.monitor.dpi)
        } else {
            scale(WHEEL_SCROLL_STEP, self.monitor.dpi)
        };
        let next = (self.scroll_offset + step).clamp(0, self.layout.max_scroll);
        if next != self.scroll_offset || cleared_focus {
            self.scroll_offset = next;
            self.relayout();
            self.present(runtime);
        }
    }

    pub(super) fn on_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> PostClickAction {
        if self.leave_keyboard_focus() {
            self.present(runtime);
        }
        let Some(target) = self
            .layout
            .hit_test_with_editing(x, y, self.snapshot.editing)
        else {
            return PostClickAction::None;
        };
        let Some(outcome) = control_center_action(&self.snapshot, target) else {
            return PostClickAction::None;
        };
        PostClickAction::Command(outcome.command, outcome.dismiss)
    }

    pub(super) fn on_key(&mut self, runtime: &RuntimeContext, key: BarKey) -> PostClickAction {
        if !self.visible {
            let _ = self.leave_keyboard_focus();
            return PostClickAction::None;
        }
        let targets = control_center_focus_targets(&self.layout, &self.snapshot);
        match self.focus.handle(key, &targets) {
            ControlCenterFocusOutcome::Moved(_) => {
                self.ensure_focused_visible();
                self.present(runtime);
                PostClickAction::None
            }
            ControlCenterFocusOutcome::Exited => PostClickAction::Close,
            ControlCenterFocusOutcome::AdjustSlider { target, delta } => {
                self.step_slider(runtime, target, delta);
                PostClickAction::None
            }
            ControlCenterFocusOutcome::Activate(target) => {
                let Some(outcome) = control_center_action(&self.snapshot, target) else {
                    return PostClickAction::None;
                };
                PostClickAction::Command(outcome.command, outcome.dismiss)
            }
            ControlCenterFocusOutcome::Unchanged => PostClickAction::None,
        }
    }
}
