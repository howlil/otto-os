use crate::layout::{Rect, scale};

use super::cover::COVER_RIGHT_MARGIN_DIP;
use super::cover::COVER_SAMPLE_GAP_DIP;
use super::cover::COVER_SAMPLE_WIDTH_DIP;
use super::cover::NATIVE_CAPTION_BUTTON_DIP;
use super::cover::NATIVE_CAPTION_HEIGHT_DIP;
use super::cover::NATIVE_CORNER_RADIUS_DIP;
use super::layout::COVER_ACTIONS;
use super::layout::WindowControlAction;
use super::layout::WindowControlsLayout;
use super::layout::WindowControlsMetrics;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCaptionStyle {
    pub has_caption: bool,
    pub has_system_menu: bool,
    pub can_minimize: bool,
    pub can_maximize: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCaption {
    pub frame_right: i32,
    pub top: i32,
    pub height: i32,
    pub slots: u8,
    pub can_minimize: bool,
    pub rounded_top_right: bool,
    pub top_border_inset: bool,
    pub dpi: u32,
    pub measured_width: i32,
}

impl NativeCaption {
    #[must_use]
    pub const fn new(
        style: NativeCaptionStyle,
        frame_right: i32,
        top: i32,
        height: i32,
        rounded_top_right: bool,
        dpi: u32,
    ) -> Option<Self> {
        if !style.has_caption || !style.has_system_menu || height <= 0 {
            return None;
        }
        let slots = if style.can_minimize || style.can_maximize {
            3
        } else {
            1
        };
        Some(Self {
            frame_right,
            top,
            height,
            slots,
            can_minimize: style.can_minimize,
            rounded_top_right,
            top_border_inset: rounded_top_right,
            dpi,
            measured_width: 0,
        })
    }

    #[must_use]
    pub const fn with_top_border_inset(self, top_border_inset: bool) -> Self {
        Self {
            top_border_inset,
            ..self
        }
    }

    #[must_use]
    pub fn standard(
        style: NativeCaptionStyle,
        frame_right: i32,
        top: i32,
        rounded_top_right: bool,
        dpi: u32,
    ) -> Option<Self> {
        Self::new(
            style,
            frame_right,
            top,
            scale(NATIVE_CAPTION_HEIGHT_DIP, dpi),
            rounded_top_right,
            dpi,
        )
    }

    #[must_use]
    pub fn measured(self, width: i32) -> Self {
        let standard = self.standard_width();
        let plausible = width * 4 >= standard * 3 && width * 4 <= standard * 5;
        Self {
            measured_width: if plausible { width } else { 0 },
            ..self
        }
    }

    pub(super) fn standard_width(&self) -> i32 {
        scale(NATIVE_CAPTION_BUTTON_DIP, self.dpi) * i32::from(self.slots)
    }

    pub(super) fn button_span(&self) -> i32 {
        if self.measured_width > 0 {
            self.measured_width
        } else {
            self.standard_width()
        }
    }

    #[must_use]
    pub fn cover_rect(&self) -> Rect {
        let width = self.button_span();
        Rect {
            x: self.frame_right - width,
            y: self.top,
            width,
            height: self.height,
        }
    }

    #[must_use]
    pub fn minimize_button(&self) -> Option<Rect> {
        if self.slots != 3 || !self.can_minimize {
            return None;
        }
        let cover = self.cover_rect();
        Some(Rect {
            width: cover.width / i32::from(self.slots),
            ..cover
        })
    }

    #[must_use]
    pub fn sample_rect(&self) -> Rect {
        let cover = self.cover_rect();
        let gap = scale(COVER_SAMPLE_GAP_DIP, self.dpi).max(1);
        let width = scale(COVER_SAMPLE_WIDTH_DIP, self.dpi).max(1);
        Rect {
            x: cover.x - gap - width,
            y: cover.y,
            width,
            height: cover.height,
        }
    }

    #[must_use]
    pub fn clipped_sample_rect(&self, frame_left: i32, min_top: i32) -> Rect {
        let base = self.sample_rect();
        let min_left = frame_left + scale(COVER_SAMPLE_GAP_DIP, self.dpi).max(1);
        let left = base.x.max(min_left);
        let right = base.right();
        let top = base.y.max(min_top);
        let bottom = base.bottom();
        Rect {
            x: left,
            y: top,
            width: (right - left).max(0),
            height: (bottom - top).max(0),
        }
    }

    #[must_use]
    pub fn corner_radius(&self) -> f32 {
        if self.rounded_top_right {
            scale(NATIVE_CORNER_RADIUS_DIP, self.dpi) as f32
        } else {
            0.0
        }
    }
}

#[must_use]
pub fn compute_cover_layout(
    caption: NativeCaption,
    metrics: &WindowControlsMetrics,
) -> Option<WindowControlsLayout> {
    let dpi = caption.dpi;
    let cover = caption.cover_rect();
    if cover.is_empty() {
        return None;
    }
    let slots = usize::from(caption.slots);
    let radius = scale(metrics.button_radius, dpi) as f32;
    let spacing = scale(metrics.button_spacing, dpi) as f32;
    let hit_radius = radius + scale(metrics.hit_slop, dpi) as f32;
    let right_margin = scale(COVER_RIGHT_MARGIN_DIP, dpi) as f32;
    let close_x = (cover.width as f32 - right_margin).max(radius);
    let (centers_x, actions) = if slots == 1 {
        ([close_x, 0.0, 0.0], [WindowControlAction::Close; 3])
    } else {
        let zoom_x = close_x - spacing;
        let min_x = zoom_x - spacing;
        ([min_x, zoom_x, close_x], COVER_ACTIONS)
    };
    Some(WindowControlsLayout {
        width: cover.width,
        height: cover.height,
        offset_x: 0,
        offset_y: 0,
        radius,
        hit_radius,
        center_y: cover.height as f32 * 0.5,
        centers_x,
        actions,
        slots,
        cell_width: 0.0,
    })
}
