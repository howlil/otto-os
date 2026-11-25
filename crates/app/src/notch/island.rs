use std::time::{Duration, Instant};

use otto_core::{
    ClockTime, CompactContent, CompactSlotKind, IslandInputs, MotionMetrics, NotchExpandedTab,
    NotchLayoutInput, NotificationLayoutInput, NotificationStage, NowPlaying, PomodoroStatus,
    PrivacyLayoutState, ShelfLayoutInput, TrackKey,
};

use super::NotchController;
use super::NotchUpdate;
use super::PanelTrigger;

impl NotchController {
    pub(crate) fn sync_compact_title_width(
        &mut self,
        now: Instant,
        now_unix_ms: i64,
        measure: impl FnOnce(&str) -> f32,
    ) -> bool {
        let mode = otto_core::resolve_island(&self.island_inputs(now));
        let compact_media = matches!(
            mode,
            otto_core::IslandMode::Compact(CompactContent::Media(_))
        );
        let holds_width = matches!(
            mode,
            otto_core::IslandMode::Expanded
                | otto_core::IslandMode::Alert
                | otto_core::IslandMode::Hud
                | otto_core::IslandMode::Notification(_)
        ) || (compact_media && self.media_peek.is_some());
        if holds_width {
            return false;
        }
        let lyric = if compact_media {
            self.display_media(now_unix_ms)
                .and_then(|media| self.compact_lyric_line(&media))
        } else {
            None
        };
        let target = match lyric {
            None => 0.0,
            Some(line) if line.trim().is_empty() => return false,
            Some(line) => match &self.compact_title_measured {
                Some((text, points)) if *text == line => *points,
                _ => {
                    let points = measure(&line);
                    self.compact_title_measured = Some((line, points));
                    points
                }
            },
        };
        let moved = (target - self.compact_title_target).abs() > f32::EPSILON;
        self.compact_title_target = target;
        moved
    }

    pub(super) fn paused_hold_track(&self, now: Instant) -> Option<TrackKey> {
        let media = self
            .media
            .as_ref()
            .filter(|media| !media.playing && !media.title.trim().is_empty())?;
        let hold = Duration::from_millis(MotionMetrics::default().media_pause_hold_ms);
        let paused_at = self.media_paused_at?;
        (now < paused_at + hold).then(|| media.track_key())
    }

    pub(crate) fn set_clock(&mut self, clock: ClockTime) {
        self.clock_time = clock;
    }

    pub(crate) fn set_presented(&mut self, presented: bool) {
        self.presented = presented;
    }

    pub(crate) fn collapse_immediately(&mut self) {
        self.hud_kind = None;
        self.hud_until = None;
        self.hud_expansion = 0.0;
        self.hud_level = 0.0;
        self.compact_expansion = 0.0;
        self.compact_velocity = 0.0;
        self.secondary_expansion = 0.0;
        self.secondary_velocity = 0.0;
        self.panel_expansion = 0.0;
        self.panel_velocity = 0.0;
        self.press_active = false;
        self.press_progress = 0.0;
        self.press_velocity = 0.0;
        self.hover_since = None;
        self.panel_trigger = PanelTrigger::Closed;
        self.opened_by_shelf_drag = false;
        self.opened_by_task_editor = false;
        self.tasks.cancel_editor();
        self.modal_picker_open = false;
        self.drag_out_active = false;
        self.shelf_drag_leave_until = None;
        self.shelf.drag_hover = false;
        self.seek_drag = None;
        self.category_picker_open = false;
        self.custom_time_input = None;
        self.hover_blocked = false;
        self.presented = false;
    }

    pub(super) fn has_compact_focus(&self) -> bool {
        self.focus_enabled && self.pomodoro.is_compact_active()
    }

    pub(super) fn has_alert(&self) -> bool {
        self.focus_enabled && self.pomodoro.status == PomodoroStatus::Alert
    }

    pub(super) fn dismiss_active_alert(&mut self) {
        self.pomodoro.dismiss_alert();
        self.alert_until = None;
    }

    pub(super) fn island_inputs(&self, now: Instant) -> IslandInputs {
        IslandInputs {
            alert: self.has_alert() && self.alert_until.is_none_or(|until| now < until),
            panel_requested: self.has_openable_panel()
                && (self.panel_trigger.is_open() || self.is_panel_held_open()),
            hud: self.hud_enabled && self.hud_until.is_some_and(|until| now < until),
            notification: self.notification_stage(now),
            focus_active: self.has_compact_focus(),
            call_active: self.call.is_active(self.millis(now)),
            media: self
                .media
                .as_ref()
                .filter(|media| self.media_enabled && media.is_active())
                .map(NowPlaying::track_key),
            media_paused_hold: if self.media_enabled {
                self.paused_hold_track(now)
            } else {
                None
            },
            media_primary: self.media_primary,
            dismissed: self.dismissed.clone(),
            dismissed_secondary: self.dismissed_secondary.clone(),
        }
    }

    #[cfg(test)]
    pub(super) fn island_mode(&self, now: Instant) -> otto_core::IslandMode {
        otto_core::resolve_island(&self.island_inputs(now))
    }

    pub(super) fn record_dismissed(&mut self, content: CompactContent) {
        if self.dismissed.as_ref() == Some(&content)
            || self.dismissed_secondary.as_ref() == Some(&content)
        {
            return;
        }
        if self.dismissed.is_none() {
            self.dismissed = Some(content);
        } else {
            self.dismissed_secondary = Some(content);
        }
    }

    pub(super) fn clear_focus_dismissed(&mut self) {
        if self.dismissed == Some(CompactContent::Focus) {
            self.dismissed = self.dismissed_secondary.take();
        } else if self.dismissed_secondary == Some(CompactContent::Focus) {
            self.dismissed_secondary = None;
        }
    }

    pub(super) fn sync_tab_to_primary(&mut self, now: Instant) {
        let Some(primary) = self.island_inputs(now).compact_content() else {
            self.ensure_valid_tab();
            return;
        };
        let target = match primary.kind() {
            CompactSlotKind::Focus | CompactSlotKind::Call => NotchExpandedTab::Pomodoro,
            CompactSlotKind::Media => NotchExpandedTab::Media,
        };
        if self.is_tab_enabled(target) {
            self.notch_tab = target;
            self.previous_tab = target;
            self.tab_blend = 1.0;
        } else {
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn dismiss_compact(&mut self, now: Instant) -> NotchUpdate {
        if matches!(
            self.panel_trigger,
            PanelTrigger::Pinned | PanelTrigger::PinnedHovered
        ) {
            return NotchUpdate::default();
        }
        let inputs = self.island_inputs(now);
        let Some(content) = inputs.compact_content() else {
            return NotchUpdate::default();
        };
        let had_secondary = inputs.secondary_compact_content().is_some();
        self.record_dismissed(content);
        if had_secondary {
            self.compact_swap_blend = 0.0;
            self.sync_tab_to_primary(now);
        }
        self.panel_trigger = PanelTrigger::Closed;
        self.hover_since = None;
        self.hover_blocked = true;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn dismiss_secondary_compact(&mut self, now: Instant) -> NotchUpdate {
        let Some(content) = self.island_inputs(now).secondary_compact_content() else {
            return NotchUpdate::default();
        };
        self.record_dismissed(content);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn sync_clock(&mut self, now: Instant) -> NotchUpdate {
        let (mut changed, effect) = self.step_pomodoro_clock(now);
        if self.step_scene_at(now, self.motion) {
            changed = true;
        }
        let call_shown = self.call.is_active(self.millis(now));
        if call_shown != self.call_shown {
            self.call_shown = call_shown;
            changed = true;
        }
        NotchUpdate {
            changed: changed
                || call_shown
                || self.shows_live_media_progress()
                || self.shows_live_lyrics()
                || self.scene_frame_active(),
            effect,
        }
    }

    pub(crate) fn layout_input(&self, control_center_enabled: bool) -> NotchLayoutInput {
        let inputs = self.island_inputs(self.last_anim_instant);
        let compact_primary = inputs
            .compact_content()
            .map(|content| content.kind())
            .or_else(|| {
                (self.compact_expansion > 0.0)
                    .then_some(self.last_primary_slot)
                    .flatten()
            });
        let compact_secondary = inputs
            .secondary_compact_content()
            .map(|content| content.kind())
            .or_else(|| {
                (self.secondary_expansion > 0.0)
                    .then_some(self.last_secondary_slot)
                    .flatten()
            });
        NotchLayoutInput {
            hud_expansion: self.hud_expansion,
            compact_expansion: self.compact_expansion.clamp(0.0, 1.0),
            secondary_expansion: self.secondary_expansion.clamp(0.0, 1.0),
            compact_swap_blend: self.compact_swap_blend,
            panel_expansion: self.panel_expansion.clamp(0.0, 1.0),
            press_progress: self.press_progress.clamp(0.0, 1.0),
            compact_primary,
            compact_secondary,
            pomodoro_surface: self.active_surface(),
            pomodoro_category_picker: self.category_picker_open,
            pomodoro_category_count: self
                .focus_categories
                .categories
                .len()
                .min(otto_core::NOTCH_CATEGORY_OPTIONS) as u8,
            heatmap_filter_count: (self.focus_categories.categories.len() + 1)
                .min(otto_core::NOTCH_HEATMAP_FILTERS) as u8,
            active_tab: self.notch_tab,
            previous_tab: self.previous_tab,
            top_bar_enabled: true,
            notch_enabled: true,
            control_center_enabled,
            focus_enabled: self.focus_enabled,
            shelf_enabled: self.shelf_enabled,
            clipboard_enabled: self.clipboard_enabled,
            tasks_enabled: self.tasks_enabled,
            media_present: self.media_enabled && self.media.is_some(),
            media_live: self
                .media
                .as_ref()
                .is_some_and(|media| self.media_enabled && !media.has_known_duration()),
            lyrics_progress: self.lyrics_progress,
            stats_progress: self.stats_progress,
            stats_range: self.stats_range,
            tab_blend: self.tab_blend,
            compact_title_points: self.compact_title_points.round() as i32,
            privacy: if self.privacy_enabled {
                PrivacyLayoutState {
                    compact_dots: self.privacy.show_in_bar(),
                    panel_mic_control: self.privacy.show_panel_control(),
                }
            } else {
                PrivacyLayoutState {
                    compact_dots: false,
                    panel_mic_control: false,
                }
            },
            shelf: ShelfLayoutInput {
                visible_count: u8::try_from(self.shelf.visible_items().len()).unwrap_or(0),
                has_pager: self.shelf.page_count() > 1,
                has_items: !self.shelf.items.is_empty(),
            },
            clipboard: self.clipboard.layout_input(),
            hidden_items: otto_core::BarItemSet::default(),
            notification: self.notification_stage(self.last_anim_instant).map(
                |stage| match stage {
                    NotificationStage::Peek => NotificationLayoutInput::peek(),
                    NotificationStage::Ringing => NotificationLayoutInput::ringing(),
                    NotificationStage::Banner => {
                        let empty_body = self
                            .notifications
                            .current(self.millis(self.last_anim_instant))
                            .is_some_and(|(n, _)| n.body.trim().is_empty());
                        NotificationLayoutInput::banner(
                            empty_body,
                            self.notifications.pending_count() > 0,
                        )
                    }
                },
            ),
            tasks: self.tasks.layout_input(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_notch_pressed(&mut self, pressed: bool) -> bool {
        if self.press_active == pressed {
            return false;
        }
        self.press_active = pressed;
        true
    }

    pub(crate) fn collapse_from_gesture(&mut self) -> NotchUpdate {
        if self.collapse_notification_gesture(self.last_anim_instant) {
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        if self.is_panel_held_open() || !self.panel_trigger.is_open() {
            return NotchUpdate::default();
        }
        self.panel_trigger = PanelTrigger::Closed;
        self.hover_since = None;
        self.hover_blocked = true;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn collapse_from_external_focus_change(&mut self) -> bool {
        let notif_collapsed = self.collapse_notifications_all();
        if self.is_panel_held_open() || self.has_alert() {
            return notif_collapsed;
        }
        if self.panel_trigger != PanelTrigger::Closed {
            self.panel_trigger = PanelTrigger::Closed;
            self.hover_since = None;
            self.hover_blocked = false;
            return true;
        }
        notif_collapsed
    }

    pub(super) fn handle_notch_click(&mut self) -> NotchUpdate {
        if self.has_alert() {
            self.dismiss_active_alert();
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        if self.notification_stage(self.last_anim_instant) == Some(NotificationStage::Peek) {
            return self.handle_notification_click(otto_core::HitTarget::NotificationCard);
        }
        if self.panel_expansion < 0.25 {
            if !self.has_openable_panel() {
                return NotchUpdate::default();
            }
            self.panel_trigger = PanelTrigger::Pinned;
            self.hover_blocked = false;
            self.hover_since = None;
            if matches!(
                self.notch_tab,
                NotchExpandedTab::Pomodoro | NotchExpandedTab::Media
            ) {
                self.sync_tab_to_primary(Instant::now());
            }
            self.ensure_valid_tab();
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        self.collapse_from_gesture()
    }

    pub(super) fn handle_secondary_notch_click(&mut self) -> NotchUpdate {
        let now = Instant::now();
        if self
            .island_inputs(now)
            .secondary_compact_content()
            .is_none()
        {
            return NotchUpdate::default();
        }
        self.media_primary = !self.media_primary;
        self.compact_swap_blend = 0.0;
        self.sync_tab_to_primary(now);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }
}
