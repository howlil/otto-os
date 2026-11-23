use std::time::Instant;

use otto_core::{
    CallTracker, CompactSlotKind, LyricsStatus, NotchExpandedTab, NotificationQueue,
    PomodoroStatus, PrivacyState, TaskAddOutcome, TaskEditOutcome, TaskEditorMode, TasksState,
};

use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;
use super::PanelTrigger;
use super::TaskSubmitResult;

impl NotchController {
    pub(crate) fn set_lyrics_in_compact(&mut self, enabled: bool) {
        self.lyrics_in_compact = enabled;
    }

    pub(crate) const fn shelf_enabled(&self) -> bool {
        self.shelf_enabled
    }

    pub(crate) const fn clipboard_enabled(&self) -> bool {
        self.clipboard_enabled
    }

    pub(crate) const fn tasks_enabled(&self) -> bool {
        self.tasks_enabled
    }

    pub(crate) const fn tasks_state(&self) -> &TasksState {
        &self.tasks
    }

    pub(crate) const fn is_task_editor_open(&self) -> bool {
        self.tasks_enabled && self.tasks.is_editing()
    }

    pub(super) const fn is_panel_held_open(&self) -> bool {
        (self.shelf_enabled && self.is_shelf_hold_open()) || self.is_task_editor_open()
    }

    pub(crate) fn replace_tasks(&mut self, tasks: TasksState) {
        self.tasks = tasks;
        self.opened_by_task_editor = false;
    }

    #[cfg(test)]
    pub(crate) fn add_task(&mut self, text: &str) -> bool {
        matches!(self.tasks.add(text), TaskAddOutcome::Added(_))
    }

    pub(crate) fn open_quick_capture_add(&mut self) -> NotchUpdate {
        if !self.tasks_enabled {
            return NotchUpdate::default();
        }
        if !self.panel_trigger.is_open() && self.panel_expansion < 0.55 {
            self.opened_by_task_editor = true;
        }
        self.dismiss_active_alert();
        self.panel_trigger = PanelTrigger::Pinned;
        self.hover_blocked = false;
        self.hover_since = None;
        let _ = self.switch_tab(NotchExpandedTab::Tasks);
        if self.panel_expansion < 0.95 {
            self.panel_expansion = 1.0;
            self.panel_velocity = 0.0;
        }
        self.previous_tab = NotchExpandedTab::Tasks;
        self.tab_blend = 1.0;
        let _ = self.tasks.begin_add();
        NotchUpdate {
            changed: true,
            effect: Some(NotchEffect::OpenTaskEditor {
                initial_text: String::new(),
                allow_keep_open: true,
            }),
        }
    }

    pub(crate) fn submit_task_editor(&mut self, text: &str, keep_open: bool) -> TaskSubmitResult {
        match self.tasks.editor {
            Some(TaskEditorMode::Edit(id)) => {
                let outcome = self.tasks.edit(id, text);
                self.finish_task_editor();
                match outcome {
                    TaskEditOutcome::Updated => TaskSubmitResult::Saved,
                    TaskEditOutcome::Unchanged => TaskSubmitResult::Unchanged,
                    TaskEditOutcome::Rejected => TaskSubmitResult::Rejected,
                }
            }
            Some(TaskEditorMode::Add) | None => {
                let outcome = self.tasks.add(text);
                if !keep_open {
                    self.finish_task_editor();
                }
                match outcome {
                    TaskAddOutcome::Added(_) => TaskSubmitResult::Saved,
                    TaskAddOutcome::Rejected => TaskSubmitResult::Rejected,
                }
            }
        }
    }

    pub(crate) fn cancel_task_editor(&mut self) -> bool {
        let had_editor = self.tasks.cancel_editor();
        let closed_panel = self.close_editor_opened_panel();
        had_editor || closed_panel
    }

    pub(super) fn finish_task_editor(&mut self) {
        let _ = self.tasks.cancel_editor();
        let _ = self.close_editor_opened_panel();
    }

    fn close_editor_opened_panel(&mut self) -> bool {
        if std::mem::take(&mut self.opened_by_task_editor)
            && self.panel_trigger == PanelTrigger::Pinned
        {
            self.panel_trigger = PanelTrigger::Closed;
            self.hover_since = None;
            return true;
        }
        false
    }

    pub(crate) const fn media_enabled(&self) -> bool {
        self.media_enabled
    }

    pub(crate) const fn privacy_enabled(&self) -> bool {
        self.privacy_enabled
    }

    pub(crate) const fn hud_enabled(&self) -> bool {
        self.hud_enabled
    }

    pub(crate) fn is_tab_enabled(&self, tab: NotchExpandedTab) -> bool {
        match tab {
            NotchExpandedTab::Pomodoro | NotchExpandedTab::Stats => self.focus_enabled,
            NotchExpandedTab::Shelf => self.shelf_enabled,
            NotchExpandedTab::Clipboard => self.clipboard_enabled,
            NotchExpandedTab::Tasks => self.tasks_enabled,
            NotchExpandedTab::Media => self.media_enabled && self.media.is_some(),
        }
    }

    pub(crate) fn has_openable_panel(&self) -> bool {
        self.focus_enabled
            || self.shelf_enabled
            || self.clipboard_enabled
            || self.tasks_enabled
            || (self.media_enabled && self.media.is_some())
    }

    pub(crate) fn first_enabled_tab(&self) -> Option<NotchExpandedTab> {
        [
            NotchExpandedTab::Pomodoro,
            NotchExpandedTab::Stats,
            NotchExpandedTab::Shelf,
            NotchExpandedTab::Clipboard,
            NotchExpandedTab::Tasks,
            NotchExpandedTab::Media,
        ]
        .into_iter()
        .find(|&tab| self.is_tab_enabled(tab))
    }

    pub(super) fn ensure_valid_tab(&mut self) {
        if self.is_tab_enabled(self.notch_tab) {
            return;
        }
        if let Some(fallback) = self.first_enabled_tab() {
            self.notch_tab = fallback;
            self.previous_tab = fallback;
            self.tab_blend = 1.0;
        } else {
            self.panel_trigger = PanelTrigger::Closed;
            self.panel_expansion = 0.0;
            self.panel_velocity = 0.0;
            self.hover_since = None;
        }
    }

    pub(super) fn clear_compact_slot(&mut self, slot: CompactSlotKind) {
        if self.last_primary_slot == Some(slot) {
            self.last_primary_slot = None;
            self.compact_expansion = 0.0;
            self.compact_velocity = 0.0;
        }
        if self.last_secondary_slot == Some(slot) {
            self.last_secondary_slot = None;
            self.secondary_expansion = 0.0;
            self.secondary_velocity = 0.0;
        }
    }

    pub(crate) fn set_focus_enabled(&mut self, enabled: bool) {
        if self.focus_enabled == enabled {
            return;
        }
        self.focus_enabled = enabled;
        if !enabled {
            match self.pomodoro.status {
                PomodoroStatus::Running => self.pomodoro.toggle_run(),
                PomodoroStatus::Alert => self.pomodoro.dismiss_alert(),
                PomodoroStatus::Idle | PomodoroStatus::Paused => {}
            }
            match self.quick_timer.status {
                PomodoroStatus::Running => self.quick_timer.toggle_run(),
                PomodoroStatus::Alert => self.quick_timer.dismiss_alert(),
                PomodoroStatus::Idle | PomodoroStatus::Paused => {}
            }
            if self.stopwatch.running {
                self.stopwatch.toggle_run();
            }
            self.alert_until = None;
            self.peek = None;
            self.glow_presence = 0.0;
            self.dim_presence = 0.0;
            self.pulse_presence = 0.0;
            self.stats_progress = 0.0;
            self.stats_selected_day = None;
            self.clear_compact_slot(CompactSlotKind::Focus);
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_shelf_enabled(&mut self, enabled: bool) {
        if self.shelf_enabled == enabled {
            return;
        }
        self.shelf_enabled = enabled;
        if !enabled {
            self.opened_by_shelf_drag = false;
            self.modal_picker_open = false;
            self.drag_out_active = false;
            self.shelf_drag_leave_until = None;
            self.shelf.drag_hover = false;
            self.shelf.clear();
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_clipboard_enabled(&mut self, enabled: bool) {
        if self.clipboard_enabled == enabled {
            return;
        }
        self.clipboard_enabled = enabled;
        if !enabled {
            self.clipboard.clear();
            self.clipboard_copied_until = None;
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_tasks_enabled(&mut self, enabled: bool) {
        if self.tasks_enabled == enabled {
            return;
        }
        self.tasks_enabled = enabled;
        if !enabled {
            self.tasks.cancel_editor();
            self.opened_by_task_editor = false;
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_media_enabled(&mut self, enabled: bool) {
        if self.media_enabled == enabled {
            return;
        }
        self.media_enabled = enabled;
        if !enabled {
            self.media = None;
            self.lyrics = None;
            self.lyrics_status = LyricsStatus::Idle;
            self.lyrics_expanded = false;
            self.lyrics_progress = 0.0;
            self.pending_seek = None;
            self.seek_drag = None;
            self.media_paused_at = None;
            self.media_peek = None;
            self.media_dim_presence = 0.0;
            self.compact_title_measured = None;
            self.compact_title_target = 0.0;
            self.compact_title_points = 0.0;
            self.clear_compact_slot(CompactSlotKind::Media);
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn release_all_resources(&mut self) {
        self.collapse_immediately();
        match self.pomodoro.status {
            PomodoroStatus::Running => self.pomodoro.toggle_run(),
            PomodoroStatus::Alert => self.pomodoro.dismiss_alert(),
            PomodoroStatus::Idle | PomodoroStatus::Paused => {}
        }
        match self.quick_timer.status {
            PomodoroStatus::Running => self.quick_timer.toggle_run(),
            PomodoroStatus::Alert => self.quick_timer.dismiss_alert(),
            PomodoroStatus::Idle | PomodoroStatus::Paused => {}
        }
        if self.stopwatch.running {
            self.stopwatch.toggle_run();
        }
        self.alert_until = None;
        self.peek = None;
        self.glow_presence = 0.0;
        self.dim_presence = 0.0;
        self.pulse_presence = 0.0;
        self.media = None;
        self.lyrics = None;
        self.lyrics_status = LyricsStatus::Idle;
        self.lyrics_expanded = false;
        self.lyrics_progress = 0.0;
        self.stats_progress = 0.0;
        self.stats_selected_day = None;
        self.pending_seek = None;
        self.media_paused_at = None;
        self.media_peek = None;
        self.media_dim_presence = 0.0;
        self.compact_title_measured = None;
        self.compact_title_target = 0.0;
        self.compact_title_points = 0.0;
        self.shelf.clear();
        self.clipboard.clear();
        self.clipboard_copied_until = None;
        self.tasks = TasksState::default();
        self.privacy = PrivacyState::default();
        self.notifications = NotificationQueue::default();
        self.call = CallTracker::default();
        self.call_shown = false;
        self.dismissed = None;
        self.dismissed_secondary = None;
        self.last_primary_slot = None;
        self.last_secondary_slot = None;
    }

    pub(crate) fn set_privacy_enabled(&mut self, enabled: bool) {
        if self.privacy_enabled == enabled {
            return;
        }
        self.privacy_enabled = enabled;
        if !enabled {
            self.privacy = PrivacyState::default();
        }
    }

    pub(crate) fn set_hud_enabled(&mut self, enabled: bool) {
        if self.hud_enabled == enabled {
            return;
        }
        self.hud_enabled = enabled;
        if !enabled {
            self.hud_kind = None;
            self.hud_until = None;
            self.hud_expansion = 0.0;
            self.hud_level = 0.0;
        }
    }

    pub(crate) fn focus_active(&self) -> bool {
        self.has_compact_focus()
    }

    pub(crate) fn open_media_tab(&mut self) {
        if !self.media_enabled || self.media.is_none() {
            return;
        }
        self.dismiss_active_alert();
        self.panel_trigger = PanelTrigger::Pinned;
        self.hover_blocked = false;
        self.hover_since = None;
        let _ = self.switch_tab(NotchExpandedTab::Media);
    }

    pub(crate) fn active_tab(&self) -> NotchExpandedTab {
        self.notch_tab
    }

    pub(crate) fn is_panel_target_open(&self, now: Instant) -> bool {
        let alert_active = self.has_alert() && self.alert_until.is_none_or(|until| now < until);
        self.panel_trigger.is_open() || alert_active
    }

    pub(crate) fn switch_tab(&mut self, target: NotchExpandedTab) -> bool {
        if target == self.notch_tab || !self.is_tab_enabled(target) {
            return false;
        }
        if target != NotchExpandedTab::Tasks {
            self.tasks.cancel_editor();
            self.opened_by_task_editor = false;
        }
        if target == self.previous_tab && self.tab_blend < 1.0 {
            self.previous_tab = self.notch_tab;
            self.notch_tab = target;
            self.tab_blend = (1.0 - self.tab_blend).clamp(0.0, 1.0);
        } else {
            let dominant = if self.tab_blend >= 0.5 {
                self.notch_tab
            } else {
                self.previous_tab
            };
            self.previous_tab = dominant;
            self.notch_tab = target;
            self.tab_blend = 0.0;
        }
        true
    }
}
