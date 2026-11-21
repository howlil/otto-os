use std::time::Instant;

use otto_core::{FocusSubMode, HitTarget, NotchExpandedTab};

use super::NotchController;
use super::NotchHotkey;
use super::NotchUpdate;
use super::PanelTrigger;

impl NotchController {
    pub(crate) fn handle_hotkey(&mut self, hotkey: NotchHotkey) -> NotchUpdate {
        let now = Instant::now();
        let now_ms = self.millis(now);
        match hotkey {
            NotchHotkey::ToggleNotch => {
                if self.is_panel_target_open(now) {
                    self.panel_trigger = PanelTrigger::Closed;
                    self.hover_since = None;
                    self.opened_by_task_editor = false;
                    self.tasks.cancel_editor();
                    self.dismiss_active_alert();
                } else {
                    if !self.has_openable_panel() {
                        return NotchUpdate::default();
                    }
                    self.ensure_valid_tab();
                    self.panel_trigger = PanelTrigger::Pinned;
                }
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            NotchHotkey::TogglePomodoro => {
                if !self.focus_enabled {
                    return NotchUpdate::default();
                }
                self.clear_focus_dismissed();
                match self.focus_mode {
                    FocusSubMode::Pomodoro => {
                        let previous = self.pomodoro.status;
                        self.pomodoro.toggle_run();
                        self.note_pomodoro_transition(previous, now);
                        self.clock.restart(now_ms);
                        self.alert_until = None;
                        NotchUpdate {
                            changed: true,
                            effect: None,
                        }
                    }
                    FocusSubMode::Timer => {
                        self.quick_timer.toggle_run();
                        self.timer_clock.restart(now_ms);
                        self.alert_until = None;
                        NotchUpdate {
                            changed: true,
                            effect: None,
                        }
                    }
                    FocusSubMode::Stopwatch => {
                        self.stopwatch_clock.sync(&mut self.stopwatch, now_ms, true);
                        self.stopwatch.toggle_run();
                        self.stopwatch_clock.restart(now_ms);
                        NotchUpdate {
                            changed: true,
                            effect: None,
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn handle_pointer_move(&mut self, inside_notch: bool) {
        if inside_notch {
            if self.panel_expansion > 0.55 && self.panel_trigger == PanelTrigger::Pinned {
                self.panel_trigger = PanelTrigger::PinnedHovered;
            }
            if self.hover_since.is_none()
                && self.panel_trigger == PanelTrigger::Closed
                && !self.hover_blocked
                && self.has_openable_panel()
            {
                self.hover_since = Some(Instant::now());
            }
        } else {
            self.hover_since = None;
            self.hover_blocked = false;
        }
    }

    pub(crate) fn handle_pointer_leave(&mut self, cursor_inside_notch: bool) -> bool {
        if cursor_inside_notch || self.is_panel_held_open() {
            return false;
        }
        let press_cleared = std::mem::take(&mut self.press_active);
        self.hover_since = None;
        self.hover_blocked = false;
        if matches!(
            self.panel_trigger,
            PanelTrigger::Hovered | PanelTrigger::PinnedHovered
        ) {
            self.panel_trigger = PanelTrigger::Closed;
            return true;
        }
        press_cleared
    }

    pub(crate) fn handle_click(&mut self, target: HitTarget) -> NotchUpdate {
        self.press_active = false;
        match target {
            HitTarget::FocusMode(mode) => self.handle_focus_mode_click(mode),
            HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroPreset(_)
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive => self.handle_pomodoro_click(target),
            HitTarget::NotchTabPomodoro
            | HitTarget::NotchTabShelf
            | HitTarget::NotchTabClipboard
            | HitTarget::NotchTabTasks
            | HitTarget::NotchTabMedia => self.handle_tab_click(target),
            HitTarget::ShelfAdd
            | HitTarget::ShelfClear
            | HitTarget::ShelfPager
            | HitTarget::ShelfItem(_)
            | HitTarget::ShelfCopyItem(_)
            | HitTarget::ShelfRemoveItem(_) => self.handle_shelf_click(target),
            HitTarget::ClipboardPauseToggle
            | HitTarget::ClipboardClear
            | HitTarget::ClipboardPager
            | HitTarget::ClipboardItem(_)
            | HitTarget::ClipboardRemoveItem(_) => self.handle_clipboard_click(target),
            HitTarget::TasksAdd
            | HitTarget::TasksClearDone
            | HitTarget::TasksPager
            | HitTarget::TaskItem(_)
            | HitTarget::TaskCheck(_)
            | HitTarget::TaskEdit(_)
            | HitTarget::TaskRemove(_) => self.handle_tasks_click(target),
            HitTarget::PrivacyIndicator => self.handle_privacy_indicator_click(),
            HitTarget::PrivacyMicMute => self.handle_privacy_mic_click(),
            HitTarget::MediaPlayPause | HitTarget::MediaPrevious | HitTarget::MediaNext => {
                self.handle_media_click(target)
            }
            HitTarget::MediaLyricsToggle => self.toggle_lyrics(),
            HitTarget::NotificationCard
            | HitTarget::NotificationClose
            | HitTarget::NotificationAccept
            | HitTarget::NotificationDecline => self.handle_notification_click(target),
            HitTarget::Notch => self.handle_notch_click(),
            HitTarget::NotchSecondary => self.handle_secondary_notch_click(),
            _ => NotchUpdate::default(),
        }
    }

    pub(super) fn handle_tab_click(&mut self, target: HitTarget) -> NotchUpdate {
        let tab = match target {
            HitTarget::NotchTabPomodoro => NotchExpandedTab::Pomodoro,
            HitTarget::NotchTabShelf => NotchExpandedTab::Shelf,
            HitTarget::NotchTabClipboard => NotchExpandedTab::Clipboard,
            HitTarget::NotchTabTasks => NotchExpandedTab::Tasks,
            HitTarget::NotchTabMedia => NotchExpandedTab::Media,
            _ => return NotchUpdate::default(),
        };
        NotchUpdate {
            changed: self.switch_tab(tab),
            effect: None,
        }
    }

    pub(crate) fn handle_wheel(&mut self, target: HitTarget, delta: i16) -> NotchUpdate {
        if self.panel_expansion <= 0.55
            || !matches!(
                target,
                HitTarget::Notch
                    | HitTarget::FocusMode(_)
                    | HitTarget::PomodoroPreset(_)
                    | HitTarget::PomodoroPlayPause
                    | HitTarget::PomodoroReset
                    | HitTarget::PomodoroSkip
                    | HitTarget::NotchTabShelf
                    | HitTarget::NotchTabClipboard
                    | HitTarget::NotchTabTasks
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
                    | HitTarget::PrivacyMicMute
            )
        {
            return NotchUpdate::default();
        }
        let steps = i32::from(delta / 120).clamp(-10, 10);
        let step = if steps == 0 {
            if delta > 0 { 1 } else { -1 }
        } else {
            steps
        };
        let changed = match self.notch_tab {
            NotchExpandedTab::Pomodoro => match self.focus_mode {
                FocusSubMode::Pomodoro => self.pomodoro.adjust_minutes(step),
                FocusSubMode::Timer => self.quick_timer.adjust_minutes(step),
                FocusSubMode::Stopwatch => false,
            },
            NotchExpandedTab::Shelf => self.shelf.step_page(-step),
            NotchExpandedTab::Clipboard => self.clipboard.step_page((-step) as isize),
            NotchExpandedTab::Tasks => self.tasks.step_page((-step) as isize),
            NotchExpandedTab::Media => false,
        };
        NotchUpdate {
            changed,
            effect: None,
        }
    }
}
