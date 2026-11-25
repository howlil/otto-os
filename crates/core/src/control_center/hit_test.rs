use crate::layout::Rect;

use super::grid::{CONTROL_GRID_COLUMNS, CONTROL_GRID_MAX_ROWS, ControlKind};
use super::layout_types::{ControlCenterHitTarget, ControlCenterLayout, ControlTileLayout};
use super::model::ControlCenterPage;

impl ControlCenterLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<ControlCenterHitTarget> {
        self.hit_test_with_editing(x, y, false)
    }

    #[must_use]
    pub fn hit_test_with_editing(
        &self,
        x: i32,
        y: i32,
        editing: bool,
    ) -> Option<ControlCenterHitTarget> {
        if self.page == ControlCenterPage::Main {
            return self.hit_test_main(x, y, editing);
        }
        if self.back_button.contains(x, y) {
            return Some(ControlCenterHitTarget::BackButton);
        }
        if self.header_toggle.contains(x, y) {
            return Some(ControlCenterHitTarget::HeaderToggle);
        }
        if self.page == ControlCenterPage::Display && self.display_slider.contains(x, y) {
            return Some(ControlCenterHitTarget::DisplaySlider);
        }
        if self.page == ControlCenterPage::Sound {
            if self.sound_mute.contains(x, y) {
                return Some(ControlCenterHitTarget::SoundMuteToggle);
            }
            if self.sound_slider.contains(x, y) {
                return Some(ControlCenterHitTarget::SoundSlider);
            }
        }
        if self.list_viewport.contains(x, y) {
            for (index, rect) in self.items.iter().enumerate() {
                if rect.contains(x, y) {
                    let target = if self.page == ControlCenterPage::Gallery {
                        ControlCenterHitTarget::GalleryItem(index)
                    } else {
                        ControlCenterHitTarget::SubPanelItem(index)
                    };
                    return Some(target);
                }
            }
        }
        if self.footer_primary.contains(x, y) {
            return Some(ControlCenterHitTarget::FooterPrimary);
        }
        if self.footer_secondary.contains(x, y) {
            return Some(ControlCenterHitTarget::FooterSecondary);
        }
        if !self.canvas_panel.contains(x, y) && self.window.contains(x, y) {
            return Some(ControlCenterHitTarget::CanvasBackground);
        }
        None
    }

    pub(super) fn hit_test_main(
        &self,
        x: i32,
        y: i32,
        editing: bool,
    ) -> Option<ControlCenterHitTarget> {
        if self.toolbar_add.contains(x, y) {
            return Some(ControlCenterHitTarget::ToolbarAdd);
        }
        if self.toolbar_edit.contains(x, y) {
            return Some(ControlCenterHitTarget::ToolbarEdit);
        }
        if editing && self.toolbar_reset.contains(x, y) {
            return Some(ControlCenterHitTarget::ToolbarReset);
        }
        if editing && self.add_control_button.contains(x, y) {
            return Some(ControlCenterHitTarget::AddControlButton);
        }
        if editing {
            for tile in self.tiles.iter().rev() {
                if tile.remove_badge.contains(x, y) {
                    return Some(ControlCenterHitTarget::TileRemove(tile.id));
                }
            }
            for tile in self.tiles.iter().rev() {
                if tile.resize_handle.contains(x, y) {
                    return Some(ControlCenterHitTarget::TileResize(tile.id));
                }
                if tile.rect.contains(x, y) {
                    return Some(ControlCenterHitTarget::TileMove(tile.id));
                }
            }
        } else {
            for tile in self.tiles.iter().rev() {
                if let Some(hit) = hit_test_tile(tile, x, y) {
                    return Some(hit);
                }
            }
        }
        self.window
            .contains(x, y)
            .then_some(ControlCenterHitTarget::CanvasBackground)
    }

    #[must_use]
    pub fn slider_rect(&self, target: ControlCenterHitTarget) -> Option<Rect> {
        match target {
            ControlCenterHitTarget::DisplaySlider if !self.display_slider.is_empty() => {
                Some(self.display_slider)
            }
            ControlCenterHitTarget::SoundSlider if !self.sound_slider.is_empty() => {
                Some(self.sound_slider)
            }
            ControlCenterHitTarget::TileSlider(id) => self
                .tiles
                .iter()
                .find(|tile| tile.id == id && !tile.slider_rect.is_empty())
                .map(|tile| tile.slider_rect),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_slider_vertical(&self, target: ControlCenterHitTarget) -> bool {
        match target {
            ControlCenterHitTarget::DisplaySlider => self
                .tiles
                .iter()
                .any(|tile| tile.kind == ControlKind::Brightness && tile.slider_vertical),
            ControlCenterHitTarget::SoundSlider => self
                .tiles
                .iter()
                .any(|tile| tile.kind == ControlKind::Volume && tile.slider_vertical),
            ControlCenterHitTarget::TileSlider(id) => self
                .tiles
                .iter()
                .any(|tile| tile.id == id && tile.slider_vertical),
            _ => false,
        }
    }

    #[must_use]
    pub fn slider_value_at(&self, target: ControlCenterHitTarget, x: i32) -> Option<u8> {
        let rect = self.slider_rect(target)?;
        if x <= rect.x {
            return Some(0);
        }
        if x >= rect.right() {
            return Some(100);
        }
        let rel = i64::from(x - rect.x);
        let width = i64::from(rect.width.max(1));
        let percent = ((rel * 100 + width / 2) / width).clamp(0, 100);
        u8::try_from(percent).ok()
    }

    #[must_use]
    pub fn slider_value_at_point(
        &self,
        target: ControlCenterHitTarget,
        x: i32,
        y: i32,
    ) -> Option<u8> {
        if self.page == ControlCenterPage::Main && self.is_slider_vertical(target) {
            let rect = self.slider_rect(target)?;
            if y >= rect.bottom() {
                return Some(0);
            }
            if y <= rect.y {
                return Some(100);
            }
            let rel = i64::from(rect.bottom() - y);
            let height = i64::from(rect.height.max(1));
            let percent = ((rel * 100 + height / 2) / height).clamp(0, 100);
            return u8::try_from(percent).ok();
        }
        self.slider_value_at(target, x)
    }

    #[must_use]
    pub fn grid_cell_at(&self, x: i32, y: i32) -> (u8, u8) {
        let step = (self.cell_px + self.cell_gap_px).max(1);
        let rel_x = (x - self.grid_origin.0).max(0);
        let rel_y = (y - self.grid_origin.1).max(0);
        let col = (rel_x / step).clamp(0, i32::from(CONTROL_GRID_COLUMNS - 1)) as u8;
        let row = (rel_y / step).clamp(0, i32::from(CONTROL_GRID_MAX_ROWS - 1)) as u8;
        (col, row)
    }

    #[must_use]
    pub fn grid_resize_span(&self, tile_id: u32, x: i32, y: i32) -> Option<(u8, u8)> {
        let tile = self.tiles.iter().find(|tile| tile.id == tile_id)?;
        let step = (self.cell_px + self.cell_gap_px).max(1);
        let rel_w = (x - tile.rect.x + step / 3).max(self.cell_px);
        let rel_h = (y - tile.rect.y + step / 3).max(self.cell_px);
        let raw_cols =
            ((rel_w + self.cell_gap_px) / step).clamp(1, i32::from(CONTROL_GRID_COLUMNS)) as u8;
        let raw_rows = ((rel_h + self.cell_gap_px) / step).clamp(1, 2) as u8;
        let snapped = tile.kind.snap_size(raw_cols, raw_rows);
        Some((snapped.cols, snapped.rows))
    }

    #[must_use]
    pub fn grid_rect_for(&self, col: u8, row: u8, cols: u8, rows: u8) -> Rect {
        Rect {
            x: self.grid_origin.0 + i32::from(col) * (self.cell_px + self.cell_gap_px),
            y: self.grid_origin.1 + i32::from(row) * (self.cell_px + self.cell_gap_px),
            width: i32::from(cols) * self.cell_px
                + i32::from(cols.saturating_sub(1)) * self.cell_gap_px,
            height: i32::from(rows) * self.cell_px
                + i32::from(rows.saturating_sub(1)) * self.cell_gap_px,
        }
    }

    #[must_use]
    pub fn target_rect(&self, target: ControlCenterHitTarget) -> Option<Rect> {
        let rect = match target {
            ControlCenterHitTarget::CanvasBackground => self.window,
            ControlCenterHitTarget::ToolbarAdd => self.toolbar_add,
            ControlCenterHitTarget::ToolbarEdit => self.toolbar_edit,
            ControlCenterHitTarget::ToolbarReset => self.toolbar_reset,
            ControlCenterHitTarget::AddControlButton => self.add_control_button,
            ControlCenterHitTarget::TileRemove(id) => self
                .tiles
                .iter()
                .find(|tile| tile.id == id)
                .map(|tile| tile.remove_badge)
                .unwrap_or_default(),
            ControlCenterHitTarget::TileResize(id) => self
                .tiles
                .iter()
                .find(|tile| tile.id == id)
                .map(|tile| tile.resize_handle)
                .unwrap_or_default(),
            ControlCenterHitTarget::TileMove(id) | ControlCenterHitTarget::TileAction(id) => self
                .tiles
                .iter()
                .find(|tile| tile.id == id)
                .map(|tile| tile.rect)
                .unwrap_or_default(),
            ControlCenterHitTarget::TileSlider(id) => self
                .tiles
                .iter()
                .find(|tile| tile.id == id)
                .map(|tile| tile.slider_rect)
                .unwrap_or_default(),
            ControlCenterHitTarget::TileDetail(id) => self
                .tiles
                .iter()
                .find(|tile| tile.id == id)
                .map(|tile| tile.detail_rect)
                .unwrap_or_default(),
            ControlCenterHitTarget::GalleryItem(index)
            | ControlCenterHitTarget::SubPanelItem(index) => {
                let item = *self.items.get(index)?;
                return self.list_viewport.intersection(&item);
            }
            ControlCenterHitTarget::WifiToggle => self.wifi_toggle,
            ControlCenterHitTarget::WifiOpenPage => self.wifi_row,
            ControlCenterHitTarget::BluetoothToggle => self.bluetooth_toggle,
            ControlCenterHitTarget::BluetoothOpenPage => self.bluetooth_row,
            ControlCenterHitTarget::FocusToggle => self.focus_toggle,
            ControlCenterHitTarget::FocusOpenSettings => self.focus_settings,
            ControlCenterHitTarget::BatteryTile => self.battery_tile,
            ControlCenterHitTarget::DisplayOpenPage => self.display_header,
            ControlCenterHitTarget::DisplaySlider => self.display_slider,
            ControlCenterHitTarget::SoundOpenPage => self.sound_header,
            ControlCenterHitTarget::SoundMuteToggle => self.sound_mute,
            ControlCenterHitTarget::SoundSlider => self.sound_slider,
            ControlCenterHitTarget::MediaBody => self.media_body,
            ControlCenterHitTarget::MediaPrev => self.media_prev,
            ControlCenterHitTarget::MediaPlayPause => self.media_play_pause,
            ControlCenterHitTarget::MediaNext => self.media_next,
            ControlCenterHitTarget::BackButton => self.back_button,
            ControlCenterHitTarget::HeaderToggle => self.header_toggle,
            ControlCenterHitTarget::FooterPrimary => self.footer_primary,
            ControlCenterHitTarget::FooterSecondary => self.footer_secondary,
        };
        (!rect.is_empty()).then_some(rect)
    }
}

fn hit_test_tile(tile: &ControlTileLayout, x: i32, y: i32) -> Option<ControlCenterHitTarget> {
    if !tile.rect.contains(x, y) {
        return None;
    }
    match tile.kind {
        ControlKind::Connectivity => {
            if tile.cols == 1 && tile.rows == 1 {
                return Some(ControlCenterHitTarget::WifiOpenPage);
            }
            if tile.rows == 1 {
                if tile.primary_rect.contains(x, y) {
                    return Some(ControlCenterHitTarget::WifiToggle);
                }
                if tile.tertiary_rect.contains(x, y) {
                    return Some(ControlCenterHitTarget::BluetoothToggle);
                }
                return Some(ControlCenterHitTarget::WifiOpenPage);
            }
            if tile.primary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::WifiToggle)
            } else if tile.secondary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::WifiOpenPage)
            } else if tile.tertiary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::BluetoothToggle)
            } else if tile.detail_rect.contains(x, y) {
                Some(ControlCenterHitTarget::BluetoothOpenPage)
            } else {
                Some(ControlCenterHitTarget::WifiOpenPage)
            }
        }
        ControlKind::Wifi => {
            if tile.cols >= 2 && tile.primary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::WifiToggle)
            } else if tile.cols >= 2 {
                Some(ControlCenterHitTarget::WifiOpenPage)
            } else {
                Some(ControlCenterHitTarget::WifiToggle)
            }
        }
        ControlKind::Bluetooth => {
            if tile.cols >= 2 && tile.primary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::BluetoothToggle)
            } else if tile.cols >= 2 {
                Some(ControlCenterHitTarget::BluetoothOpenPage)
            } else {
                Some(ControlCenterHitTarget::BluetoothToggle)
            }
        }
        ControlKind::Focus => {
            if tile.detail_rect.contains(x, y) {
                Some(ControlCenterHitTarget::FocusOpenSettings)
            } else {
                Some(ControlCenterHitTarget::FocusToggle)
            }
        }
        ControlKind::Battery => Some(ControlCenterHitTarget::BatteryTile),
        ControlKind::Brightness => {
            if tile.detail_rect.contains(x, y) {
                Some(ControlCenterHitTarget::DisplayOpenPage)
            } else if tile.slider_rect.contains(x, y) {
                Some(ControlCenterHitTarget::DisplaySlider)
            } else {
                Some(ControlCenterHitTarget::DisplayOpenPage)
            }
        }
        ControlKind::Volume => {
            if tile.detail_rect.contains(x, y) {
                Some(ControlCenterHitTarget::SoundOpenPage)
            } else if tile.primary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::SoundMuteToggle)
            } else if tile.slider_rect.contains(x, y) {
                Some(ControlCenterHitTarget::SoundSlider)
            } else {
                Some(ControlCenterHitTarget::SoundOpenPage)
            }
        }
        ControlKind::NowPlaying => {
            if tile.secondary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::MediaPrev)
            } else if tile.tertiary_rect.contains(x, y) {
                Some(ControlCenterHitTarget::MediaPlayPause)
            } else if tile.detail_rect.contains(x, y) {
                Some(ControlCenterHitTarget::MediaNext)
            } else {
                Some(ControlCenterHitTarget::MediaBody)
            }
        }
        _ => Some(ControlCenterHitTarget::TileAction(tile.id)),
    }
}
