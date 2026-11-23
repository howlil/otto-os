use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use otto_core::{
    FocusCatalogApp, NotificationAccess, NotificationCenterSnapshot, NotificationSummary,
    compact_app_id, format_relative_age, normalize_app_key,
};
use windows::ApplicationModel::AppDisplayInfo;
use windows::Foundation::{Size, TypedEventHandler};
use windows::Storage::Streams::IInputStream;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::core::Interface;

const WINDOWS_TO_UNIX_EPOCH_SECS: i64 = 11_644_473_600;
const NOTIF_CACHE_TTL: Duration = Duration::from_millis(1500);
const NOTIF_QUERY_TIMEOUT: Duration = Duration::from_millis(750);
const NOTIF_ACCESS_TIMEOUT: Duration = Duration::from_secs(30);
const NOTIF_LOGO_TIMEOUT: Duration = Duration::from_millis(250);
const NOTIF_LOGO_MISS_TTL: Duration = Duration::from_secs(30);
const MAX_FETCHED_NOTIFICATIONS: usize = 24;
const MAX_LOGO_BYTES: usize = 256 * 1024;
const MAX_LOGO_CACHE_ENTRIES: usize = 48;
const LOGO_REQUEST_SIZE: f32 = 64.0;
const MAX_TITLE_CHARS: usize = 96;
const MAX_BODY_CHARS: usize = 160;
const MAX_APP_NAME_CHARS: usize = 48;
const RAW_BGRA_ICON_MAGIC: &[u8; 4] = b"BGRA";
const RAW_BGRA_HEADER_LEN: usize = 12;

#[derive(Default)]
struct NotifCacheState {
    fetched_at: Option<Instant>,
    snapshot: NotificationCenterSnapshot,
}

struct LogoCacheEntry {
    bytes: Option<Arc<[u8]>>,
    cached_at: Instant,
}

type LogoCacheMap = HashMap<String, LogoCacheEntry>;

static NOTIF_CACHE: OnceLock<Mutex<NotifCacheState>> = OnceLock::new();
static NOTIF_LOGO_CACHE: OnceLock<Mutex<LogoCacheMap>> = OnceLock::new();
static NOTIF_CATALOG_CACHE: OnceLock<Mutex<Option<Vec<FocusCatalogApp>>>> = OnceLock::new();
static NOTIF_LISTENER_SUB: OnceLock<Mutex<Option<(UserNotificationListener, i64)>>> =
    OnceLock::new();
static NOTIF_DIRTY: AtomicBool = AtomicBool::new(true);
static NOTIF_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static NOTIF_PRIMED: AtomicBool = AtomicBool::new(false);

fn cache_lock() -> &'static Mutex<NotifCacheState> {
    NOTIF_CACHE.get_or_init(|| Mutex::new(NotifCacheState::default()))
}

fn logo_cache_lock() -> &'static Mutex<LogoCacheMap> {
    NOTIF_LOGO_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn catalog_cache_lock() -> &'static Mutex<Option<Vec<FocusCatalogApp>>> {
    NOTIF_CATALOG_CACHE.get_or_init(|| Mutex::new(None))
}

fn listener_sub_lock() -> &'static Mutex<Option<(UserNotificationListener, i64)>> {
    NOTIF_LISTENER_SUB.get_or_init(|| Mutex::new(None))
}

fn is_notif_shutting_down() -> bool {
    NOTIF_SHUTTING_DOWN.load(Ordering::Acquire)
}

fn logo_cache_key<'a>(app_id: &'a str, app_name: &'a str) -> &'a str {
    let trimmed_id = app_id.trim();
    if !trimmed_id.is_empty() {
        return trimmed_id;
    }
    app_name.trim()
}

enum CachedLogo {
    Hit(Arc<[u8]>),
    RecentMiss,
    Expired,
}

fn lookup_cached_logo(cache_key: &str, now: Instant) -> CachedLogo {
    let Ok(guard) = logo_cache_lock().lock() else {
        return CachedLogo::Expired;
    };
    let Some(entry) = guard.get(cache_key) else {
        return CachedLogo::Expired;
    };
    if let Some(bytes) = &entry.bytes {
        return CachedLogo::Hit(bytes.clone());
    }
    if now.saturating_duration_since(entry.cached_at) < NOTIF_LOGO_MISS_TTL {
        CachedLogo::RecentMiss
    } else {
        CachedLogo::Expired
    }
}

fn store_cached_logo(cache_key: &str, bytes: Option<Arc<[u8]>>, now: Instant) {
    if let Ok(mut guard) = logo_cache_lock().lock() {
        if guard.len() >= MAX_LOGO_CACHE_ENTRIES {
            guard.clear();
        }
        guard.insert(
            cache_key.to_owned(),
            LogoCacheEntry {
                bytes,
                cached_at: now,
            },
        );
    }
}

fn encode_bgra_icon_bytes(bitmap: &otto_core::IconBitmap) -> Option<Arc<[u8]>> {
    let expected = usize::try_from(bitmap.width)
        .ok()?
        .checked_mul(usize::try_from(bitmap.height).ok()?)?
        .checked_mul(4)?;
    if expected == 0 || bitmap.pixels.len() != expected {
        return None;
    }
    let mut out = Vec::with_capacity(RAW_BGRA_HEADER_LEN + expected);
    out.extend_from_slice(RAW_BGRA_ICON_MAGIC);
    out.extend_from_slice(&bitmap.width.to_le_bytes());
    out.extend_from_slice(&bitmap.height.to_le_bytes());
    out.extend_from_slice(&bitmap.pixels);
    Some(Arc::from(out.into_boxed_slice()))
}

fn catalog_entry_matches_notification(
    entry: &FocusCatalogApp,
    app_id: &str,
    app_name: &str,
) -> bool {
    let trimmed_id = app_id.trim();
    if !trimmed_id.is_empty() {
        if entry
            .icon_source
            .strip_prefix("shell:AppsFolder\\")
            .is_some_and(|identity| identity.eq_ignore_ascii_case(trimmed_id))
        {
            return true;
        }
        let id_compact = compact_app_id(trimmed_id);
        let id_prefix = trimmed_id
            .split(['.', '!', '\\', '/'])
            .next()
            .map(normalize_app_key)
            .unwrap_or_default();
        if (!id_compact.is_empty()
            && (compact_app_id(&entry.block_key) == id_compact
                || compact_app_id(&entry.display_name) == id_compact))
            || (id_prefix.len() >= 3 && entry.block_key == id_prefix)
        {
            return true;
        }
    }
    let norm_name = normalize_app_key(app_name);
    let compact_name = compact_app_id(app_name);
    (!norm_name.is_empty()
        && (normalize_app_key(&entry.display_name) == norm_name || entry.block_key == norm_name))
        || (!compact_name.is_empty()
            && (compact_app_id(&entry.display_name) == compact_name
                || compact_app_id(&entry.block_key) == compact_name))
}

fn find_catalog_icon_source(app_id: &str, app_name: &str) -> Option<(String, Option<String>)> {
    let mut guard = catalog_cache_lock().lock().ok()?;
    let catalog = guard.get_or_insert_with(crate::spotlight::collect_focus_catalog_apps);
    catalog
        .iter()
        .find(|entry| catalog_entry_matches_notification(entry, app_id, app_name))
        .map(|entry| {
            let detail = (!entry.detail.is_empty()).then(|| entry.detail.clone());
            (entry.icon_source.clone(), detail)
        })
}

fn resolve_fallback_icon_bytes(app_id: &str, app_name: &str) -> Option<Arc<[u8]>> {
    if is_notif_shutting_down() {
        return None;
    }
    let trimmed_id = app_id.trim();
    let trimmed_name = app_name.trim();
    let bitmap = (!trimmed_id.is_empty())
        .then(|| {
            let apps_folder = format!("shell:AppsFolder\\{trimmed_id}");
            crate::dock::extract_app_icon_bitmap(&apps_folder, None)
        })
        .flatten()
        .or_else(|| {
            find_catalog_icon_source(trimmed_id, trimmed_name).and_then(|(source, detail)| {
                crate::dock::extract_app_icon_bitmap(&source, detail.as_deref())
            })
        })
        .or_else(|| {
            (!trimmed_id.is_empty())
                .then(|| crate::dock::extract_app_icon_bitmap(trimmed_id, None))
                .flatten()
        })
        .or_else(|| {
            (!trimmed_name.is_empty())
                .then(|| crate::dock::extract_app_icon_bitmap(trimmed_name, None))
                .flatten()
        })
        .or_else(|| {
            let norm = normalize_app_key(trimmed_name);
            (!norm.is_empty())
                .then(|| crate::dock::extract_app_icon_bitmap(&norm, None))
                .flatten()
        })?;
    encode_bgra_icon_bytes(&bitmap)
}

fn hydrate_missing_icons(items: &mut [NotificationSummary]) {
    if is_notif_shutting_down() || items.iter().all(|item| item.icon_bytes.is_some()) {
        return;
    }
    let _com = crate::com::ComGuard::single_threaded();
    for item in items {
        if is_notif_shutting_down() {
            break;
        }
        if item.icon_bytes.is_some() {
            continue;
        }
        let key = logo_cache_key(&item.app_id, &item.app_name);
        if key.is_empty() {
            continue;
        }
        let now = Instant::now();
        match lookup_cached_logo(key, now) {
            CachedLogo::Hit(bytes) => {
                item.icon_bytes = Some(bytes);
                continue;
            }
            CachedLogo::RecentMiss => continue,
            CachedLogo::Expired => {}
        }
        let resolved = resolve_fallback_icon_bytes(&item.app_id, &item.app_name);
        store_cached_logo(key, resolved.clone(), now);
        item.icon_bytes = resolved;
    }
}

fn read_display_logo(display: &AppDisplayInfo, cache_key: &str) -> Option<Arc<[u8]>> {
    if is_notif_shutting_down() || cache_key.is_empty() {
        return None;
    }
    let now = Instant::now();
    match lookup_cached_logo(cache_key, now) {
        CachedLogo::Hit(bytes) => return Some(bytes),
        CachedLogo::RecentMiss => return None,
        CachedLogo::Expired => {}
    }
    let deadline = now + NOTIF_LOGO_TIMEOUT;
    let bytes = display
        .GetLogo(Size {
            Width: LOGO_REQUEST_SIZE,
            Height: LOGO_REQUEST_SIZE,
        })
        .ok()
        .and_then(|stream_ref| crate::winrt_io::await_until!(stream_ref.OpenReadAsync(), deadline))
        .and_then(|stream| {
            let size = stream.Size().ok()?;
            if size == 0 || size > MAX_LOGO_BYTES as u64 {
                return None;
            }
            let input = stream.cast::<IInputStream>().ok()?;
            let raw = crate::winrt_io::read_stream_bounded(
                &input,
                MAX_LOGO_BYTES,
                deadline,
                &is_notif_shutting_down,
            )?;
            (!raw.is_empty()).then(|| Arc::from(raw.into_boxed_slice()))
        });
    if bytes.is_some() {
        store_cached_logo(cache_key, bytes.clone(), Instant::now());
    }
    bytes
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

    let (app_id, app_name, icon_bytes) = match item.AppInfo() {
        Ok(info) => {
            let id_str = info
                .AppUserModelId()
                .ok()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            let display_info = info.DisplayInfo().ok();
            let display = display_info
                .as_ref()
                .and_then(|d| d.DisplayName().ok())
                .map(|s| truncate_chars(&s.to_string_lossy(), MAX_APP_NAME_CHARS))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| String::from("Windows"));
            let logo_key = logo_cache_key(&id_str, &display);
            let icon = if let Some(info_display) = display_info.as_ref() {
                read_display_logo(info_display, logo_key)
            } else if let CachedLogo::Hit(bytes) = lookup_cached_logo(logo_key, Instant::now()) {
                Some(bytes)
            } else {
                None
            };
            (id_str, display, icon)
        }
        Err(_) => (String::new(), String::from("Windows"), None),
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
        title.clone_from(&app_name);
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
            icon_bytes,
        },
    ))
}

fn query_notifications_live() -> NotificationCenterSnapshot {
    let packaged = is_packaged_app();
    let (access, mut items) = {
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
        let Some(list) = crate::winrt_io::await_until!(
            listener.GetNotificationsAsync(NotificationKinds::Toast),
            Instant::now() + NOTIF_QUERY_TIMEOUT
        ) else {
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
        collected.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        let items = collected
            .into_iter()
            .take(MAX_FETCHED_NOTIFICATIONS)
            .map(|(_, item)| item)
            .collect::<Vec<_>>();
        (NotificationAccess::Allowed, items)
    };
    hydrate_missing_icons(&mut items);
    NotificationCenterSnapshot { access, items }
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

fn activate_notification_source(item: &NotificationSummary) {
    let windows = crate::window::model::enumerate_windows(crate::window::ops::current_process_id());
    let target_aumid =
        (!item.app_id.is_empty()).then(|| otto_core::AppIdentity::user_model_id(&item.app_id));
    if let Some(window) = windows.into_iter().find(|window| {
        window.role == otto_core::WindowRole::Primary
            && (target_aumid
                .as_ref()
                .is_some_and(|aumid| &window.app == aumid)
                || window.app_name.eq_ignore_ascii_case(&item.app_name))
    }) {
        crate::dock::focus_window_hwnd(window.identity.hwnd);
        return;
    }
    if !item.app_id.is_empty() {
        let target = format!("shell:AppsFolder\\{}", item.app_id);
        let wide = windows::core::HSTRING::from(target.as_str());
        let _ = crate::actions::shell_exec_ok(
            windows::core::PCWSTR(wide.as_ptr()),
            windows::core::PCWSTR::null(),
        );
    }
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
        let source = cache_lock().lock().ok().and_then(|mut guard| {
            let found = guard
                .snapshot
                .items
                .iter()
                .find(|item| item.id == id)
                .cloned();
            guard.snapshot.items.retain(|item| item.id != id);
            found
        });
        if let Some(item) = source {
            activate_notification_source(&item);
        }
    }
    ok
}

#[must_use]
pub fn remove_notification(id: u32) -> bool {
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
pub fn take_notification_changes() -> Option<NotificationCenterSnapshot> {
    if NOTIF_SHUTTING_DOWN.load(Ordering::Acquire) {
        return None;
    }
    if !NOTIF_PRIMED.swap(true, Ordering::AcqRel) || NOTIF_DIRTY.load(Ordering::Acquire) {
        return Some(read_notifications_snapshot());
    }
    None
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
    let status = crate::winrt_io::await_until!(
        listener.RequestAccessAsync(),
        Instant::now() + NOTIF_ACCESS_TIMEOUT
    )
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

pub fn resume_notifications_task() {
    NOTIF_SHUTTING_DOWN.store(false, Ordering::Release);
    NOTIF_DIRTY.store(true, Ordering::Release);
}

pub fn shutdown_notifications_task() {
    NOTIF_SHUTTING_DOWN.store(true, Ordering::Release);
    NOTIF_PRIMED.store(false, Ordering::Release);
    if let Ok(mut guard) = listener_sub_lock().lock()
        && let Some((listener, token)) = guard.take()
    {
        let _ = listener.RemoveNotificationChanged(token);
    }
    if let Some(cache) = NOTIF_CACHE.get()
        && let Ok(mut guard) = cache.lock()
    {
        *guard = NotifCacheState::default();
    }
    if let Some(logos) = NOTIF_LOGO_CACHE.get()
        && let Ok(mut guard) = logos.lock()
    {
        guard.clear();
    }
    if let Some(catalog) = NOTIF_CATALOG_CACHE.get()
        && let Ok(mut guard) = catalog.lock()
    {
        *guard = None;
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
                icon_bytes: None,
            },
            NotificationSummary {
                id: 2,
                app_id: String::from("Microsoft.OutlookForWindows_8wekyb3d8bbwe!App"),
                app_name: String::from("Outlook"),
                title: String::from("Mail"),
                body: String::new(),
                timestamp_label: String::from("2m"),
                icon_bytes: None,
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

    #[test]
    fn bgra_icon_encoder_and_catalog_matcher_resolve_desktop_and_msix_apps() {
        let bitmap = otto_core::IconBitmap {
            width: 2,
            height: 2,
            pixels: vec![0xFF; 16],
        };
        let encoded = encode_bgra_icon_bytes(&bitmap).expect("encoded bgra");
        assert_eq!(&encoded[..4], RAW_BGRA_ICON_MAGIC);
        assert_eq!(
            u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]),
            2
        );
        assert_eq!(
            u32::from_le_bytes([encoded[8], encoded[9], encoded[10], encoded[11]]),
            2
        );
        assert_eq!(&encoded[12..], &bitmap.pixels[..]);

        let chrome = FocusCatalogApp {
            block_key: String::from("chrome"),
            display_name: String::from("Google Chrome"),
            detail: String::from("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            icon_source: String::from("shell:AppsFolder\\Chrome"),
        };
        assert!(catalog_entry_matches_notification(
            &chrome,
            "Chrome.Profile1",
            "Google Chrome"
        ));
        let whatsapp = FocusCatalogApp {
            block_key: String::from("whatsapp"),
            display_name: String::from("WhatsApp"),
            detail: String::new(),
            icon_source: String::from(
                "shell:AppsFolder\\5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
            ),
        };
        assert!(catalog_entry_matches_notification(
            &whatsapp,
            "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
            "WhatsApp"
        ));
    }
}
