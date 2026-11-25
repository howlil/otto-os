use crate::layout::Rect;

use super::buffer::RgbBuffer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassShape {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub radius: f32,
}

impl GlassShape {
    #[must_use]
    pub fn from_rect(rect: Rect, radius: f32) -> Self {
        let half_short_side = rect.width.min(rect.height).max(0) as f32 * 0.5;
        Self {
            left: rect.x as f32,
            top: rect.y as f32,
            right: rect.right() as f32,
            bottom: rect.bottom() as f32,
            radius: radius.clamp(0.0, half_short_side),
        }
    }

    #[must_use]
    pub fn distance(&self, x: f32, y: f32) -> f32 {
        let center_x = (self.left + self.right) * 0.5;
        let center_y = (self.top + self.bottom) * 0.5;
        let reach_x = (self.right - self.left) * 0.5 - self.radius;
        let reach_y = (self.bottom - self.top) * 0.5 - self.radius;
        let offset_x = (x - center_x).abs() - reach_x;
        let offset_y = (y - center_y).abs() - reach_y;
        let ox = offset_x.max(0.0);
        let oy = offset_y.max(0.0);
        let outside = if ox > 0.0 && oy > 0.0 {
            ox.hypot(oy)
        } else {
            ox + oy
        };
        outside + offset_x.max(offset_y).min(0.0) - self.radius
    }

    #[must_use]
    pub fn normal(&self, x: f32, y: f32) -> (f32, f32) {
        let gradient_x = self.distance(x + 1.0, y) - self.distance(x - 1.0, y);
        let gradient_y = self.distance(x, y + 1.0) - self.distance(x, y - 1.0);
        let length = gradient_x.hypot(gradient_y);
        if length < 1e-4 {
            (0.0, 0.0)
        } else {
            (gradient_x / length, gradient_y / length)
        }
    }

    #[must_use]
    pub fn coverage(&self, x: f32, y: f32) -> f32 {
        (0.5 - self.distance(x, y)).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn pixel_bounds(&self) -> Rect {
        let left = self.left.floor() as i32 - 1;
        let top = self.top.floor() as i32 - 1;
        let right = self.right.ceil() as i32 + 1;
        let bottom = self.bottom.ceil() as i32 + 1;
        Rect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlassBackdrop<'a> {
    pub sharp: &'a RgbBuffer,
    pub soft: &'a RgbBuffer,
    pub soft_factor: usize,
    pub origin: (f32, f32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlassBitmap {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
