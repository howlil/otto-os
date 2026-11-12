use crate::app_model::{WindowCommand, WindowInfo, WindowPresence};
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
    pub offset_x: i32,
    pub offset_y: i32,
    pub button_radius: i32,
    pub button_spacing: i32,
    pub left_margin: i32,
    pub hit_slop: i32,
}

pub type TrafficLightMetrics = WindowControlsMetrics;

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
}

pub type TrafficLightLayout = WindowControlsLayout;

impl WindowControlsLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        let (fx, fy) = (x as f32, y as f32);
        let hit_r = self.hit_radius;
        let dy = fy - self.center_y;
        if dy.abs() > hit_r {
            return None;
        }
        let actions = [
            WindowControlAction::Close,
            WindowControlAction::Minimize,
            WindowControlAction::ToggleMaximize,
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
    let offset_x = scale(metrics.offset_x, dpi);
    let offset_y = scale(metrics.offset_y, dpi);
    let radius = scale(metrics.button_radius, dpi) as f32;
    let spacing = scale(metrics.button_spacing, dpi) as f32;
    let hit_radius = radius + scale(metrics.hit_slop, dpi) as f32;
    let left_margin = scale(metrics.left_margin, dpi) as f32;
    let close_x = left_margin;
    let min_x = close_x + spacing;
    let max_x = min_x + spacing;
    let center_y = height as f32 * 0.5;

    WindowControlsLayout {
        width,
        height,
        offset_x,
        offset_y,
        radius,
        hit_radius,
        center_y,
        centers_x: [close_x, min_x, max_x],
    }
}

#[must_use]
pub fn compute_traffic_light_layout(metrics: &TrafficLightMetrics, dpi: u32) -> TrafficLightLayout {
    compute_window_controls_layout(metrics, dpi)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowControlsState {
    pub close: bool,
    pub minimize: bool,
    pub zoom: bool,
}

impl WindowControlsState {
    #[must_use]
    pub const fn of(window: &WindowInfo) -> Self {
        Self {
            close: window.capability(WindowCommand::Close).is_available(),
            minimize: window.capability(WindowCommand::Minimize).is_available(),
            zoom: window.capability(WindowCommand::Zoom).is_available()
                || window
                    .capability(WindowCommand::ToggleFullScreen)
                    .is_available(),
        }
    }

    #[must_use]
    pub const fn allows(self, action: WindowControlAction) -> bool {
        match action {
            WindowControlAction::Close => self.close,
            WindowControlAction::Minimize => self.minimize,
            WindowControlAction::ToggleMaximize => self.zoom,
        }
    }

    #[must_use]
    pub const fn any(self) -> bool {
        self.close || self.minimize || self.zoom
    }
}

#[must_use]
pub const fn accepts_window_controls(window: &WindowInfo) -> bool {
    window.traits.title_bar
        && !window.full_screen
        && matches!(window.presence, WindowPresence::Visible)
        && WindowControlsState::of(window).any()
}

#[must_use]
pub const fn window_controls_top(
    window_top: i32,
    offset_y: i32,
    bar_height: Option<i32>,
    maximized: bool,
) -> i32 {
    let natural = window_top + offset_y;
    match bar_height {
        Some(bar) if maximized && natural < bar + offset_y => bar + offset_y,
        Some(bar) if natural < bar => bar,
        _ => natural,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_model::{AppIdentity, WindowIdentity, WindowRole, WindowTraits};

    fn window(traits: WindowTraits) -> WindowInfo {
        WindowInfo {
            identity: WindowIdentity {
                hwnd: 1,
                process_id: 2,
                thread_id: 3,
                generation: 4,
            },
            app: AppIdentity::executable("app.exe"),
            app_name: String::from("App"),
            executable: String::from("app.exe"),
            title: String::from("App"),
            role: WindowRole::Primary,
            presence: WindowPresence::Visible,
            zoomed: false,
            full_screen: false,
            active: true,
            traits,
        }
    }

    #[test]
    fn controls_attach_only_to_visible_captioned_windows_that_can_act() {
        assert!(accepts_window_controls(&window(WindowTraits::STANDARD)));
        let mut borderless = WindowTraits::STANDARD;
        borderless.title_bar = false;
        assert!(!accepts_window_controls(&window(borderless)));
        let mut full = window(WindowTraits::STANDARD);
        full.full_screen = true;
        assert!(!accepts_window_controls(&full));
        let mut inert = WindowTraits::STANDARD;
        inert.closable = false;
        inert.minimizable = false;
        inert.maximizable = false;
        assert!(!accepts_window_controls(&window(inert)));
    }

    #[test]
    fn disabled_caption_buttons_disable_the_matching_control() {
        let mut dialog = WindowTraits::STANDARD;
        dialog.minimizable = false;
        dialog.maximizable = false;
        let state = WindowControlsState::of(&window(dialog));
        assert!(state.allows(WindowControlAction::Close));
        assert!(!state.allows(WindowControlAction::Minimize));
        assert!(!state.allows(WindowControlAction::ToggleMaximize));
        let mut blocked = WindowTraits::STANDARD;
        blocked.blocked_by_modal = true;
        assert!(!WindowControlsState::of(&window(blocked)).any());
    }

    #[test]
    fn full_screen_capable_window_keeps_the_green_control_without_zoom() {
        let mut browser = WindowTraits::STANDARD;
        browser.maximizable = false;
        browser.full_screen_shortcut = true;
        assert!(WindowControlsState::of(&window(browser)).zoom);
    }

    #[test]
    fn top_is_clamped_below_the_bar_only_on_the_bar_monitor() {
        assert_eq!(window_controls_top(0, 9, Some(48), false), 48);
        assert_eq!(window_controls_top(48, 9, Some(48), true), 57);
        assert_eq!(window_controls_top(300, 9, Some(48), false), 309);
        assert_eq!(window_controls_top(-400, 9, None, false), -391);
        assert_eq!(window_controls_top(0, 9, None, true), 9);
    }

    #[test]
    fn window_controls_hit_test_identifies_red_yellow_green_on_left() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 96);
        let cy = layout.center_y as i32;
        assert_eq!(
            layout.hit_test(layout.centers_x[0] as i32, cy),
            Some(WindowControlAction::Close)
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[1] as i32, cy),
            Some(WindowControlAction::Minimize)
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[2] as i32, cy),
            Some(WindowControlAction::ToggleMaximize)
        );
        assert!(layout.centers_x[0] < layout.centers_x[1]);
        assert!(layout.centers_x[1] < layout.centers_x[2]);
        assert_eq!(layout.hit_test(2, 2), None);
    }

    #[test]
    fn hit_slop_scales_with_the_monitor_dpi() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 192);
        let cy = layout.center_y as i32;
        let just_inside_close = layout.centers_x[0] as i32 + 18;
        assert_eq!(
            layout.hit_test(just_inside_close, cy),
            Some(WindowControlAction::Close)
        );
    }

    #[test]
    fn layout_follows_each_monitor_dpi_so_mixed_dpi_controls_stay_aligned() {
        let metrics = WindowControlsMetrics::MACOS_SONOMA;
        for dpi in [96, 120, 144, 192] {
            let layout = compute_window_controls_layout(&metrics, dpi);
            assert_eq!(layout.width, scale(metrics.width, dpi));
            assert_eq!(layout.height, scale(metrics.height, dpi));
            assert_eq!(layout.offset_y, scale(metrics.offset_y, dpi));
            let last_edge = layout.centers_x[2] + layout.radius;
            assert!(last_edge <= layout.width as f32, "dpi {dpi}");
            assert!(layout.center_y + layout.radius <= layout.height as f32);
            let cy = layout.center_y as i32;
            assert_eq!(
                layout.hit_test(layout.centers_x[2] as i32, cy),
                Some(WindowControlAction::ToggleMaximize),
                "dpi {dpi}"
            );
        }
        let standard = compute_window_controls_layout(&metrics, 96);
        let high = compute_window_controls_layout(&metrics, 192);
        assert_eq!(high.width, standard.width * 2);
        assert!((high.radius - standard.radius * 2.0).abs() <= 1.0);
    }
}
