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
        }
        Some(match stage {
            NotificationStage::Ringing => NotificationStage::Ringing,
            NotificationStage::Peek | NotificationStage::Banner => NotificationStage::Banner,
        })
    }

    pub(crate) fn sync_notifications(&mut self, now: Instant, hovered: bool) -> bool {
        self.notifications.tick(self.millis(now), hovered)
    }

    pub(crate) fn ingest_notifications(
        &mut self,
        items: &[NotificationSummary],
        now: Instant,
    ) -> bool {
        self.notifications.ingest(items, self.millis(now), true)
    }

    pub(crate) fn notification_presentation(
        &self,
        now: Instant,
    ) -> Option<NotificationPresentation> {
        let stage = self.notification_stage(now)?;
        let (notification, _) = self.notifications.current(self.millis(now))?;
        Some(otto_core::present_notification(notification, stage))
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
