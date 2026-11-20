use super::geometry::Rect;
use super::geometry::scale;
use super::hit::HitTarget;

pub const NOTIFICATION_BANNER_HEIGHT_POINTS: i32 = 88;
pub const NOTIFICATION_RING_HEIGHT_POINTS: i32 = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationLayoutInput {
    pub ringing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationLayout {
    pub ringing: bool,
    pub card: Rect,
    pub badge: Rect,
    pub meta: Rect,
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
    ringing: bool,
) -> NotificationLayout {
    let pts = |value: i32| scale(value, dpi);
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
            ringing,
            card,
            badge: at(16, 28, 40, 40),
            meta: Rect::default(),
            title: at(68, 24, 188, 20),
            subtitle: at(68, 46, 188, 16),
            close: Rect::default(),
            accept: at(320, 26, 44, 44),
            decline: at(268, 26, 44, 44),
        };
    }
    NotificationLayout {
        ringing,
        card,
        badge: at(16, 26, 36, 36),
        meta: at(64, 12, 248, 14),
        title: at(64, 28, 300, 18),
        subtitle: at(64, 48, 300, 32),
        close: at(340, 10, 24, 24),
        accept: Rect::default(),
        decline: Rect::default(),
    }
}
