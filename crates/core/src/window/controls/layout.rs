use crate::layout::scale;

use super::cover::RESIZE_BORDER_DIP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlAction {
    ToggleMaximize,
    Minimize,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Top,
    TopRight,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowControlsMetrics {
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub button_radius: i32,
    pub button_spacing: i32,
    pub left_margin: i32,
    pub hit_slop: i32,
}

impl WindowControlsMetrics {
    pub const MACOS_SONOMA: Self = Self {
        width: 72,
        height: 28,
        offset_x: 10,
        offset_y: 6,
        button_radius: 6,
        button_spacing: 20,
        left_margin: 12,
        hit_slop: 4,
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
    pub offset_x: i32,
    pub offset_y: i32,
    pub radius: f32,
    pub hit_radius: f32,
    pub center_y: f32,
    pub centers_x: [f32; 3],
    pub actions: [WindowControlAction; 3],
    pub slots: usize,
    pub cell_width: f32,
}

impl WindowControlsLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        if self.cell_width > 0.0 {
            return self.cell_hit_test(x, y);
        }
        let (fx, fy) = (x as f32, y as f32);
        let hit_r = self.hit_radius;
        let dy = fy - self.center_y;
        if dy.abs() > hit_r {
            return None;
        }
        for (cx, action) in self
            .centers_x
            .into_iter()
            .zip(self.actions)
            .take(self.slots)
        {
            let dx = fx - cx;
            if dx * dx + dy * dy <= hit_r * hit_r {
                return Some(action);
            }
        }
        None
    }

    pub(super) fn cell_hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        if x < 0 || y < 0 || y >= self.height {
            return None;
        }
        let slot = (x as f32 / self.cell_width) as usize;
        (slot < self.slots).then(|| self.actions[slot])
    }

    #[must_use]
    pub fn hits_group(&self, x: i32, y: i32) -> bool {
        if self.slots == 0 || x < 0 || y < 0 || x >= self.width || y >= self.height {
            return false;
        }
        let (fx, fy) = (x as f32, y as f32);
        let hit_r = self.hit_radius;
        if (fy - self.center_y).abs() > hit_r {
            return false;
        }
        let first = self.centers_x[0];
        let last = self.centers_x[self.slots - 1];
        let min_x = first.min(last) - hit_r;
        let max_x = first.max(last) + hit_r;
        fx >= min_x && fx <= max_x
    }

    #[must_use]
    pub fn center_of(&self, action: WindowControlAction) -> Option<f32> {
        self.actions
            .into_iter()
            .zip(self.centers_x)
            .take(self.slots)
            .find_map(|(candidate, center)| (candidate == action).then_some(center))
    }

    #[must_use]
    pub fn resize_edge(
        &self,
        x: i32,
        y: i32,
        maximized: bool,
        resizable: bool,
        dpi: u32,
    ) -> Option<ResizeEdge> {
        if maximized || !resizable || x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        if self.hit_test(x, y).is_some() {
            return None;
        }
        let edge = scale(RESIZE_BORDER_DIP, dpi).max(2);
        let on_top = y < edge;
        let on_right = x >= self.width - edge;
        match (on_top, on_right) {
            (true, true) => Some(ResizeEdge::TopRight),
            (true, false) => Some(ResizeEdge::Top),
            (false, true) => Some(ResizeEdge::Right),
            (false, false) => None,
        }
    }
}

pub(super) const COVER_ACTIONS: [WindowControlAction; 3] = [
    WindowControlAction::Minimize,
    WindowControlAction::ToggleMaximize,
    WindowControlAction::Close,
];

impl Default for WindowControlsLayout {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            offset_x: 0,
            offset_y: 0,
            radius: 0.0,
            hit_radius: 0.0,
            center_y: 0.0,
            centers_x: [0.0; 3],
            actions: COVER_ACTIONS,
            slots: 0,
            cell_width: 0.0,
        }
    }
}
