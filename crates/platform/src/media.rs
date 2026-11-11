use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use otto_core::{MediaCommand, NowPlaying};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

const MEDIA_CACHE_TTL: Duration = Duration::from_millis(250);

enum MediaTask {
    Refresh,
    Command(MediaCommand),
}

#[derive(Default)]
struct MediaCache {
    snapshot: Option<NowPlaying>,
    refreshed_at: Option<Instant>,
    refresh_pending: bool,
}

struct MediaRuntime {
    sender: mpsc::Sender<MediaTask>,
    cache: Arc<Mutex<MediaCache>>,
    handle: JoinHandle<()>,
}

static MEDIA_RUNTIME: OnceLock<Mutex<Option<MediaRuntime>>> = OnceLock::new();

fn runtime() -> Option<(mpsc::Sender<MediaTask>, Arc<Mutex<MediaCache>>)> {
    let runtime = MEDIA_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut guard = runtime.lock().ok()?;
    if guard.is_none() {
        let (sender, receiver) = mpsc::channel();
        let cache = Arc::new(Mutex::new(MediaCache::default()));
        let worker_cache = Arc::clone(&cache);
        let handle = thread::spawn(move || media_worker(receiver, worker_cache.as_ref()));
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
    let mut snapshot = None;
    let mut request_refresh = false;
    if let Ok(mut state) = cache.lock() {
        snapshot.clone_from(&state.snapshot);
        let stale = state
            .refreshed_at
            .is_none_or(|refreshed_at| refreshed_at.elapsed() >= MEDIA_CACHE_TTL);
        if stale && !state.refresh_pending {
            state.refresh_pending = true;
            request_refresh = true;
        }
    }
    if request_refresh
        && sender.send(MediaTask::Refresh).is_err()
        && let Ok(mut state) = cache.lock()
    {
        state.refresh_pending = false;
    }
    snapshot
}

pub fn send_media_command(command: MediaCommand) {
    if let Some((sender, _)) = runtime() {
        let _ = sender.send(MediaTask::Command(command));
    }
}

pub fn shutdown_media_task() {
    let Some(runtime) = MEDIA_RUNTIME
        .get()
        .and_then(|cell| cell.lock().ok().and_then(|mut guard| guard.take()))
    else {
        return;
    };
    drop(runtime.sender);
    let _ = runtime.handle.join();
}

fn media_worker(receiver: mpsc::Receiver<MediaTask>, cache: &Mutex<MediaCache>) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
    let mut last_track = (String::new(), String::new());
    let mut last_thumbnail: Option<Arc<[u8]>> = None;
    for task in receiver {
        match task {
            MediaTask::Refresh => {
                let snapshot = if initialized {
                    read_now_playing_sync(&mut last_track, &mut last_thumbnail)
                } else {
                    None
                };
                if let Ok(mut state) = cache.lock() {
                    state.snapshot = snapshot;
                    state.refreshed_at = Some(Instant::now());
                    state.refresh_pending = false;
                }
            }
            MediaTask::Command(command) => {
                if initialized {
                    send_media_command_sync(command);
                }
            }
        }
    }
    if initialized {
        unsafe { CoUninitialize() };
    }
}

fn current_session() -> Option<GlobalSystemMediaTransportControlsSession> {
    GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .ok()?
        .join()
        .ok()?
        .GetCurrentSession()
        .ok()
}

fn read_thumbnail_bytes(
    stream_ref: &windows::Storage::Streams::IRandomAccessStreamReference,
) -> Option<Arc<[u8]>> {
    let stream = stream_ref.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 10 * 1024 * 1024 {
        return None;
    }
    let reader = windows::Storage::Streams::DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size).ok()?.join().ok()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(Arc::from(bytes.into_boxed_slice()))
}

fn read_now_playing_sync(
    last_track: &mut (String, String),
    last_thumbnail: &mut Option<Arc<[u8]>>,
) -> Option<NowPlaying> {
    let session = current_session()?;
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
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

    let thumbnail = if last_track.0 == title && last_track.1 == artist {
        last_thumbnail.clone()
    } else {
        let thumb = props
            .Thumbnail()
            .ok()
            .and_then(|t| read_thumbnail_bytes(&t));
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

fn send_media_command_sync(command: MediaCommand) {
    let Some(session) = current_session() else {
        return;
    };
    let _ = match command {
        MediaCommand::TogglePlayPause => session.TryTogglePlayPauseAsync(),
        MediaCommand::Next => session.TrySkipNextAsync(),
        MediaCommand::Previous => session.TrySkipPreviousAsync(),
        MediaCommand::Seek(target_ms) => {
            let nanos_100 = (target_ms as i64).saturating_mul(10_000);
            session.TryChangePlaybackPositionAsync(nanos_100)
        }
    };
}
