use crate::notch::clipboard::CLIPBOARD_PAGE_SIZE;
use crate::notch::focus_stats_view::{FocusStatsRange, NOTCH_STATS_DAYS};
use crate::notch::hud::HudGeometry;
use crate::notch::island::CompactSlotKind;
use crate::notch::media::NotchExpandedTab;
use crate::notch::pomodoro::FocusSubMode;
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
    pub focus_mode: FocusSubMode,
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

#[derive(Debug, Clone, PartialEq)]
pub struct PomodoroPanelLayout {
    pub surface: PomodoroSurface,
    pub icon: Rect,
    pub time: Rect,
    pub subtitle: Rect,
    pub session_label: Rect,
    pub segments: [Rect; 4],
    pub footer_left: Rect,
    pub footer_right: Rect,
    pub presets: [Rect; 4],
    pub focus_modes: [Rect; 3],
    pub play_button: Rect,
    pub reset_button: Rect,
    pub skip_button: Rect,
    pub alert_start_button: Rect,
    pub alert_extend_button: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FocusStatsPanelLayout {
    pub body: Rect,
    pub range_today: Rect,
    pub range_week: Rect,
    pub open_settings: Rect,
    pub days: [Rect; NOTCH_STATS_DAYS],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShelfPanelLayout {
    pub add_button: Rect,
    pub clear_button: Rect,
    pub pager_button: Rect,
    pub dropzone: Rect,
    pub cards: [Rect; SHELF_PAGE_SIZE],
    pub copy_buttons: [Rect; SHELF_PAGE_SIZE],
    pub remove_buttons: [Rect; SHELF_PAGE_SIZE],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardPanelLayout {
    pub pause_button: Rect,
    pub clear_button: Rect,
    pub pager_button: Rect,
    pub body: Rect,
    pub rows: [Rect; CLIPBOARD_PAGE_SIZE],
    pub remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct PrivacyPanelLayout {
    pub dots: Rect,
    pub mic_button: Rect,
}

#[derive(Debug, Clone, PartialEq)]
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
            NotchExpandedTab::Pomodoro => {
                for (mode, rect) in FocusSubMode::ALL.into_iter().zip(self.pomodoro.focus_modes) {
                    if rect.contains(x, y) {
                        return HitTarget::FocusMode(mode);
                    }
                }
                for (rect, target) in [
                    (self.pomodoro.play_button, HitTarget::PomodoroPlayPause),
                    (self.pomodoro.reset_button, HitTarget::PomodoroReset),
                    (self.pomodoro.skip_button, HitTarget::PomodoroSkip),
                ] {
                    if rect.contains(x, y) {
                        return target;
                    }
                }
                for (index, preset) in self.pomodoro.presets.iter().enumerate() {
                    if preset.contains(x, y) {
                        return HitTarget::PomodoroPreset(index as u8);
                    }
                }
            }
            NotchExpandedTab::Stats => {
                for (rect, target) in [
                    (
                        self.stats.range_today,
                        HitTarget::FocusStatsRange(FocusStatsRange::Today),
                    ),
                    (
                        self.stats.range_week,
                        HitTarget::FocusStatsRange(FocusStatsRange::Week),
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
            }
            NotchExpandedTab::Shelf => return self.hit_test_shelf(x, y),
            NotchExpandedTab::Clipboard => return self.hit_test_clipboard(x, y),
            NotchExpandedTab::Tasks => return self.hit_test_tasks(x, y),
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
            | HitTarget::FocusMode(_)
            | HitTarget::FocusStatsRange(_)
            | HitTarget::FocusStatsDay(_)
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
    #[allow(clippy::too_many_lines)]
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
        let rect = match target {
            HitTarget::NotchTabPomodoro => self.notch_tab_pomodoro,
            HitTarget::NotchTabStats => self.notch_tab_stats,
            HitTarget::NotchTabShelf => self.notch_tab_shelf,
            HitTarget::NotchTabClipboard => self.notch_tab_clipboard,
            HitTarget::NotchTabTasks => self.notch_tab_tasks,
            HitTarget::NotchTabMedia => self.notch_tab_media,
            HitTarget::FocusMode(mode) if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro.focus_modes[mode.index()]
            }
            HitTarget::PomodoroPreset(index) if self.active_tab == NotchExpandedTab::Pomodoro => {
                *self.pomodoro.presets.get(usize::from(index))?
            }
            HitTarget::PomodoroPlayPause if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro.play_button
            }
            HitTarget::PomodoroReset if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro.reset_button
            }
            HitTarget::PomodoroSkip if self.active_tab == NotchExpandedTab::Pomodoro => {
                self.pomodoro.skip_button
            }
            HitTarget::FocusStatsRange(FocusStatsRange::Today)
                if self.active_tab == NotchExpandedTab::Stats =>
            {
                self.stats.range_today
            }
            HitTarget::FocusStatsRange(FocusStatsRange::Week)
                if self.active_tab == NotchExpandedTab::Stats =>
            {
                self.stats.range_week
            }
            HitTarget::FocusStatsOpenSettings if self.active_tab == NotchExpandedTab::Stats => {
                self.stats.open_settings
            }
            HitTarget::FocusStatsDay(index) if self.active_tab == NotchExpandedTab::Stats => {
                *self.stats.days.get(usize::from(index))?
            }
            HitTarget::ShelfAdd if self.active_tab == NotchExpandedTab::Shelf => {
                self.shelf.add_button
            }
            HitTarget::ShelfClear if self.active_tab == NotchExpandedTab::Shelf => {
                self.shelf.clear_button
            }
            HitTarget::ShelfPager if self.active_tab == NotchExpandedTab::Shelf => {
                self.shelf.pager_button
            }
            HitTarget::ShelfItem(index) if self.active_tab == NotchExpandedTab::Shelf => {
                *self.shelf.cards.get(usize::from(index))?
            }
            HitTarget::ShelfCopyItem(index) if self.active_tab == NotchExpandedTab::Shelf => {
                *self.shelf.copy_buttons.get(usize::from(index))?
            }
            HitTarget::ShelfRemoveItem(index) if self.active_tab == NotchExpandedTab::Shelf => {
                *self.shelf.remove_buttons.get(usize::from(index))?
            }
            HitTarget::ClipboardPauseToggle if self.active_tab == NotchExpandedTab::Clipboard => {
                self.clipboard.pause_button
            }
            HitTarget::ClipboardClear if self.active_tab == NotchExpandedTab::Clipboard => {
                self.clipboard.clear_button
            }
            HitTarget::ClipboardPager if self.active_tab == NotchExpandedTab::Clipboard => {
                self.clipboard.pager_button
            }
            HitTarget::ClipboardItem(index) if self.active_tab == NotchExpandedTab::Clipboard => {
                *self.clipboard.rows.get(usize::from(index))?
            }
            HitTarget::ClipboardRemoveItem(index)
                if self.active_tab == NotchExpandedTab::Clipboard =>
            {
                *self.clipboard.remove_buttons.get(usize::from(index))?
            }
            HitTarget::TasksAdd if self.active_tab == NotchExpandedTab::Tasks => {
                self.tasks.add_button
            }
            HitTarget::TasksClearDone if self.active_tab == NotchExpandedTab::Tasks => {
                self.tasks.clear_button
            }
            HitTarget::TasksPager if self.active_tab == NotchExpandedTab::Tasks => {
                self.tasks.pager_button
            }
            HitTarget::TaskItem(index) if self.active_tab == NotchExpandedTab::Tasks => {
                *self.tasks.rows.get(usize::from(index))?
            }
            HitTarget::TaskCheck(index) if self.active_tab == NotchExpandedTab::Tasks => {
                *self.tasks.check_buttons.get(usize::from(index))?
            }
            HitTarget::TaskEdit(index) if self.active_tab == NotchExpandedTab::Tasks => {
                *self.tasks.edit_buttons.get(usize::from(index))?
            }
            HitTarget::TaskRemove(index) if self.active_tab == NotchExpandedTab::Tasks => {
                *self.tasks.remove_buttons.get(usize::from(index))?
            }
            HitTarget::PrivacyMicMute => self.privacy.mic_button,
            HitTarget::MediaPrevious if self.active_tab == NotchExpandedTab::Media => {
                self.media.prev_button
            }
            HitTarget::MediaPlayPause if self.active_tab == NotchExpandedTab::Media => {
                self.media.play_button
            }
            HitTarget::MediaNext if self.active_tab == NotchExpandedTab::Media => {
                self.media.next_button
            }
            HitTarget::MediaLyricsToggle if self.active_tab == NotchExpandedTab::Media => {
                self.media.lyrics_toggle
            }
            HitTarget::MediaSeekbar if self.active_tab == NotchExpandedTab::Media => {
                self.media.seekbar
            }
            _ => return None,
        };
        (!rect.is_empty()).then_some(rect)
    }
}
