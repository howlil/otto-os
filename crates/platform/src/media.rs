use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{MediaCapabilities, NORMAL_PLAYBACK_RATE_PERMILLE, TrackKey};
pub use otto_core::{MediaCommand, NowPlaying};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Storage::Streams::{IInputStream, IRandomAccessStreamReference};
use windows::core::Interface;

use crate::com::ComGuard;
use crate::winrt_io::{await_until, read_stream_bounded};

pub const MEDIA_CACHE_TTL: Duration = Duration::from_millis(250);
pub const MEDIA_STALE_AFTER: Duration = Duration::from_secs(3);
pub const MEDIA_OPERATION_TIMEOUT: Duration = Duration::from_millis(1_500);
pub const MEDIA_QUEUE_DEPTH: usize = 8;
pub const MEDIA_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
pub const MAX_THUMBNAIL_BYTES: usize = 10 * 1024 * 1024;
const ARTWORK_RETRY_BASE: Duration = Duration::from_secs(1);
const ARTWORK_MAX_ATTEMPTS: u32 = 4;
const WINDOWS_TO_UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
const TICKS_PER_MILLISECOND: i64 = 10_000;

enum MediaTask {
    Refresh,
    Command(MediaCommand),
}

#[derive(Default)]
struct MediaCache {
    snapshot: Option<NowPlaying>,
    refreshed_at: Option<Instant>,
    requested_at: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
struct CacheRead {
    snapshot: Option<NowPlaying>,
    request_refresh: bool,
}

impl MediaCache {
    fn read(&mut self, now: Instant) -> CacheRead {
        let age = |moment: Option<Instant>| moment.map(|at| now.saturating_duration_since(at));
        let fresh = age(self.refreshed_at).is_some_and(|elapsed| elapsed < MEDIA_CACHE_TTL);
        let stalled = age(self.requested_at).is_some_and(|elapsed| elapsed >= MEDIA_STALE_AFTER);
        if stalled {
            self.snapshot = None;
            self.requested_at = None;
        }
        let request_refresh = !fresh && self.requested_at.is_none();
        if request_refresh {
            self.requested_at = Some(now);
        }
        CacheRead {
            snapshot: self.snapshot.clone(),
            request_refresh,
        }
    }

    fn store(&mut self, snapshot: Option<NowPlaying>, now: Instant) {
        self.snapshot = snapshot;
        self.refreshed_at = Some(now);
        self.requested_at = None;
    }

    fn abandon_request(&mut self) {
        self.requested_at = None;
    }
}

#[derive(Default)]
struct ArtworkCache {
    key: Option<TrackKey>,
    bytes: Option<Arc<[u8]>>,
    attempts: u32,
    retry_at: Option<Instant>,
}

impl ArtworkCache {
    fn cached(&self, key: &TrackKey) -> Option<Arc<[u8]>> {
        if self.key.as_ref() == Some(key) {
            self.bytes.clone()
        } else {
            None
        }
    }

    fn should_fetch(&self, key: &TrackKey, now: Instant) -> bool {
        if self.key.as_ref() != Some(key) {
            return true;
        }
        self.bytes.is_none()
            && self.attempts < ARTWORK_MAX_ATTEMPTS
            && self.retry_at.is_none_or(|at| now >= at)
    }

    fn record(&mut self, key: &TrackKey, bytes: Option<Arc<[u8]>>, now: Instant) {
        if self.key.as_ref() != Some(key) {
            self.key = Some(key.clone());
            self.attempts = 0;
        }
        self.attempts = self.attempts.saturating_add(1);
        self.retry_at = bytes
            .is_none()
            .then(|| now + ARTWORK_RETRY_BASE * 2u32.saturating_pow(self.attempts - 1));
        self.bytes = bytes;
    }
}

fn unix_ms_from_windows_ticks(ticks: i64) -> Option<i64> {
    (ticks > WINDOWS_TO_UNIX_EPOCH_TICKS)
        .then(|| (ticks - WINDOWS_TO_UNIX_EPOCH_TICKS) / TICKS_PER_MILLISECOND)
}

fn rate_permille(rate: f64) -> u32 {
    if rate.is_finite() && rate > 0.0 {
        (rate * f64::from(NORMAL_PLAYBACK_RATE_PERMILLE)).round() as u32
    } else {
        NORMAL_PLAYBACK_RATE_PERMILLE
    }
}

struct MediaRuntime {
    sender: mpsc::SyncSender<MediaTask>,
    cache: Arc<Mutex<MediaCache>>,
    handle: JoinHandle<()>,
}

static MEDIA_RUNTIME: OnceLock<Mutex<Option<MediaRuntime>>> = OnceLock::new();

fn runtime() -> Option<(mpsc::SyncSender<MediaTask>, Arc<Mutex<MediaCache>>)> {
    let runtime = MEDIA_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut guard = runtime.lock().ok()?;
    if guard.is_none() {
        let (sender, receiver) = mpsc::sync_channel(MEDIA_QUEUE_DEPTH);
        let cache = Arc::new(Mutex::new(MediaCache::default()));
        let worker_cache = Arc::clone(&cache);
        let handle = thread::Builder::new()
            .name(String::from("otto-media"))
            .spawn(move || media_worker(&receiver, worker_cache.as_ref()))
            .ok()?;
        *guard = Some(MediaRuntime {
            sender,
            cache,
            handle,
        });
    }
    let active = guard.as_ref()?;
    Some((active.sender.clone(), Arc::clone(&active.cache)))
}

#[must_use]
pub fn read_now_playing() -> Option<NowPlaying> {
    let (sender, cache) = runtime()?;
    let read = cache.lock().ok()?.read(Instant::now());
    if read.request_refresh
        && sender.try_send(MediaTask::Refresh).is_err()
        && let Ok(mut state) = cache.lock()
    {
        state.abandon_request();
    }
    read.snapshot
}

#[must_use]
pub fn send_media_command(command: MediaCommand) -> bool {
    runtime().is_some_and(|(sender, _)| sender.try_send(MediaTask::Command(command)).is_ok())
}

pub fn shutdown_media_task() {
    let Some(runtime) = MEDIA_RUNTIME
        .get()
        .and_then(|cell| cell.lock().ok().and_then(|mut guard| guard.take()))
    else {
        return;
    };
    drop(runtime.sender);
    let _ = crate::worker::join_within(runtime.handle, MEDIA_SHUTDOWN_TIMEOUT);
}

fn media_worker(receiver: &mpsc::Receiver<MediaTask>, cache: &Mutex<MediaCache>) {
    let com = ComGuard::multithreaded();
    let mut artwork = ArtworkCache::default();
    for task in receiver {
        let deadline = Instant::now() + MEDIA_OPERATION_TIMEOUT;
        match task {
            MediaTask::Refresh => {
                let snapshot = com
                    .as_ref()
                    .and_then(|_| read_now_playing_sync(&mut artwork, deadline));
                if let Ok(mut state) = cache.lock() {
                    state.store(snapshot, Instant::now());
                }
            }
            MediaTask::Command(command) => {
                if com.is_none() {
                    continue;
                }
                let outcome = send_media_command_sync(command, deadline);
                if outcome != CommandResult::Accepted {
                    crate::actions::signal_command_failure();
                }
                let readback =
                    read_now_playing_sync(&mut artwork, Instant::now() + MEDIA_OPERATION_TIMEOUT);
                if let Ok(mut state) = cache.lock() {
                    state.store(readback, Instant::now());
                }
            }
        }
    }
}

fn current_session(deadline: Instant) -> Option<GlobalSystemMediaTransportControlsSession> {
    await_until!(
        GlobalSystemMediaTransportControlsSessionManager::RequestAsync(),
        deadline
    )?
    .GetCurrentSession()
    .ok()
}

fn read_thumbnail_bytes(
    stream_ref: &IRandomAccessStreamReference,
    deadline: Instant,
) -> Option<Arc<[u8]>> {
    let stream = await_until!(stream_ref.OpenReadAsync(), deadline)?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_THUMBNAIL_BYTES as u64 {
        return None;
    }
    let input = stream.cast::<IInputStream>().ok()?;
    let bytes = read_stream_bounded(&input, MAX_THUMBNAIL_BYTES, deadline, &|| false)?;
    (!bytes.is_empty()).then(|| Arc::from(bytes.into_boxed_slice()))
}

fn read_now_playing_sync(artwork: &mut ArtworkCache, deadline: Instant) -> Option<NowPlaying> {
    let session = current_session(deadline)?;
    let props = await_until!(session.TryGetMediaPropertiesAsync(), deadline)?;
    let title = props.Title().ok()?.to_string();
    if title.trim().is_empty() {
        return None;
    }
    let artist = props.Artist().map(|a| a.to_string()).unwrap_or_default();
    let source_app = session
        .SourceAppUserModelId()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let info = session.GetPlaybackInfo().ok();
    let playing = info
        .as_ref()
        .and_then(|info| info.PlaybackStatus().ok())
        .is_some_and(|status| {
            status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
        });
    let playback_rate_permille = info
        .as_ref()
        .and_then(|info| info.PlaybackRate().ok())
        .and_then(|rate| rate.Value().ok())
        .map_or(NORMAL_PLAYBACK_RATE_PERMILLE, rate_permille);
    let capabilities = info.as_ref().and_then(|info| info.Controls().ok()).map_or(
        MediaCapabilities::ALL,
        |controls| MediaCapabilities {
            play_pause: controls.IsPlayPauseToggleEnabled().unwrap_or(false)
                || controls.IsPlayEnabled().unwrap_or(false)
                || controls.IsPauseEnabled().unwrap_or(false),
            next: controls.IsNextEnabled().unwrap_or(false),
            previous: controls.IsPreviousEnabled().unwrap_or(false),
            seek: controls.IsPlaybackPositionEnabled().unwrap_or(false),
        },
    );
    let timeline = session.GetTimelineProperties().ok();
    let ms = |span: windows::core::Result<windows::Foundation::TimeSpan>| {
        span.ok()
            .map_or(0, |value| (value.Duration / 10_000).max(0) as u64)
    };
    let (position_ms, duration_ms, position_updated_unix_ms) =
        timeline.as_ref().map_or((0, 0, None), |timeline| {
            let start = ms(timeline.StartTime());
            let end = ms(timeline.EndTime());
            let position = ms(timeline.Position()).saturating_sub(start);
            let updated = timeline
                .LastUpdatedTime()
                .ok()
                .and_then(|time| unix_ms_from_windows_ticks(time.UniversalTime));
            (position, end.saturating_sub(start), updated)
        });

    let mut media = NowPlaying {
        source_app,
        title,
        artist,
        playing,
        position_ms,
        duration_ms,
        position_updated_unix_ms,
        playback_rate_permille,
        capabilities,
        thumbnail: None,
    };
    let key = media.track_key();
    let now = Instant::now();
    media.thumbnail = if artwork.should_fetch(&key, now) {
        let fetched = props
            .Thumbnail()
            .ok()
            .and_then(|reference| read_thumbnail_bytes(&reference, deadline));
        artwork.record(&key, fetched.clone(), now);
        fetched
    } else {
        artwork.cached(&key)
    };
    Some(media)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandResult {
    Accepted,
    Rejected,
    NoSession,
    TimedOut,
}

fn command_result(session_found: bool, reply: Option<bool>) -> CommandResult {
    match (session_found, reply) {
        (false, _) => CommandResult::NoSession,
        (true, Some(true)) => CommandResult::Accepted,
        (true, Some(false)) => CommandResult::Rejected,
        (true, None) => CommandResult::TimedOut,
    }
}

fn send_media_command_sync(command: MediaCommand, deadline: Instant) -> CommandResult {
    let Some(session) = current_session(deadline) else {
        return command_result(false, None);
    };
    let reply = match command {
        MediaCommand::TogglePlayPause => await_until!(session.TryTogglePlayPauseAsync(), deadline),
        MediaCommand::Next => await_until!(session.TrySkipNextAsync(), deadline),
        MediaCommand::Previous => await_until!(session.TrySkipPreviousAsync(), deadline),
        MediaCommand::Seek(target_ms) => {
            let nanos_100 = i64::try_from(target_ms)
                .unwrap_or(i64::MAX)
                .saturating_mul(10_000);
            await_until!(session.TryChangePlaybackPositionAsync(nanos_100), deadline)
        }
    };
    command_result(true, reply)
}

#[cfg(test)]
mod tests {
    use super::{
        ARTWORK_MAX_ATTEMPTS, ARTWORK_RETRY_BASE, ArtworkCache, CacheRead, CommandResult,
        MEDIA_CACHE_TTL, MEDIA_STALE_AFTER, MediaCache, command_result, rate_permille,
        unix_ms_from_windows_ticks,
    };
    use otto_core::{NowPlaying, TrackKey};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn key(title: &str) -> TrackKey {
        TrackKey {
            source_app: String::from("Spotify.exe"),
            title: String::from(title),
            artist: String::from("Artist"),
        }
    }

    #[test]
    fn artwork_is_reused_for_the_same_track_and_fetched_for_a_new_one() {
        let mut cache = ArtworkCache::default();
        let now = Instant::now();
        assert!(cache.should_fetch(&key("A"), now));
        let bytes: Arc<[u8]> = Arc::from(vec![1u8, 2, 3].into_boxed_slice());
        cache.record(&key("A"), Some(Arc::clone(&bytes)), now);
        assert!(!cache.should_fetch(&key("A"), now));
        assert_eq!(cache.cached(&key("A")), Some(bytes));
        assert!(cache.should_fetch(&key("B"), now));
        assert_eq!(cache.cached(&key("B")), None);
    }

    #[test]
    fn failed_artwork_retries_with_backoff_and_then_gives_up() {
        let mut cache = ArtworkCache::default();
        let start = Instant::now();
        cache.record(&key("A"), None, start);
        assert!(!cache.should_fetch(&key("A"), start + ARTWORK_RETRY_BASE / 2));
        assert!(cache.should_fetch(&key("A"), start + ARTWORK_RETRY_BASE));
        let mut now = start;
        for _ in 1..ARTWORK_MAX_ATTEMPTS {
            now += ARTWORK_RETRY_BASE * 16;
            cache.record(&key("A"), None, now);
        }
        assert!(!cache.should_fetch(&key("A"), now + Duration::from_hours(1)));
        assert!(cache.should_fetch(&key("B"), now));
    }

    #[test]
    fn command_replies_classify_into_accepted_and_failures() {
        assert_eq!(command_result(true, Some(true)), CommandResult::Accepted);
        assert_eq!(command_result(true, Some(false)), CommandResult::Rejected);
        assert_eq!(command_result(true, None), CommandResult::TimedOut);
        assert_eq!(command_result(false, Some(true)), CommandResult::NoSession);
    }

    #[test]
    fn timeline_times_and_rates_convert_to_the_core_units() {
        assert_eq!(
            unix_ms_from_windows_ticks(116_444_736_000_000_000 + 25_000),
            Some(2)
        );
        assert_eq!(unix_ms_from_windows_ticks(0), None);
        assert_eq!(rate_permille(1.0), 1_000);
        assert_eq!(rate_permille(1.5), 1_500);
        assert_eq!(rate_permille(f64::NAN), 1_000);
        assert_eq!(rate_permille(-2.0), 1_000);
    }

    fn track() -> NowPlaying {
        NowPlaying {
            title: String::from("Track"),
            ..NowPlaying::default()
        }
    }

    #[test]
    fn first_read_requests_one_refresh_and_coalesces_the_rest() {
        let mut cache = MediaCache::default();
        let start = Instant::now();
        assert_eq!(
            cache.read(start),
            CacheRead {
                snapshot: None,
                request_refresh: true
            }
        );
        assert!(
            !cache
                .read(start + Duration::from_millis(50))
                .request_refresh
        );
    }

    #[test]
    fn fresh_snapshot_is_served_without_a_new_request_until_it_ages() {
        let mut cache = MediaCache::default();
        let start = Instant::now();
        let _ = cache.read(start);
        cache.store(Some(track()), start);
        let fresh = cache.read(start + MEDIA_CACHE_TTL / 2);
        assert_eq!(fresh.snapshot, Some(track()));
        assert!(!fresh.request_refresh);
        let aged = cache.read(start + MEDIA_CACHE_TTL);
        assert_eq!(aged.snapshot, Some(track()));
        assert!(aged.request_refresh);
    }

    #[test]
    fn stalled_provider_clears_the_snapshot_and_allows_a_retry() {
        let mut cache = MediaCache::default();
        let start = Instant::now();
        cache.store(Some(track()), start);
        let requested = start + MEDIA_CACHE_TTL;
        assert!(cache.read(requested).request_refresh);
        let waiting = cache.read(requested + MEDIA_STALE_AFTER / 2);
        assert_eq!(waiting.snapshot, Some(track()));
        assert!(!waiting.request_refresh);
        let stalled = cache.read(requested + MEDIA_STALE_AFTER);
        assert_eq!(stalled.snapshot, None);
        assert!(stalled.request_refresh);
    }

    #[test]
    fn rejected_request_can_be_retried_immediately() {
        let mut cache = MediaCache::default();
        let start = Instant::now();
        assert!(cache.read(start).request_refresh);
        cache.abandon_request();
        assert!(cache.read(start).request_refresh);
    }
}
