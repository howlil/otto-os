use crate::notch::clipboard::CLIPBOARD_PAGE_SIZE;
use crate::notch::focus_stats_view::{FocusStatsRange, NOTCH_STATS_DAYS};
use crate::notch::hud::HudGeometry;
use crate::notch::island::CompactSlotKind;
use crate::notch::media::NotchExpandedTab;
use crate::notch::pomodoro_view::PomodoroSurface;
use crate::notch::shelf::SHELF_PAGE_SIZE;
use crate::notch::tasks::TASKS_PAGE_SIZE;

use super::geometry::Rect;
use super::geometry::scale;
use super::hit::HitTarget;
use super::notification::NotificationLayout;

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
    pub active_tab: NotchExpandedTab,
    pub previous_tab: NotchExpandedTab,
    pub compact_ring: Rect,
    pub compact_peek: Rect,
    pub compact_cycle_steps: [Rect; 4],
    pub compact_time: Rect,
    pub compact_title: Rect,
    pub compact_trailing: Rect,
    pub notch_tab_strip: Rect,
    pub notch_tab_pomodoro: Rect,
    pub notch_tab_stats: Rect,
    pub notch_tab_shelf: Rect,
    pub notch_tab_clipboard: Rect,
    pub notch_tab_tasks: Rect,
    pub notch_tab_media: Rect,
    pub lyrics_progress: f32,
    pub stats_progress: f32,
    pub tab_blend: f32,
    pub press_progress: f32,
    pub icon_size: i32,
    pub notification: Option<NotificationLayout>,
    pub hud: HudGeometry,
    pub pomodoro: PomodoroPanelLayout,
    pub stats: FocusStatsPanelLayout,
    pub shelf: ShelfPanelLayout,
    pub clipboard: ClipboardPanelLayout,
    pub tasks: TasksPanelLayout,
    pub media: MediaPanelLayout,
    pub privacy: PrivacyPanelLayout,
    pub slots: BarSlotsLayout,
}

pub const NOTCH_CATEGORY_OPTIONS: usize = 6;
pub const NOTCH_HEATMAP_FILTERS: usize = 5;
pub const NOTCH_HEATMAP_CELLS: usize = 112;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PomodoroPanelLayout {
    pub surface: PomodoroSurface,
    pub icon: Rect,
    pub time: Rect,
    pub subtitle: Rect,
    pub session_label: Rect,
    pub category_pill: Rect,
    pub open_settings: Rect,
    pub category_options: [Rect; NOTCH_CATEGORY_OPTIONS],
    pub duration_dec: Rect,
    pub duration_inc: Rect,
    pub segments: [Rect; 4],
    pub footer_left: Rect,
    pub footer_right: Rect,
    pub presets: [Rect; 4],
    pub play_button: Rect,
    pub reset_button: Rect,
    pub skip_button: Rect,
    pub alert_start_button: Rect,
    pub alert_extend_button: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusStatsPanelLayout {
    pub body: Rect,
    pub range_today: Rect,
    pub range_week: Rect,
    pub range_heatmap: Rect,
    pub open_settings: Rect,
    pub days: [Rect; NOTCH_STATS_DAYS],
    pub heatmap_filters: [Rect; NOTCH_HEATMAP_FILTERS],
    pub heatmap_grid: Rect,
    pub heatmap_cells: Vec<Rect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfPanelLayout {
    pub add_button: Rect,
    pub clear_button: Rect,
    pub pager_button: Rect,
    pub dropzone: Rect,
    pub cards: [Rect; SHELF_PAGE_SIZE],
    pub copy_buttons: [Rect; SHELF_PAGE_SIZE],
    pub remove_buttons: [Rect; SHELF_PAGE_SIZE],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardPanelLayout {
    pub pause_button: Rect,
    pub clear_button: Rect,
    pub pager_button: Rect,
    pub body: Rect,
    pub rows: [Rect; CLIPBOARD_PAGE_SIZE],
    pub remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasksPanelLayout {
    pub add_button: Rect,
    pub clear_button: Rect,
    pub pager_button: Rect,
    pub editor: Rect,
    pub editor_input: Rect,
    pub body: Rect,
    pub rows: [Rect; TASKS_PAGE_SIZE],
    pub check_buttons: [Rect; TASKS_PAGE_SIZE],
    pub edit_buttons: [Rect; TASKS_PAGE_SIZE],
    pub remove_buttons: [Rect; TASKS_PAGE_SIZE],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPanelLayout {
    pub cover_art: Rect,
    pub source_label: Rect,
    pub title: Rect,
    pub artist: Rect,
    pub reason: Rect,
    pub equalizer: Rect,
    pub lyrics_toggle: Rect,
    pub lyrics_drawer: Rect,
    pub seekbar: Rect,
    pub play_button: Rect,
    pub prev_button: Rect,
    pub next_button: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyPanelLayout {
    pub dots: Rect,
    pub mic_button: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarSlotsLayout {
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
    pub notification_slot: Rect,
    pub clock_slot: Rect,
}

impl Layout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<HitTarget> {
        if self.panel_expansion > 0.55
            && self.notch.contains(x, y)
            && let Some(card) = &self.notification
        {
            return Some(card.hit_test(x, y));
        }
        if self.panel_expansion > 0.55 && self.notch.contains(x, y) {
            return Some(self.hit_test_expanded_notch(x, y));
        }

        if self.slots.start_slot.contains(x, y) {
            Some(HitTarget::Start)
        } else if self.slots.app_slot.contains(x, y) {
            Some(HitTarget::AppMenu)
        } else if self.slots.file_slot.contains(x, y) {
            Some(HitTarget::FileMenu)
        } else if self.slots.edit_slot.contains(x, y) {
            Some(HitTarget::EditMenu)
        } else if self.slots.view_slot.contains(x, y) {
            Some(HitTarget::ViewMenu)
        } else if self.slots.window_slot.contains(x, y) {
            Some(HitTarget::WindowMenu)
        } else if self.slots.help_slot.contains(x, y) {
            Some(HitTarget::HelpMenu)
        } else if self.slots.overflow_slot.contains(x, y) {
            Some(HitTarget::MenuOverflow)
        } else if self.privacy.dots.contains(x, y) {
            Some(HitTarget::PrivacyIndicator)
        } else if self.notch.contains(x, y) {
            Some(HitTarget::Notch)
        } else if self.secondary_expansion > 0.25 && self.notch_secondary.contains(x, y) {
            Some(HitTarget::NotchSecondary)
        } else if self.slots.tray_slot.contains(x, y) {
            Some(HitTarget::Tray)
        } else if self.slots.status_slot.contains(x, y) {
            if self.slots.status_wifi_rect.contains(x, y) {
                Some(HitTarget::StatusWifi)
            } else if self.slots.status_volume_rect.contains(x, y) {
                Some(HitTarget::StatusVolume)
            } else if self.slots.status_battery_rect.contains(x, y) {
                Some(HitTarget::StatusBattery)
            } else {
                None
            }
        } else if self.slots.search_slot.contains(x, y) {
            Some(HitTarget::Search)
        } else if self.slots.control_center_slot.contains(x, y) {
            Some(HitTarget::ControlCenter)
        } else if self.slots.notification_slot.contains(x, y) {
            Some(HitTarget::Notifications)
        } else if self.slots.clock_slot.contains(x, y) {
            Some(HitTarget::Clock)
        } else {
            None
        }
    }

    pub(super) fn hit_test_expanded_notch(&self, x: i32, y: i32) -> HitTarget {
        if self.pomodoro.surface == PomodoroSurface::Alert {
            if self.pomodoro.alert_start_button.contains(x, y) {
                return HitTarget::PomodoroStartNext;
            }
            if self.pomodoro.alert_extend_button.contains(x, y) {
                return HitTarget::PomodoroExtendFive;
            }
            return HitTarget::Notch;
        }
        for (rect, target) in [
            (self.notch_tab_pomodoro, HitTarget::NotchTabPomodoro),
            (self.notch_tab_stats, HitTarget::NotchTabStats),
            (self.notch_tab_shelf, HitTarget::NotchTabShelf),
            (self.notch_tab_clipboard, HitTarget::NotchTabClipboard),
            (self.notch_tab_tasks, HitTarget::NotchTabTasks),
            (self.notch_tab_media, HitTarget::NotchTabMedia),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        if self.privacy.mic_button.contains(x, y) {
            return HitTarget::PrivacyMicMute;
        }
        match self.active_tab {
            NotchExpandedTab::Media => {
                for (rect, target) in [
                    (self.media.lyrics_toggle, HitTarget::MediaLyricsToggle),
                    (self.media.play_button, HitTarget::MediaPlayPause),
                    (self.media.prev_button, HitTarget::MediaPrevious),
                    (self.media.next_button, HitTarget::MediaNext),
                    (self.media.seekbar, HitTarget::MediaSeekbar),
                ] {
                    if rect.contains(x, y) {
                        return target;
                    }
                }
            }
            NotchExpandedTab::Pomodoro => return self.hit_test_pomodoro(x, y),
            NotchExpandedTab::Stats => return self.hit_test_stats(x, y),
            NotchExpandedTab::Shelf => return self.hit_test_shelf(x, y),
            NotchExpandedTab::Clipboard => return self.hit_test_clipboard(x, y),
            NotchExpandedTab::Tasks => return self.hit_test_tasks(x, y),
        }
        HitTarget::Notch
    }

    fn hit_test_pomodoro(&self, x: i32, y: i32) -> HitTarget {
        if self.pomodoro.open_settings.contains(x, y) {
            return HitTarget::PomodoroOpenSettings;
        }
        if self.pomodoro.category_pill.contains(x, y) {
            return HitTarget::PomodoroCategoryPill;
        }
        for (index, option) in self.pomodoro.category_options.iter().enumerate() {
            if option.contains(x, y) {
                return HitTarget::PomodoroCategoryOption(index as u8);
            }
        }
        for (rect, target) in [
            (self.pomodoro.duration_dec, HitTarget::PomodoroDurationDec),
            (self.pomodoro.duration_inc, HitTarget::PomodoroDurationInc),
            (self.pomodoro.play_button, HitTarget::PomodoroPlayPause),
            (self.pomodoro.reset_button, HitTarget::PomodoroReset),
            (self.pomodoro.skip_button, HitTarget::PomodoroSkip),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        if matches!(
            self.pomodoro.surface,
            PomodoroSurface::Setup | PomodoroSurface::Paused | PomodoroSurface::Ready
        ) && self.pomodoro.time.contains(x, y)
        {
            return HitTarget::PomodoroTimeEdit;
        }
        for (index, preset) in self.pomodoro.presets.iter().enumerate() {
            if preset.contains(x, y) {
                return HitTarget::PomodoroPreset(index as u8);
            }
        }
        HitTarget::Notch
    }

    fn hit_test_stats(&self, x: i32, y: i32) -> HitTarget {
        for (rect, target) in [
            (
                self.stats.range_today,
                HitTarget::FocusStatsRange(FocusStatsRange::Today),
            ),
            (
                self.stats.range_week,
                HitTarget::FocusStatsRange(FocusStatsRange::Week),
            ),
            (
                self.stats.range_heatmap,
                HitTarget::FocusStatsRange(FocusStatsRange::Heatmap),
            ),
            (self.stats.open_settings, HitTarget::FocusStatsOpenSettings),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        for (index, day) in self.stats.days.iter().enumerate() {
            if day.contains(x, y) {
                return HitTarget::FocusStatsDay(index as u8);
            }
        }
        for (index, filter) in self.stats.heatmap_filters.iter().enumerate() {
            if filter.contains(x, y) {
                return HitTarget::FocusHeatmapFilter(index as u8);
            }
        }
        for (index, cell) in self.stats.heatmap_cells.iter().enumerate() {
            if cell.contains(x, y) {
                return HitTarget::FocusHeatmapCell(index as u8);
            }
        }
        HitTarget::Notch
    }

    pub(super) fn hit_test_clipboard(&self, x: i32, y: i32) -> HitTarget {
        for (rect, target) in [
            (self.clipboard.pause_button, HitTarget::ClipboardPauseToggle),
            (self.clipboard.clear_button, HitTarget::ClipboardClear),
            (self.clipboard.pager_button, HitTarget::ClipboardPager),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        for (index, row) in self.clipboard.rows.iter().enumerate() {
            if row.is_empty() {
                continue;
            }
            if self.clipboard.remove_buttons[index].contains(x, y) {
                return HitTarget::ClipboardRemoveItem(index as u8);
            }
            if row.contains(x, y) {
                return HitTarget::ClipboardItem(index as u8);
            }
        }
        HitTarget::Notch
    }

    pub(super) fn hit_test_tasks(&self, x: i32, y: i32) -> HitTarget {
        for (rect, target) in [
            (self.tasks.add_button, HitTarget::TasksAdd),
            (self.tasks.clear_button, HitTarget::TasksClearDone),
            (self.tasks.pager_button, HitTarget::TasksPager),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        for (index, row) in self.tasks.rows.iter().enumerate() {
            if row.is_empty() {
                continue;
            }
            if self.tasks.remove_buttons[index].contains(x, y) {
                return HitTarget::TaskRemove(index as u8);
            }
            if self.tasks.edit_buttons[index].contains(x, y) {
                return HitTarget::TaskEdit(index as u8);
            }
            if self.tasks.check_buttons[index].contains(x, y) {
                return HitTarget::TaskCheck(index as u8);
            }
            if row.contains(x, y) {
                return HitTarget::TaskItem(index as u8);
            }
        }
        HitTarget::Notch
    }

    pub(super) fn hit_test_shelf(&self, x: i32, y: i32) -> HitTarget {
        for (rect, target) in [
            (self.shelf.add_button, HitTarget::ShelfAdd),
            (self.shelf.clear_button, HitTarget::ShelfClear),
            (self.shelf.pager_button, HitTarget::ShelfPager),
        ] {
            if rect.contains(x, y) {
                return target;
            }
        }
        for (index, card) in self.shelf.cards.iter().enumerate() {
            if card.is_empty() {
                continue;
            }
            if self.shelf.remove_buttons[index].contains(x, y) {
                return HitTarget::ShelfRemoveItem(index as u8);
            }
            if self.shelf.copy_buttons[index].contains(x, y) {
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
        self.slots.hidden_targets.contains(&target)
            || (matches!(
                target,
                HitTarget::StatusWifi | HitTarget::StatusVolume | HitTarget::StatusBattery
            ) && self
                .slots
                .hidden_targets
                .contains(&HitTarget::QuickSettings))
    }

    #[must_use]
    pub fn menu_anchor_rect(&self, target: HitTarget) -> Option<Rect> {
        if self.is_hidden(target) {
            return Some(self.slots.overflow_slot);
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
            HitTarget::Start => Some(self.slots.start_slot),
            HitTarget::AppMenu => Some(self.slots.app_slot),
            HitTarget::FileMenu => Some(self.slots.file_slot),
            HitTarget::EditMenu => Some(self.slots.edit_slot),
            HitTarget::ViewMenu => Some(self.slots.view_slot),
            HitTarget::WindowMenu => Some(self.slots.window_slot),
            HitTarget::HelpMenu => Some(self.slots.help_slot),
            HitTarget::MenuOverflow => Some(self.slots.overflow_slot),
            HitTarget::Notch
            | HitTarget::NotchSecondary
            | HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive
            | HitTarget::PomodoroPreset(_)
            | HitTarget::PomodoroCategoryPill
            | HitTarget::PomodoroCategoryOption(_)
            | HitTarget::PomodoroDurationDec
            | HitTarget::PomodoroDurationInc
            | HitTarget::PomodoroTimeEdit
            | HitTarget::PomodoroOpenSettings
            | HitTarget::FocusStatsRange(_)
            | HitTarget::FocusStatsDay(_)
            | HitTarget::FocusHeatmapFilter(_)
            | HitTarget::FocusHeatmapCell(_)
            | HitTarget::FocusStatsOpenSettings
            | HitTarget::NotchTabPomodoro
            | HitTarget::NotchTabStats
            | HitTarget::NotchTabShelf
            | HitTarget::NotchTabClipboard
            | HitTarget::NotchTabTasks
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
            | HitTarget::TasksAdd
            | HitTarget::TasksClearDone
            | HitTarget::TasksPager
            | HitTarget::TaskItem(_)
            | HitTarget::TaskCheck(_)
            | HitTarget::TaskEdit(_)
            | HitTarget::TaskRemove(_)
            | HitTarget::PrivacyIndicator
            | HitTarget::PrivacyMicMute
            | HitTarget::MediaPlayPause
            | HitTarget::MediaPrevious
            | HitTarget::MediaNext
            | HitTarget::MediaSeekbar
            | HitTarget::MediaLyricsToggle
            | HitTarget::NotificationCard
            | HitTarget::NotificationClose
            | HitTarget::NotificationAccept
            | HitTarget::NotificationDecline => None,
            HitTarget::Tray => Some(self.slots.tray_slot),
            HitTarget::StatusWifi => Some(self.slots.status_wifi_rect),
            HitTarget::StatusVolume => Some(self.slots.status_volume_rect),
            HitTarget::StatusBattery => Some(self.slots.status_battery_rect),
            HitTarget::QuickSettings => Some(self.slots.status_slot),
            HitTarget::Search => Some(self.slots.search_slot),
            HitTarget::ControlCenter => Some(self.slots.control_center_slot),
            HitTarget::Notifications => Some(self.slots.notification_slot),
            HitTarget::Clock => Some(self.slots.clock_slot),
        }
    }
}

impl Layout {
    #[must_use]
    pub fn notch_control_rect(&self, target: HitTarget) -> Option<Rect> {
        if self.panel_expansion <= 0.55 {
            return None;
        }
        if let Some(card) = &self.notification {
            return card.control_rect(target);
        }
        if self.pomodoro.surface == PomodoroSurface::Alert {
            let rect = match target {
                HitTarget::PomodoroExtendFive => self.pomodoro.alert_extend_button,
                HitTarget::PomodoroStartNext => self.pomodoro.alert_start_button,
                _ => return None,
            };
            return (!rect.is_empty()).then_some(rect);
        }
        let rect = self
            .notch_shared_control_rect(target)
            .or_else(|| self.notch_active_tab_control_rect(target))?;
        (!rect.is_empty()).then_some(rect)
    }

    const fn notch_shared_control_rect(&self, target: HitTarget) -> Option<Rect> {
        Some(match target {
            HitTarget::NotchTabPomodoro => self.notch_tab_pomodoro,
            HitTarget::NotchTabStats => self.notch_tab_stats,
            HitTarget::NotchTabShelf => self.notch_tab_shelf,
            HitTarget::NotchTabClipboard => self.notch_tab_clipboard,
            HitTarget::NotchTabTasks => self.notch_tab_tasks,
            HitTarget::NotchTabMedia => self.notch_tab_media,
            HitTarget::PrivacyMicMute => self.privacy.mic_button,
            _ => return None,
        })
    }

    fn notch_active_tab_control_rect(&self, target: HitTarget) -> Option<Rect> {
        match self.active_tab {
            NotchExpandedTab::Pomodoro => self.pomodoro_control_rect(target),
            NotchExpandedTab::Stats => self.stats_control_rect(target),
            NotchExpandedTab::Shelf => self.shelf_control_rect(target),
            NotchExpandedTab::Clipboard => self.clipboard_control_rect(target),
            NotchExpandedTab::Tasks => self.tasks_control_rect(target),
            NotchExpandedTab::Media => self.media_control_rect(target),
        }
    }

    fn pomodoro_control_rect(&self, target: HitTarget) -> Option<Rect> {
        match target {
            HitTarget::PomodoroOpenSettings => Some(self.pomodoro.open_settings),
            HitTarget::PomodoroCategoryPill => Some(self.pomodoro.category_pill),
            HitTarget::PomodoroCategoryOption(index) => self
                .pomodoro
                .category_options
                .get(usize::from(index))
                .copied(),
            HitTarget::PomodoroDurationDec => Some(self.pomodoro.duration_dec),
            HitTarget::PomodoroDurationInc => Some(self.pomodoro.duration_inc),
            HitTarget::PomodoroTimeEdit => matches!(
                self.pomodoro.surface,
                PomodoroSurface::Setup | PomodoroSurface::Paused | PomodoroSurface::Ready
            )
            .then_some(self.pomodoro.time),
            HitTarget::PomodoroPreset(index) => {
                self.pomodoro.presets.get(usize::from(index)).copied()
            }
            HitTarget::PomodoroPlayPause => Some(self.pomodoro.play_button),
            HitTarget::PomodoroReset => Some(self.pomodoro.reset_button),
            HitTarget::PomodoroSkip => Some(self.pomodoro.skip_button),
            _ => None,
        }
    }

    fn stats_control_rect(&self, target: HitTarget) -> Option<Rect> {
        match target {
            HitTarget::FocusStatsRange(FocusStatsRange::Today) => Some(self.stats.range_today),
            HitTarget::FocusStatsRange(FocusStatsRange::Week) => Some(self.stats.range_week),
            HitTarget::FocusStatsRange(FocusStatsRange::Heatmap) => Some(self.stats.range_heatmap),
            HitTarget::FocusStatsOpenSettings => Some(self.stats.open_settings),
            HitTarget::FocusStatsDay(index) => self.stats.days.get(usize::from(index)).copied(),
            HitTarget::FocusHeatmapFilter(index) => {
                self.stats.heatmap_filters.get(usize::from(index)).copied()
            }
            HitTarget::FocusHeatmapCell(index) => {
                self.stats.heatmap_cells.get(usize::from(index)).copied()
            }
            _ => None,
        }
    }

    fn shelf_control_rect(&self, target: HitTarget) -> Option<Rect> {
        match target {
            HitTarget::ShelfAdd => Some(self.shelf.add_button),
            HitTarget::ShelfClear => Some(self.shelf.clear_button),
            HitTarget::ShelfPager => Some(self.shelf.pager_button),
            HitTarget::ShelfItem(index) => self.shelf.cards.get(usize::from(index)).copied(),
            HitTarget::ShelfCopyItem(index) => {
                self.shelf.copy_buttons.get(usize::from(index)).copied()
            }
            HitTarget::ShelfRemoveItem(index) => {
                self.shelf.remove_buttons.get(usize::from(index)).copied()
            }
            _ => None,
        }
    }

    fn clipboard_control_rect(&self, target: HitTarget) -> Option<Rect> {
        match target {
            HitTarget::ClipboardPauseToggle => Some(self.clipboard.pause_button),
            HitTarget::ClipboardClear => Some(self.clipboard.clear_button),
            HitTarget::ClipboardPager => Some(self.clipboard.pager_button),
            HitTarget::ClipboardItem(index) => self.clipboard.rows.get(usize::from(index)).copied(),
            HitTarget::ClipboardRemoveItem(index) => self
                .clipboard
                .remove_buttons
                .get(usize::from(index))
                .copied(),
            _ => None,
        }
    }

    fn tasks_control_rect(&self, target: HitTarget) -> Option<Rect> {
        match target {
            HitTarget::TasksAdd => Some(self.tasks.add_button),
            HitTarget::TasksClearDone => Some(self.tasks.clear_button),
            HitTarget::TasksPager => Some(self.tasks.pager_button),
            HitTarget::TaskItem(index) => self.tasks.rows.get(usize::from(index)).copied(),
            HitTarget::TaskCheck(index) => {
                self.tasks.check_buttons.get(usize::from(index)).copied()
            }
            HitTarget::TaskEdit(index) => self.tasks.edit_buttons.get(usize::from(index)).copied(),
            HitTarget::TaskRemove(index) => {
                self.tasks.remove_buttons.get(usize::from(index)).copied()
            }
            _ => None,
        }
    }

    const fn media_control_rect(&self, target: HitTarget) -> Option<Rect> {
        Some(match target {
            HitTarget::MediaPrevious => self.media.prev_button,
            HitTarget::MediaPlayPause => self.media.play_button,
            HitTarget::MediaNext => self.media.next_button,
            HitTarget::MediaLyricsToggle => self.media.lyrics_toggle,
            HitTarget::MediaSeekbar => self.media.seekbar,
            _ => return None,
        })
    }
}
