use super::commands::CMD_NOTIF_DISMISS_BASE;
use super::commands::NOTIFICATION_BADGE_COLORS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationBadge {
    pub initial: char,
    pub color: [u8; 3],
}

#[must_use]
pub fn notification_badge(app_name: &str) -> NotificationBadge {
    let initial = app_name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map_or('?', |c| c.to_uppercase().next().unwrap_or(c));
    let hash = app_name.bytes().fold(0x811c_9dc5_u32, |acc, byte| {
        (acc ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    NotificationBadge {
        initial,
        color: NOTIFICATION_BADGE_COLORS[hash as usize % NOTIFICATION_BADGE_COLORS.len()],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSummary {
    pub id: u32,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationAccess {
    Allowed,
    Denied,
    #[default]
    RequiresPackageIdentity,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NotificationCenterSnapshot {
    pub access: NotificationAccess,
    pub items: Vec<NotificationSummary>,
}

#[must_use]
pub const fn notification_dismiss_index(cmd: usize, len: usize) -> Option<usize> {
    if cmd < CMD_NOTIF_DISMISS_BASE {
        return None;
    }
    let idx = cmd - CMD_NOTIF_DISMISS_BASE;
    if idx < len { Some(idx) } else { None }
}

#[must_use]
pub fn format_relative_age(elapsed_secs: u64) -> String {
    if elapsed_secs < 60 {
        String::from("now")
    } else if elapsed_secs < 3600 {
        format!("{}m", elapsed_secs / 60)
    } else if elapsed_secs < 86_400 {
        format!("{}h", elapsed_secs / 3600)
    } else {
        format!("{}d", elapsed_secs / 86_400)
    }
}
