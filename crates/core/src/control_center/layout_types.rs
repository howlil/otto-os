use crate::layout::Rect;

use super::model::ControlCenterPage;
use super::snapshot::SubPanelGroupLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCenterMetrics {
    pub width: i32,
    pub padding: i32,
    pub gap: i32,
    pub shell_radius: i32,
    pub card_radius: i32,
    pub connectivity_height: i32,
    pub display_card_height: i32,
    pub sound_card_height: i32,
    pub bottom_row_height: i32,
    pub header_height: i32,
    pub slider_card_height: i32,
    pub row_height: i32,
    pub row_gap: i32,
    pub footer_height: i32,
    pub max_visible_rows: i32,
    pub group_header_height: i32,
    pub group_gap: i32,
    pub error_banner_height: i32,
}

impl ControlCenterMetrics {
    pub const DEFAULT: Self = Self {
        width: 376,
        padding: 12,
        gap: 8,
        shell_radius: 26,
        card_radius: 14,
        connectivity_height: 144,
        display_card_height: 64,
        sound_card_height: 64,
        bottom_row_height: 128,
        header_height: 40,
        slider_card_height: 64,
        row_height: 44,
        row_gap: 0,
        footer_height: 36,
        max_visible_rows: 6,
        group_header_height: 24,
        group_gap: 12,
        error_banner_height: 28,
    };
}

impl Default for ControlCenterMetrics {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCenterPlacement {
    pub bounds: Rect,
    pub dpi: u32,
    pub anchor_right: i32,
    pub anchor_y: i32,
    pub page: ControlCenterPage,
    pub item_count: usize,
    pub scroll_offset: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterHitTarget {
    WifiToggle,
    WifiOpenPage,
    BluetoothToggle,
    BluetoothOpenPage,
    FocusToggle,
    FocusOpenSettings,
    BatteryTile,
    DisplayOpenPage,
    DisplaySlider,
    SoundOpenPage,
    SoundMuteToggle,
    SoundSlider,
    MediaBody,
    MediaPrev,
    MediaPlayPause,
    MediaNext,
    BackButton,
    HeaderToggle,
    SubPanelItem(usize),
    FooterPrimary,
    FooterSecondary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControlCenterLayout {
    pub window: Rect,
    pub page: ControlCenterPage,
    pub shell_radius: i32,
    pub card_radius: i32,
    pub connectivity_card: Rect,
    pub wifi_toggle: Rect,
    pub wifi_row: Rect,
    pub bluetooth_toggle: Rect,
    pub bluetooth_row: Rect,
    pub focus_toggle: Rect,
    pub focus_settings: Rect,
    pub battery_tile: Rect,
    pub display_card: Rect,
    pub display_header: Rect,
    pub display_icon: Rect,
    pub display_slider: Rect,
    pub sound_card: Rect,
    pub sound_header: Rect,
    pub sound_mute: Rect,
    pub sound_slider: Rect,
    pub media_card: Rect,
    pub media_body: Rect,
    pub media_prev: Rect,
    pub media_play_pause: Rect,
    pub media_next: Rect,
    pub back_button: Rect,
    pub header_rect: Rect,
    pub header_toggle: Rect,
    pub list_viewport: Rect,
    pub groups: Vec<SubPanelGroupLayout>,
    pub items: Vec<Rect>,
    pub scroll_offset: i32,
    pub max_scroll: i32,
    pub row_height: i32,
    pub footer_primary: Rect,
    pub footer_secondary: Rect,
}
