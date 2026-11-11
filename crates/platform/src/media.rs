use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
    let mut last_track = (String::new(), String::new());
    let mut last_thumbnail: Option<Arc<[u8]>> = None;
    for task in receiver {
        let deadline = Instant::now() + MEDIA_OPERATION_TIMEOUT;
        match task {
            MediaTask::Refresh => {
                let snapshot = com.as_ref().and_then(|_| {
                    read_now_playing_sync(&mut last_track, &mut last_thumbnail, deadline)
                });
                if let Ok(mut state) = cache.lock() {
                    state.store(snapshot, Instant::now());
                }
            }
            MediaTask::Command(command) => {
                if com.is_some() {
                    send_media_command_sync(command, deadline);
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

fn read_now_playing_sync(
    last_track: &mut (String, String),
    last_thumbnail: &mut Option<Arc<[u8]>>,
    deadline: Instant,
) -> Option<NowPlaying> {
    let session = current_session(deadline)?;
    let props = await_until!(session.TryGetMediaPropertiesAsync(), deadline)?;
    let title = props.Title().ok()?.to_string();
    if title.trim().is_empty() {
        return None;
    }
    let artist = props.Artist().map(|a| a.to_string()).unwrap_or_default();
    let playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .is_ok_and(|status| {
            status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
        });
    let (position_ms, duration_ms) = session.GetTimelineProperties().ok().map_or((0, 0), |tl| {
        let pos = tl
            .Position()
            .ok()
            .map_or(0, |p| (p.Duration / 10_000).max(0) as u64);
        let end = tl
            .EndTime()
            .ok()
            .map_or(0, |e| (e.Duration / 10_000).max(0) as u64);
        (pos, end)
    });

    let thumbnail = if last_track.0 == title && last_track.1 == artist && last_thumbnail.is_some() {
        last_thumbnail.clone()
    } else {
        let thumb = props
            .Thumbnail()
            .ok()
            .and_then(|reference| read_thumbnail_bytes(&reference, deadline));
        *last_track = (title.clone(), artist.clone());
        last_thumbnail.clone_from(&thumb);
        thumb
    };

    Some(NowPlaying {
        title,
        artist,
        playing,
        position_ms,
        duration_ms,
        thumbnail,
    })
}

fn send_media_command_sync(command: MediaCommand, deadline: Instant) {
    let Some(session) = current_session(deadline) else {
        return;
    };
    let _ = match command {
        MediaCommand::TogglePlayPause => session.TryTogglePlayPauseAsync(),
        MediaCommand::Next => session.TrySkipNextAsync(),
        MediaCommand::Previous => session.TrySkipPreviousAsync(),
        MediaCommand::Seek(target_ms) => {
            let nanos_100 = i64::try_from(target_ms)
                .unwrap_or(i64::MAX)
                .saturating_mul(10_000);
            session.TryChangePlaybackPositionAsync(nanos_100)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{CacheRead, MEDIA_CACHE_TTL, MEDIA_STALE_AFTER, MediaCache};
    use otto_core::NowPlaying;
    use std::time::{Duration, Instant};

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
