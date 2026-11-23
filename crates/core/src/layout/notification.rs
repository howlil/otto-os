use crate::notch::notch_notification::NotificationStage;

use super::geometry::Rect;
use super::geometry::scale;
use super::hit::HitTarget;

pub const NOTIFICATION_BANNER_HEIGHT_POINTS: i32 = 92;
pub const NOTIFICATION_RING_HEIGHT_POINTS: i32 = 96;
pub const NOTIFICATION_STACK_LIP_HEIGHT_POINTS: i32 = 4;
pub const NOTIFICATION_STACK_EXTRA_PANEL_POINTS: i32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationLayoutInput {
    pub stage: NotificationStage,
    pub empty_body: bool,
    pub has_queue: bool,
}

impl NotificationLayoutInput {
    #[must_use]
    pub const fn peek() -> Self {
        Self {
            stage: NotificationStage::Peek,
            empty_body: false,
            has_queue: false,
        }
    }

    #[must_use]
    pub const fn banner(empty_body: bool, has_queue: bool) -> Self {
        Self {
            stage: NotificationStage::Banner,
            empty_body,
            has_queue,
        }
    }

    #[must_use]
    pub const fn ringing() -> Self {
        Self {
            stage: NotificationStage::Ringing,
            empty_body: false,
            has_queue: false,
        }
    }

    #[must_use]
    pub const fn is_ringing(self) -> bool {
        matches!(self.stage, NotificationStage::Ringing)
    }

    #[must_use]
    pub const fn is_panel_card(self) -> bool {
        matches!(
            self.stage,
            NotificationStage::Banner | NotificationStage::Ringing
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationLayout {
    pub ringing: bool,
    pub card: Rect,
    pub stack_lip: Rect,
    pub badge: Rect,
    pub meta: Rect,
    pub trailing_meta: Rect,
    pub title: Rect,
    pub subtitle: Rect,
    pub close: Rect,
    pub accept: Rect,
    pub decline: Rect,
}

impl NotificationLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> HitTarget {
        if self.ringing && self.accept.contains(x, y) {
            HitTarget::NotificationAccept
        } else if self.ringing && self.decline.contains(x, y) {
            HitTarget::NotificationDecline
        } else if !self.ringing && self.close.contains(x, y) {
            HitTarget::NotificationClose
        } else {
            HitTarget::NotificationCard
        }
    }

    #[must_use]
    pub fn control_rect(&self, target: HitTarget) -> Option<Rect> {
        let rect = match target {
            HitTarget::NotificationCard => self.card,
            HitTarget::NotificationClose if !self.ringing => self.close,
            HitTarget::NotificationAccept if self.ringing => self.accept,
            HitTarget::NotificationDecline if self.ringing => self.decline,
            _ => return None,
        };
        (!rect.is_empty()).then_some(rect)
    }

    #[must_use]
    pub fn keyboard_targets(&self) -> Vec<HitTarget> {
        if self.ringing {
            vec![
                HitTarget::NotificationDecline,
                HitTarget::NotificationAccept,
            ]
        } else {
            vec![HitTarget::NotificationCard, HitTarget::NotificationClose]
        }
    }
}

pub(super) fn compute_notification_layout(
    notch: Rect,
    dpi: u32,
    input: NotificationLayoutInput,
) -> NotificationLayout {
    let pts = |value: i32| scale(value, dpi);
    let ringing = input.is_ringing();
    let width = pts(380);
    let height = pts(if ringing {
        NOTIFICATION_RING_HEIGHT_POINTS
    } else {
        NOTIFICATION_BANNER_HEIGHT_POINTS
    });
    let origin_x = notch.x;
    let origin_y = notch.y;
    let at = |x: i32, y: i32, w: i32, h: i32| Rect {
        x: origin_x + pts(x),
        y: origin_y + pts(y),
        width: pts(w),
        height: pts(h),
    };
    let card = Rect {
        x: origin_x,
        y: origin_y,
        width,
        height,
    };
    if ringing {
        return NotificationLayout {
            ringing: true,
            card,
            stack_lip: Rect::default(),
            badge: at(16, 24, 48, 48),
            meta: at(76, 28, 176, 16),
            trailing_meta: Rect::default(),
            title: at(76, 46, 176, 22),
            subtitle: Rect::default(),
            close: Rect::default(),
            decline: at(264, 26, 44, 44),
            accept: at(320, 26, 44, 44),
        };
    }
    let stack_lip = if input.has_queue {
        at(
            12,
            NOTIFICATION_BANNER_HEIGHT_POINTS,
            356,
            NOTIFICATION_STACK_LIP_HEIGHT_POINTS,
        )
    } else {
        Rect::default()
    };
    if input.empty_body {
        return NotificationLayout {
            ringing: false,
            card,
            stack_lip,
            badge: at(16, 24, 44, 44),
            meta: at(72, 26, 232, 16),
            trailing_meta: at(308, 26, 56, 16),
            title: at(72, 46, 292, 18),
            subtitle: Rect::default(),
            close: at(340, 22, 24, 24),
            accept: Rect::default(),
            decline: Rect::default(),
        };
    }
    NotificationLayout {
        ringing: false,
        card,
        stack_lip,
        badge: at(16, 24, 44, 44),
        meta: at(72, 12, 232, 16),
        trailing_meta: at(308, 12, 56, 16),
        title: at(72, 32, 292, 18),
        subtitle: at(72, 50, 292, 32),
        close: at(340, 8, 24, 24),
        accept: Rect::default(),
        decline: Rect::default(),
    }
}
