use std::collections::VecDeque;

use crate::menu::{NotificationBadge, NotificationSummary, notification_badge};

pub const NOTIFICATION_PEEK_MS: u64 = 1_200;
pub const NOTIFICATION_BASE_HOLD_MS: u64 = 4_500;
pub const NOTIFICATION_HOLD_PER_CHAR_MS: u64 = 40;
pub const NOTIFICATION_MAX_HOLD_MS: u64 = 9_000;
pub const NOTIFICATION_HOVER_GRACE_MS: u64 = 1_500;
pub const NOTIFICATION_COALESCE_MS: u64 = 3_000;
pub const CALL_RING_TIMEOUT_MS: u64 = 60_000;
pub const NOTIFICATION_QUEUE_LIMIT: usize = 4;
const HOLD_FREE_CHARS: usize = 60;
const REDACTED_BODY: &str = "New notification";
const MISSED_CALL_LABEL: &str = "Missed Call";

const CALL_APPS: [&str; 8] = [
    "teams",
    "zoom",
    "skype",
    "whatsapp",
    "discord",
    "telegram",
    "yourphone",
    "phone link",
];

const CALL_PHRASES: [&str; 9] = [
    "incoming call",
    "incoming video call",
    "incoming voice call",
    "is calling",
    "calling you",
    "panggilan masuk",
    "panggilan video masuk",
    "memanggil anda",
    "ringing",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Message,
    Call,
    VideoCall,
}

impl NotificationKind {
    #[must_use]
    pub const fn is_call(self) -> bool {
        matches!(self, Self::Call | Self::VideoCall)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationStage {
    Peek,
    Banner,
    Ringing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotchNotification {
    pub id: u32,
    pub app_id: String,
    pub app_name: String,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    pub badge: NotificationBadge,
    pub count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationPresentation {
    pub app_name: String,
    pub headline: String,
    pub subtitle: String,
    pub badge: NotificationBadge,
    pub count: u16,
    pub accept_label: Option<&'static str>,
    pub decline_label: Option<&'static str>,
}

#[must_use]
pub fn classify_notification(app_id: &str, title: &str, body: &str) -> NotificationKind {
    let app = app_id.to_lowercase();
    if !CALL_APPS.iter().any(|known| app.contains(known)) {
        return NotificationKind::Message;
    }
    let text = format!("{title} {body}").to_lowercase();
    if !CALL_PHRASES.iter().any(|phrase| text.contains(phrase)) {
        return NotificationKind::Message;
    }
    if text.contains("video") {
        NotificationKind::VideoCall
    } else {
        NotificationKind::Call
    }
}

#[must_use]
pub fn notification_hold_ms(body: &str) -> u64 {
    let extra = body.chars().count().saturating_sub(HOLD_FREE_CHARS) as u64;
    (NOTIFICATION_BASE_HOLD_MS + extra * NOTIFICATION_HOLD_PER_CHAR_MS)
        .min(NOTIFICATION_MAX_HOLD_MS)
}

#[must_use]
pub fn present_notification(
    notification: &NotchNotification,
    stage: NotificationStage,
) -> NotificationPresentation {
    let ringing = stage == NotificationStage::Ringing;
    let subtitle = match notification.kind {
        NotificationKind::Call if ringing => String::from("Incoming Call"),
        NotificationKind::VideoCall if ringing => String::from("Incoming Video Call"),
        _ => notification.body.clone(),
    };
    NotificationPresentation {
        app_name: notification.app_name.clone(),
        headline: notification.title.clone(),
        subtitle,
        badge: notification.badge,
        count: notification.count,
        accept_label: ringing.then_some("Open Call Window"),
        decline_label: ringing.then_some("Dismiss Call Alert"),
    }
}

#[derive(Debug, Clone)]
struct Active {
    notification: NotchNotification,
    started_ms: u64,
    deadline_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationQueue {
    primed: bool,
    seen: Vec<u32>,
    current: Option<Active>,
    pending: VecDeque<NotchNotification>,
}

impl NotificationQueue {
    pub fn ingest(
        &mut self,
        items: &[NotificationSummary],
        now_ms: u64,
        show_previews: bool,
    ) -> bool {
        let live: Vec<u32> = items.iter().map(|item| item.id).collect();
        let mut changed = self.drop_vanished(&live);
        if !self.primed {
            self.primed = true;
            self.seen = live;
            return changed;
        }
        let arrivals: Vec<&NotificationSummary> = items
            .iter()
            .filter(|item| !self.seen.contains(&item.id))
            .collect();
        self.seen = live;
        for item in arrivals.into_iter().rev() {
            self.arrive(build_notification(item, show_previews), now_ms);
            changed = true;
        }
        changed
    }

    pub fn tick(&mut self, now_ms: u64, hovered: bool) -> bool {
        let Some(active) = self.current.as_mut() else {
            return self.promote(now_ms);
        };
        if active.notification.kind.is_call() {
            if now_ms < active.started_ms + CALL_RING_TIMEOUT_MS {
                return false;
            }
            let missed = missed_call(&active.notification);
            self.current = None;
            self.pending.push_front(missed);
            self.promote(now_ms);
            return true;
        }
        if hovered {
            active.deadline_ms = active.deadline_ms.max(now_ms + NOTIFICATION_HOVER_GRACE_MS);
            return false;
        }
        if now_ms >= active.deadline_ms {
            self.current = None;
            self.promote(now_ms);
            return true;
        }
        false
    }

    pub fn dismiss_current(&mut self, now_ms: u64) -> Option<u32> {
        let id = self.current.take().map(|active| active.notification.id);
        self.promote(now_ms);
        id
    }

    #[must_use]
    pub fn current(&self, now_ms: u64) -> Option<(&NotchNotification, NotificationStage)> {
        let active = self.current.as_ref()?;
        let stage = if active.notification.kind.is_call() {
            NotificationStage::Ringing
        } else if now_ms < active.started_ms + NOTIFICATION_PEEK_MS {
            NotificationStage::Peek
        } else {
            NotificationStage::Banner
        };
        Some((&active.notification, stage))
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub fn next_wake_ms(&self, now_ms: u64) -> Option<u64> {
        let active = self.current.as_ref()?;
        let peek_end = active.started_ms + NOTIFICATION_PEEK_MS;
        Some(if active.notification.kind.is_call() {
            active.started_ms + CALL_RING_TIMEOUT_MS
        } else if now_ms < peek_end {
            peek_end
        } else {
            active.deadline_ms
        })
    }

    fn drop_vanished(&mut self, live: &[u32]) -> bool {
        let before = self.pending.len();
        self.pending
            .retain(|notification| live.contains(&notification.id));
        let mut changed = before != self.pending.len();
        if let Some(active) = &self.current
            && active.notification.kind.is_call()
            && !live.contains(&active.notification.id)
        {
            self.current = None;
            changed = true;
        }
        changed
    }

    fn arrive(&mut self, notification: NotchNotification, now_ms: u64) {
        if let Some(active) = self.current.as_mut() {
            let same_thread = !notification.kind.is_call()
                && !active.notification.kind.is_call()
                && active.notification.app_id == notification.app_id
                && now_ms.saturating_sub(active.started_ms) < NOTIFICATION_COALESCE_MS;
            if same_thread {
                let count = active.notification.count + 1;
                active.deadline_ms = now_ms + notification_hold_ms(&notification.body);
                active.notification = NotchNotification {
                    count,
                    ..notification
                };
                return;
            }
            if notification.kind.is_call() && !active.notification.kind.is_call() {
                let displaced = self.current.take().map(|old| old.notification);
                if let Some(old) = displaced {
                    self.pending.push_front(old);
                }
                self.start(notification, now_ms);
                return;
            }
        }
        if self.current.is_none() {
            self.start(notification, now_ms);
            return;
        }
        if self.pending.len() < NOTIFICATION_QUEUE_LIMIT {
            self.pending.push_back(notification);
        }
    }

    fn promote(&mut self, now_ms: u64) -> bool {
        if self.current.is_some() {
            return false;
        }
        let next = self
            .pending
            .iter()
            .position(|notification| notification.kind.is_call())
            .and_then(|index| self.pending.remove(index))
            .or_else(|| self.pending.pop_front());
        match next {
            Some(notification) => {
                self.start(notification, now_ms);
                true
            }
            None => false,
        }
    }

    fn start(&mut self, notification: NotchNotification, now_ms: u64) {
        let deadline_ms = now_ms + notification_hold_ms(&notification.body);
        self.current = Some(Active {
            notification,
            started_ms: now_ms,
            deadline_ms,
        });
    }
}

fn build_notification(item: &NotificationSummary, show_previews: bool) -> NotchNotification {
    let kind = classify_notification(&item.app_id, &item.title, &item.body);
    let (title, body) = if show_previews || kind.is_call() {
        (item.title.clone(), item.body.clone())
    } else {
        (item.app_name.clone(), String::from(REDACTED_BODY))
    };
    NotchNotification {
        id: item.id,
        app_id: item.app_id.clone(),
        app_name: item.app_name.clone(),
        kind,
        title,
        body,
        badge: notification_badge(&item.app_name),
        count: 1,
    }
}

fn missed_call(call: &NotchNotification) -> NotchNotification {
    NotchNotification {
        kind: NotificationKind::Message,
        body: String::from(MISSED_CALL_LABEL),
        ..call.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CALL_RING_TIMEOUT_MS, NOTIFICATION_BASE_HOLD_MS, NOTIFICATION_HOVER_GRACE_MS,
        NOTIFICATION_MAX_HOLD_MS, NOTIFICATION_PEEK_MS, NOTIFICATION_QUEUE_LIMIT, NotificationKind,
        NotificationQueue, NotificationStage, classify_notification, notification_hold_ms,
        present_notification,
    };
    use crate::menu::NotificationSummary;

    fn item(id: u32, app: &str, title: &str, body: &str) -> NotificationSummary {
        NotificationSummary {
            id,
            app_id: format!("{app}.App"),
            app_name: String::from(app),
            title: String::from(title),
            body: String::from(body),
            timestamp_label: String::from("now"),
        }
    }

    fn primed() -> NotificationQueue {
        let mut queue = NotificationQueue::default();
        queue.ingest(&[], 0, true);
        queue
    }

    #[test]
    fn calls_need_a_call_app_and_a_call_phrase() {
        assert_eq!(
            classify_notification("MSTeams.App", "Ayu", "Incoming video call"),
            NotificationKind::VideoCall
        );
        assert_eq!(
            classify_notification("WhatsApp.App", "Budi", "Panggilan masuk"),
            NotificationKind::Call
        );
        assert_eq!(
            classify_notification("Zoom.App", "Ayu", "is calling you"),
            NotificationKind::Call
        );
        assert_eq!(
            classify_notification("MSTeams.App", "Ayu", "see you at the video call tomorrow"),
            NotificationKind::Message
        );
        assert_eq!(
            classify_notification("Mail.App", "Boss", "Incoming call from the office"),
            NotificationKind::Message
        );
    }

    #[test]
    fn hold_grows_with_long_bodies_and_is_capped() {
        assert_eq!(notification_hold_ms("short"), NOTIFICATION_BASE_HOLD_MS);
        assert!(notification_hold_ms(&"x".repeat(100)) > NOTIFICATION_BASE_HOLD_MS);
        assert_eq!(
            notification_hold_ms(&"x".repeat(5_000)),
            NOTIFICATION_MAX_HOLD_MS
        );
    }

    #[test]
    fn existing_notifications_at_startup_do_not_pop() {
        let mut queue = NotificationQueue::default();
        let old = [item(1, "Mail", "Old", "Already there")];
        assert!(!queue.ingest(&old, 100, true));
        assert!(queue.current(100).is_none());
        assert!(queue.ingest(
            &[old[0].clone(), item(2, "Mail", "New", "Fresh")],
            200,
            true
        ));
        assert_eq!(queue.current(200).map(|(n, _)| n.id), Some(2));
    }

    #[test]
    fn a_message_peeks_then_banners_then_leaves() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Mail", "Ayu", "Lunch?")], 1_000, true);
        assert_eq!(
            queue.current(1_000).map(|(_, stage)| stage),
            Some(NotificationStage::Peek)
        );
        let banner = 1_000 + NOTIFICATION_PEEK_MS;
        assert_eq!(
            queue.current(banner).map(|(_, stage)| stage),
            Some(NotificationStage::Banner)
        );
        assert!(!queue.tick(1_000 + NOTIFICATION_BASE_HOLD_MS - 1, false));
        assert!(queue.tick(1_000 + NOTIFICATION_BASE_HOLD_MS, false));
        assert!(queue.current(10_000).is_none());
    }

    #[test]
    fn hovering_keeps_the_banner_and_leaves_a_grace_period() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Mail", "Ayu", "Lunch?")], 0, true);
        let late = NOTIFICATION_BASE_HOLD_MS + 2_000;
        assert!(!queue.tick(late, true));
        assert!(queue.current(late).is_some());
        assert!(!queue.tick(late + NOTIFICATION_HOVER_GRACE_MS - 1, false));
        assert!(queue.tick(late + NOTIFICATION_HOVER_GRACE_MS, false));
    }

    #[test]
    fn a_burst_from_one_app_coalesces_into_a_count() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Chat", "Ayu", "one")], 0, true);
        queue.ingest(
            &[item(1, "Chat", "Ayu", "one"), item(2, "Chat", "Ayu", "two")],
            1_000,
            true,
        );
        let (shown, _) = queue.current(1_000).expect("current");
        assert_eq!((shown.id, shown.count, shown.body.as_str()), (2, 2, "two"));
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn other_apps_queue_in_order_up_to_the_limit() {
        let mut queue = primed();
        let mut items = Vec::new();
        for id in 1..=8 {
            items.push(item(id, &format!("App{id}"), "T", "B"));
            queue.ingest(&items, u64::from(id) * 10_000, true);
        }
        assert!(queue.pending_count() <= NOTIFICATION_QUEUE_LIMIT);
        let first = queue.current(0).map(|(n, _)| n.id).expect("current");
        assert_eq!(first, 1);
        assert_eq!(queue.dismiss_current(90_000), Some(1));
        assert_eq!(queue.current(90_000).map(|(n, _)| n.id), Some(2));
    }

    #[test]
    fn a_call_preempts_a_message_and_the_message_returns_after() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Mail", "Ayu", "Lunch?")], 0, true);
        let both = [
            item(1, "Mail", "Ayu", "Lunch?"),
            item(2, "MSTeams", "Budi", "Incoming video call"),
        ];
        queue.ingest(&both, 500, true);
        let (call, stage) = queue.current(500).expect("call");
        assert_eq!(
            (call.id, call.kind, stage),
            (2, NotificationKind::VideoCall, NotificationStage::Ringing)
        );
        assert!(!queue.tick(500 + CALL_RING_TIMEOUT_MS - 1, true));
        assert_eq!(queue.dismiss_current(2_000), Some(2));
        assert_eq!(queue.current(2_000).map(|(n, _)| n.id), Some(1));
    }

    #[test]
    fn a_ringing_call_ends_when_its_toast_disappears() {
        let mut queue = primed();
        let ring = item(1, "Zoom", "Ayu", "is calling you");
        queue.ingest(std::slice::from_ref(&ring), 0, true);
        assert!(queue.current(10).is_some());
        assert!(queue.ingest(&[], 5_000, true));
        assert!(queue.current(5_000).is_none());
    }

    #[test]
    fn an_unanswered_call_becomes_a_missed_call_banner() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Zoom", "Ayu", "is calling you")], 0, true);
        assert!(queue.tick(CALL_RING_TIMEOUT_MS, false));
        let (missed, stage) = queue.current(CALL_RING_TIMEOUT_MS).expect("missed");
        assert_eq!(missed.kind, NotificationKind::Message);
        assert_eq!(missed.body, "Missed Call");
        assert_eq!(stage, NotificationStage::Peek);
    }

    #[test]
    fn the_next_wake_is_the_banner_swap_then_the_deadline_or_the_ring_timeout() {
        let mut queue = primed();
        assert_eq!(queue.next_wake_ms(0), None);
        queue.ingest(&[item(1, "Mail", "Ayu", "Lunch?")], 0, true);
        assert_eq!(queue.next_wake_ms(0), Some(NOTIFICATION_PEEK_MS));
        assert_eq!(
            queue.next_wake_ms(NOTIFICATION_PEEK_MS),
            Some(NOTIFICATION_BASE_HOLD_MS)
        );
        queue.dismiss_current(10);
        queue.ingest(&[item(2, "Zoom", "Ayu", "is calling you")], 20, true);
        assert_eq!(queue.next_wake_ms(20), Some(20 + CALL_RING_TIMEOUT_MS));
    }

    #[test]
    fn previews_off_hides_message_text_but_never_the_caller() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Mail", "Ayu", "Secret plan")], 0, false);
        let (hidden, _) = queue.current(0).expect("message");
        assert_eq!(
            (hidden.title.as_str(), hidden.body.as_str()),
            ("Mail", "New notification")
        );
        queue.dismiss_current(100);
        queue.ingest(
            &[
                item(1, "Mail", "Ayu", "Secret plan"),
                item(2, "Zoom", "Ayu", "is calling you"),
            ],
            200,
            false,
        );
        let (call, _) = queue.current(200).expect("call");
        assert_eq!(call.title, "Ayu");
    }

    #[test]
    fn ringing_presentation_offers_open_and_dismiss_actions_only_while_ringing() {
        let mut queue = primed();
        queue.ingest(&[item(1, "Zoom", "Ayu", "Incoming video call")], 0, true);
        let (call, stage) = queue.current(0).expect("call");
        let ringing = present_notification(call, stage);
        assert_eq!(ringing.subtitle, "Incoming Video Call");
        assert!(ringing.accept_label.is_some() && ringing.decline_label.is_some());
        let banner = present_notification(call, NotificationStage::Banner);
        assert!(banner.accept_label.is_none());
    }
}
