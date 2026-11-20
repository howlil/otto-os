use crate::preferences::NotchStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub bar_height: i32,
    pub notch_top_inset: i32,
    pub notch_width: i32,
    pub notch_compact_width: i32,
    pub notch_focus_compact_width: i32,
    pub notch_hud_width: i32,
    pub notch_expanded_width: i32,
    pub notch_height: i32,
    pub notch_compact_height: i32,
    pub notch_expanded_height: i32,
    pub notch_lyrics_height: i32,
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
    pub overflow_slot_width: i32,
    pub left_slot_gap: i32,
    pub tray_slot_width: i32,
    pub status_slot_width: i32,
    pub search_slot_width: i32,
    pub control_center_slot_width: i32,
    pub notification_slot_width: i32,
    pub clock_slot_width: i32,
    pub right_slot_gap: i32,
    pub hud_pad_points: i32,
    pub hud_gap_points: i32,
    pub hud_glyph_points: i32,
    pub hud_label_points: i32,
    pub hud_value_points: i32,
    pub hud_meter_height_points: i32,
}

impl Metrics {
    pub const MACBOOK_PRO_14: Self = Self {
        bar_height: 32,
        notch_top_inset: 0,
        notch_width: 200,
        notch_compact_width: 292,
        notch_focus_compact_width: 200,
        notch_hud_width: 356,
        notch_expanded_width: 380,
        notch_height: 32,
        notch_compact_height: 32,
        notch_expanded_height: 144,
        notch_lyrics_height: 254,
        notch_corner_radius: 10,
        notch_expanded_corner_radius: 22,
        notch_ear_radius: 6,
        notch_expanded_ear_radius: 8,
        icon_size: 15,
        edge_padding: 8,
        start_slot_width: 34,
        app_slot_min_width: 52,
        app_slot_max_width: 132,
        file_slot_width: 40,
        edit_slot_width: 40,
        view_slot_width: 46,
        window_slot_width: 64,
        help_slot_width: 44,
        overflow_slot_width: 26,
        left_slot_gap: 2,
        tray_slot_width: 28,
        status_slot_width: 108,
        search_slot_width: 30,
        control_center_slot_width: 30,
        notification_slot_width: 30,
        clock_slot_width: 128,
        right_slot_gap: 4,
        hud_pad_points: 16,
        hud_gap_points: 10,
        hud_glyph_points: 16,
        hud_label_points: 88,
        hud_value_points: 40,
        hud_meter_height_points: 4,
    };

    pub const DYNAMIC_ISLAND: Self = Self {
        bar_height: 40,
        notch_top_inset: 4,
        notch_width: 120,
        notch_height: 32,
        notch_focus_compact_width: 184,
        notch_compact_height: 36,
        notch_corner_radius: 16,
        notch_expanded_corner_radius: 28,
        notch_ear_radius: 0,
        notch_expanded_ear_radius: 0,
        ..Self::MACBOOK_PRO_14
    };

    #[must_use]
    pub const fn for_style(style: NotchStyle) -> Self {
        match style {
            NotchStyle::ClassicNotch => Self::MACBOOK_PRO_14,
            NotchStyle::DynamicIsland => Self::DYNAMIC_ISLAND,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::MACBOOK_PRO_14
    }
}
