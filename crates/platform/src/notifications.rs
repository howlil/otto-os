use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use otto_core::{
    NotificationAccess, NotificationCenterSnapshot, NotificationSummary, format_relative_age,
};
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;

const WINDOWS_TO_UNIX_EPOCH_SECS: i64 = 11_644_473_600;
const NOTIF_CACHE_TTL: Duration = Duration::from_millis(1500);
const MAX_FETCHED_NOTIFICATIONS: usize = 24;
const MAX_TITLE_CHARS: usize = 96;
const MAX_BODY_CHARS: usize = 160;
const MAX_APP_NAME_CHARS: usize = 48;

#[derive(Default)]
struct NotifCacheState {
    fetched_at: Option<Instant>,
    snapshot: NotificationCenterSnapshot,
}

static NOTIF_CACHE: OnceLock<Mutex<NotifCacheState>> = OnceLock::new();
static NOTIF_LISTENER_SUB: OnceLock<Mutex<Option<(UserNotificationListener, i64)>>> =
    OnceLock::new();
static NOTIF_DIRTY: AtomicBool = AtomicBool::new(true);
static NOTIF_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

fn cache_lock() -> &'static Mutex<NotifCacheState> {
    NOTIF_CACHE.get_or_init(|| Mutex::new(NotifCacheState::default()))
}

fn listener_sub_lock() -> &'static Mutex<Option<(UserNotificationListener, i64)>> {
    NOTIF_LISTENER_SUB.get_or_init(|| Mutex::new(None))
}

#[must_use]
pub(crate) fn is_packaged_app() -> bool {
    let mut len = 0u32;
    let err = unsafe { GetCurrentPackageFullName(&raw mut len, None) };
    err == ERROR_INSUFFICIENT_BUFFER
}

#[must_use]
const fn classify_listener_access(
    status: Option<UserNotificationListenerAccessStatus>,
    packaged: bool,
) -> NotificationAccess {
    match status {
        Some(UserNotificationListenerAccessStatus::Allowed) => NotificationAccess::Allowed,
        Some(UserNotificationListenerAccessStatus::Denied) => NotificationAccess::Denied,
        Some(UserNotificationListenerAccessStatus::Unspecified) | None => {
            if packaged {
                NotificationAccess::Unavailable
            } else {
                NotificationAccess::RequiresPackageIdentity
            }
        }
        Some(_) => NotificationAccess::Unavailable,
    }
}

fn truncate_chars(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let mut out = String::new();
    for ch in trimmed.chars().take(max_chars) {
        out.push(ch);
    }
    if trimmed.chars().count() > max_chars && max_chars > 1 {
        out.pop();
        out.push('…');
    }
    out
}

fn split_notification_lines(lines: Vec<String>) -> (String, String) {
    let mut non_empty = lines
        .into_iter()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty());
    let title = non_empty
        .next()
        .map(|s| truncate_chars(&s, MAX_TITLE_CHARS))
        .unwrap_or_default();
    let rest: Vec<String> = non_empty.collect();
    let body = truncate_chars(&rest.join(" "), MAX_BODY_CHARS);
    (title, body)
}

#[must_use]
const fn creation_time_elapsed_secs(universal_time_100ns: i64, now_unix_secs: u64) -> u64 {
    if universal_time_100ns <= 0 {
        return 0;
    }
    let windows_secs = universal_time_100ns / 10_000_000;
    let unix_secs = windows_secs - WINDOWS_TO_UNIX_EPOCH_SECS;
    if unix_secs <= 0 {
        return now_unix_secs;
    }
    now_unix_secs.saturating_sub(unix_secs as u64)
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn ensure_listener_subscribed(listener: &UserNotificationListener) {
    if NOTIF_SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    let Ok(mut guard) = listener_sub_lock().lock() else {
        return;
    };
    if guard.is_some() {
        return;
    }
    if let Ok(token) = listener.NotificationChanged(&TypedEventHandler::new(|_, _| {
        NOTIF_DIRTY.store(true, Ordering::Release);
        crate::idle_wake::request_idle_wake();
        Ok(())
    })) {
        *guard = Some((listener.clone(), token));
    }
}

fn extract_notification_summary(
    item: &UserNotification,
    now_unix: u64,
) -> Option<(i64, NotificationSummary)> {
    let id = item.Id().ok()?;
    let creation_100ns = item.CreationTime().ok().map_or(0, |dt| dt.UniversalTime);
    let elapsed = creation_time_elapsed_secs(creation_100ns, now_unix);
    let timestamp_label = format_relative_age(elapsed);

    let (app_id, app_name) = match item.AppInfo() {
        Ok(info) => {
            let id_str = info
                .AppUserModelId()
                .ok()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            let display = info
                .DisplayInfo()
                .ok()
                .and_then(|d| d.DisplayName().ok())
                .map(|s| truncate_chars(&s.to_string_lossy(), MAX_APP_NAME_CHARS))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| String::from("Windows"));
            (id_str, display)
        }
        Err(_) => (String::new(), String::from("Windows")),
    };

    let mut lines = Vec::new();
    if let Ok(notif) = item.Notification()
        && let Ok(visual) = notif.Visual()
        && let Ok(binding_name) = KnownNotificationBindings::ToastGeneric()
        && let Ok(binding) = visual.GetBinding(&binding_name)
        && let Ok(elements) = binding.GetTextElements()
        && let Ok(count) = elements.Size()
    {
        for idx in 0..count {
            if let Ok(elem) = elements.GetAt(idx)
                && let Ok(text) = elem.Text()
            {
                lines.push(text.to_string_lossy());
            }
        }
    }
    let (mut title, body) = split_notification_lines(lines);
    if title.is_empty() {
        title = app_name.clone();
    }

    Some((
        creation_100ns,
        NotificationSummary {
            id,
            app_id,
            app_name,
            title,
            body,
            timestamp_label,
        },
    ))
}

fn query_notifications_live() -> NotificationCenterSnapshot {
    let packaged = is_packaged_app();
    let _com = crate::com::ComGuard::multithreaded();
    let Ok(listener) = UserNotificationListener::Current() else {
        return NotificationCenterSnapshot {
            access: classify_listener_access(None, packaged),
            items: Vec::new(),
        };
    };
    let status = listener.GetAccessStatus().ok();
    let access = classify_listener_access(status, packaged);
    if access != NotificationAccess::Allowed {
        return NotificationCenterSnapshot {
            access,
            items: Vec::new(),
        };
    }

    ensure_listener_subscribed(&listener);
    let Ok(list) = listener
        .GetNotificationsAsync(NotificationKinds::Toast)
        .and_then(|op| op.join())
    else {
        return NotificationCenterSnapshot {
            access: NotificationAccess::Allowed,
            items: Vec::new(),
        };
    };

    let now_unix = current_unix_secs();
    let count = list.Size().unwrap_or(0);
    let mut collected = Vec::with_capacity(count as usize);
    for idx in 0..count {
        if let Ok(item) = list.GetAt(idx)
            && let Some(summary) = extract_notification_summary(&item, now_unix)
        {
            collected.push(summary);
        }
    }
    collected.sort_by(|a, b| b.0.cmp(&a.0));
    let items = collected
        .into_iter()
        .take(MAX_FETCHED_NOTIFICATIONS)
        .map(|(_, item)| item)
        .collect();
    NotificationCenterSnapshot {
        access: NotificationAccess::Allowed,
        items,
    }
}

#[must_use]
pub fn read_notifications_snapshot() -> NotificationCenterSnapshot {
    if NOTIF_SHUTTING_DOWN.load(Ordering::Acquire) {
        return cache_lock().lock().map_or_else(
            |_| NotificationCenterSnapshot::default(),
            |g| g.snapshot.clone(),
        );
    }
    let now = Instant::now();
    if !NOTIF_DIRTY.load(Ordering::Acquire)
        && let Ok(guard) = cache_lock().lock()
        && let Some(fetched_at) = guard.fetched_at
        && now.saturating_duration_since(fetched_at) < NOTIF_CACHE_TTL
    {
        return guard.snapshot.clone();
    }

    let snapshot = query_notifications_live();
    NOTIF_DIRTY.store(false, Ordering::Release);
    if let Ok(mut guard) = cache_lock().lock() {
        guard.fetched_at = Some(Instant::now());
        guard.snapshot = snapshot.clone();
    }
    snapshot
}

#[must_use]
pub fn dismiss_notification(id: u32) -> bool {
    if NOTIF_SHUTTING_DOWN.load(Ordering::Acquire) {
        return false;
    }
    let _com = crate::com::ComGuard::multithreaded();
    let Ok(listener) = UserNotificationListener::Current() else {
        return false;
    };
    let ok = listener.RemoveNotification(id).is_ok();
    if ok {
        NOTIF_DIRTY.store(true, Ordering::Release);
        if let Ok(mut guard) = cache_lock().lock() {
            guard.snapshot.items.retain(|item| item.id != id);
        }
    }
    ok
}

#[must_use]
pub fn clear_notifications() -> bool {
    if NOTIF_SHUTTING_DOWN.load(Ordering::Acquire) {
        return false;
    }
    let _com = crate::com::ComGuard::multithreaded();
    let Ok(listener) = UserNotificationListener::Current() else {
        return false;
    };
    let ok = listener.ClearNotifications().is_ok();
    if ok {
        NOTIF_DIRTY.store(true, Ordering::Release);
        if let Ok(mut guard) = cache_lock().lock() {
            guard.snapshot.items.clear();
        }
    }
    ok
}

#[must_use]
pub fn request_notification_access() -> NotificationAccess {
    if NOTIF_SHUTTING_DOWN.load(Ordering::Acquire) {
        return NotificationAccess::Unavailable;
    }
    let packaged = is_packaged_app();
    let _com = crate::com::ComGuard::multithreaded();
    let Ok(listener) = UserNotificationListener::Current() else {
        return classify_listener_access(None, packaged);
    };
    let status = listener
        .RequestAccessAsync()
        .and_then(|op| op.join())
        .ok()
        .or_else(|| listener.GetAccessStatus().ok());
    let access = classify_listener_access(status, packaged);
    NOTIF_DIRTY.store(true, Ordering::Release);
    access
}

#[must_use]
pub fn aggregate_app_counts(items: &[NotificationSummary]) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for item in items {
        let key = item.app_id.trim().to_ascii_lowercase();
        if !key.is_empty() {
            *counts.entry(key).or_insert(0) += 1;
        }
        let name_key = item.app_name.trim().to_ascii_lowercase();
        if !name_key.is_empty() && name_key != item.app_id.trim().to_ascii_lowercase() {
            *counts.entry(name_key).or_insert(0) += 1;
        }
    }
    counts
}

#[must_use]
pub fn read_notification_app_counts() -> HashMap<String, u32> {
    let snapshot = read_notifications_snapshot();
    if snapshot.access != NotificationAccess::Allowed {
        return HashMap::new();
    }
    aggregate_app_counts(&snapshot.items)
}

pub(crate) fn resume_notifications_task() {
    NOTIF_SHUTTING_DOWN.store(false, Ordering::Release);
    NOTIF_DIRTY.store(true, Ordering::Release);
}

pub fn shutdown_notifications_task() {
    NOTIF_SHUTTING_DOWN.store(true, Ordering::Release);
    if let Ok(mut guard) = listener_sub_lock().lock()
        && let Some((listener, token)) = guard.take()
    {
        let _ = listener.RemoveNotificationChanged(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_listener_access_respects_packaged_and_allowed_states() {
        assert_eq!(
            classify_listener_access(Some(UserNotificationListenerAccessStatus::Allowed), false),
            NotificationAccess::Allowed
        );
        assert_eq!(
            classify_listener_access(Some(UserNotificationListenerAccessStatus::Denied), true),
            NotificationAccess::Denied
        );
        assert_eq!(
            classify_listener_access(
                Some(UserNotificationListenerAccessStatus::Unspecified),
                false
            ),
            NotificationAccess::RequiresPackageIdentity
        );
        assert_eq!(
            classify_listener_access(
                Some(UserNotificationListenerAccessStatus::Unspecified),
                true
            ),
            NotificationAccess::Unavailable
        );
        assert_eq!(
            classify_listener_access(None, false),
            NotificationAccess::RequiresPackageIdentity
        );
    }

    #[test]
    fn notification_lines_and_timestamps_format_cleanly() {
        let (title, body) = split_notification_lines(vec![
            String::from("  Build   Finished "),
            String::new(),
            String::from("All 419 tests"),
            String::from("passed cleanly"),
        ]);
        assert_eq!(title, "Build Finished");
        assert_eq!(body, "All 419 tests passed cleanly");

        let now = 1_760_000_000u64;
        let five_mins_ago = ((now - 300) as i64 + WINDOWS_TO_UNIX_EPOCH_SECS) * 10_000_000;
        assert_eq!(creation_time_elapsed_secs(five_mins_ago, now), 300);
        assert_eq!(format_relative_age(300), "5m");
    }

    #[test]
    fn app_counts_group_by_aumid_and_display_name() {
        let items = vec![
            NotificationSummary {
                id: 1,
                app_id: String::from("Slack"),
                app_name: String::from("Slack"),
                title: String::from("Ping"),
                body: String::new(),
                timestamp_label: String::from("now"),
            },
            NotificationSummary {
                id: 2,
                app_id: String::from("Microsoft.OutlookForWindows_8wekyb3d8bbwe!App"),
                app_name: String::from("Outlook"),
                title: String::from("Mail"),
                body: String::new(),
                timestamp_label: String::from("2m"),
            },
        ];
        let counts = aggregate_app_counts(&items);
        assert_eq!(counts.get("slack"), Some(&1));
        assert_eq!(counts.get("outlook"), Some(&1));
        assert_eq!(
            counts.get("microsoft.outlookforwindows_8wekyb3d8bbwe!app"),
            Some(&1)
        );
    }

    #[test]
    fn shutdown_blocks_live_notifications_until_resumed() {
        struct ResumeGuard;
        impl Drop for ResumeGuard {
            fn drop(&mut self) {
                resume_notifications_task();
            }
        }
        let _guard = ResumeGuard;
        shutdown_notifications_task();
        assert!(NOTIF_SHUTTING_DOWN.load(Ordering::Acquire));
        assert!(!dismiss_notification(1));
        assert!(!clear_notifications());
        assert_eq!(
            request_notification_access(),
            NotificationAccess::Unavailable
        );
    }
}
