use crate::layout::scale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlAction {
    ToggleMaximize,
    Minimize,
    Close,
}

pub type TrafficLightAction = WindowControlAction;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowControlsMetrics {
    pub width: i32,
    pub height: i32,
    pub offset_right: i32,
    pub offset_y: i32,
    pub button_radius: i32,
    pub button_spacing: i32,
    pub right_margin: i32,
}

pub type TrafficLightMetrics = WindowControlsMetrics;

impl WindowControlsMetrics {
    pub const MACOS_SONOMA: Self = Self {
        width: 138,
        height: 32,
        offset_right: 1,
        offset_y: 1,
        button_radius: 6,
        button_spacing: 22,
        right_margin: 22,
    };
}

impl Default for WindowControlsMetrics {
    fn default() -> Self {
        Self::MACOS_SONOMA
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowControlsLayout {
    pub width: i32,
    pub height: i32,
    pub offset_right: i32,
    pub offset_y: i32,
    pub radius: f32,
    pub center_y: f32,
    pub centers_x: [f32; 3],
}

pub type TrafficLightLayout = WindowControlsLayout;

impl WindowControlsLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        let (fx, fy) = (x as f32, y as f32);
        let hit_r = self.radius + 4.0;
        let dy = fy - self.center_y;
        if dy.abs() > hit_r {
            return None;
        }
        let actions = [
            WindowControlAction::ToggleMaximize,
            WindowControlAction::Minimize,
            WindowControlAction::Close,
        ];
        for (cx, action) in self.centers_x.into_iter().zip(actions) {
            let dx = fx - cx;
            if dx * dx + dy * dy <= hit_r * hit_r {
                return Some(action);
            }
        }
        None
    }
}

#[must_use]
pub fn compute_window_controls_layout(
    metrics: &WindowControlsMetrics,
    dpi: u32,
) -> WindowControlsLayout {
    let width = scale(metrics.width, dpi);
    let height = scale(metrics.height, dpi);
    let offset_right = scale(metrics.offset_right, dpi);
    let offset_y = scale(metrics.offset_y, dpi);
    let radius = scale(metrics.button_radius, dpi) as f32;
    let spacing = scale(metrics.button_spacing, dpi) as f32;
    let right_margin = scale(metrics.right_margin, dpi) as f32;
    let close_x = width as f32 - right_margin;
    let min_x = close_x - spacing;
    let max_x = min_x - spacing;
    let center_y = height as f32 * 0.5;

    WindowControlsLayout {
        width,
        height,
        offset_right,
        offset_y,
        radius,
        center_y,
        centers_x: [max_x, min_x, close_x],
    }
}

#[must_use]
pub fn compute_traffic_light_layout(metrics: &TrafficLightMetrics, dpi: u32) -> TrafficLightLayout {
    compute_window_controls_layout(metrics, dpi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_controls_hit_test_identifies_green_yellow_red_on_right() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 96);
        let cy = layout.center_y as i32;
        assert_eq!(
            layout.hit_test(layout.centers_x[0] as i32, cy),
            Some(WindowControlAction::ToggleMaximize)
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[1] as i32, cy),
            Some(WindowControlAction::Minimize)
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[2] as i32, cy),
            Some(WindowControlAction::Close)
        );
        assert!(layout.centers_x[2] > layout.centers_x[1]);
        assert!(layout.centers_x[1] > layout.centers_x[0]);
        assert_eq!(layout.hit_test(2, 2), None);
    }
}
