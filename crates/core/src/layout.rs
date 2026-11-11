use crate::media::NotchExpandedTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    #[must_use]
    pub const fn right(&self) -> i32 {
        self.x + self.width
    }

    #[must_use]
    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }

    #[must_use]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    Start,
    AppMenu,
    FileMenu,
    EditMenu,
    ViewMenu,
    WindowMenu,
    HelpMenu,
    Notch,
    PomodoroPhase,
    PomodoroPlayPause,
    PomodoroReset,
    PomodoroSkip,
    PomodoroStartNext,
    PomodoroExtendFive,
    NotchTabPomodoro,
    NotchTabMedia,
    MediaPlayPause,
    MediaPrevious,
    MediaNext,
    Tray,
    QuickSettings,
    Search,
    ControlCenter,
    Clock,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchLayoutInput {
    pub hud_expansion: f32,
    pub compact_expansion: f32,
    pub panel_expansion: f32,
    pub alert_mode: bool,
    pub active_tab: NotchExpandedTab,
    pub control_center_enabled: bool,
}

impl Default for NotchLayoutInput {
    fn default() -> Self {
        Self {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            panel_expansion: 0.0,
            alert_mode: false,
            active_tab: NotchExpandedTab::Pomodoro,
            control_center_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub notch_width: i32,
    pub notch_compact_width: i32,
    pub notch_hud_width: i32,
    pub notch_expanded_width: i32,
    pub notch_height: i32,
    pub notch_expanded_height: i32,
    pub notch_corner_radius: i32,
    pub notch_expanded_corner_radius: i32,
    pub notch_ear_radius: i32,
    pub notch_expanded_ear_radius: i32,
    pub icon_size: i32,
    pub edge_padding: i32,
    pub start_slot_width: i32,
    pub app_slot_min_width: i32,
    pub app_slot_max_width: i32,
    pub file_slot_width: i32,
    pub edit_slot_width: i32,
    pub view_slot_width: i32,
    pub window_slot_width: i32,
    pub help_slot_width: i32,
    pub left_slot_gap: i32,
    pub tray_slot_width: i32,
    pub status_slot_width: i32,
    pub search_slot_width: i32,
    pub control_center_slot_width: i32,
    pub clock_slot_width: i32,
    pub right_slot_gap: i32,
}

impl Metrics {
    pub const MACBOOK_PRO_14: Self = Self {
        notch_width: 200,
        notch_compact_width: 292,
        notch_hud_width: 356,
        notch_expanded_width: 380,
        notch_height: 32,
        notch_expanded_height: 116,
        notch_corner_radius: 10,
        notch_expanded_corner_radius: 22,
        notch_ear_radius: 6,
        notch_expanded_ear_radius: 8,
        icon_size: 15,
        edge_padding: 8,
        start_slot_width: 34,
        app_slot_min_width: 52,
        app_slot_max_width: 132,
        file_slot_width: 36,
        edit_slot_width: 36,
        view_slot_width: 42,
        window_slot_width: 60,
        help_slot_width: 42,
        left_slot_gap: 2,
        tray_slot_width: 28,
        status_slot_width: 108,
        search_slot_width: 30,
        control_center_slot_width: 30,
        clock_slot_width: 160,
        right_slot_gap: 4,
    };
}

impl Default for Metrics {
    fn default() -> Self {
        Self::MACBOOK_PRO_14
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub bar: Rect,
    pub notch: Rect,
    pub panel_window: Rect,
    pub notch_corner_radius: i32,
    pub notch_ear_radius: i32,
    pub panel_expansion: f32,
    pub compact_expansion: f32,
    pub alert_mode: bool,
    pub active_tab: NotchExpandedTab,
    pub pomodoro_phase_slot: Rect,
    pub pomodoro_play_button: Rect,
    pub pomodoro_reset_button: Rect,
    pub pomodoro_skip_button: Rect,
    pub pomodoro_alert_start_button: Rect,
    pub pomodoro_alert_extend_button: Rect,
    pub notch_tab_pomodoro: Rect,
    pub notch_tab_media: Rect,
    pub media_play_button: Rect,
    pub media_prev_button: Rect,
    pub media_next_button: Rect,
    pub icon_size: i32,
    pub start_slot: Rect,
    pub app_slot: Rect,
    pub file_slot: Rect,
    pub edit_slot: Rect,
    pub view_slot: Rect,
    pub window_slot: Rect,
    pub help_slot: Rect,
    pub tray_slot: Rect,
    pub status_slot: Rect,
    pub search_slot: Rect,
    pub control_center_slot: Rect,
    pub clock_slot: Rect,
}

impl Layout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<HitTarget> {
        if self.panel_expansion > 0.55 && self.notch.contains(x, y) {
            if self.alert_mode {
                if self.pomodoro_alert_start_button.contains(x, y) {
                    return Some(HitTarget::PomodoroStartNext);
                }
                if self.pomodoro_alert_extend_button.contains(x, y) {
                    return Some(HitTarget::PomodoroExtendFive);
                }
                return Some(HitTarget::Notch);
            }
            if self.notch_tab_pomodoro.contains(x, y) {
                return Some(HitTarget::NotchTabPomodoro);
            }
            if self.notch_tab_media.contains(x, y) {
                return Some(HitTarget::NotchTabMedia);
            }
            if self.active_tab == NotchExpandedTab::Media {
                if self.media_play_button.contains(x, y) {
                    return Some(HitTarget::MediaPlayPause);
                }
                if self.media_prev_button.contains(x, y) {
                    return Some(HitTarget::MediaPrevious);
                }
                if self.media_next_button.contains(x, y) {
                    return Some(HitTarget::MediaNext);
                }
            } else {
                if self.pomodoro_play_button.contains(x, y) {
                    return Some(HitTarget::PomodoroPlayPause);
                }
                if self.pomodoro_reset_button.contains(x, y) {
                    return Some(HitTarget::PomodoroReset);
                }
                if self.pomodoro_skip_button.contains(x, y) {
                    return Some(HitTarget::PomodoroSkip);
                }
                if self.pomodoro_phase_slot.contains(x, y) {
                    return Some(HitTarget::PomodoroPhase);
                }
            }
            return Some(HitTarget::Notch);
        }

        if self.start_slot.contains(x, y) {
            Some(HitTarget::Start)
        } else if self.app_slot.contains(x, y) {
            Some(HitTarget::AppMenu)
        } else if self.file_slot.contains(x, y) {
            Some(HitTarget::FileMenu)
        } else if self.edit_slot.contains(x, y) {
            Some(HitTarget::EditMenu)
        } else if self.view_slot.contains(x, y) {
            Some(HitTarget::ViewMenu)
        } else if self.window_slot.contains(x, y) {
            Some(HitTarget::WindowMenu)
        } else if self.help_slot.contains(x, y) {
            Some(HitTarget::HelpMenu)
        } else if self.notch.contains(x, y) {
            Some(HitTarget::Notch)
        } else if self.tray_slot.contains(x, y) {
            Some(HitTarget::Tray)
        } else if self.status_slot.contains(x, y) {
            Some(HitTarget::QuickSettings)
        } else if self.search_slot.contains(x, y) {
            Some(HitTarget::Search)
        } else if self.control_center_slot.contains(x, y) {
            Some(HitTarget::ControlCenter)
        } else if self.clock_slot.contains(x, y) {
            Some(HitTarget::Clock)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn slot_rect(&self, target: HitTarget) -> Option<Rect> {
        match target {
            HitTarget::Start => Some(self.start_slot),
            HitTarget::AppMenu => Some(self.app_slot),
            HitTarget::FileMenu => Some(self.file_slot),
            HitTarget::EditMenu => Some(self.edit_slot),
            HitTarget::ViewMenu => Some(self.view_slot),
            HitTarget::WindowMenu => Some(self.window_slot),
            HitTarget::HelpMenu => Some(self.help_slot),
            HitTarget::Notch
            | HitTarget::PomodoroPhase
            | HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive
            | HitTarget::NotchTabPomodoro
            | HitTarget::NotchTabMedia
            | HitTarget::MediaPlayPause
            | HitTarget::MediaPrevious
            | HitTarget::MediaNext => None,
            HitTarget::Tray => Some(self.tray_slot),
            HitTarget::QuickSettings => Some(self.status_slot),
            HitTarget::Search => Some(self.search_slot),
            HitTarget::ControlCenter => Some(self.control_center_slot),
            HitTarget::Clock => Some(self.clock_slot),
        }
    }
}

#[must_use]
pub const fn scale(points: i32, dpi: u32) -> i32 {
    (points * dpi as i32 + 48) / 96
}

fn lerp_points(start: i32, end: i32, t: f32) -> i32 {
    start + ((end - start) as f32 * t.clamp(0.0, 1.0)).round() as i32
}

fn estimate_app_slot_points(metrics: &Metrics, app_name: &str) -> i32 {
    let mut text_pts = 18;
    for ch in app_name.chars().take(16) {
        text_pts += match ch {
            'i' | 'l' | 'I' | 't' | 'f' | 'j' | 'r' | ' ' | '.' | '-' => 5,
            'm' | 'w' | 'M' | 'W' => 10,
            _ => 8,
        };
    }
    text_pts.clamp(metrics.app_slot_min_width, metrics.app_slot_max_width)
}

fn place_left_slot(cursor: &mut i32, width: i32, gap: i32, max_right: i32, height: i32) -> Rect {
    let x = (*cursor).min(max_right);
    let clamped_width = width.min((max_right - x).max(0));
    *cursor = (x + clamped_width + gap).min(max_right);
    Rect {
        x,
        y: 0,
        width: clamped_width,
        height,
    }
}

fn place_right_slot(cursor: &mut i32, width: i32, gap: i32, min_left: i32, height: i32) -> Rect {
    let x = (*cursor - width).max(min_left);
    *cursor = x - gap;
    Rect {
        x,
        y: 0,
        width,
        height,
    }
}

#[must_use]
pub fn compute_layout(metrics: &Metrics, screen_width: i32, dpi: u32) -> Layout {
    compute_layout_for_app(metrics, screen_width, dpi, "Finder")
}

#[must_use]
pub fn compute_layout_for_app(
    metrics: &Metrics,
    screen_width: i32,
    dpi: u32,
    app_name: &str,
) -> Layout {
    compute_layout_with_hud(metrics, screen_width, dpi, app_name, 0.0)
}

#[must_use]
pub fn compute_layout_with_hud(
    metrics: &Metrics,
    screen_width: i32,
    dpi: u32,
    app_name: &str,
    hud_expansion: f32,
) -> Layout {
    compute_layout_with_notch(
        metrics,
        screen_width,
        dpi,
        app_name,
        NotchLayoutInput {
            hud_expansion,
            compact_expansion: 0.0,
            panel_expansion: 0.0,
            alert_mode: false,
            active_tab: NotchExpandedTab::Pomodoro,
            control_center_enabled: true,
        },
    )
}

struct ExpandedNotchControls {
    phase_slot: Rect,
    play_button: Rect,
    reset_button: Rect,
    skip_button: Rect,
    alert_start_button: Rect,
    alert_extend_button: Rect,
    notch_tab_pomodoro: Rect,
    notch_tab_media: Rect,
    media_play_button: Rect,
    media_prev_button: Rect,
    media_next_button: Rect,
}

fn compute_expanded_notch_controls(notch: Rect, dpi: u32) -> ExpandedNotchControls {
    let pad_x = scale(18, dpi);
    let tab_y = scale(7, dpi);
    let tab_h = scale(20, dpi);
    let tab_pomo_w = scale(86, dpi);
    let tab_media_w = scale(96, dpi);
    let tab_gap = scale(6, dpi);

    let notch_tab_pomodoro = Rect {
        x: notch.x + pad_x,
        y: tab_y,
        width: tab_pomo_w,
        height: tab_h,
    };
    let notch_tab_media = Rect {
        x: notch_tab_pomodoro.right() + tab_gap,
        y: tab_y,
        width: tab_media_w,
        height: tab_h,
    };
    let phase_slot = notch_tab_pomodoro;

    let row_cy = scale(70, dpi);
    let play_size = scale(38, dpi);
    let secondary_size = scale(30, dpi);
    let btn_gap = scale(8, dpi);

    let skip_x = notch.right() - pad_x - secondary_size;
    let reset_x = skip_x - btn_gap - secondary_size;
    let play_x = reset_x - scale(10, dpi) - play_size;

    let skip_button = Rect {
        x: skip_x,
        y: row_cy - secondary_size / 2,
        width: secondary_size,
        height: secondary_size,
    };
    let reset_button = Rect {
        x: reset_x,
        y: row_cy - secondary_size / 2,
        width: secondary_size,
        height: secondary_size,
    };
    let play_button = Rect {
        x: play_x,
        y: row_cy - play_size / 2,
        width: play_size,
        height: play_size,
    };

    let media_next_x = notch.right() - pad_x - secondary_size;
    let media_play_x = media_next_x - btn_gap - play_size;
    let media_prev_x = media_play_x - btn_gap - secondary_size;

    let media_next_button = Rect {
        x: media_next_x,
        y: row_cy - secondary_size / 2,
        width: secondary_size,
        height: secondary_size,
    };
    let media_play_button = Rect {
        x: media_play_x,
        y: row_cy - play_size / 2,
        width: play_size,
        height: play_size,
    };
    let media_prev_button = Rect {
        x: media_prev_x,
        y: row_cy - secondary_size / 2,
        width: secondary_size,
        height: secondary_size,
    };

    let alert_btn_h = scale(30, dpi);
    let alert_start_w = scale(92, dpi);
    let alert_extend_w = scale(48, dpi);
    let alert_cy = scale(68, dpi);
    let alert_start_x = notch.right() - pad_x - alert_start_w;
    let alert_extend_x = alert_start_x - btn_gap - alert_extend_w;

    let alert_start_button = Rect {
        x: alert_start_x,
        y: alert_cy - alert_btn_h / 2,
        width: alert_start_w,
        height: alert_btn_h,
    };
    let alert_extend_button = Rect {
        x: alert_extend_x,
        y: alert_cy - alert_btn_h / 2,
        width: alert_extend_w,
        height: alert_btn_h,
    };

    ExpandedNotchControls {
        phase_slot,
        play_button,
        reset_button,
        skip_button,
        alert_start_button,
        alert_extend_button,
        notch_tab_pomodoro,
        notch_tab_media,
        media_play_button,
        media_prev_button,
        media_next_button,
    }
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn compute_layout_with_notch(
    metrics: &Metrics,
    screen_width: i32,
    dpi: u32,
    app_name: &str,
    notch_input: NotchLayoutInput,
) -> Layout {
    let bar_height = scale(metrics.notch_height, dpi);
    let hud_t = notch_input.hud_expansion.clamp(0.0, 1.0);
    let compact_t = notch_input.compact_expansion.clamp(0.0, 1.0);
    let panel_t = notch_input.panel_expansion.clamp(0.0, 1.0);

    let compact_pts = lerp_points(metrics.notch_width, metrics.notch_compact_width, compact_t);
    let hud_pts = lerp_points(metrics.notch_width, metrics.notch_hud_width, hud_t);
    let collapsed_pts = compact_pts.max(hud_pts);
    let notch_pts_w = lerp_points(collapsed_pts, metrics.notch_expanded_width, panel_t);
    let notch_pts_h = lerp_points(metrics.notch_height, metrics.notch_expanded_height, panel_t);

    let notch_width = scale(notch_pts_w, dpi);
    let notch_height = scale(notch_pts_h, dpi);
    let padding = scale(metrics.edge_padding, dpi);
    let left_gap = scale(metrics.left_slot_gap, dpi);
    let right_gap = scale(metrics.right_slot_gap, dpi);

    let bar = Rect {
        x: 0,
        y: 0,
        width: screen_width,
        height: bar_height,
    };
    let notch = Rect {
        x: (screen_width - notch_width) / 2,
        y: 0,
        width: notch_width,
        height: notch_height,
    };

    let panel_pts_w = metrics.notch_expanded_width + metrics.notch_expanded_ear_radius * 2 + 8;
    let panel_pts_h = metrics.notch_expanded_height + 4;
    let panel_w = scale(panel_pts_w, dpi);
    let panel_h = scale(panel_pts_h, dpi);
    let panel_window = Rect {
        x: (screen_width - panel_w) / 2,
        y: 0,
        width: panel_w,
        height: panel_h,
    };

    let controls = compute_expanded_notch_controls(notch, dpi);

    let left_limit = (notch.x - left_gap).max(padding);
    let mut lx = padding;
    let mut next_left =
        |pts: i32| place_left_slot(&mut lx, scale(pts, dpi), left_gap, left_limit, bar_height);

    let start_slot = next_left(metrics.start_slot_width);
    let app_slot = next_left(estimate_app_slot_points(metrics, app_name));
    let file_slot = next_left(metrics.file_slot_width);
    let edit_slot = next_left(metrics.edit_slot_width);
    let view_slot = next_left(metrics.view_slot_width);
    let window_slot = next_left(metrics.window_slot_width);
    let help_slot = next_left(metrics.help_slot_width);

    let right_limit = notch.right();
    let mut rx = screen_width - padding;
    let mut next_right =
        |pts: i32| place_right_slot(&mut rx, scale(pts, dpi), right_gap, right_limit, bar_height);

    let clock_slot = next_right(metrics.clock_slot_width);
    let control_center_width = if notch_input.control_center_enabled {
        metrics.control_center_slot_width
    } else {
        0
    };
    let control_center_slot = next_right(control_center_width);
    let search_slot = next_right(metrics.search_slot_width);
    let status_slot = next_right(metrics.status_slot_width);
    let tray_slot = next_right(metrics.tray_slot_width);

    let corner_pts = lerp_points(
        metrics.notch_corner_radius,
        metrics.notch_expanded_corner_radius,
        panel_t,
    );
    let ear_pts = lerp_points(
        metrics.notch_ear_radius,
        metrics.notch_expanded_ear_radius,
        panel_t,
    );

    Layout {
        bar,
        notch,
        panel_window,
        notch_corner_radius: scale(corner_pts, dpi),
        notch_ear_radius: scale(ear_pts, dpi),
        panel_expansion: panel_t,
        compact_expansion: compact_t,
        alert_mode: notch_input.alert_mode,
        active_tab: notch_input.active_tab,
        pomodoro_phase_slot: controls.phase_slot,
        pomodoro_play_button: controls.play_button,
        pomodoro_reset_button: controls.reset_button,
        pomodoro_skip_button: controls.skip_button,
        pomodoro_alert_start_button: controls.alert_start_button,
        pomodoro_alert_extend_button: controls.alert_extend_button,
        notch_tab_pomodoro: controls.notch_tab_pomodoro,
        notch_tab_media: controls.notch_tab_media,
        media_play_button: controls.media_play_button,
        media_prev_button: controls.media_prev_button,
        media_next_button: controls.media_next_button,
        icon_size: scale(metrics.icon_size, dpi),
        start_slot,
        app_slot,
        file_slot,
        edit_slot,
        view_slot,
        window_slot,
        help_slot,
        tray_slot,
        status_slot,
        search_slot,
        control_center_slot,
        clock_slot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: Metrics = Metrics::MACBOOK_PRO_14;

    #[test]
    fn bar_height_equals_notch_height_and_spans_screen() {
        let layout = compute_layout(&M, 1920, 96);
        assert_eq!(
            layout.bar,
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 32
            }
        );
        assert_eq!(layout.notch.height, layout.bar.height);
    }

    #[test]
    fn notch_is_centered_with_ears_and_bottom_radius() {
        let layout = compute_layout(&M, 1920, 96);
        assert_eq!(layout.notch.x, 860);
        assert_eq!(layout.notch.width, 200);
        assert_eq!(layout.notch_corner_radius, 10);
        assert_eq!(layout.notch_ear_radius, 6);
    }

    #[test]
    fn expanded_notch_uses_apple_dynamic_island_dimensions() {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                alert_mode: false,
                active_tab: NotchExpandedTab::Pomodoro,
                control_center_enabled: true,
            },
        );
        assert_eq!(layout.bar.height, 32);
        assert_eq!(layout.notch.width, 380);
        assert_eq!(layout.notch.height, 116);
        assert_eq!(layout.notch_corner_radius, 22);
        assert_eq!(layout.notch_ear_radius, 8);
        assert_eq!(
            layout.hit_test(
                layout.pomodoro_play_button.x + 10,
                layout.pomodoro_play_button.y + 10
            ),
            Some(HitTarget::PomodoroPlayPause)
        );
        assert_eq!(
            layout.hit_test(
                layout.pomodoro_reset_button.x + 8,
                layout.pomodoro_reset_button.y + 8
            ),
            Some(HitTarget::PomodoroReset)
        );
        assert_eq!(
            layout.hit_test(
                layout.pomodoro_skip_button.x + 8,
                layout.pomodoro_skip_button.y + 8
            ),
            Some(HitTarget::PomodoroSkip)
        );
        assert_eq!(
            layout.hit_test(
                layout.notch_tab_pomodoro.x + 8,
                layout.notch_tab_pomodoro.y + 8
            ),
            Some(HitTarget::NotchTabPomodoro)
        );
    }

    #[test]
    fn expanded_notch_supports_media_tab_and_playback_controls() {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                alert_mode: false,
                active_tab: NotchExpandedTab::Media,
                control_center_enabled: true,
            },
        );
        assert_eq!(
            layout.hit_test(
                layout.notch_tab_pomodoro.x + 8,
                layout.notch_tab_pomodoro.y + 8,
            ),
            Some(HitTarget::NotchTabPomodoro)
        );
        assert_eq!(
            layout.hit_test(layout.notch_tab_media.x + 8, layout.notch_tab_media.y + 8,),
            Some(HitTarget::NotchTabMedia)
        );
        assert_eq!(
            layout.hit_test(
                layout.media_play_button.x + 8,
                layout.media_play_button.y + 8,
            ),
            Some(HitTarget::MediaPlayPause)
        );
        assert_eq!(
            layout.hit_test(
                layout.media_prev_button.x + 8,
                layout.media_prev_button.y + 8,
            ),
            Some(HitTarget::MediaPrevious)
        );
        assert_eq!(
            layout.hit_test(
                layout.media_next_button.x + 8,
                layout.media_next_button.y + 8,
            ),
            Some(HitTarget::MediaNext)
        );
    }

    #[test]
    fn compact_active_pomodoro_widens_notch_to_292_points() {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 1.0,
                panel_expansion: 0.0,
                alert_mode: false,
                active_tab: NotchExpandedTab::Pomodoro,
                control_center_enabled: true,
            },
        );
        assert_eq!(layout.notch.width, 292);
        assert_eq!(layout.notch.height, 32);
    }

    #[test]
    fn dimensions_scale_with_dpi() {
        let layout = compute_layout(&M, 3840, 192);
        assert_eq!(layout.bar.height, 64);
        assert_eq!(layout.notch.width, 400);
        assert_eq!(layout.notch_ear_radius, 12);
        assert_eq!(scale(10, 144), 15);
    }

    #[test]
    fn start_is_left_and_tray_status_clock_are_right_of_notch() {
        let layout = compute_layout_for_app(&M, 1920, 96, "Finder");
        assert!(layout.start_slot.right() <= layout.app_slot.x);
        assert!(layout.app_slot.right() <= layout.file_slot.x);
        assert!(layout.file_slot.right() <= layout.edit_slot.x);
        assert!(layout.edit_slot.right() <= layout.view_slot.x);
        assert!(layout.view_slot.right() <= layout.window_slot.x);
        assert!(layout.window_slot.right() <= layout.help_slot.x);
        assert!(layout.help_slot.right() < layout.notch.x);
        assert!(layout.tray_slot.x >= layout.notch.right());
        assert!(layout.status_slot.x >= layout.tray_slot.right());
        assert!(layout.search_slot.x >= layout.status_slot.right());
        assert!(layout.control_center_slot.x >= layout.search_slot.right());
        assert!(layout.clock_slot.x >= layout.control_center_slot.right());
        assert!(layout.clock_slot.right() <= layout.bar.width);
    }

    #[test]
    fn hit_test_identifies_each_slot() {
        let layout = compute_layout(&M, 1920, 96);
        assert_eq!(
            layout.hit_test(layout.start_slot.x + 4, 16),
            Some(HitTarget::Start)
        );
        assert_eq!(
            layout.hit_test(layout.app_slot.x + 4, 16),
            Some(HitTarget::AppMenu)
        );
        assert_eq!(
            layout.hit_test(layout.file_slot.x + 4, 16),
            Some(HitTarget::FileMenu)
        );
        assert_eq!(
            layout.hit_test(layout.edit_slot.x + 4, 16),
            Some(HitTarget::EditMenu)
        );
        assert_eq!(
            layout.hit_test(layout.view_slot.x + 4, 16),
            Some(HitTarget::ViewMenu)
        );
        assert_eq!(
            layout.hit_test(layout.window_slot.x + 4, 16),
            Some(HitTarget::WindowMenu)
        );
        assert_eq!(
            layout.hit_test(layout.help_slot.x + 4, 16),
            Some(HitTarget::HelpMenu)
        );
        assert_eq!(
            layout.hit_test(layout.notch.x + 50, 16),
            Some(HitTarget::Notch)
        );
        assert_eq!(
            layout.hit_test(layout.tray_slot.x + 4, 16),
            Some(HitTarget::Tray)
        );
        assert_eq!(
            layout.hit_test(layout.status_slot.x + 10, 16),
            Some(HitTarget::QuickSettings)
        );
        assert_eq!(
            layout.hit_test(layout.search_slot.x + 4, 16),
            Some(HitTarget::Search)
        );
        assert_eq!(
            layout.hit_test(layout.control_center_slot.x + 4, 16),
            Some(HitTarget::ControlCenter)
        );
        assert_eq!(
            layout.hit_test(layout.clock_slot.x + 10, 16),
            Some(HitTarget::Clock)
        );
        assert_eq!(layout.hit_test(750, 16), None);
    }

    #[test]
    fn disabled_control_center_has_no_hit_target() {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                control_center_enabled: false,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(layout.control_center_slot.width, 0);
        assert_eq!(layout.hit_test(layout.control_center_slot.x, 16), None);
    }
}
