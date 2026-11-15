use crate::clipboard::{CLIPBOARD_PAGE_SIZE, ClipboardLayoutInput};
use crate::hud::{HudGeometry, compute_hud_geometry};
use crate::island::CompactSlotKind;
use crate::media::NotchExpandedTab;
use crate::pomodoro::FocusSubMode;
use crate::pomodoro_view::PomodoroSurface;
use crate::preferences::NotchStyle;
use crate::privacy::PRIVACY_COMPACT_EXTRA_POINTS;
use crate::shelf::SHELF_PAGE_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: self.right().max(other.right()) - x,
            height: self.bottom().max(other.bottom()) - y,
        }
    }

    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then_some(Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }

    #[must_use]
    pub fn contains_rect(&self, other: &Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
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
    MenuOverflow,
    Notch,
    NotchSecondary,
    PomodoroPlayPause,
    PomodoroReset,
    PomodoroSkip,
    PomodoroStartNext,
    PomodoroExtendFive,
    PomodoroPreset(u8),
    FocusMode(FocusSubMode),
    NotchTabPomodoro,
    NotchTabShelf,
    NotchTabClipboard,
    NotchTabMedia,
    ShelfAdd,
    ShelfClear,
    ShelfPager,
    ShelfItem(u8),
    ShelfCopyItem(u8),
    ShelfRemoveItem(u8),
    ClipboardPauseToggle,
    ClipboardClear,
    ClipboardPager,
    ClipboardItem(u8),
    ClipboardRemoveItem(u8),
    PrivacyIndicator,
    PrivacyMicMute,
    MediaPlayPause,
    MediaPrevious,
    MediaNext,
    MediaSeekbar,
    MediaLyricsToggle,
    Tray,
    StatusWifi,
    StatusVolume,
    StatusBattery,
    QuickSettings,
    Search,
    ControlCenter,
    Clock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrivacyLayoutState {
    pub compact_dots: bool,
    pub panel_mic_control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShelfLayoutInput {
    pub visible_count: u8,
    pub has_pager: bool,
    pub has_items: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchLayoutInput {
    pub hud_expansion: f32,
    pub compact_expansion: f32,
    pub secondary_expansion: f32,
    pub compact_swap_blend: f32,
    pub compact_primary: Option<CompactSlotKind>,
    pub compact_secondary: Option<CompactSlotKind>,
    pub panel_expansion: f32,
    pub pomodoro_surface: PomodoroSurface,
    pub focus_mode: FocusSubMode,
    pub active_tab: NotchExpandedTab,
    pub previous_tab: NotchExpandedTab,
    pub control_center_enabled: bool,
    pub focus_enabled: bool,
    pub shelf_enabled: bool,
    pub clipboard_enabled: bool,
    pub media_present: bool,
    pub lyrics_progress: f32,
    pub tab_blend: f32,
    pub compact_title_points: i32,
    pub media_live: bool,
    pub privacy: PrivacyLayoutState,
    pub shelf: ShelfLayoutInput,
    pub clipboard: ClipboardLayoutInput,
}

impl Default for NotchLayoutInput {
    fn default() -> Self {
        Self {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            secondary_expansion: 0.0,
            compact_swap_blend: 1.0,
            compact_primary: None,
            compact_secondary: None,
            panel_expansion: 0.0,
            pomodoro_surface: PomodoroSurface::Setup,
            focus_mode: FocusSubMode::Pomodoro,
            active_tab: NotchExpandedTab::Pomodoro,
            previous_tab: NotchExpandedTab::Pomodoro,
            control_center_enabled: true,
            focus_enabled: true,
            shelf_enabled: true,
            clipboard_enabled: true,
            media_present: true,
            lyrics_progress: 0.0,
            tab_blend: 1.0,
            compact_title_points: 0,
            media_live: false,
            privacy: PrivacyLayoutState::default(),
            shelf: ShelfLayoutInput::default(),
            clipboard: ClipboardLayoutInput::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub bar_height: i32,
    pub notch_top_inset: i32,
    pub notch_width: i32,
    pub notch_compact_width: i32,
    pub notch_hud_width: i32,
    pub notch_expanded_width: i32,
    pub notch_height: i32,
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
        notch_hud_width: 356,
        notch_expanded_width: 380,
        notch_height: 32,
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
        file_slot_width: 36,
        edit_slot_width: 36,
        view_slot_width: 42,
        window_slot_width: 60,
        help_slot_width: 42,
        overflow_slot_width: 26,
        left_slot_gap: 2,
        tray_slot_width: 28,
        status_slot_width: 108,
        search_slot_width: 30,
        control_center_slot_width: 30,
        clock_slot_width: 160,
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
        notch_corner_radius: 16,
        notch_expanded_corner_radius: 24,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub bar: Rect,
    pub notch: Rect,
    pub notch_secondary: Rect,
    pub panel_window: Rect,
    pub notch_corner_radius: i32,
    pub notch_ear_radius: i32,
    pub panel_expansion: f32,
    pub compact_expansion: f32,
    pub secondary_expansion: f32,
    pub compact_swap_blend: f32,
    pub compact_primary: Option<CompactSlotKind>,
    pub compact_secondary: Option<CompactSlotKind>,
    pub pomodoro_surface: PomodoroSurface,
    pub focus_mode: FocusSubMode,
    pub active_tab: NotchExpandedTab,
    pub previous_tab: NotchExpandedTab,
    pub compact_ring: Rect,
    pub compact_peek: Rect,
    pub compact_time: Rect,
    pub compact_title: Rect,
    pub compact_trailing: Rect,
    pub notch_tab_strip: Rect,
    pub pomodoro_icon: Rect,
    pub pomodoro_time: Rect,
    pub pomodoro_subtitle: Rect,
    pub pomodoro_session_label: Rect,
    pub pomodoro_segments: [Rect; 4],
    pub pomodoro_footer_left: Rect,
    pub pomodoro_footer_right: Rect,
    pub pomodoro_presets: [Rect; 4],
    pub pomodoro_focus_modes: [Rect; 3],
    pub pomodoro_play_button: Rect,
    pub pomodoro_reset_button: Rect,
    pub pomodoro_skip_button: Rect,
    pub pomodoro_alert_start_button: Rect,
    pub pomodoro_alert_extend_button: Rect,
    pub notch_tab_pomodoro: Rect,
    pub notch_tab_shelf: Rect,
    pub notch_tab_clipboard: Rect,
    pub notch_tab_media: Rect,
    pub shelf_add_button: Rect,
    pub shelf_clear_button: Rect,
    pub shelf_pager_button: Rect,
    pub shelf_dropzone: Rect,
    pub shelf_cards: [Rect; SHELF_PAGE_SIZE],
    pub shelf_copy_buttons: [Rect; SHELF_PAGE_SIZE],
    pub shelf_remove_buttons: [Rect; SHELF_PAGE_SIZE],
    pub clipboard_pause_button: Rect,
    pub clipboard_clear_button: Rect,
    pub clipboard_pager_button: Rect,
    pub clipboard_body: Rect,
    pub clipboard_rows: [Rect; CLIPBOARD_PAGE_SIZE],
    pub clipboard_remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
    pub privacy_dots: Rect,
    pub privacy_mic_button: Rect,
    pub media_cover_art: Rect,
    pub media_source_label: Rect,
    pub media_title: Rect,
    pub media_artist: Rect,
    pub media_reason: Rect,
    pub media_equalizer: Rect,
    pub media_lyrics_toggle: Rect,
    pub media_lyrics_drawer: Rect,
    pub media_seekbar: Rect,
    pub media_play_button: Rect,
    pub media_prev_button: Rect,
    pub media_next_button: Rect,
    pub lyrics_progress: f32,
    pub tab_blend: f32,
    pub icon_size: i32,
    pub start_slot: Rect,
    pub app_slot: Rect,
    pub file_slot: Rect,
    pub edit_slot: Rect,
    pub view_slot: Rect,
    pub window_slot: Rect,
    pub help_slot: Rect,
    pub overflow_slot: Rect,
    pub hidden_targets: Vec<HitTarget>,
    pub tray_slot: Rect,
    pub status_slot: Rect,
    pub status_wifi_rect: Rect,
    pub status_volume_rect: Rect,
    pub status_battery_rect: Rect,
    pub search_slot: Rect,
    pub control_center_slot: Rect,
    pub clock_slot: Rect,
    pub hud: HudGeometry,
}

impl Layout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<HitTarget> {
        if self.panel_expansion > 0.55 && self.notch.contains(x, y) {
            return Some(self.hit_test_expanded_notch(x, y));
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
        } else if self.overflow_slot.contains(x, y) {
            Some(HitTarget::MenuOverflow)
        } else if self.privacy_dots.contains(x, y) {
            Some(HitTarget::PrivacyIndicator)
        } else if self.notch.contains(x, y) {
            Some(HitTarget::Notch)
        } else if self.secondary_expansion > 0.25 && self.notch_secondary.contains(x, y) {
            Some(HitTarget::NotchSecondary)
        } else if self.tray_slot.contains(x, y) {
            Some(HitTarget::Tray)
        } else if self.status_slot.contains(x, y) {
            if self.status_wifi_rect.contains(x, y) {
                Some(HitTarget::StatusWifi)
            } else if self.status_volume_rect.contains(x, y) {
                Some(HitTarget::StatusVolume)
            } else if self.status_battery_rect.contains(x, y) {
                Some(HitTarget::StatusBattery)
            } else {
                Some(HitTarget::QuickSettings)
            }
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

    fn hit_test_expanded_notch(&self, x: i32, y: i32) -> HitTarget {
        if self.pomodoro_surface == PomodoroSurface::Alert {
            if self.pomodoro_alert_start_button.contains(x, y) {
                return HitTarget::PomodoroStartNext;
            }
            if self.pomodoro_alert_extend_button.contains(x, y) {
                return HitTarget::PomodoroExtendFive;
            }
            return HitTarget::Notch;
        }
        for (rect, target) in [
            (self.notch_tab_pomodoro, HitTarget::NotchTabPomodoro),
            (self.notch_tab_shelf, HitTarget::NotchTabShelf),
            (self.notch_tab_clipboard, HitTarget::NotchTabClipboard),
            (self.notch_tab_media, HitTarget::NotchTabMedia),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        if self.privacy_mic_button.contains(x, y) {
            return HitTarget::PrivacyMicMute;
        }
        match self.active_tab {
            NotchExpandedTab::Media => {
                for (rect, target) in [
                    (self.media_lyrics_toggle, HitTarget::MediaLyricsToggle),
                    (self.media_play_button, HitTarget::MediaPlayPause),
                    (self.media_prev_button, HitTarget::MediaPrevious),
                    (self.media_next_button, HitTarget::MediaNext),
                    (self.media_seekbar, HitTarget::MediaSeekbar),
                ] {
                    if rect.contains(x, y) {
                        return target;
                    }
                }
            }
            NotchExpandedTab::Pomodoro => {
                for (mode, rect) in FocusSubMode::ALL.into_iter().zip(self.pomodoro_focus_modes) {
                    if rect.contains(x, y) {
                        return HitTarget::FocusMode(mode);
                    }
                }
                for (rect, target) in [
                    (self.pomodoro_play_button, HitTarget::PomodoroPlayPause),
                    (self.pomodoro_reset_button, HitTarget::PomodoroReset),
                    (self.pomodoro_skip_button, HitTarget::PomodoroSkip),
                ] {
                    if rect.contains(x, y) {
                        return target;
                    }
                }
                for (index, preset) in self.pomodoro_presets.iter().enumerate() {
                    if preset.contains(x, y) {
                        return HitTarget::PomodoroPreset(index as u8);
                    }
                }
            }
            NotchExpandedTab::Shelf => return self.hit_test_shelf(x, y),
            NotchExpandedTab::Clipboard => return self.hit_test_clipboard(x, y),
        }
        HitTarget::Notch
    }

    fn hit_test_clipboard(&self, x: i32, y: i32) -> HitTarget {
        for (rect, target) in [
            (self.clipboard_pause_button, HitTarget::ClipboardPauseToggle),
            (self.clipboard_clear_button, HitTarget::ClipboardClear),
            (self.clipboard_pager_button, HitTarget::ClipboardPager),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        for (index, row) in self.clipboard_rows.iter().enumerate() {
            if row.is_empty() {
                continue;
            }
            if self.clipboard_remove_buttons[index].contains(x, y) {
                return HitTarget::ClipboardRemoveItem(index as u8);
            }
            if row.contains(x, y) {
                return HitTarget::ClipboardItem(index as u8);
            }
        }
        HitTarget::Notch
    }

    fn hit_test_shelf(&self, x: i32, y: i32) -> HitTarget {
        for (rect, target) in [
            (self.shelf_add_button, HitTarget::ShelfAdd),
            (self.shelf_clear_button, HitTarget::ShelfClear),
            (self.shelf_pager_button, HitTarget::ShelfPager),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        for (index, card) in self.shelf_cards.iter().enumerate() {
            if card.is_empty() {
                continue;
            }
            if self.shelf_remove_buttons[index].contains(x, y) {
                return HitTarget::ShelfRemoveItem(index as u8);
            }
            if self.shelf_copy_buttons[index].contains(x, y) {
                return HitTarget::ShelfCopyItem(index as u8);
            }
            if card.contains(x, y) {
                return HitTarget::ShelfItem(index as u8);
            }
        }
        HitTarget::Notch
    }

    #[must_use]
    pub fn is_hidden(&self, target: HitTarget) -> bool {
        self.hidden_targets.contains(&target)
    }

    #[must_use]
    pub fn menu_anchor_rect(&self, target: HitTarget) -> Option<Rect> {
        if self.is_hidden(target) {
            return Some(self.overflow_slot);
        }
        self.slot_rect(target)
    }

    #[must_use]
    pub fn popup_anchor_y(&self, dpi: u32) -> i32 {
        self.bar.height + scale(4, dpi)
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
            HitTarget::MenuOverflow => Some(self.overflow_slot),
            HitTarget::Notch
            | HitTarget::NotchSecondary
            | HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive
            | HitTarget::PomodoroPreset(_)
            | HitTarget::FocusMode(_)
            | HitTarget::NotchTabPomodoro
            | HitTarget::NotchTabShelf
            | HitTarget::NotchTabClipboard
            | HitTarget::NotchTabMedia
            | HitTarget::ShelfAdd
            | HitTarget::ShelfClear
            | HitTarget::ShelfPager
            | HitTarget::ShelfItem(_)
            | HitTarget::ShelfCopyItem(_)
            | HitTarget::ShelfRemoveItem(_)
            | HitTarget::ClipboardPauseToggle
            | HitTarget::ClipboardClear
            | HitTarget::ClipboardPager
            | HitTarget::ClipboardItem(_)
            | HitTarget::ClipboardRemoveItem(_)
            | HitTarget::PrivacyIndicator
            | HitTarget::PrivacyMicMute
            | HitTarget::MediaPlayPause
            | HitTarget::MediaPrevious
            | HitTarget::MediaNext
            | HitTarget::MediaSeekbar
            | HitTarget::MediaLyricsToggle => None,
            HitTarget::Tray => Some(self.tray_slot),
            HitTarget::StatusWifi => Some(self.status_wifi_rect),
            HitTarget::StatusVolume => Some(self.status_volume_rect),
            HitTarget::StatusBattery => Some(self.status_battery_rect),
            HitTarget::QuickSettings => Some(self.status_slot),
            HitTarget::Search => Some(self.search_slot),
            HitTarget::ControlCenter => Some(self.control_center_slot),
            HitTarget::Clock => Some(self.clock_slot),
        }
    }
}

impl Layout {
    #[must_use]
    pub fn notch_control_rect(&self, target: HitTarget) -> Option<Rect> {
        if self.panel_expansion <= 0.55 {
            return None;
        }
        if self.pomodoro_surface == PomodoroSurface::Alert {
            let rect = match target {
                HitTarget::PomodoroExtendFive => self.pomodoro_alert_extend_button,
                HitTarget::PomodoroStartNext => self.pomodoro_alert_start_button,
                _ => return None,
            };
            return (!rect.is_empty()).then_some(rect);
        }
        let rect = match target {
            HitTarget::NotchTabPomodoro => self.notch_tab_pomodoro,
            HitTarget::NotchTabShelf => self.notch_tab_shelf,
            HitTarget::NotchTabClipboard => self.notch_tab_clipboard,
            HitTarget::NotchTabMedia => self.notch_tab_media,
            HitTarget::FocusMode(mode) if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro_focus_modes[mode.index()]
            }
            HitTarget::PomodoroPreset(index) if self.active_tab == NotchExpandedTab::Pomodoro => {
                *self.pomodoro_presets.get(usize::from(index))?
            }
            HitTarget::PomodoroPlayPause if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro_play_button
            }
            HitTarget::PomodoroReset if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro_reset_button
            }
            HitTarget::PomodoroSkip if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro_skip_button
            }
            HitTarget::ShelfAdd if self.active_tab == NotchExpandedTab::Shelf => {
                self.shelf_add_button
            }
            HitTarget::ShelfClear if self.active_tab == NotchExpandedTab::Shelf => {
                self.shelf_clear_button
            }
            HitTarget::ShelfPager if self.active_tab == NotchExpandedTab::Shelf => {
                self.shelf_pager_button
            }
            HitTarget::ShelfItem(index) if self.active_tab == NotchExpandedTab::Shelf => {
                *self.shelf_cards.get(usize::from(index))?
            }
            HitTarget::ShelfCopyItem(index) if self.active_tab == NotchExpandedTab::Shelf => {
                *self.shelf_copy_buttons.get(usize::from(index))?
            }
            HitTarget::ShelfRemoveItem(index) if self.active_tab == NotchExpandedTab::Shelf => {
                *self.shelf_remove_buttons.get(usize::from(index))?
            }
            HitTarget::ClipboardPauseToggle if self.active_tab == NotchExpandedTab::Clipboard => {
                self.clipboard_pause_button
            }
            HitTarget::ClipboardClear if self.active_tab == NotchExpandedTab::Clipboard => {
                self.clipboard_clear_button
            }
            HitTarget::ClipboardPager if self.active_tab == NotchExpandedTab::Clipboard => {
                self.clipboard_pager_button
            }
            HitTarget::ClipboardItem(index) if self.active_tab == NotchExpandedTab::Clipboard => {
                *self.clipboard_rows.get(usize::from(index))?
            }
            HitTarget::ClipboardRemoveItem(index)
                if self.active_tab == NotchExpandedTab::Clipboard =>
            {
                *self.clipboard_remove_buttons.get(usize::from(index))?
            }
            HitTarget::PrivacyMicMute => self.privacy_mic_button,
            HitTarget::MediaPrevious if self.active_tab == NotchExpandedTab::Media => {
                self.media_prev_button
            }
            HitTarget::MediaPlayPause if self.active_tab == NotchExpandedTab::Media => {
                self.media_play_button
            }
            HitTarget::MediaNext if self.active_tab == NotchExpandedTab::Media => {
                self.media_next_button
            }
            HitTarget::MediaLyricsToggle if self.active_tab == NotchExpandedTab::Media => {
                self.media_lyrics_toggle
            }
            HitTarget::MediaSeekbar if self.active_tab == NotchExpandedTab::Media => {
                self.media_seekbar
            }
            _ => return None,
        };
        (!rect.is_empty()).then_some(rect)
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

const LEFT_TARGETS: [HitTarget; 7] = [
    HitTarget::Start,
    HitTarget::AppMenu,
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
];

const RIGHT_VISUAL_ORDER: [HitTarget; 5] = [
    HitTarget::Clock,
    HitTarget::ControlCenter,
    HitTarget::Search,
    HitTarget::QuickSettings,
    HitTarget::Tray,
];

const RIGHT_KEEP_PRIORITY: [HitTarget; 5] = [
    HitTarget::Clock,
    HitTarget::ControlCenter,
    HitTarget::QuickSettings,
    HitTarget::Search,
    HitTarget::Tray,
];

struct SlotRow {
    slots: Vec<(HitTarget, Rect)>,
    hidden: Vec<HitTarget>,
    overflow: Rect,
}

impl SlotRow {
    fn rect(&self, target: HitTarget) -> Rect {
        self.slots
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map_or(self.overflow_anchor(), |(_, rect)| *rect)
    }

    const fn overflow_anchor(&self) -> Rect {
        Rect {
            x: self.overflow.x,
            y: 0,
            width: 0,
            height: self.overflow.height,
        }
    }
}

fn resolve_right_slots(
    widths: &[(HitTarget, i32)],
    gap: i32,
    left_limit: i32,
    right_edge: i32,
    height: i32,
) -> (Vec<(HitTarget, Rect)>, Vec<HitTarget>) {
    let available = (right_edge - left_limit).max(0);
    let width_of = |target: HitTarget| {
        widths
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map_or(0, |(_, width)| *width)
    };
    let mut kept = Vec::new();
    let mut hidden = Vec::new();
    let mut used = 0;
    for target in RIGHT_KEEP_PRIORITY {
        let width = width_of(target);
        if width <= 0 {
            continue;
        }
        let needed = if kept.is_empty() { width } else { width + gap };
        if used + needed <= available {
            used += needed;
            kept.push(target);
        } else {
            hidden.push(target);
        }
    }
    let mut cursor = right_edge;
    let mut slots = Vec::new();
    for target in RIGHT_VISUAL_ORDER {
        if !kept.contains(&target) {
            continue;
        }
        let width = width_of(target);
        slots.push((
            target,
            Rect {
                x: cursor - width,
                y: 0,
                width,
                height,
            },
        ));
        cursor -= width + gap;
    }
    (slots, hidden)
}

struct LeftSlotInput<'a> {
    widths: &'a [i32; 7],
    overflow_width: i32,
    gap: i32,
    start: i32,
    limit: i32,
    height: i32,
    force_overflow: bool,
}

fn resolve_left_slots(input: &LeftSlotInput<'_>) -> SlotRow {
    let total: i32 = input.widths.iter().sum::<i32>() + input.gap * 6;
    let needs_overflow = input.force_overflow || input.start + total > input.limit;
    let reserved = if needs_overflow {
        input.overflow_width + input.gap
    } else {
        0
    };
    let mut cursor = input.start;
    let mut slots = Vec::new();
    let mut hidden = Vec::new();
    for (target, width) in LEFT_TARGETS.into_iter().zip(*input.widths) {
        if hidden.is_empty() && cursor + width + reserved <= input.limit {
            slots.push((
                target,
                Rect {
                    x: cursor,
                    y: 0,
                    width,
                    height: input.height,
                },
            ));
            cursor += width + input.gap;
        } else {
            hidden.push(target);
        }
    }
    let overflow = Rect {
        x: cursor.min(input.limit),
        y: 0,
        width: if needs_overflow {
            input.overflow_width.min((input.limit - cursor).max(0))
        } else {
            0
        },
        height: input.height,
    };
    SlotRow {
        slots,
        hidden,
        overflow,
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
            ..NotchLayoutInput::default()
        },
    )
}

struct ExpandedNotchControls {
    notch_tab_strip: Rect,
    icon: Rect,
    time: Rect,
    subtitle: Rect,
    session_label: Rect,
    segments: [Rect; 4],
    footer_left: Rect,
    footer_right: Rect,
    presets: [Rect; 4],
    focus_modes: [Rect; 3],
    play_button: Rect,
    reset_button: Rect,
    skip_button: Rect,
    alert_start_button: Rect,
    alert_extend_button: Rect,
    notch_tab_pomodoro: Rect,
    notch_tab_shelf: Rect,
    notch_tab_clipboard: Rect,
    notch_tab_media: Rect,
    shelf_add_button: Rect,
    shelf_clear_button: Rect,
    shelf_pager_button: Rect,
    shelf_dropzone: Rect,
    shelf_cards: [Rect; SHELF_PAGE_SIZE],
    shelf_copy_buttons: [Rect; SHELF_PAGE_SIZE],
    shelf_remove_buttons: [Rect; SHELF_PAGE_SIZE],
    clipboard_pause_button: Rect,
    clipboard_clear_button: Rect,
    clipboard_pager_button: Rect,
    clipboard_body: Rect,
    clipboard_rows: [Rect; CLIPBOARD_PAGE_SIZE],
    clipboard_remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
    privacy_mic_button: Rect,
    media_cover_art: Rect,
    media_source_label: Rect,
    media_title: Rect,
    media_artist: Rect,
    media_reason: Rect,
    media_equalizer: Rect,
    media_lyrics_toggle: Rect,
    media_lyrics_drawer: Rect,
    media_seekbar: Rect,
    media_play_button: Rect,
    media_prev_button: Rect,
    media_next_button: Rect,
}

struct HeroButtons {
    left: Rect,
    primary: Rect,
    right: Rect,
}

fn hero_buttons(
    content_right: i32,
    hero_cy: i32,
    primary: i32,
    secondary: i32,
    gap: i32,
) -> HeroButtons {
    let right = Rect {
        x: content_right - secondary,
        y: hero_cy - secondary / 2,
        width: secondary,
        height: secondary,
    };
    let middle = Rect {
        x: right.x - gap - primary,
        y: hero_cy - primary / 2,
        width: primary,
        height: primary,
    };
    let left = Rect {
        x: middle.x - gap - secondary,
        y: hero_cy - secondary / 2,
        width: secondary,
        height: secondary,
    };
    HeroButtons {
        left,
        primary: middle,
        right,
    }
}

struct CompactNotchGeometry {
    ring: Rect,
    peek: Rect,
    time: Rect,
    title: Rect,
    trailing: Rect,
    privacy_dots: Rect,
}

fn compute_compact_notch_geometry(
    notch: Rect,
    bar_height: i32,
    dpi: u32,
    privacy_dots_visible: bool,
    media_live: bool,
) -> CompactNotchGeometry {
    let ring_size = scale(COMPACT_RING_POINTS, dpi);
    let band_y = notch.y;
    let band_height = (bar_height - notch.y * 2).max(ring_size);
    let ring = Rect {
        x: notch.x + scale(COMPACT_RING_INSET_POINTS, dpi),
        y: band_y + (band_height - ring_size) / 2,
        width: ring_size,
        height: ring_size,
    };
    let privacy_reserve = if privacy_dots_visible {
        scale(PRIVACY_COMPACT_EXTRA_POINTS, dpi)
    } else {
        0
    };
    let privacy_dots = if privacy_dots_visible {
        let dots_w = scale(COMPACT_PRIVACY_WIDTH_POINTS, dpi);
        let dots_inset = if notch.y > 0 {
            scale(COMPACT_FLOATING_PRIVACY_INSET_POINTS, dpi)
        } else {
            scale(COMPACT_PRIVACY_INSET_POINTS, dpi)
        };
        Rect {
            x: notch.right() - dots_inset - dots_w,
            y: band_y,
            width: dots_w,
            height: band_height,
        }
    } else {
        Rect::default()
    };
    let time_width = scale(COMPACT_TIME_WIDTH_POINTS, dpi);
    let time = Rect {
        x: notch.right() - scale(COMPACT_TIME_INSET_POINTS, dpi) - time_width - privacy_reserve,
        y: band_y,
        width: time_width,
        height: band_height,
    };
    let peek_gap = scale(COMPACT_PEEK_GAP_POINTS, dpi);
    let peek_left = ring.right() + peek_gap;
    let peek = Rect {
        x: peek_left,
        y: band_y,
        width: (time.x - peek_gap - peek_left).max(0),
        height: band_height,
    };
    let trailing_width_pts = if media_live {
        COMPACT_LIVE_TRAILING_WIDTH_POINTS
    } else {
        COMPACT_TRAILING_WIDTH_POINTS
    };
    let trailing_width = scale(trailing_width_pts, dpi);
    let trailing = Rect {
        x: notch.right()
            - scale(COMPACT_TRAILING_INSET_POINTS, dpi)
            - trailing_width
            - privacy_reserve,
        y: band_y,
        width: trailing_width,
        height: band_height,
    };
    let title = Rect {
        x: peek_left,
        y: band_y,
        width: (trailing.x - peek_gap - peek_left).max(0),
        height: band_height,
    };
    CompactNotchGeometry {
        ring,
        peek,
        time,
        title,
        trailing,
        privacy_dots,
    }
}

const COMPACT_RING_POINTS: i32 = 20;
const COMPACT_RING_INSET_POINTS: i32 = 8;
const COMPACT_TIME_WIDTH_POINTS: i32 = 56;
const COMPACT_TIME_INSET_POINTS: i32 = 14;
const COMPACT_PEEK_GAP_POINTS: i32 = 8;
const COMPACT_TRAILING_WIDTH_POINTS: i32 = 16;
const COMPACT_LIVE_TRAILING_WIDTH_POINTS: i32 = 34;
const COMPACT_TRAILING_INSET_POINTS: i32 = 12;
const COMPACT_PRIVACY_INSET_POINTS: i32 = 6;
const COMPACT_FLOATING_PRIVACY_INSET_POINTS: i32 = 8;
const COMPACT_PRIVACY_WIDTH_POINTS: i32 = 14;
const COMPACT_TITLE_SLACK_POINTS: i32 = 6;
const COMPACT_MAX_WIDTH_POINTS: i32 = 520;
const SECONDARY_NOTCH_SIZE_POINTS: i32 = 28;
const SECONDARY_NOTCH_GAP_POINTS: i32 = 8;
const SECONDARY_NOTCH_MIN_SIZE_POINTS: i32 = 12;
const SECONDARY_NOTCH_SWAP_PULL_POINTS: i32 = 4;

fn compute_secondary_notch_rect(
    notch: Rect,
    metrics: &Metrics,
    dpi: u32,
    secondary_expansion: f32,
    swap_blend: f32,
) -> Rect {
    let t = secondary_expansion.clamp(0.0, 1.0);
    if t <= 0.005 {
        return Rect::default();
    }
    let floating = metrics.notch_top_inset > 0;
    let max_size_points = if floating {
        metrics.notch_height
    } else {
        metrics.notch_height.min(SECONDARY_NOTCH_SIZE_POINTS)
    };
    let target_size_pts = max_size_points.max(SECONDARY_NOTCH_MIN_SIZE_POINTS);
    let size_pts = lerp_points(SECONDARY_NOTCH_MIN_SIZE_POINTS, target_size_pts, t);
    let size = scale(size_pts, dpi);
    let swap_pull = (swap_blend.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
    let pull_px = (scale(SECONDARY_NOTCH_SWAP_PULL_POINTS, dpi) as f32 * swap_pull).round() as i32;
    let start_x = notch.right() - size;
    let target_x = notch.right() + scale(SECONDARY_NOTCH_GAP_POINTS, dpi) - pull_px;
    let x = start_x + ((target_x - start_x) as f32 * t).round() as i32;
    let y = if floating {
        let collapsed_notch_h = scale(metrics.notch_height, dpi);
        notch.y + (collapsed_notch_h - size) / 2
    } else {
        (scale(metrics.bar_height, dpi) - size) / 2
    };
    Rect {
        x,
        y,
        width: size,
        height: size,
    }
}

fn compact_width_points(metrics: &Metrics, title_points: i32, media_live: bool) -> i32 {
    if title_points <= 0 {
        return metrics.notch_compact_width;
    }
    let trailing_pts = if media_live {
        COMPACT_LIVE_TRAILING_WIDTH_POINTS
    } else {
        COMPACT_TRAILING_WIDTH_POINTS
    };
    let chrome = COMPACT_RING_INSET_POINTS
        + COMPACT_RING_POINTS
        + COMPACT_PEEK_GAP_POINTS * 2
        + trailing_pts
        + COMPACT_TRAILING_INSET_POINTS;
    (chrome + title_points + COMPACT_TITLE_SLACK_POINTS)
        .min(COMPACT_MAX_WIDTH_POINTS.max(metrics.notch_compact_width))
        .max(metrics.notch_compact_width)
}

const PANEL_PAD_POINTS: i32 = 16;
const PANEL_TAB_TOP_POINTS: i32 = 6;
const PANEL_TAB_HEIGHT_POINTS: i32 = 24;
const PANEL_TAB_ACTIVE_WIDTH_POINTS: i32 = 64;
const PANEL_TAB_ICON_WIDTH_POINTS: i32 = 24;
const PANEL_TAB_GAP_POINTS: i32 = 4;
const PANEL_TAB_STRIP_INSET_POINTS: i32 = 2;
const PANEL_SESSION_LABEL_WIDTH_POINTS: i32 = 72;
const FOCUS_MODE_WIDTH_POINTS: i32 = 24;
const PANEL_HERO_CENTER_POINTS: i32 = 62;
const PANEL_HERO_ICON_POINTS: i32 = 48;
const PANEL_HERO_TEXT_GAP_POINTS: i32 = 12;
const PANEL_TIME_HEIGHT_POINTS: i32 = 32;
const PANEL_SUBTITLE_HEIGHT_POINTS: i32 = 16;
const PANEL_PRIMARY_BUTTON_POINTS: i32 = 40;
const PANEL_SECONDARY_BUTTON_POINTS: i32 = 32;
const PANEL_BUTTON_GAP_POINTS: i32 = 8;
const PANEL_PROGRESS_TOP_POINTS: i32 = 98;
const PANEL_PROGRESS_HEIGHT_POINTS: i32 = 4;
const PANEL_SEGMENT_GAP_POINTS: i32 = 6;
const PANEL_FOOTER_TOP_POINTS: i32 = 108;
const PANEL_FOOTER_HEIGHT_POINTS: i32 = 20;
const PANEL_FOOTER_LEFT_SHARE_PERCENT: i32 = 64;
const PANEL_PRESET_TOP_POINTS: i32 = 98;
const PANEL_PRESET_WIDTH_POINTS: i32 = 58;
const PANEL_PRESET_HEIGHT_POINTS: i32 = 24;
const MEDIA_LYRICS_TOGGLE_POINTS: i32 = 24;
const MEDIA_SOURCE_LABEL_WIDTH_POINTS: i32 = 96;
const MEDIA_TITLE_HEIGHT_POINTS: i32 = 20;
const MEDIA_ARTIST_HEIGHT_POINTS: i32 = 16;
const MEDIA_REASON_HEIGHT_POINTS: i32 = 28;
const MEDIA_EQUALIZER_WIDTH_POINTS: i32 = 24;
const MEDIA_EQUALIZER_HEIGHT_POINTS: i32 = 10;
const MEDIA_SEEK_TOP_POINTS: i32 = 98;
const MEDIA_SEEK_HEIGHT_POINTS: i32 = 24;
const MEDIA_DRAWER_TOP_GAP_POINTS: i32 = 8;
const MEDIA_DRAWER_BOTTOM_GAP_POINTS: i32 = 12;
const PANEL_ALERT_TITLE_TOP_POINTS: i32 = 40;
const PANEL_ALERT_TITLE_HEIGHT_POINTS: i32 = 24;
const PANEL_ALERT_BUTTON_HEIGHT_POINTS: i32 = 32;
const PANEL_ALERT_START_WIDTH_POINTS: i32 = 104;
const PANEL_ALERT_EXTEND_WIDTH_POINTS: i32 = 72;
const PRIVACY_COMPACT_MIC_WIDTH_POINTS: i32 = 40;
const PRIVACY_PANEL_MIC_WIDTH_POINTS: i32 = 88;

const SHELF_ADD_WIDTH_POINTS: i32 = 40;
const SHELF_CLEAR_WIDTH_POINTS: i32 = 38;
const SHELF_PAGER_WIDTH_POINTS: i32 = 28;
const SHELF_BODY_TOP_POINTS: i32 = 38;
const SHELF_BODY_HEIGHT_POINTS: i32 = 90;
const SHELF_GRID_COLS: usize = 3;
const SHELF_GRID_GAP_POINTS: i32 = 6;
const SHELF_CARD_MINI_POINTS: i32 = 18;
const SHELF_CARD_MINI_PAD_POINTS: i32 = 3;
const SHELF_CARD_MINI_GAP_POINTS: i32 = 2;

struct ShelfControlsGeometry {
    add_button: Rect,
    clear_button: Rect,
    pager_button: Rect,
    mic_button: Rect,
    dropzone: Rect,
    cards: [Rect; SHELF_PAGE_SIZE],
    copy_buttons: [Rect; SHELF_PAGE_SIZE],
    remove_buttons: [Rect; SHELF_PAGE_SIZE],
}

struct ShelfHeaderButtons {
    add: Rect,
    clear: Rect,
    pager: Rect,
    mic: Rect,
}

fn compute_shelf_header_buttons(
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    shelf: ShelfLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> ShelfHeaderButtons {
    let gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let clear = if shelf.has_items {
        let clear_w = scale(SHELF_CLEAR_WIDTH_POINTS, dpi);
        Rect {
            x: content_right - clear_w,
            y: tab_y,
            width: clear_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let add_right = if clear.is_empty() {
        content_right
    } else {
        clear.x - gap
    };
    let add_w = scale(SHELF_ADD_WIDTH_POINTS, dpi);
    let add = Rect {
        x: add_right - add_w,
        y: tab_y,
        width: add_w,
        height: tab_h,
    };
    let pager = if shelf.has_pager {
        let pager_w = scale(SHELF_PAGER_WIDTH_POINTS, dpi);
        Rect {
            x: add.x - gap - pager_w,
            y: tab_y,
            width: pager_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let mic_right = if pager.is_empty() { add.x } else { pager.x };
    let mic = if panel_mic_control {
        let mic_w = scale(PRIVACY_COMPACT_MIC_WIDTH_POINTS, dpi);
        Rect {
            x: mic_right - gap - mic_w,
            y: tab_y,
            width: mic_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    ShelfHeaderButtons {
        add,
        clear,
        pager,
        mic,
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_shelf_controls(
    notch_y: i32,
    content_left: i32,
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    shelf: ShelfLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> ShelfControlsGeometry {
    let header =
        compute_shelf_header_buttons(content_right, tab_y, tab_h, shelf, panel_mic_control, dpi);
    let body_top = notch_y + scale(SHELF_BODY_TOP_POINTS, dpi);
    let body_h = scale(SHELF_BODY_HEIGHT_POINTS, dpi);
    let body_w = (content_right - content_left).max(0);
    let dropzone = Rect {
        x: content_left,
        y: body_top,
        width: body_w,
        height: body_h,
    };
    let grid_gap = scale(SHELF_GRID_GAP_POINTS, dpi);
    let card_w = ((body_w - grid_gap * 2) / SHELF_GRID_COLS as i32).max(0);
    let card_h = ((body_h - grid_gap) / 2).max(0);
    let mini_size = scale(SHELF_CARD_MINI_POINTS, dpi);
    let mini_pad = scale(SHELF_CARD_MINI_PAD_POINTS, dpi);
    let mini_gap = scale(SHELF_CARD_MINI_GAP_POINTS, dpi);
    let visible = usize::from(shelf.visible_count).min(SHELF_PAGE_SIZE);
    let mut cards = [Rect::default(); SHELF_PAGE_SIZE];
    let mut copy_buttons = [Rect::default(); SHELF_PAGE_SIZE];
    let mut remove_buttons = [Rect::default(); SHELF_PAGE_SIZE];
    for index in 0..visible {
        let col = (index % SHELF_GRID_COLS) as i32;
        let row = (index / SHELF_GRID_COLS) as i32;
        let card = Rect {
            x: content_left + col * (card_w + grid_gap),
            y: body_top + row * (card_h + grid_gap),
            width: card_w,
            height: card_h,
        };
        let remove = Rect {
            x: card.right() - mini_pad - mini_size,
            y: card.y + mini_pad,
            width: mini_size,
            height: mini_size,
        };
        let copy = Rect {
            x: remove.x - mini_gap - mini_size,
            y: card.y + mini_pad,
            width: mini_size,
            height: mini_size,
        };
        cards[index] = card;
        copy_buttons[index] = copy;
        remove_buttons[index] = remove;
    }
    ShelfControlsGeometry {
        add_button: header.add,
        clear_button: header.clear,
        pager_button: header.pager,
        mic_button: header.mic,
        dropzone,
        cards,
        copy_buttons,
        remove_buttons,
    }
}

const CLIPBOARD_PAUSE_WIDTH_POINTS: i32 = 42;
const CLIPBOARD_CLEAR_WIDTH_POINTS: i32 = 38;
const CLIPBOARD_PAGER_WIDTH_POINTS: i32 = 28;
const CLIPBOARD_BODY_TOP_POINTS: i32 = 38;
const CLIPBOARD_BODY_HEIGHT_POINTS: i32 = 90;
const CLIPBOARD_ROW_GAP_POINTS: i32 = 3;
const CLIPBOARD_ROW_REMOVE_POINTS: i32 = 16;
const CLIPBOARD_ROW_REMOVE_PAD_POINTS: i32 = 4;

struct ClipboardControlsGeometry {
    pause_button: Rect,
    clear_button: Rect,
    pager_button: Rect,
    mic_button: Rect,
    body: Rect,
    rows: [Rect; CLIPBOARD_PAGE_SIZE],
    remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
}

#[allow(clippy::too_many_arguments)]
fn compute_clipboard_controls(
    notch_y: i32,
    content_left: i32,
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    clipboard: ClipboardLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> ClipboardControlsGeometry {
    let gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let clear_button = if clipboard.has_items {
        let clear_w = scale(CLIPBOARD_CLEAR_WIDTH_POINTS, dpi);
        Rect {
            x: content_right - clear_w,
            y: tab_y,
            width: clear_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let pause_right = if clear_button.is_empty() {
        content_right
    } else {
        clear_button.x - gap
    };
    let pause_w = scale(CLIPBOARD_PAUSE_WIDTH_POINTS, dpi);
    let pause_button = Rect {
        x: pause_right - pause_w,
        y: tab_y,
        width: pause_w,
        height: tab_h,
    };
    let pager_button = if clipboard.has_pager {
        let pager_w = scale(CLIPBOARD_PAGER_WIDTH_POINTS, dpi);
        Rect {
            x: pause_button.x - gap - pager_w,
            y: tab_y,
            width: pager_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let mic_right = if pager_button.is_empty() {
        pause_button.x
    } else {
        pager_button.x
    };
    let mic_button = if panel_mic_control {
        let mic_w = scale(PRIVACY_COMPACT_MIC_WIDTH_POINTS, dpi);
        Rect {
            x: mic_right - gap - mic_w,
            y: tab_y,
            width: mic_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };

    let body_top = notch_y + scale(CLIPBOARD_BODY_TOP_POINTS, dpi);
    let body_h = scale(CLIPBOARD_BODY_HEIGHT_POINTS, dpi);
    let body_w = (content_right - content_left).max(0);
    let body = Rect {
        x: content_left,
        y: body_top,
        width: body_w,
        height: body_h,
    };
    let row_gap = scale(CLIPBOARD_ROW_GAP_POINTS, dpi);
    let row_h =
        ((body_h - row_gap * (CLIPBOARD_PAGE_SIZE as i32 - 1)) / CLIPBOARD_PAGE_SIZE as i32).max(0);
    let remove_size = scale(CLIPBOARD_ROW_REMOVE_POINTS, dpi);
    let remove_pad = scale(CLIPBOARD_ROW_REMOVE_PAD_POINTS, dpi);
    let visible = usize::from(clipboard.visible_count).min(CLIPBOARD_PAGE_SIZE);
    let mut rows = [Rect::default(); CLIPBOARD_PAGE_SIZE];
    let mut remove_buttons = [Rect::default(); CLIPBOARD_PAGE_SIZE];
    for index in 0..visible {
        let row = Rect {
            x: content_left,
            y: body_top + (row_h + row_gap) * index as i32,
            width: body_w,
            height: row_h,
        };
        let remove = Rect {
            x: row.right() - remove_pad - remove_size,
            y: row.y + (row_h - remove_size) / 2,
            width: remove_size,
            height: remove_size,
        };
        rows[index] = row;
        remove_buttons[index] = remove;
    }
    ClipboardControlsGeometry {
        pause_button,
        clear_button,
        pager_button,
        mic_button,
        body,
        rows,
        remove_buttons,
    }
}

fn compute_focus_mode_rects(
    surface: PomodoroSurface,
    session_left: i32,
    tab_y: i32,
    tab_h: i32,
    dpi: u32,
) -> [Rect; 3] {
    if surface == PomodoroSurface::Alert {
        return [Rect::default(); 3];
    }
    let gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let item_w = scale(FOCUS_MODE_WIDTH_POINTS, dpi);
    let total_w = item_w * 3 + gap * 2;
    let x0 = session_left - gap - total_w;
    let r0 = Rect {
        x: x0,
        y: tab_y,
        width: item_w,
        height: tab_h,
    };
    let r1 = Rect {
        x: r0.right() + gap,
        y: tab_y,
        width: item_w,
        height: tab_h,
    };
    let r2 = Rect {
        x: r1.right() + gap,
        y: tab_y,
        width: item_w,
        height: tab_h,
    };
    [r0, r1, r2]
}

#[allow(clippy::too_many_lines)]
fn compute_expanded_notch_controls(
    notch: Rect,
    dpi: u32,
    notch_input: &NotchLayoutInput,
    metrics: &Metrics,
) -> ExpandedNotchControls {
    let oy = notch.y;
    let lyrics_progress = notch_input.lyrics_progress.clamp(0.0, 1.0);
    let surface = notch_input.pomodoro_surface;
    let active_tab = notch_input.active_tab;
    let media_present = notch_input.media_present;
    let panel_height_points = metrics.notch_expanded_height;
    let lyrics_height_points = metrics.notch_lyrics_height;
    let pad_x = scale(PANEL_PAD_POINTS, dpi);
    let tab_y = oy + scale(PANEL_TAB_TOP_POINTS, dpi);
    let tab_h = scale(PANEL_TAB_HEIGHT_POINTS, dpi);
    let tab_active_w = scale(PANEL_TAB_ACTIVE_WIDTH_POINTS, dpi);
    let tab_icon_w = scale(PANEL_TAB_ICON_WIDTH_POINTS, dpi);
    let tab_gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let strip_inset = scale(PANEL_TAB_STRIP_INSET_POINTS, dpi);
    let tab_width = |tab: NotchExpandedTab| {
        if active_tab == tab {
            tab_active_w
        } else {
            tab_icon_w
        }
    };

    let mut cursor_x = notch.x + pad_x;
    let mut first_tab_x: Option<i32> = None;
    let mut strip_right = cursor_x;
    let mut allocate_tab = |tab: NotchExpandedTab, enabled: bool| -> Rect {
        if !enabled {
            return Rect::default();
        }
        let rect = Rect {
            x: cursor_x,
            y: tab_y,
            width: tab_width(tab),
            height: tab_h,
        };
        first_tab_x.get_or_insert(rect.x);
        strip_right = rect.right();
        cursor_x = rect.right() + tab_gap;
        rect
    };

    let notch_tab_pomodoro = allocate_tab(NotchExpandedTab::Pomodoro, notch_input.focus_enabled);
    let notch_tab_shelf = allocate_tab(NotchExpandedTab::Shelf, notch_input.shelf_enabled);
    let notch_tab_clipboard =
        allocate_tab(NotchExpandedTab::Clipboard, notch_input.clipboard_enabled);
    let notch_tab_media = allocate_tab(NotchExpandedTab::Media, media_present);
    let notch_tab_strip = match first_tab_x {
        Some(left) => Rect {
            x: left - strip_inset,
            y: tab_y - strip_inset,
            width: strip_right - left + strip_inset * 2,
            height: tab_h + strip_inset * 2,
        },
        None => Rect::default(),
    };
    let session_label_w = scale(PANEL_SESSION_LABEL_WIDTH_POINTS, dpi);
    let session_label = Rect {
        x: notch.right() - pad_x - session_label_w,
        y: tab_y,
        width: session_label_w,
        height: tab_h,
    };
    let focus_modes = compute_focus_mode_rects(surface, session_label.x, tab_y, tab_h, dpi);

    let content_left = notch.x + pad_x;
    let content_right = notch.right() - pad_x;
    let content_width = content_right - content_left;
    let hero_cy = oy + scale(PANEL_HERO_CENTER_POINTS, dpi);
    let icon_size = scale(PANEL_HERO_ICON_POINTS, dpi);
    let text_gap = scale(PANEL_HERO_TEXT_GAP_POINTS, dpi);
    let primary = scale(PANEL_PRIMARY_BUTTON_POINTS, dpi);
    let secondary = scale(PANEL_SECONDARY_BUTTON_POINTS, dpi);
    let btn_gap = scale(PANEL_BUTTON_GAP_POINTS, dpi);
    let alert = surface == PomodoroSurface::Alert;
    let setup = surface == PomodoroSurface::Setup;

    let icon = Rect {
        x: content_left,
        y: hero_cy - icon_size / 2,
        width: icon_size,
        height: icon_size,
    };
    let text_left = icon.right() + text_gap;

    let mut play_button = Rect::default();
    let mut reset_button = Rect::default();
    let mut skip_button = Rect::default();
    if !alert {
        if setup
            || (notch_input.focus_mode == FocusSubMode::Stopwatch
                && surface == PomodoroSurface::Ready)
        {
            play_button = Rect {
                x: content_right - primary,
                y: hero_cy - primary / 2,
                width: primary,
                height: primary,
            };
        } else {
            let buttons = hero_buttons(content_right, hero_cy, primary, secondary, btn_gap);
            play_button = buttons.primary;
            reset_button = buttons.left;
            skip_button = if notch_input.focus_mode == FocusSubMode::Pomodoro
                || (notch_input.focus_mode == FocusSubMode::Stopwatch
                    && surface == PomodoroSurface::Running)
            {
                buttons.right
            } else {
                Rect::default()
            };
        }
    }
    let text_right = if alert {
        content_right
    } else if !reset_button.is_empty() {
        reset_button.x - btn_gap
    } else {
        play_button.x - btn_gap
    };
    let text_width = (text_right - text_left).max(0);

    let (time, subtitle) = if alert {
        let title_h = scale(PANEL_ALERT_TITLE_HEIGHT_POINTS, dpi);
        let title_top = oy + scale(PANEL_ALERT_TITLE_TOP_POINTS, dpi);
        (
            Rect {
                x: text_left,
                y: title_top,
                width: text_width,
                height: title_h,
            },
            Rect {
                x: text_left,
                y: title_top + title_h,
                width: text_width,
                height: scale(PANEL_SUBTITLE_HEIGHT_POINTS, dpi),
            },
        )
    } else {
        let time_h = scale(PANEL_TIME_HEIGHT_POINTS, dpi);
        let subtitle_h = scale(PANEL_SUBTITLE_HEIGHT_POINTS, dpi);
        let block_top = hero_cy - i32::midpoint(time_h, subtitle_h);
        (
            Rect {
                x: text_left,
                y: block_top,
                width: text_width,
                height: time_h,
            },
            Rect {
                x: text_left,
                y: block_top + time_h,
                width: text_width,
                height: subtitle_h,
            },
        )
    };

    let mut segments = [Rect::default(); 4];
    let mut footer_left = Rect::default();
    let mut footer_right = Rect::default();
    if !alert && !setup {
        let segment_gap = scale(PANEL_SEGMENT_GAP_POINTS, dpi);
        let segment_w = (content_width - segment_gap * 3) / 4;
        let segment_y = oy + scale(PANEL_PROGRESS_TOP_POINTS, dpi);
        let segment_h = scale(PANEL_PROGRESS_HEIGHT_POINTS, dpi);
        for (index, segment) in segments.iter_mut().enumerate() {
            let x = content_left + (segment_w + segment_gap) * index as i32;
            let width = if index == 3 {
                content_right - x
            } else {
                segment_w
            };
            *segment = Rect {
                x,
                y: segment_y,
                width,
                height: segment_h,
            };
        }
        let footer_y = oy + scale(PANEL_FOOTER_TOP_POINTS, dpi);
        let footer_h = scale(PANEL_FOOTER_HEIGHT_POINTS, dpi);
        let footer_left_w = content_width * PANEL_FOOTER_LEFT_SHARE_PERCENT / 100;
        footer_left = Rect {
            x: content_left,
            y: footer_y,
            width: footer_left_w,
            height: footer_h,
        };
        footer_right = Rect {
            x: content_left + footer_left_w,
            y: footer_y,
            width: content_width - footer_left_w,
            height: footer_h,
        };
    }

    let mut presets = [Rect::default(); 4];
    if setup {
        let preset_w = scale(PANEL_PRESET_WIDTH_POINTS, dpi);
        let preset_h = scale(PANEL_PRESET_HEIGHT_POINTS, dpi);
        let preset_y = oy + scale(PANEL_PRESET_TOP_POINTS, dpi);
        for (index, preset) in presets.iter_mut().enumerate() {
            *preset = Rect {
                x: content_left + (preset_w + btn_gap) * index as i32,
                y: preset_y,
                width: preset_w,
                height: preset_h,
            };
        }
    }

    let alert_btn_h = scale(PANEL_ALERT_BUTTON_HEIGHT_POINTS, dpi);
    let alert_start_w = scale(PANEL_ALERT_START_WIDTH_POINTS, dpi);
    let alert_extend_w = scale(PANEL_ALERT_EXTEND_WIDTH_POINTS, dpi);
    let alert_y = oy + scale(panel_height_points - PANEL_PAD_POINTS, dpi) - alert_btn_h;
    let alert_start_x = content_right - alert_start_w;
    let alert_extend_x = alert_start_x - btn_gap - alert_extend_w;
    let alert_start_button = Rect {
        x: alert_start_x,
        y: alert_y,
        width: alert_start_w,
        height: alert_btn_h,
    };
    let alert_extend_button = Rect {
        x: alert_extend_x,
        y: alert_y,
        width: alert_extend_w,
        height: alert_btn_h,
    };

    let media_cover_art = icon;
    let lyrics_toggle_size = scale(MEDIA_LYRICS_TOGGLE_POINTS, dpi);
    let media_lyrics_toggle = Rect {
        x: content_right - lyrics_toggle_size,
        y: tab_y,
        width: lyrics_toggle_size,
        height: lyrics_toggle_size,
    };
    let source_width = scale(MEDIA_SOURCE_LABEL_WIDTH_POINTS, dpi);
    let media_source_label = Rect {
        x: media_lyrics_toggle.x - btn_gap - source_width,
        y: tab_y,
        width: source_width,
        height: tab_h,
    };

    let media_buttons = hero_buttons(content_right, hero_cy, primary, secondary, btn_gap);
    let media_text_width = (media_buttons.left.x - btn_gap - text_left).max(0);
    let title_h = scale(MEDIA_TITLE_HEIGHT_POINTS, dpi);
    let artist_h = scale(MEDIA_ARTIST_HEIGHT_POINTS, dpi);
    let media_title = Rect {
        x: text_left,
        y: icon.y,
        width: media_text_width,
        height: title_h,
    };
    let media_artist = Rect {
        x: text_left,
        y: icon.y + title_h,
        width: media_text_width,
        height: artist_h,
    };
    let media_reason = Rect {
        x: text_left,
        y: icon.y + title_h,
        width: media_text_width,
        height: scale(MEDIA_REASON_HEIGHT_POINTS, dpi),
    };
    let equalizer_h = scale(MEDIA_EQUALIZER_HEIGHT_POINTS, dpi);
    let media_equalizer = Rect {
        x: text_left,
        y: icon.bottom() - equalizer_h,
        width: scale(MEDIA_EQUALIZER_WIDTH_POINTS, dpi),
        height: equalizer_h,
    };

    let seek_travel = (lyrics_height_points - panel_height_points).max(0);
    let media_seek_y = oy
        + scale(
            lerp_points(
                MEDIA_SEEK_TOP_POINTS,
                MEDIA_SEEK_TOP_POINTS + seek_travel,
                lyrics_progress,
            ),
            dpi,
        );
    let media_seekbar = Rect {
        x: content_left,
        y: media_seek_y,
        width: content_width,
        height: scale(MEDIA_SEEK_HEIGHT_POINTS, dpi),
    };
    let drawer_top = icon.bottom() + scale(MEDIA_DRAWER_TOP_GAP_POINTS, dpi);
    let media_lyrics_drawer = Rect {
        x: content_left,
        y: drawer_top,
        width: content_width,
        height: (media_seek_y - scale(MEDIA_DRAWER_BOTTOM_GAP_POINTS, dpi) - drawer_top).max(0),
    };
    let shelf = compute_shelf_controls(
        oy,
        content_left,
        content_right,
        tab_y,
        tab_h,
        notch_input.shelf,
        notch_input.privacy.panel_mic_control,
        dpi,
    );
    let clipboard = compute_clipboard_controls(
        oy,
        content_left,
        content_right,
        tab_y,
        tab_h,
        notch_input.clipboard,
        notch_input.privacy.panel_mic_control,
        dpi,
    );
    let privacy_mic_button = match active_tab {
        NotchExpandedTab::Shelf => shelf.mic_button,
        NotchExpandedTab::Clipboard => clipboard.mic_button,
        NotchExpandedTab::Media if notch_input.privacy.panel_mic_control => {
            let mic_w = scale(PRIVACY_PANEL_MIC_WIDTH_POINTS, dpi);
            Rect {
                x: content_right - mic_w,
                y: tab_y,
                width: mic_w,
                height: tab_h,
            }
        }
        _ => Rect::default(),
    };

    ExpandedNotchControls {
        notch_tab_strip,
        icon,
        time,
        subtitle,
        session_label,
        segments,
        footer_left,
        footer_right,
        presets,
        focus_modes,
        play_button,
        reset_button,
        skip_button,
        alert_start_button,
        alert_extend_button,
        notch_tab_pomodoro,
        notch_tab_shelf,
        notch_tab_clipboard,
        notch_tab_media,
        shelf_add_button: shelf.add_button,
        shelf_clear_button: shelf.clear_button,
        shelf_pager_button: shelf.pager_button,
        shelf_dropzone: shelf.dropzone,
        shelf_cards: shelf.cards,
        shelf_copy_buttons: shelf.copy_buttons,
        shelf_remove_buttons: shelf.remove_buttons,
        clipboard_pause_button: clipboard.pause_button,
        clipboard_clear_button: clipboard.clear_button,
        clipboard_pager_button: clipboard.pager_button,
        clipboard_body: clipboard.body,
        clipboard_rows: clipboard.rows,
        clipboard_remove_buttons: clipboard.remove_buttons,
        privacy_mic_button,
        media_cover_art,
        media_source_label,
        media_title,
        media_artist,
        media_reason,
        media_equalizer,
        media_lyrics_toggle,
        media_lyrics_drawer,
        media_seekbar,
        media_play_button: media_buttons.primary,
        media_prev_button: media_buttons.left,
        media_next_button: media_buttons.right,
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
    let bar_height = scale(metrics.bar_height, dpi);
    let notch_y = scale(metrics.notch_top_inset, dpi);
    let hud_t = notch_input.hud_expansion.clamp(0.0, 1.0);
    let compact_t = notch_input.compact_expansion.clamp(0.0, 1.0);
    let panel_t = notch_input.panel_expansion.clamp(0.0, 1.0);
    let privacy_extra_pts = if notch_input.privacy.compact_dots {
        PRIVACY_COMPACT_EXTRA_POINTS
    } else {
        0
    };

    let base_pts = metrics.notch_width + privacy_extra_pts;
    let compact_target = compact_width_points(
        metrics,
        notch_input.compact_title_points,
        notch_input.media_live,
    ) + privacy_extra_pts;
    let compact_pts = lerp_points(base_pts, compact_target, compact_t);
    let hud_pts = lerp_points(base_pts, metrics.notch_hud_width + privacy_extra_pts, hud_t);
    let collapsed_pts = compact_pts.max(hud_pts);
    let lyrics_t = notch_input.lyrics_progress.clamp(0.0, 1.0);
    let target_expanded_h = lerp_points(
        metrics.notch_expanded_height,
        metrics.notch_lyrics_height,
        lyrics_t,
    );
    let notch_pts_w = lerp_points(collapsed_pts, metrics.notch_expanded_width, panel_t);
    let notch_pts_h = lerp_points(metrics.notch_height, target_expanded_h, panel_t);

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
        y: notch_y,
        width: notch_width,
        height: notch_height,
    };
    let hud_width = scale(metrics.notch_hud_width, dpi);
    let hud_geometry = compute_hud_geometry(
        Rect {
            x: (screen_width - hud_width) / 2,
            y: notch_y,
            width: hud_width,
            height: scale(metrics.notch_height, dpi),
        },
        metrics,
        dpi,
    );

    let panel_pts_w = metrics.notch_expanded_width + metrics.notch_expanded_ear_radius * 2 + 8;
    let panel_pts_h = metrics.notch_top_inset + target_expanded_h + 4;
    let panel_w = scale(panel_pts_w, dpi);
    let panel_h = scale(panel_pts_h, dpi);
    let panel_window = Rect {
        x: (screen_width - panel_w) / 2,
        y: 0,
        width: panel_w,
        height: panel_h,
    };

    let controls = compute_expanded_notch_controls(notch, dpi, &notch_input, metrics);
    let compact = compute_compact_notch_geometry(
        notch,
        bar_height,
        dpi,
        notch_input.privacy.compact_dots && panel_t <= 0.55,
        notch_input.media_live,
    );
    let secondary_t = notch_input.secondary_expansion.clamp(0.0, 1.0);
    let swap_blend = notch_input.compact_swap_blend.clamp(0.0, 1.0);
    let notch_secondary = compute_secondary_notch_rect(
        notch,
        metrics,
        dpi,
        secondary_t * (1.0 - panel_t),
        swap_blend,
    );

    let control_center_width = if notch_input.control_center_enabled {
        metrics.control_center_slot_width
    } else {
        0
    };
    let right_widths = [
        (HitTarget::Clock, scale(metrics.clock_slot_width, dpi)),
        (HitTarget::ControlCenter, scale(control_center_width, dpi)),
        (HitTarget::Search, scale(metrics.search_slot_width, dpi)),
        (
            HitTarget::QuickSettings,
            scale(metrics.status_slot_width, dpi),
        ),
        (HitTarget::Tray, scale(metrics.tray_slot_width, dpi)),
    ];
    let right_start = notch.right().max(notch_secondary.right()) + right_gap;
    let (right_slots, right_hidden) = resolve_right_slots(
        &right_widths,
        right_gap,
        right_start,
        screen_width - padding,
        bar_height,
    );
    let left_widths = [
        scale(metrics.start_slot_width, dpi),
        scale(estimate_app_slot_points(metrics, app_name), dpi),
        scale(metrics.file_slot_width, dpi),
        scale(metrics.edit_slot_width, dpi),
        scale(metrics.view_slot_width, dpi),
        scale(metrics.window_slot_width, dpi),
        scale(metrics.help_slot_width, dpi),
    ];
    let mut row = resolve_left_slots(&LeftSlotInput {
        widths: &left_widths,
        overflow_width: scale(metrics.overflow_slot_width, dpi),
        gap: left_gap,
        start: padding,
        limit: (notch.x - left_gap).max(padding),
        height: bar_height,
        force_overflow: !right_hidden.is_empty(),
    });
    row.slots.extend(right_slots);
    row.hidden.extend(right_hidden);

    let start_slot = row.rect(HitTarget::Start);
    let app_slot = row.rect(HitTarget::AppMenu);
    let file_slot = row.rect(HitTarget::FileMenu);
    let edit_slot = row.rect(HitTarget::EditMenu);
    let view_slot = row.rect(HitTarget::ViewMenu);
    let window_slot = row.rect(HitTarget::WindowMenu);
    let help_slot = row.rect(HitTarget::HelpMenu);
    let clock_slot = row.rect(HitTarget::Clock);
    let control_center_slot = row.rect(HitTarget::ControlCenter);
    let search_slot = row.rect(HitTarget::Search);
    let status_slot = row.rect(HitTarget::QuickSettings);
    let icon_size = scale(metrics.icon_size, dpi);
    let (status_wifi_rect, status_volume_rect, status_battery_rect) =
        compute_status_sub_rects(status_slot, icon_size);
    let tray_slot = row.rect(HitTarget::Tray);

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
        notch_secondary,
        panel_window,
        notch_corner_radius: scale(corner_pts, dpi),
        notch_ear_radius: scale(ear_pts, dpi),
        panel_expansion: panel_t,
        compact_expansion: compact_t,
        secondary_expansion: secondary_t * (1.0 - panel_t),
        compact_swap_blend: swap_blend,
        compact_primary: notch_input.compact_primary,
        compact_secondary: notch_input.compact_secondary,
        pomodoro_surface: notch_input.pomodoro_surface,
        focus_mode: notch_input.focus_mode,
        active_tab: notch_input.active_tab,
        previous_tab: notch_input.previous_tab,
        compact_ring: compact.ring,
        compact_peek: compact.peek,
        compact_time: compact.time,
        compact_title: compact.title,
        compact_trailing: compact.trailing,
        notch_tab_strip: controls.notch_tab_strip,
        pomodoro_icon: controls.icon,
        pomodoro_time: controls.time,
        pomodoro_subtitle: controls.subtitle,
        pomodoro_session_label: controls.session_label,
        pomodoro_segments: controls.segments,
        pomodoro_footer_left: controls.footer_left,
        pomodoro_footer_right: controls.footer_right,
        pomodoro_presets: controls.presets,
        pomodoro_focus_modes: controls.focus_modes,
        pomodoro_play_button: controls.play_button,
        pomodoro_reset_button: controls.reset_button,
        pomodoro_skip_button: controls.skip_button,
        pomodoro_alert_start_button: controls.alert_start_button,
        pomodoro_alert_extend_button: controls.alert_extend_button,
        notch_tab_pomodoro: controls.notch_tab_pomodoro,
        notch_tab_shelf: controls.notch_tab_shelf,
        notch_tab_clipboard: controls.notch_tab_clipboard,
        notch_tab_media: controls.notch_tab_media,
        shelf_add_button: controls.shelf_add_button,
        shelf_clear_button: controls.shelf_clear_button,
        shelf_pager_button: controls.shelf_pager_button,
        shelf_dropzone: controls.shelf_dropzone,
        shelf_cards: controls.shelf_cards,
        shelf_copy_buttons: controls.shelf_copy_buttons,
        shelf_remove_buttons: controls.shelf_remove_buttons,
        clipboard_pause_button: controls.clipboard_pause_button,
        clipboard_clear_button: controls.clipboard_clear_button,
        clipboard_pager_button: controls.clipboard_pager_button,
        clipboard_body: controls.clipboard_body,
        clipboard_rows: controls.clipboard_rows,
        clipboard_remove_buttons: controls.clipboard_remove_buttons,
        privacy_dots: compact.privacy_dots,
        privacy_mic_button: controls.privacy_mic_button,
        media_cover_art: controls.media_cover_art,
        media_source_label: controls.media_source_label,
        media_title: controls.media_title,
        media_artist: controls.media_artist,
        media_reason: controls.media_reason,
        media_equalizer: controls.media_equalizer,
        media_lyrics_toggle: controls.media_lyrics_toggle,
        media_lyrics_drawer: controls.media_lyrics_drawer,
        media_seekbar: controls.media_seekbar,
        media_play_button: controls.media_play_button,
        media_prev_button: controls.media_prev_button,
        media_next_button: controls.media_next_button,
        lyrics_progress: notch_input.lyrics_progress.clamp(0.0, 1.0),
        tab_blend: notch_input.tab_blend.clamp(0.0, 1.0),
        icon_size,
        start_slot,
        app_slot,
        file_slot,
        edit_slot,
        view_slot,
        window_slot,
        help_slot,
        overflow_slot: row.overflow,
        hidden_targets: row.hidden,
        tray_slot,
        status_slot,
        status_wifi_rect,
        status_volume_rect,
        status_battery_rect,
        search_slot,
        control_center_slot,
        clock_slot,
        hud: hud_geometry,
    }
}

fn compute_status_sub_rects(status_slot: Rect, icon_size: i32) -> (Rect, Rect, Rect) {
    if status_slot.is_empty() {
        return (Rect::default(), Rect::default(), Rect::default());
    }
    let unit = icon_size.max(1);
    let wifi_end =
        (status_slot.x + (unit * 148 + 50) / 100).clamp(status_slot.x, status_slot.right());
    let volume_end = (status_slot.x + (unit * 256 + 50) / 100).clamp(wifi_end, status_slot.right());
    let wifi = Rect {
        x: status_slot.x,
        y: status_slot.y,
        width: (wifi_end - status_slot.x).max(0),
        height: status_slot.height,
    };
    let volume = Rect {
        x: wifi_end,
        y: status_slot.y,
        width: (volume_end - wifi_end).max(0),
        height: status_slot.height,
    };
    let battery = Rect {
        x: volume_end,
        y: status_slot.y,
        width: (status_slot.right() - volume_end).max(0),
        height: status_slot.height,
    };
    (wifi, volume, battery)
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
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Pomodoro,
                control_center_enabled: true,
                lyrics_progress: 0.0,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(layout.bar.height, 32);
        assert_eq!(layout.notch.width, 380);
        assert_eq!(layout.notch.height, 144);
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
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                control_center_enabled: true,
                lyrics_progress: 0.0,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
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
        assert_eq!(
            layout.hit_test(layout.media_seekbar.x + 8, layout.media_seekbar.y + 4,),
            Some(HitTarget::MediaSeekbar)
        );
        assert!(layout.media_cover_art.bottom() <= layout.media_seekbar.y);
        assert!(layout.media_play_button.bottom() < layout.media_seekbar.y);
        assert!(layout.media_prev_button.right() < layout.media_play_button.x);
        assert!(layout.media_play_button.right() < layout.media_next_button.x);
        assert_eq!(layout.notch.right() - layout.media_next_button.right(), 16);
        let hero_center = layout.media_cover_art.y + layout.media_cover_art.height / 2;
        for button in [
            layout.media_prev_button,
            layout.media_play_button,
            layout.media_next_button,
        ] {
            assert_eq!(button.y + button.height / 2, hero_center);
        }
        assert!(layout.media_lyrics_toggle.bottom() <= layout.media_cover_art.y);
        assert_eq!(
            layout.notch.right() - layout.media_lyrics_toggle.right(),
            16
        );
        assert!(layout.media_source_label.right() < layout.media_lyrics_toggle.x);
    }

    #[test]
    fn media_hero_shares_the_pomodoro_grid() {
        let media = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(media.media_cover_art, media.pomodoro_icon);
        assert_eq!(media.media_prev_button, media.pomodoro_reset_button);
        assert_eq!(media.media_play_button, media.pomodoro_play_button);
        assert_eq!(media.media_next_button, media.pomodoro_skip_button);
        assert_eq!(media.media_title.x, media.pomodoro_time.x);
        assert_eq!(media.media_title.width, media.pomodoro_time.width);
        assert!(media.media_title.bottom() <= media.media_artist.y);
        assert!(media.media_artist.bottom() <= media.media_equalizer.y);
        assert!(media.media_reason.contains_rect(&media.media_artist));
        assert!(media.media_equalizer.bottom() <= media.media_cover_art.bottom());
        assert!(media.media_reason.right() < media.media_prev_button.x);
    }

    #[test]
    fn lyrics_progress_interpolates_panel_drawer_and_seekbar_between_closed_and_open() {
        let layout_at = |lyrics_progress: f32| {
            compute_layout_with_notch(
                &M,
                1920,
                96,
                "Finder",
                NotchLayoutInput {
                    hud_expansion: 0.0,
                    compact_expansion: 0.0,
                    panel_expansion: 1.0,
                    pomodoro_surface: PomodoroSurface::Running,
                    active_tab: NotchExpandedTab::Media,
                    control_center_enabled: true,
                    lyrics_progress,
                    tab_blend: 1.0,
                    ..NotchLayoutInput::default()
                },
            )
        };
        let closed = layout_at(0.0);
        let mid = layout_at(0.5);
        let open = layout_at(1.0);
        assert!(closed.notch.height < mid.notch.height && mid.notch.height < open.notch.height);
        assert!(
            closed.media_lyrics_drawer.height < mid.media_lyrics_drawer.height
                && mid.media_lyrics_drawer.height < open.media_lyrics_drawer.height
        );
        assert!(
            closed.media_seekbar.y < mid.media_seekbar.y
                && mid.media_seekbar.y < open.media_seekbar.y
        );
        assert_eq!(open.notch.height, 254);
        assert_eq!(closed.notch.height, 144);
        assert!((layout_at(2.0).lyrics_progress - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn expanded_media_controls_meet_target_size_and_fit_notch() {
        for lyrics_progress in [0.0, 1.0] {
            let layout = compute_layout_with_notch(
                &M,
                1920,
                96,
                "Finder",
                NotchLayoutInput {
                    hud_expansion: 0.0,
                    compact_expansion: 0.0,
                    panel_expansion: 1.0,
                    pomodoro_surface: PomodoroSurface::Running,
                    active_tab: NotchExpandedTab::Media,
                    control_center_enabled: true,
                    lyrics_progress,
                    tab_blend: 1.0,
                    ..NotchLayoutInput::default()
                },
            );
            assert!(layout.media_lyrics_toggle.width >= 24);
            assert!(layout.media_lyrics_toggle.height >= 24);
            assert!(layout.media_seekbar.height >= 24);
            assert!(layout.media_prev_button.width >= 24);
            assert!(layout.media_next_button.width >= 24);
            assert!(layout.media_play_button.width >= 24);
            assert!(layout.media_play_button.bottom() <= layout.notch.height - 4);
            assert!(layout.media_cover_art.bottom() <= layout.media_seekbar.y);
        }
    }

    #[test]
    fn expanded_notch_supports_lyrics_drawer_and_toggle() {
        let layout_collapsed = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                control_center_enabled: true,
                lyrics_progress: 0.0,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(layout_collapsed.notch.height, 144);
        assert_eq!(layout_collapsed.media_lyrics_drawer.height, 0);
        assert_eq!(
            layout_collapsed.hit_test(
                layout_collapsed.media_lyrics_toggle.x + 4,
                layout_collapsed.media_lyrics_toggle.y + 4,
            ),
            Some(HitTarget::MediaLyricsToggle)
        );

        let layout_expanded = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                control_center_enabled: true,
                lyrics_progress: 1.0,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(layout_expanded.notch.height, 254);
        assert!(layout_expanded.media_lyrics_drawer.height > 100);
        assert!(layout_expanded.media_cover_art.bottom() <= layout_expanded.media_lyrics_drawer.y);
        assert!(layout_expanded.media_lyrics_drawer.bottom() <= layout_expanded.media_seekbar.y);
        assert!(
            layout_expanded.media_play_button.bottom() <= layout_expanded.media_lyrics_drawer.y
        );
        assert!(layout_expanded.media_seekbar.bottom() <= layout_expanded.notch.height - 16);
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
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Pomodoro,
                control_center_enabled: true,
                lyrics_progress: 0.0,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
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
            layout.hit_test(layout.status_wifi_rect.x + 8, 16),
            Some(HitTarget::StatusWifi)
        );
        assert_eq!(
            layout.hit_test(layout.status_volume_rect.x + 8, 16),
            Some(HitTarget::StatusVolume)
        );
        assert_eq!(
            layout.hit_test(layout.status_battery_rect.x + 8, 16),
            Some(HitTarget::StatusBattery)
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
    fn status_slot_splits_into_disjoint_icon_rects_across_dpis() {
        for dpi in [96, 144, 192] {
            let layout = compute_layout(&M, 1920, dpi);
            let wifi = layout.status_wifi_rect;
            let vol = layout.status_volume_rect;
            let bat = layout.status_battery_rect;
            assert!(wifi.width > 0 && vol.width > 0 && bat.width > 0);
            assert_eq!(wifi.x, layout.status_slot.x);
            assert_eq!(wifi.right(), vol.x);
            assert_eq!(vol.right(), bat.x);
            assert_eq!(bat.right(), layout.status_slot.right());
            let cy = layout.status_slot.height / 2;
            assert_eq!(
                layout.hit_test(wifi.x + wifi.width / 2, cy),
                Some(HitTarget::StatusWifi)
            );
            assert_eq!(
                layout.hit_test(vol.x + vol.width / 2, cy),
                Some(HitTarget::StatusVolume)
            );
            assert_eq!(
                layout.hit_test(bat.x + bat.width / 2, cy),
                Some(HitTarget::StatusBattery)
            );
        }
    }

    const EVERY_SLOT: [HitTarget; 12] = [
        HitTarget::Start,
        HitTarget::AppMenu,
        HitTarget::FileMenu,
        HitTarget::EditMenu,
        HitTarget::ViewMenu,
        HitTarget::WindowMenu,
        HitTarget::HelpMenu,
        HitTarget::Tray,
        HitTarget::QuickSettings,
        HitTarget::Search,
        HitTarget::ControlCenter,
        HitTarget::Clock,
    ];

    fn assert_every_item_reachable(layout: &Layout) {
        let mut visible: Vec<Rect> = Vec::new();
        for target in EVERY_SLOT {
            let rect = layout.slot_rect(target).unwrap();
            if layout.is_hidden(target) {
                assert_eq!(rect.width, 0, "{target:?} is hidden but still has width");
                assert!(layout.overflow_slot.width > 0, "{target:?} has no overflow");
                continue;
            }
            assert!(
                rect.width > 0,
                "{target:?} is neither visible nor in overflow"
            );
            let hit = layout.hit_test(rect.x + rect.width / 2, rect.height / 2);
            if target == HitTarget::QuickSettings {
                assert!(matches!(
                    hit,
                    Some(
                        HitTarget::QuickSettings
                            | HitTarget::StatusWifi
                            | HitTarget::StatusVolume
                            | HitTarget::StatusBattery
                    )
                ));
            } else {
                assert_eq!(hit, Some(target));
            }
            visible.push(rect);
        }
        if layout.overflow_slot.width > 0 {
            let slot = layout.overflow_slot;
            assert_eq!(
                layout.hit_test(slot.x + slot.width / 2, slot.height / 2),
                Some(HitTarget::MenuOverflow)
            );
            visible.push(slot);
        }
        visible.push(Rect {
            height: layout.bar.height,
            ..layout.notch
        });
        visible.sort_by_key(|rect| rect.x);
        for pair in visible.windows(2) {
            assert!(
                pair[0].right() <= pair[1].x,
                "{:?} overlaps {:?}",
                pair[0],
                pair[1]
            );
        }
        assert!(visible.first().unwrap().x >= 0);
        assert!(visible.last().unwrap().right() <= layout.bar.width);
    }

    #[test]
    fn wide_screens_show_every_slot_without_overflow() {
        for (width, dpi) in [(1920, 96), (2560, 144), (3840, 192)] {
            let layout = compute_layout_for_app(&M, width, dpi, "Finder");
            assert!(layout.hidden_targets.is_empty(), "{width}@{dpi}");
            assert_eq!(layout.overflow_slot.width, 0);
            assert_every_item_reachable(&layout);
        }
    }

    #[test]
    fn small_and_high_scale_screens_keep_every_item_reachable() {
        for (width, dpi) in [
            (1366, 96),
            (1366, 144),
            (1280, 120),
            (1024, 96),
            (1024, 144),
            (800, 96),
            (1920, 240),
            (1280, 192),
        ] {
            for app_name in ["Finder", "Visual Studio Installer"] {
                let layout = compute_layout_for_app(&M, width, dpi, app_name);
                assert_every_item_reachable(&layout);
            }
        }
    }

    #[test]
    fn widened_notch_never_overlaps_menu_slots() {
        for (width, dpi) in [(1366, 144), (1024, 96), (1920, 96)] {
            let layout = compute_layout_with_notch(
                &M,
                width,
                dpi,
                "Finder",
                NotchLayoutInput {
                    hud_expansion: 1.0,
                    compact_expansion: 1.0,
                    ..NotchLayoutInput::default()
                },
            );
            assert_every_item_reachable(&layout);
        }
    }

    #[test]
    fn hidden_items_anchor_their_menus_at_the_overflow_slot() {
        let layout = compute_layout_for_app(&M, 800, 96, "Finder");
        assert!(!layout.hidden_targets.is_empty());
        let hidden = layout.hidden_targets[0];
        assert_eq!(layout.menu_anchor_rect(hidden), Some(layout.overflow_slot));
        assert_eq!(
            layout.menu_anchor_rect(HitTarget::Start),
            Some(layout.start_slot)
        );
    }

    fn pomodoro_layout(surface: PomodoroSurface, dpi: u32) -> Layout {
        compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                pomodoro_surface: surface,
                ..NotchLayoutInput::default()
            },
        )
    }

    fn pomodoro_hit_rects(layout: &Layout) -> Vec<Rect> {
        let mut rects = vec![
            layout.notch_tab_pomodoro,
            layout.notch_tab_shelf,
            layout.notch_tab_media,
        ];
        if layout.pomodoro_surface == PomodoroSurface::Alert {
            rects.push(layout.pomodoro_alert_extend_button);
            rects.push(layout.pomodoro_alert_start_button);
        } else {
            rects.extend(layout.pomodoro_focus_modes);
            rects.push(layout.pomodoro_play_button);
            rects.push(layout.pomodoro_reset_button);
            rects.push(layout.pomodoro_skip_button);
            rects.extend(layout.pomodoro_presets);
        }
        rects.retain(|rect| !rect.is_empty());
        rects
    }

    #[test]
    fn pomodoro_controls_fit_the_notch_and_keep_their_gaps_at_every_dpi() {
        for dpi in [96, 120, 144, 192] {
            let gap = scale(4, dpi);
            let minimum = scale(24, dpi);
            for surface in [
                PomodoroSurface::Setup,
                PomodoroSurface::Ready,
                PomodoroSurface::Running,
                PomodoroSurface::Paused,
                PomodoroSurface::Alert,
            ] {
                let layout = pomodoro_layout(surface, dpi);
                let rects = pomodoro_hit_rects(&layout);
                for (index, rect) in rects.iter().enumerate() {
                    assert!(
                        layout.notch.contains_rect(rect),
                        "{surface:?}@{dpi}: {rect:?} leaves the notch"
                    );
                    assert!(
                        rect.width >= minimum && rect.height >= minimum,
                        "{surface:?}@{dpi}: {rect:?} is below the hit minimum"
                    );
                    assert!(
                        rect.bottom() <= layout.notch.height - scale(16, dpi) + 1
                            || rect.y < scale(40, dpi),
                        "{surface:?}@{dpi}: {rect:?} crosses the bottom inset"
                    );
                    for other in &rects[index + 1..] {
                        let separated_x =
                            rect.right() + gap <= other.x || other.right() + gap <= rect.x;
                        let separated_y =
                            rect.bottom() + gap <= other.y || other.bottom() + gap <= rect.y;
                        assert!(
                            separated_x || separated_y,
                            "{surface:?}@{dpi}: {rect:?} is closer than the gap to {other:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn presets_exist_only_in_setup_and_resolve_to_their_index() {
        let setup = pomodoro_layout(PomodoroSurface::Setup, 96);
        for (index, preset) in setup.pomodoro_presets.iter().enumerate() {
            assert_eq!(
                setup.hit_test(preset.x + 4, preset.y + 4),
                Some(HitTarget::PomodoroPreset(index as u8))
            );
            assert_eq!(
                setup.notch_control_rect(HitTarget::PomodoroPreset(index as u8)),
                Some(*preset)
            );
        }
        assert!(setup.pomodoro_reset_button.is_empty());
        assert!(setup.pomodoro_skip_button.is_empty());
        assert!(setup.pomodoro_segments.iter().all(Rect::is_empty));

        let running = pomodoro_layout(PomodoroSurface::Running, 96);
        assert!(running.pomodoro_presets.iter().all(Rect::is_empty));
        assert_eq!(
            running.notch_control_rect(HitTarget::PomodoroPreset(0)),
            None
        );
        assert!(
            running
                .pomodoro_segments
                .iter()
                .all(|segment| !segment.is_empty())
        );
    }

    #[test]
    fn running_controls_read_reset_play_skip_with_the_primary_in_the_middle() {
        let layout = pomodoro_layout(PomodoroSurface::Running, 96);
        assert!(layout.pomodoro_reset_button.right() < layout.pomodoro_play_button.x);
        assert!(layout.pomodoro_play_button.right() < layout.pomodoro_skip_button.x);
        assert!(layout.pomodoro_play_button.width > layout.pomodoro_skip_button.width);
        assert_eq!(
            layout.notch.right() - layout.pomodoro_skip_button.right(),
            16
        );
        assert!(layout.pomodoro_time.right() < layout.pomodoro_reset_button.x);
    }

    #[test]
    fn alert_surface_exposes_only_its_two_buttons() {
        let layout = pomodoro_layout(PomodoroSurface::Alert, 96);
        let start = layout.pomodoro_alert_start_button;
        let extend = layout.pomodoro_alert_extend_button;
        assert_eq!(
            layout.hit_test(start.x + 4, start.y + 4),
            Some(HitTarget::PomodoroStartNext)
        );
        assert_eq!(
            layout.hit_test(extend.x + 4, extend.y + 4),
            Some(HitTarget::PomodoroExtendFive)
        );
        assert_eq!(
            layout.hit_test(
                layout.notch_tab_pomodoro.x + 4,
                layout.notch_tab_pomodoro.y + 4
            ),
            Some(HitTarget::Notch)
        );
        assert_eq!(
            layout.notch_control_rect(HitTarget::PomodoroPlayPause),
            None
        );
        assert_eq!(
            layout.notch_control_rect(HitTarget::PomodoroStartNext),
            Some(start)
        );
        assert!(layout.pomodoro_subtitle.bottom() < start.y);
    }

    #[test]
    fn progress_segments_span_the_content_width_and_the_footer_sits_below_them() {
        let layout = pomodoro_layout(PomodoroSurface::Running, 96);
        let first = layout.pomodoro_segments[0];
        let last = layout.pomodoro_segments[3];
        assert_eq!(first.x, layout.notch.x + 16);
        assert_eq!(last.right(), layout.notch.right() - 16);
        for pair in layout.pomodoro_segments.windows(2) {
            assert!(pair[0].right() < pair[1].x);
        }
        assert!(first.bottom() < layout.pomodoro_footer_left.y);
        assert!(layout.pomodoro_footer_left.right() <= layout.pomodoro_footer_right.x);
        assert!(layout.pomodoro_play_button.bottom() < first.y);
    }

    #[test]
    fn compact_geometry_leaves_a_peek_between_ring_and_time_inside_the_notch() {
        for dpi in [96, 144, 192] {
            let layout = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    compact_expansion: 1.0,
                    ..NotchLayoutInput::default()
                },
            );
            assert!(layout.notch.contains_rect(&layout.compact_ring));
            assert!(layout.notch.contains_rect(&layout.compact_time));
            assert!(layout.compact_peek.width > scale(120, dpi));
            assert!(layout.compact_ring.right() < layout.compact_peek.x);
            assert!(layout.compact_peek.right() < layout.compact_time.x);
        }
    }

    fn media_layout(lyrics_progress: f32, dpi: u32) -> Layout {
        compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Media,
                lyrics_progress,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        )
    }

    #[test]
    fn media_controls_fit_the_notch_and_keep_their_gaps_at_every_dpi() {
        for dpi in [96, 120, 144, 192] {
            let gap = scale(4, dpi);
            let minimum = scale(24, dpi);
            for lyrics_progress in [0.0, 0.5, 1.0] {
                let layout = media_layout(lyrics_progress, dpi);
                let rects = [
                    layout.notch_tab_pomodoro,
                    layout.notch_tab_shelf,
                    layout.notch_tab_media,
                    layout.media_lyrics_toggle,
                    layout.media_prev_button,
                    layout.media_play_button,
                    layout.media_next_button,
                    layout.media_seekbar,
                ];
                for (index, rect) in rects.iter().enumerate() {
                    assert!(
                        layout.notch.contains_rect(rect),
                        "{lyrics_progress}@{dpi}: {rect:?} leaves the notch"
                    );
                    assert!(
                        rect.width >= minimum && rect.height >= minimum,
                        "{lyrics_progress}@{dpi}: {rect:?} is below the hit minimum"
                    );
                    for other in &rects[index + 1..] {
                        let separated_x =
                            rect.right() + gap <= other.x || other.right() + gap <= rect.x;
                        let separated_y =
                            rect.bottom() + gap <= other.y || other.bottom() + gap <= rect.y;
                        assert!(
                            separated_x || separated_y,
                            "{lyrics_progress}@{dpi}: {rect:?} is closer than the gap to {other:?}"
                        );
                    }
                }
                assert!(layout.media_seekbar.bottom() <= layout.notch.height - scale(16, dpi));
            }
        }
    }

    #[test]
    fn opening_the_lyrics_drawer_moves_only_the_drawer_and_the_seek_bar() {
        let closed = media_layout(0.0, 96);
        let open = media_layout(1.0, 96);
        assert_eq!(closed.media_cover_art, open.media_cover_art);
        assert_eq!(closed.media_title, open.media_title);
        assert_eq!(closed.media_artist, open.media_artist);
        assert_eq!(closed.media_equalizer, open.media_equalizer);
        assert_eq!(closed.media_prev_button, open.media_prev_button);
        assert_eq!(closed.media_play_button, open.media_play_button);
        assert_eq!(closed.media_next_button, open.media_next_button);
        assert_eq!(closed.media_lyrics_toggle, open.media_lyrics_toggle);
        assert_eq!(closed.media_source_label, open.media_source_label);
        assert_eq!(closed.media_lyrics_drawer.height, 0);
        assert_eq!(open.media_lyrics_drawer.height, 102);
        assert_eq!(open.media_seekbar.y - closed.media_seekbar.y, 110);
        assert_eq!(closed.media_seekbar.y, 98);
    }

    #[test]
    fn the_seek_bar_and_drawer_never_overlap_while_the_drawer_opens() {
        let mut previous_height = 0;
        for step in 0..=20 {
            let layout = media_layout(step as f32 / 20.0, 96);
            let drawer = layout.media_lyrics_drawer;
            assert!(drawer.height >= previous_height, "step {step}");
            previous_height = drawer.height;
            if drawer.height > 0 {
                assert!(
                    drawer.bottom() + 12 <= layout.media_seekbar.y,
                    "step {step}"
                );
                assert!(layout.media_cover_art.bottom() < drawer.y, "step {step}");
            }
        }
        assert_eq!(previous_height, 102);
    }

    #[test]
    fn compact_title_sits_between_the_cover_and_the_trailing_badge() {
        for dpi in [96, 144, 192] {
            let layout = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    compact_expansion: 1.0,
                    ..NotchLayoutInput::default()
                },
            );
            assert!(layout.notch.contains_rect(&layout.compact_title));
            assert!(layout.notch.contains_rect(&layout.compact_trailing));
            assert!(layout.compact_ring.right() < layout.compact_title.x);
            assert!(layout.compact_title.right() < layout.compact_trailing.x);
            assert!(layout.compact_title.width > scale(150, dpi));
            assert_eq!(
                layout.compact_title.x - layout.notch.x,
                layout.notch.right() - layout.compact_title.right()
            );
            assert_eq!(
                layout.notch.right() - layout.compact_trailing.right(),
                scale(12, dpi)
            );

            let live = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    compact_expansion: 1.0,
                    media_live: true,
                    ..NotchLayoutInput::default()
                },
            );
            assert!(live.compact_trailing.width > layout.compact_trailing.width);
            assert!(live.compact_title.right() < live.compact_trailing.x);
        }
    }

    #[test]
    fn compact_notch_widens_to_fit_the_title_up_to_a_cap() {
        for dpi in [96, 144, 192] {
            let compact = |title_points| {
                compute_layout_with_notch(
                    &M,
                    1920,
                    dpi,
                    "Finder",
                    NotchLayoutInput {
                        compact_expansion: 1.0,
                        compact_title_points: title_points,
                        ..NotchLayoutInput::default()
                    },
                )
            };
            assert_eq!(compact(0).notch.width, scale(M.notch_compact_width, dpi));
            assert_eq!(compact(40).notch.width, scale(M.notch_compact_width, dpi));
            let wide = compact(300);
            assert!(wide.notch.width > scale(M.notch_compact_width, dpi));
            assert!(wide.compact_title.width >= scale(300, dpi) - 1);
            assert!(wide.notch.contains_rect(&wide.compact_title));
            assert_eq!(
                compact(5000).notch.width,
                scale(COMPACT_MAX_WIDTH_POINTS, dpi)
            );
        }
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
        assert!(!layout.is_hidden(HitTarget::ControlCenter));
        assert_eq!(layout.hit_test(layout.control_center_slot.x, 16), None);
    }

    #[test]
    fn four_adaptive_tabs_fit_without_overlapping_header_labels_across_dpis() {
        for dpi in [96, 120, 144, 192] {
            let gap = scale(4, dpi);
            let min_hit = scale(24, dpi);
            for active_tab in NotchExpandedTab::ALL {
                let layout = compute_layout_with_notch(
                    &M,
                    1920,
                    dpi,
                    "Finder",
                    NotchLayoutInput {
                        panel_expansion: 1.0,
                        active_tab,
                        ..NotchLayoutInput::default()
                    },
                );
                let tabs = [
                    (
                        NotchExpandedTab::Pomodoro,
                        layout.notch_tab_pomodoro,
                        HitTarget::NotchTabPomodoro,
                    ),
                    (
                        NotchExpandedTab::Shelf,
                        layout.notch_tab_shelf,
                        HitTarget::NotchTabShelf,
                    ),
                    (
                        NotchExpandedTab::Clipboard,
                        layout.notch_tab_clipboard,
                        HitTarget::NotchTabClipboard,
                    ),
                    (
                        NotchExpandedTab::Media,
                        layout.notch_tab_media,
                        HitTarget::NotchTabMedia,
                    ),
                ];
                for (tab, rect, hit) in tabs {
                    assert!(layout.notch.contains_rect(&rect));
                    assert!(rect.width >= min_hit && rect.height >= min_hit);
                    assert_eq!(layout.hit_test(rect.x + 8, rect.y + 8), Some(hit));
                    assert_eq!(layout.notch_control_rect(hit), Some(rect));
                    if tab == active_tab {
                        assert!(rect.width >= scale(64, dpi));
                    } else {
                        assert_eq!(rect.width, scale(24, dpi));
                    }
                }
                for pair in [
                    (layout.notch_tab_pomodoro, layout.notch_tab_shelf),
                    (layout.notch_tab_shelf, layout.notch_tab_clipboard),
                    (layout.notch_tab_clipboard, layout.notch_tab_media),
                ] {
                    assert_eq!(pair.1.x - pair.0.right(), gap);
                }
                assert!(layout.notch_tab_strip.right() + gap < layout.pomodoro_focus_modes[0].x);
                assert!(layout.notch_tab_strip.right() + gap < layout.pomodoro_session_label.x);
                assert!(layout.notch_tab_strip.right() + gap < layout.media_source_label.x);
            }
        }
    }

    #[test]
    fn absent_media_hides_fourth_tab_without_moving_the_first_three() {
        let with_media = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                media_present: true,
                ..NotchLayoutInput::default()
            },
        );
        let without_media = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                media_present: false,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(
            without_media.notch_tab_pomodoro,
            with_media.notch_tab_pomodoro
        );
        assert_eq!(without_media.notch_tab_shelf, with_media.notch_tab_shelf);
        assert_eq!(
            without_media.notch_tab_clipboard,
            with_media.notch_tab_clipboard
        );
        assert!(without_media.notch_tab_media.is_empty());
        assert_eq!(
            without_media.notch_control_rect(HitTarget::NotchTabMedia),
            None
        );
        assert!(without_media.notch_tab_strip.width < with_media.notch_tab_strip.width);
        assert_eq!(
            without_media.hit_test(
                with_media.notch_tab_media.x + 8,
                with_media.notch_tab_media.y + 8
            ),
            Some(HitTarget::Notch)
        );

        let shelf_only = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                focus_enabled: false,
                shelf_enabled: true,
                clipboard_enabled: false,
                media_present: false,
                ..NotchLayoutInput::default()
            },
        );
        assert!(shelf_only.notch_tab_pomodoro.is_empty());
        assert!(shelf_only.notch_tab_clipboard.is_empty());
        assert!(shelf_only.notch_tab_media.is_empty());
        assert_eq!(
            shelf_only.notch_tab_shelf.x,
            with_media.notch_tab_pomodoro.x
        );
        assert!(
            shelf_only
                .notch_tab_strip
                .contains_rect(&shelf_only.notch_tab_shelf)
        );

        let no_tabs = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                focus_enabled: false,
                shelf_enabled: false,
                clipboard_enabled: false,
                media_present: false,
                ..NotchLayoutInput::default()
            },
        );
        assert!(no_tabs.notch_tab_strip.is_empty());
        assert_eq!(
            no_tabs.notch_control_rect(HitTarget::NotchTabPomodoro),
            None
        );
        assert_eq!(no_tabs.notch_control_rect(HitTarget::NotchTabShelf), None);
        assert_eq!(
            no_tabs.notch_control_rect(HitTarget::NotchTabClipboard),
            None
        );
    }

    #[test]
    fn clipboard_header_and_rows_stay_non_overlapping_across_dpis() {
        for dpi in [96, 120, 144, 192] {
            let gap = scale(4, dpi);
            let layout = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: NotchExpandedTab::Clipboard,
                    privacy: PrivacyLayoutState {
                        compact_dots: true,
                        panel_mic_control: true,
                    },
                    clipboard: ClipboardLayoutInput {
                        visible_count: 4,
                        has_pager: true,
                        has_items: true,
                        paused: false,
                    },
                    ..NotchLayoutInput::default()
                },
            );
            let row_one = [
                layout.notch_tab_strip,
                layout.privacy_mic_button,
                layout.clipboard_pager_button,
                layout.clipboard_pause_button,
                layout.clipboard_clear_button,
            ];
            for (index, rect) in row_one.iter().enumerate() {
                assert!(layout.notch.contains_rect(rect));
                assert!(!rect.is_empty());
                if let Some(next) = row_one.get(index + 1) {
                    assert!(rect.right() + gap <= next.x);
                }
            }
            for (index, row) in layout.clipboard_rows.iter().enumerate() {
                assert!(layout.clipboard_body.contains_rect(row));
                assert!(row.contains_rect(&layout.clipboard_remove_buttons[index]));
                if let Some(next) = layout.clipboard_rows.get(index + 1) {
                    assert!(row.bottom() < next.y);
                }
            }
            assert_eq!(
                layout.hit_test(
                    layout.clipboard_remove_buttons[2].x + 4,
                    layout.clipboard_remove_buttons[2].y + 4,
                ),
                Some(HitTarget::ClipboardRemoveItem(2))
            );
            assert_eq!(
                layout.hit_test(
                    layout.clipboard_rows[2].x + 8,
                    layout.clipboard_rows[2].y + 8
                ),
                Some(HitTarget::ClipboardItem(2))
            );
        }
    }

    #[test]
    fn privacy_dots_and_panel_mic_button_keep_clearances_across_dpis() {
        for dpi in [96, 120, 144, 192] {
            let gap = scale(4, dpi);
            let collapsed = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    compact_expansion: 1.0,
                    compact_title_points: 120,
                    privacy: PrivacyLayoutState {
                        compact_dots: true,
                        panel_mic_control: true,
                    },
                    ..NotchLayoutInput::default()
                },
            );
            assert!(collapsed.notch.contains_rect(&collapsed.privacy_dots));
            assert!(collapsed.compact_time.right() + gap <= collapsed.privacy_dots.x);
            assert!(collapsed.compact_trailing.right() + gap <= collapsed.privacy_dots.x);
            assert_eq!(
                collapsed.hit_test(
                    collapsed.privacy_dots.x + 2,
                    collapsed.privacy_dots.y + collapsed.privacy_dots.height / 2,
                ),
                Some(HitTarget::PrivacyIndicator)
            );

            let shelf_open = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: NotchExpandedTab::Shelf,
                    privacy: PrivacyLayoutState {
                        compact_dots: true,
                        panel_mic_control: true,
                    },
                    ..NotchLayoutInput::default()
                },
            );
            assert!(shelf_open.privacy_dots.is_empty());
            assert!(
                shelf_open
                    .notch
                    .contains_rect(&shelf_open.privacy_mic_button)
            );
            assert!(shelf_open.notch_tab_strip.right() + gap <= shelf_open.privacy_mic_button.x);
            assert!(shelf_open.privacy_mic_button.right() + gap <= shelf_open.shelf_add_button.x);
            assert_eq!(
                shelf_open.hit_test(
                    shelf_open.privacy_mic_button.x + 4,
                    shelf_open.privacy_mic_button.y + 4,
                ),
                Some(HitTarget::PrivacyMicMute)
            );

            let media_open = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: NotchExpandedTab::Media,
                    privacy: PrivacyLayoutState {
                        compact_dots: true,
                        panel_mic_control: true,
                    },
                    ..NotchLayoutInput::default()
                },
            );
            assert!(
                media_open
                    .notch
                    .contains_rect(&media_open.privacy_mic_button)
            );
            assert!(media_open.notch_tab_strip.right() + gap <= media_open.privacy_mic_button.x);
            assert_eq!(
                media_open.hit_test(
                    media_open.privacy_mic_button.x + 4,
                    media_open.privacy_mic_button.y + 4,
                ),
                Some(HitTarget::PrivacyMicMute)
            );
        }
    }

    #[test]
    fn shelf_header_and_card_grid_stay_non_overlapping_across_dpis() {
        for dpi in [96, 120, 144, 192] {
            let gap = scale(4, dpi);
            let layout = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: NotchExpandedTab::Shelf,
                    privacy: PrivacyLayoutState {
                        compact_dots: true,
                        panel_mic_control: true,
                    },
                    shelf: ShelfLayoutInput {
                        visible_count: 6,
                        has_pager: true,
                        has_items: true,
                    },
                    ..NotchLayoutInput::default()
                },
            );
            let row_one = [
                layout.notch_tab_strip,
                layout.privacy_mic_button,
                layout.shelf_pager_button,
                layout.shelf_add_button,
                layout.shelf_clear_button,
            ];
            for (index, rect) in row_one.iter().enumerate() {
                assert!(layout.notch.contains_rect(rect));
                assert!(!rect.is_empty());
                if let Some(next) = row_one.get(index + 1) {
                    assert!(rect.right() + gap <= next.x);
                }
            }
            for (index, card) in layout.shelf_cards.iter().enumerate() {
                assert!(layout.shelf_dropzone.contains_rect(card));
                assert!(card.contains_rect(&layout.shelf_copy_buttons[index]));
                assert!(card.contains_rect(&layout.shelf_remove_buttons[index]));
                for other in &layout.shelf_cards[index + 1..] {
                    let sep_x = card.right() + gap <= other.x || other.right() + gap <= card.x;
                    let sep_y = card.bottom() + gap <= other.y || other.bottom() + gap <= card.y;
                    assert!(sep_x || sep_y);
                }
            }
            assert_eq!(
                layout.hit_test(
                    layout.shelf_remove_buttons[4].x + 4,
                    layout.shelf_remove_buttons[4].y + 4,
                ),
                Some(HitTarget::ShelfRemoveItem(4))
            );
            assert_eq!(
                layout.hit_test(
                    layout.shelf_copy_buttons[4].x + 4,
                    layout.shelf_copy_buttons[4].y + 4,
                ),
                Some(HitTarget::ShelfCopyItem(4))
            );
            assert_eq!(
                layout.hit_test(layout.shelf_cards[4].x + 8, layout.shelf_cards[4].y + 16),
                Some(HitTarget::ShelfItem(4))
            );
        }
    }

    #[test]
    fn secondary_notch_circle_sits_to_the_right_of_main_notch_and_hits_cleanly() {
        let layout = compute_layout_with_notch(
            &Metrics::MACBOOK_PRO_14,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                secondary_expansion: 1.0,
                compact_primary: Some(CompactSlotKind::Focus),
                compact_secondary: Some(CompactSlotKind::Media),
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(layout.notch_secondary.width, 28);
        assert_eq!(layout.notch_secondary.height, 28);
        assert_eq!(layout.notch_secondary.x, layout.notch.right() + 8);
        assert_eq!(layout.notch_secondary.y, 2);
        assert!(layout.tray_slot.x >= layout.notch_secondary.right() + 4);
        assert_eq!(
            layout.hit_test(layout.notch_secondary.x + 14, layout.notch_secondary.y + 14,),
            Some(HitTarget::NotchSecondary)
        );
        assert_eq!(
            layout.hit_test(layout.notch.x + 20, layout.notch.y + 14),
            Some(HitTarget::Notch)
        );
    }

    #[test]
    fn dynamic_island_metrics_float_capsule_inside_40pt_bar_with_compact_and_hud_bounds() {
        assert_eq!(
            Metrics::for_style(NotchStyle::ClassicNotch),
            Metrics::MACBOOK_PRO_14
        );
        assert_eq!(
            Metrics::for_style(NotchStyle::DynamicIsland),
            Metrics::DYNAMIC_ISLAND
        );

        let idle = compute_layout(&Metrics::DYNAMIC_ISLAND, 1920, 96);
        assert_eq!(
            idle.bar,
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 40,
            }
        );
        assert_eq!(
            idle.notch,
            Rect {
                x: 900,
                y: 4,
                width: 120,
                height: 32,
            }
        );
        assert_eq!(idle.notch_corner_radius, 16);
        assert_eq!(idle.notch_ear_radius, 0);
        assert_eq!(idle.hit_test(idle.notch.x + 10, 2), None);
        assert_eq!(idle.hit_test(idle.notch.x + 10, 38), None);
        assert_eq!(idle.hit_test(idle.notch.x + 10, 20), Some(HitTarget::Notch));

        let compact = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                secondary_expansion: 1.0,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: false,
                },
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(compact.notch.y, 4);
        assert_eq!(compact.notch.height, 32);
        assert!(compact.notch.contains_rect(&compact.compact_ring));
        assert!(compact.notch.contains_rect(&compact.compact_time));
        assert!(compact.notch.contains_rect(&compact.compact_title));
        assert!(compact.notch.contains_rect(&compact.compact_trailing));
        assert!(compact.notch.contains_rect(&compact.privacy_dots));
        assert!(compact.privacy_dots.x >= compact.compact_trailing.right());
        assert!(compact.privacy_dots.right() <= compact.notch.right() - 8);
        assert_eq!(compact.notch_secondary.y, compact.notch.y);
        assert_eq!(compact.notch_secondary.height, compact.notch.height);
        assert_eq!(compact.notch_secondary.width, compact.notch.height);

        let compact_hidpi = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            2880,
            144,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                secondary_expansion: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(compact_hidpi.notch_secondary.y, compact_hidpi.notch.y);
        assert_eq!(
            compact_hidpi.notch_secondary.height,
            compact_hidpi.notch.height
        );
        assert_eq!(
            compact_hidpi.notch_secondary.width,
            compact_hidpi.notch.height
        );

        let hud = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(hud.notch.y, 4);
        assert_eq!(hud.notch.height, 32);
        assert!(hud.notch.contains_rect(&hud.hud.glyph));
        assert!(hud.notch.contains_rect(&hud.hud.meter));
        assert!(hud.notch.contains_rect(&hud.hud.value));
        assert_eq!(idle.popup_anchor_y(96), 44);
        assert_eq!(
            compute_layout(&Metrics::DYNAMIC_ISLAND, 2880, 144).popup_anchor_y(144),
            66
        );
    }

    #[test]
    fn dynamic_island_expands_floating_card_without_ears() {
        let expanded = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                lyrics_progress: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(expanded.bar.height, 40);
        assert_eq!(expanded.notch.y, 4);
        assert_eq!(expanded.notch.width, 380);
        assert_eq!(expanded.notch.height, 254);
        assert_eq!(expanded.notch_corner_radius, 24);
        assert_eq!(expanded.notch_ear_radius, 0);
        assert!(expanded.panel_window.height >= expanded.notch.bottom() + 4);
        assert!(expanded.notch.contains_rect(&expanded.notch_tab_pomodoro));
        assert!(expanded.notch.contains_rect(&expanded.media_cover_art));
        assert!(expanded.notch.contains_rect(&expanded.media_lyrics_drawer));
        assert!(expanded.notch.contains_rect(&expanded.media_seekbar));
        assert_eq!(
            expanded.hit_test(
                expanded.media_play_button.x + 8,
                expanded.media_play_button.y + 8,
            ),
            Some(HitTarget::MediaPlayPause)
        );
    }
}
