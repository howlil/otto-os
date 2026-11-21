use otto_core::{
    COVER_FEATHER_DIP, CoverStyle, NativeCaption, ResolvedTheme, WindowControlsLayout,
    WindowControlsState, accepts_window_controls, compute_cover_layout, cover_is_dark, scale,
    window_controls_visible_under_bar,
};
use otto_platform::{
    ActiveWindowTarget, inspect_window, release_capture_if_owned, sample_caption_strip,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow};

use super::CoverState;
use super::FALLBACK_DARK_FILL;
use super::FALLBACK_LIGHT_FILL;
use super::Overlay;
use super::Placement;

impl Overlay {
    pub(super) fn bar_height_for(target: ActiveWindowTarget, bar_height_pts: i32) -> Option<i32> {
        target.on_primary.then(|| scale(bar_height_pts, target.dpi))
    }

    pub(super) fn placement(target: ActiveWindowTarget, bar_height_pts: i32) -> Placement {
        match target.caption {
            Some(caption) => {
                if window_controls_visible_under_bar(
                    caption.top,
                    caption.height,
                    Self::bar_height_for(target, bar_height_pts),
                ) {
                    Placement::Cover(caption)
                } else {
                    Placement::Hidden
                }
            }
            None => Placement::Hidden,
        }
    }

    pub(super) fn target_origin(&self) -> (f32, f32) {
        self.cover.as_ref().map_or(self.render_origin, |cover| {
            let rect = cover.caption.cover_rect();
            (rect.x as f32, rect.y as f32)
        })
    }

    pub(super) fn use_layout(&mut self, layout: WindowControlsLayout) {
        if layout != self.layout {
            self.renderer = None;
            self.layout = layout;
        }
    }

    pub(super) fn sample_bounds(
        target: ActiveWindowTarget,
        caption: NativeCaption,
        bar_height_pts: i32,
    ) -> (i32, i32) {
        let min_top = Self::bar_height_for(target, bar_height_pts).unwrap_or(caption.top);
        (target.left, min_top)
    }

    pub(super) fn sample_cover_rows(
        &self,
        target: ActiveWindowTarget,
        caption: NativeCaption,
        allow_live: bool,
    ) -> Vec<[u8; 3]> {
        let (frame_left, min_top) = Self::sample_bounds(target, caption, self.bar_height);
        if allow_live && let Some(rows) = sample_caption_strip(caption, frame_left, min_top) {
            return rows;
        }
        if let Some(existing) = self.cover.as_ref()
            && !existing.rows.is_empty()
        {
            return existing.rows.clone();
        }
        let own_settings = self
            .window
            .as_ref()
            .is_some_and(|window| window.identity.process_id == self.own_pid);
        let fallback = if own_settings {
            let color =
                otto_core::settings_background(self.appearance.theme == ResolvedTheme::Dark);
            [
                ((color >> 16) & 0xFF) as u8,
                ((color >> 8) & 0xFF) as u8,
                (color & 0xFF) as u8,
            ]
        } else {
            match self.appearance.theme {
                ResolvedTheme::Dark => FALLBACK_DARK_FILL,
                ResolvedTheme::Light => FALLBACK_LIGHT_FILL,
            }
        };
        vec![fallback; caption.height.max(1) as usize]
    }

    pub(super) fn resample_cover_fill(&mut self) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        if !target.sample_unobstructed {
            return false;
        }
        let Some(caption) = self.cover.as_ref().map(|cover| cover.caption) else {
            return false;
        };
        let rows = self.sample_cover_rows(target, caption, true);
        if self.cover.as_ref().is_some_and(|cover| cover.rows == rows) {
            return false;
        }
        self.cover = Some(CoverState { caption, rows });
        true
    }

    pub(super) fn cover_style(&self) -> Option<CoverStyle> {
        let dpi = self.target.map_or(96, |target| target.dpi);
        self.cover.as_ref().map(|cover| CoverStyle {
            rows: cover.rows.clone(),
            top_right_radius: cover.caption.corner_radius(),
            top_border_inset: cover.caption.top_border_inset,
            right_border_inset: cover.caption.rounded_top_right,
            feather_width: scale(COVER_FEATHER_DIP, dpi) as f32,
            is_dark: cover_is_dark(&cover.rows),
        })
    }

    pub(super) fn refresh_window(&mut self, target: ActiveWindowTarget, reinspect: bool) -> bool {
        if reinspect || self.window.is_none() {
            self.window = inspect_window(target.hwnd, self.own_pid);
        }
        let accepted = self.window.as_ref().is_some_and(accepts_window_controls);
        let mut state = self
            .window
            .as_ref()
            .map_or_else(WindowControlsState::default, WindowControlsState::of);
        state.active = target.active;
        self.state = state;
        accepted
    }

    pub(super) fn hide(&mut self) {
        self.stop_anim_timer();
        self.cancel_green_hover();
        self.press = None;
        self.pending_drag = None;
        let _ = release_capture_if_owned(self.hwnd);
        if self.visible {
            self.visible = false;
            self.hovered = false;
            self.alt_down = false;
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            }
        }
        self.target = None;
        self.window = None;
        self.cover = None;
        self.last_caption_click = None;
        self.renderer = None;
    }

    pub(super) fn render_origin(&self) -> (i32, i32) {
        (
            self.render_origin.0.round() as i32,
            self.render_origin.1.round() as i32,
        )
    }

    pub(super) fn sync_target(&mut self, target: Option<ActiveWindowTarget>, settle_active: bool) {
        let Some(target) = target else {
            self.hide();
            return;
        };
        let placement = Self::placement(target, self.bar_height);
        let layout = match placement {
            Placement::Hidden => None,
            Placement::Cover(caption) => compute_cover_layout(caption, &self.metrics),
        };
        let Some(layout) = layout else {
            self.hide();
            return;
        };
        self.use_layout(layout);
        let previous = self.target;
        let window_changed = previous.is_none_or(|current| current.hwnd != target.hwnd);
        let shape_changed = previous.is_none_or(|current| {
            current.maximized != target.maximized
                || current.right - current.left != target.right - target.left
                || current.bottom - current.top != target.bottom - target.top
        });
        if !self.refresh_window(target, window_changed || shape_changed) {
            self.hide();
            return;
        }
        if window_changed {
            if self.press.take().is_some() || self.pending_drag.take().is_some() {
                let _ = release_capture_if_owned(self.hwnd);
            }
            self.hovered = false;
            self.alt_down = false;
            self.cover = None;
        }
        let cover = match placement {
            Placement::Cover(caption) => Some(CoverState {
                caption,
                rows: self.sample_cover_rows(target, caption, target.sample_unobstructed),
            }),
            Placement::Hidden => None,
        };
        let cover_changed = cover != self.cover;
        self.cover = cover;
        let target_changed = previous != Some(target) || cover_changed;
        let desired_origin = self.target_origin();
        self.target = Some(target);
        let was_invisible = !self.visible;
        if was_invisible || window_changed {
            self.cancel_green_hover();
        }
        let origin_changed = self.render_origin != desired_origin;
        self.render_origin = desired_origin;
        if was_invisible {
            self.visible = true;
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNOACTIVATE);
            }
        }
        if target_changed {
            self.settle_ticks = 20;
        } else if settle_active {
            self.settle_ticks = self.settle_ticks.max(8);
        }
        if target_changed || origin_changed || was_invisible {
            self.present();
        }
        if self.settle_ticks > 0 || self.hovered {
            self.ensure_anim_timer();
        } else {
            self.stop_anim_timer();
        }
    }
}
