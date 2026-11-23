use std::time::Instant;

use otto_core::{
    CallScene, HitTarget, NotchAction, NotificationPresentation, NotificationStage,
    NotificationSummary,
};

use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;

impl NotchController {
    pub(super) fn notification_stage(&self, now: Instant) -> Option<NotificationStage> {
        let (notification, stage) = self.notifications.current(self.millis(now))?;
        let focus_active = self.is_focus_blocking_active() || self.has_compact_focus();
        if focus_active {
            let blocked_app = self
                .focus_blocklist
                .is_app_blocked(&notification.app_id, &notification.app_name)
                || self
                    .focus_blocklist
                    .blocked_apps
                    .iter()
                    .any(|blocked| notification.app_id.to_ascii_lowercase().contains(blocked));
            if blocked_app {
                return None;
            }
            if notification.kind.is_call() {
                if self.is_focus_blocking_active() && !self.focus_blocklist.allow_calls_during_focus
                {
                    return None;
                }
            } else if self.focus_blocklist.block_notifications {
                return None;
            }
        } else if self.dnd_active && !notification.kind.is_call() {
            return None;
        }
        Some(stage)
    }

    pub(crate) fn set_dnd_active(&mut self, active: bool) -> bool {
        if self.dnd_active == active {
            return false;
        }
        self.dnd_active = active;
        true
    }

    pub(crate) const fn has_unread_notifications(&self) -> bool {
        self.notification_unread_count > 0
    }

    pub(crate) fn has_collapsible_notification(&self, now: Instant) -> bool {
        matches!(
            self.notification_stage(now),
            Some(NotificationStage::Peek | NotificationStage::Banner)
        )
    }

    pub(crate) fn sync_notifications(&mut self, now: Instant, hovered: bool) -> bool {
        self.notifications.tick(self.millis(now), hovered)
    }

    pub(crate) fn hold_notification_focus(&mut self, now: Instant) {
        let _ = self.notifications.tick(self.millis(now), true);
    }

    pub(crate) fn promote_notification_peek(&mut self, now: Instant) -> bool {
        if self.notification_stage(now) != Some(NotificationStage::Peek) {
            return false;
        }
        self.notifications.promote_peek(self.millis(now))
    }

    pub(crate) fn collapse_notification_gesture(&mut self, now: Instant) -> bool {
        if !self.has_collapsible_notification(now) {
            return false;
        }
        self.notifications.collapse_current(self.millis(now))
    }

    pub(crate) fn collapse_notifications_all(&mut self) -> bool {
        self.notifications.collapse_all()
    }

    pub(crate) fn handle_notification_escape(&mut self) -> NotchUpdate {
        let now = self.last_anim_instant;
        match self.notification_stage(now) {
            Some(NotificationStage::Ringing) => {
                self.handle_notification_click(HitTarget::NotificationDecline)
            }
            Some(NotificationStage::Peek | NotificationStage::Banner) => NotchUpdate {
                changed: self.notifications.collapse_all(),
                effect: None,
            },
            None => NotchUpdate::default(),
        }
    }

    pub(crate) fn ingest_notifications(
        &mut self,
        items: &[NotificationSummary],
        now: Instant,
    ) -> bool {
        let unread_changed = self.notification_unread_count != items.len();
        self.notification_unread_count = items.len();
        self.notifications.ingest(items, self.millis(now), true) || unread_changed
    }

    pub(crate) fn notification_presentation(
        &self,
        now: Instant,
    ) -> Option<NotificationPresentation> {
        let stage = self.notification_stage(now)?;
        let (notification, _) = self.notifications.current(self.millis(now))?;
        Some(otto_core::present_notification(
            notification,
            stage,
            self.notifications.pending_count(),
        ))
    }

    pub(crate) fn notification_coalesce_progress(&self, now: Instant) -> f32 {
        self.notifications.coalesce_progress(self.millis(now))
    }

    pub(super) fn handle_notification_click(&mut self, target: HitTarget) -> NotchUpdate {
        let open = matches!(
            target,
            HitTarget::NotificationCard | HitTarget::NotificationAccept
        );
        let now_ms = self.millis(self.last_anim_instant);
        let Some(id) = self.notifications.dismiss_current(now_ms) else {
            return NotchUpdate::default();
        };
        let action = if open {
            NotchAction::NotificationOpen(id)
        } else {
            NotchAction::NotificationDismiss(id)
        };
        NotchUpdate {
            changed: true,
            effect: Some(NotchEffect::Platform(action)),
        }
    }

    pub(super) fn millis(&self, moment: Instant) -> u64 {
        u64::try_from(moment.saturating_duration_since(self.epoch).as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn call_scene(&self, now: Instant) -> Option<CallScene> {
        self.call
            .scene(self.privacy.mic.is_muted(), self.millis(now))
    }
}
