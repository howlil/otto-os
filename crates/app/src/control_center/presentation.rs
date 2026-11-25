use crate::host::RuntimeContext;
use otto_core::{
    capture_wanted, compute_control_center_layout_for_snapshot, control_center_focus_targets,
};
use otto_platform::{monitor_at, place};
use otto_render::{ControlCenterPaintState, ControlCenterRenderer};
use windows::core::Result;

use super::{ControlCenterSurface, DragMode};
use crate::settings::settings_snapshot;

impl ControlCenterSurface {
    pub(super) fn ensure_renderer(&mut self) -> Result<()> {
        if self.renderer.is_none() {
            self.renderer = Some(ControlCenterRenderer::new(self.monitor.dpi)?);
        }
        Ok(())
    }

    pub(super) fn sync_snapshot(&mut self, runtime: &RuntimeContext) {
        self.snapshot.status = self.port.read_system_status();
        self.snapshot.focus_active = runtime.notch_enabled.get()
            && runtime.focus_module_enabled.get()
            && runtime.focus_active.get();
        self.snapshot.media = if runtime.notch_enabled.get() && runtime.media_module_enabled.get() {
            self.port.read_now_playing()
        } else {
            None
        };
        self.snapshot.wifi = self.port.read_wifi_state();
        self.snapshot.bluetooth = self.port.read_bluetooth_state();
        self.snapshot.audio_outputs = self.port.read_audio_outputs();
        self.snapshot.settings = settings_snapshot(runtime, self.port.read_launch_at_login());
        self.snapshot.theme = crate::theme_state::appearance(runtime).theme;
    }

    pub(super) fn relayout(&mut self) {
        let page = self.snapshot.page;
        let bounds = self.monitor.bounds;
        self.layout = compute_control_center_layout_for_snapshot(
            &self.metrics,
            otto_core::ControlCenterPlacement {
                bounds,
                dpi: self.monitor.dpi,
                anchor_right: self.anchor_right,
                anchor_y: self.anchor_y,
                page,
                item_count: self.snapshot.subpanel_item_count(page),
                scroll_offset: self.scroll_offset,
            },
            &self.snapshot,
        );
        self.scroll_offset = self.layout.scroll_offset;
        if self.focus.is_active() {
            let targets = control_center_focus_targets(&self.layout, &self.snapshot);
            let _ = self.focus.revalidate(&targets);
        }
    }

    pub(super) fn place(&self) {
        place(self.hwnd, self.layout.window);
    }

    pub(super) fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = self.visible
            && self.hwnd != 0
            && capture_wanted(&appearance, runtime.glass_enabled.get());
        if !wanted {
            if let Some(renderer) = self.renderer.as_ref() {
                renderer.clear_backdrop();
            }
            return;
        }
        if force
            && let Some(renderer) = self.renderer.as_ref()
            && !renderer.has_backdrop()
        {
            self.prime_backdrop(runtime);
        }
    }

    pub(super) fn prime_backdrop(&mut self, runtime: &RuntimeContext) {
        if !capture_wanted(
            &crate::theme_state::appearance(runtime),
            runtime.glass_enabled.get(),
        ) || (runtime.capture_visible.get() && otto_platform::is_surface_visible(self.hwnd))
        {
            return;
        }
        if self.ensure_renderer().is_err() {
            return;
        }
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        if renderer.has_backdrop() {
            return;
        }
        let bounds = self.monitor.bounds;
        let Ok(src_w) = u32::try_from(bounds.width) else {
            return;
        };
        let Ok(src_h) = u32::try_from(bounds.height) else {
            return;
        };
        if let Some(pixels) =
            otto_platform::capture_screen_scaled_now((bounds.x, bounds.y), src_w, src_h, 480)
        {
            renderer.set_canvas_backdrop(pixels);
        }
    }

    fn active_drag_preview(&self) -> (Option<otto_core::Rect>, Option<u32>) {
        match self.drag {
            Some(DragMode::ResizeTile {
                tile_id,
                moved: true,
                preview_cols,
                preview_rows,
                ..
            }) => {
                let preview = self.snapshot.grid.tile(tile_id).map(|tile| {
                    let max_col = otto_core::CONTROL_GRID_COLUMNS.saturating_sub(preview_cols);
                    let col = tile.col.min(max_col);
                    self.layout
                        .grid_rect_for(col, tile.row, preview_cols, preview_rows)
                });
                (preview, None)
            }
            Some(DragMode::MoveTile {
                tile_id,
                moved: true,
                preview_col,
                preview_row,
                ..
            }) => {
                let preview = self.snapshot.grid.tile(tile_id).map(|tile| {
                    self.layout
                        .grid_rect_for(preview_col, preview_row, tile.cols, tile.rows)
                });
                (preview, Some(tile_id))
            }
            _ => (None, None),
        }
    }

    pub(super) fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible || self.hwnd == 0 {
            return;
        }
        let appearance = crate::theme_state::appearance(runtime);
        let (drag_preview, dragging_tile) = self.active_drag_preview();
        if self.ensure_renderer().is_err() {
            return;
        }
        if let Some(renderer) = self.renderer.as_mut() {
            let paint_state = ControlCenterPaintState {
                hovered: self.hovered,
                pressed: self.pressed.filter(|target| Some(*target) == self.hovered),
                focused: self.focus.current(),
                morph_progress: self.morph_progress,
                editing: self.snapshot.editing,
                drag_preview,
                dragging_tile,
            };
            crate::host::log_present_error(
                "control-center",
                renderer.present(
                    self.hwnd,
                    &self.layout,
                    &self.snapshot,
                    paint_state,
                    &appearance,
                ),
            );
        }
    }

    pub(super) fn rebuild(&mut self, runtime: &RuntimeContext) {
        if !self.visible {
            return;
        }
        let prev_dpi = self.monitor.dpi;
        self.monitor = monitor_at(self.anchor_right - 1, self.anchor_y);
        if self.monitor.dpi != prev_dpi {
            self.renderer = None;
        }
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.clear_backdrop();
        }
        self.sync_snapshot(runtime);
        self.relayout();
        self.place();
        self.prime_backdrop(runtime);
        self.present(runtime);
    }
}
