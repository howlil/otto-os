use crate::app_model::{WindowCommand, WindowInfo, WindowPresence};
use crate::layout::{Rect, scale};

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
    pub actions: [WindowControlAction; 3],
    pub slots: usize,
    pub cell_width: f32,
}

pub type TrafficLightLayout = WindowControlsLayout;

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

    fn cell_hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        if x < 0 || y < 0 || y >= self.height {
            return None;
        }
        let slot = (x as f32 / self.cell_width) as usize;
        (slot < self.slots).then(|| self.actions[slot])
    }

    #[must_use]
    pub fn center_of(&self, action: WindowControlAction) -> Option<f32> {
        self.actions
            .into_iter()
            .zip(self.centers_x)
            .take(self.slots)
            .find_map(|(candidate, center)| (candidate == action).then_some(center))
    }
}

const LEFT_ACTIONS: [WindowControlAction; 3] = [
    WindowControlAction::Close,
    WindowControlAction::Minimize,
    WindowControlAction::ToggleMaximize,
];

const COVER_ACTIONS: [WindowControlAction; 3] = [
    WindowControlAction::Minimize,
    WindowControlAction::ToggleMaximize,
    WindowControlAction::Close,
];

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
        actions: LEFT_ACTIONS,
        slots: 3,
        cell_width: 0.0,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverStyle {
    pub rgb: [u8; 3],
    pub top_right_radius: f32,
}

pub const NATIVE_CAPTION_BUTTON_DIP: i32 = 46;
pub const NATIVE_CORNER_RADIUS_DIP: i32 = 8;

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
    pub dpi: u32,
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
            dpi,
        })
    }

    #[must_use]
    pub fn cover_rect(&self) -> Rect {
        let width = scale(NATIVE_CAPTION_BUTTON_DIP, self.dpi) * i32::from(self.slots);
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
            width: scale(NATIVE_CAPTION_BUTTON_DIP, self.dpi),
            ..cover
        })
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
    let cell_width = cover.width as f32 / slots as f32;
    let mut centers_x = [0.0_f32; 3];
    for (index, center) in centers_x.iter_mut().enumerate().take(slots) {
        *center = cell_width * (index as f32 + 0.5);
    }
    let actions = if slots == 1 {
        [WindowControlAction::Close; 3]
    } else {
        COVER_ACTIONS
    };
    Some(WindowControlsLayout {
        width: cover.width,
        height: cover.height,
        offset_x: 0,
        offset_y: 0,
        radius: scale(metrics.button_radius, dpi) as f32,
        hit_radius: 0.0,
        center_y: cover.height as f32 * 0.5,
        centers_x,
        actions,
        slots,
        cell_width,
    })
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

    const STANDARD_STYLE: NativeCaptionStyle = NativeCaptionStyle {
        has_caption: true,
        has_system_menu: true,
        can_minimize: true,
        can_maximize: true,
    };

    fn caption(style: NativeCaptionStyle) -> NativeCaption {
        NativeCaption::new(style, 1191, 300, 44, true, 144).unwrap()
    }

    #[test]
    fn three_native_buttons_are_right_aligned_to_the_visible_frame() {
        let caption = caption(STANDARD_STYLE);
        let cover = caption.cover_rect();
        assert_eq!(cover.width, 69 * 3);
        assert_eq!(cover.right(), 1191);
        assert_eq!(cover.y, 300);
        assert_eq!(cover.height, 44);
    }

    #[test]
    fn a_close_only_window_covers_a_single_button() {
        let style = NativeCaptionStyle {
            can_minimize: false,
            can_maximize: false,
            ..STANDARD_STYLE
        };
        let caption = caption(style);
        assert_eq!(caption.slots, 1);
        assert_eq!(caption.cover_rect().width, 69);
        assert_eq!(caption.minimize_button(), None);
    }

    #[test]
    fn a_window_with_only_a_maximize_box_still_shows_three_slots_but_no_minimize() {
        let style = NativeCaptionStyle {
            can_minimize: false,
            ..STANDARD_STYLE
        };
        let caption = caption(style);
        assert_eq!(caption.slots, 3);
        assert_eq!(caption.minimize_button(), None);
    }

    #[test]
    fn the_native_minimize_button_is_the_leftmost_native_slot() {
        let caption = caption(STANDARD_STYLE);
        let button = caption.minimize_button().unwrap();
        assert_eq!(button.x, 1191 - 69 * 3);
        assert_eq!(button.width, 69);
        assert!(button.contains(button.x, 300));
        assert!(!button.contains(button.right(), 300));
    }

    #[test]
    fn windows_without_a_caption_or_system_menu_have_no_native_caption() {
        for style in [
            NativeCaptionStyle {
                has_caption: false,
                ..STANDARD_STYLE
            },
            NativeCaptionStyle {
                has_system_menu: false,
                ..STANDARD_STYLE
            },
        ] {
            assert_eq!(NativeCaption::new(style, 1191, 300, 44, true, 144), None);
        }
        assert_eq!(
            NativeCaption::new(STANDARD_STYLE, 1191, 300, 0, true, 144),
            None
        );
    }

    #[test]
    fn the_corner_radius_follows_rounding_and_dpi() {
        let rounded = caption(STANDARD_STYLE);
        assert!((rounded.corner_radius() - 12.0).abs() < 0.001);
        let square = NativeCaption::new(STANDARD_STYLE, 1191, 300, 44, false, 144).unwrap();
        assert!(square.corner_radius().abs() < 0.001);
    }

    #[test]
    fn the_cover_layout_orders_buttons_like_windows_with_mac_colours() {
        let layout =
            compute_cover_layout(caption(STANDARD_STYLE), &WindowControlsMetrics::default())
                .unwrap();
        assert_eq!(layout.actions[0], WindowControlAction::Minimize);
        assert_eq!(layout.actions[1], WindowControlAction::ToggleMaximize);
        assert_eq!(layout.actions[2], WindowControlAction::Close);
        assert_eq!(layout.width, 207);
        assert!((layout.centers_x[0] - 34.5).abs() < 0.01);
        assert!((layout.centers_x[2] - 172.5).abs() < 0.01);
    }

    #[test]
    fn the_cover_hit_test_uses_the_whole_native_button_cell() {
        let layout =
            compute_cover_layout(caption(STANDARD_STYLE), &WindowControlsMetrics::default())
                .unwrap();
        assert_eq!(layout.hit_test(0, 0), Some(WindowControlAction::Minimize));
        assert_eq!(layout.hit_test(68, 43), Some(WindowControlAction::Minimize));
        assert_eq!(
            layout.hit_test(69, 10),
            Some(WindowControlAction::ToggleMaximize)
        );
        assert_eq!(layout.hit_test(206, 10), Some(WindowControlAction::Close));
        assert_eq!(layout.hit_test(207, 10), None);
        assert_eq!(layout.hit_test(100, 44), None);
        assert_eq!(layout.hit_test(-1, 10), None);
    }

    #[test]
    fn a_single_slot_layout_only_answers_for_close() {
        let style = NativeCaptionStyle {
            can_minimize: false,
            can_maximize: false,
            ..STANDARD_STYLE
        };
        let layout =
            compute_cover_layout(caption(style), &WindowControlsMetrics::default()).unwrap();
        assert_eq!(layout.slots, 1);
        assert_eq!(layout.hit_test(30, 10), Some(WindowControlAction::Close));
        assert_eq!(layout.hit_test(70, 10), None);
        assert_eq!(layout.center_of(WindowControlAction::Close), Some(34.5));
        assert_eq!(layout.center_of(WindowControlAction::Minimize), None);
    }

    #[test]
    fn the_left_layout_keeps_its_circle_hit_test_and_order() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::default(), 96);
        assert_eq!(layout.actions[0], WindowControlAction::Close);
        assert_eq!(
            layout.center_of(WindowControlAction::ToggleMaximize),
            Some(layout.centers_x[2])
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[1] as i32, layout.center_y as i32),
            Some(WindowControlAction::Minimize)
        );
    }
}
