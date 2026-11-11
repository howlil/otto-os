use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{LyricsSheet, parse_lrc};
use windows::Web::Http::{HttpClient, HttpCompletionOption};
use windows::core::HSTRING;

use crate::winrt_io::{await_until, read_stream_bounded};

pub const MAX_LYRICS_BYTES: usize = 256 * 1024;
pub const LYRICS_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
pub const LYRICS_QUEUE_DEPTH: usize = 2;
pub const LYRICS_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);

fn cache_dir() -> Option<PathBuf> {
    let mut path = crate::file::data_dir()?;
    path.push("lyrics_cache");
    let _ = std::fs::create_dir_all(&path);
    Some(path)
}

fn lyrics_cache_key(title: &str, artist: &str) -> String {
    let mut hash = 14_695_981_039_346_656_037u64;
    for byte in artist
        .trim()
        .to_lowercase()
        .bytes()
        .chain([0])
        .chain(title.trim().to_lowercase().bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    format!("lyrics-{hash:016x}.lrc")
}

fn cache_path(title: &str, artist: &str) -> Option<PathBuf> {
    let mut path = cache_dir()?;
    path.push(lyrics_cache_key(title, artist));
    Some(path)
}

fn load_cached_lyrics(title: &str, artist: &str) -> Option<String> {
    let path = cache_path(title, artist)?;
    if std::fs::metadata(&path).ok()?.len() > MAX_LYRICS_BYTES as u64 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn save_cached_lyrics(title: &str, artist: &str, lrc: &str) {
    if lrc.trim().is_empty() || lrc.len() > MAX_LYRICS_BYTES {
        return;
    }
    if let Some(path) = cache_path(title, artist) {
        let _ = crate::file::replace_file_atomically(&path, lrc.as_bytes());
    }
}

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

fn parse_json_quoted_string(after: &str) -> Option<String> {
    if !after.starts_with('"') {
        return None;
    }
    let mut result = String::new();
    let mut chars = after[1..].chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,
            '\\' => match chars.next()? {
                '"' => result.push('"'),
                '\\' => result.push('\\'),
                '/' => result.push('/'),
                'b' => result.push('\x08'),
                'f' => result.push('\x0c'),
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?);
                    }
                    if let Ok(code) = u32::from_str_radix(&hex, 16)
                        && let Some(c) = char::from_u32(code)
                    {
                        result.push(c);
                    }
                }
                other => {
                    result.push('\\');
                    result.push(other);
                }
            },
            other => result.push(other),
        }
    }
    if result.trim().is_empty() {
        None
    } else {
        Some(result)
    }
}

fn extract_synced_lyrics_from_json(json: &str) -> Option<String> {
    let marker = "\"syncedLyrics\":";
    let mut search_from = 0;
    while let Some(rel_pos) = json[search_from..].find(marker) {
        let pos = search_from + rel_pos;
        let after = json[pos + marker.len()..].trim_start();
        if after.starts_with('"')
            && let Some(parsed) = parse_json_quoted_string(after)
        {
            return Some(parsed);
        }
        search_from = pos + marker.len();
    }
    None
}

#[must_use]
pub fn clean_track_title(title: &str) -> String {
    let mut cleaned = title.trim();
    if let Some(pos) = cleaned.find(" - ") {
        let prefix = &cleaned[..pos];
        if !prefix.trim().is_empty() {
            cleaned = prefix.trim();
        }
    }
    let mut result = String::new();
    let mut skip_depth = 0;
    for ch in cleaned.chars() {
        match ch {
            '(' | '[' => skip_depth += 1,
            ')' | ']' => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                }
            }
            _ if skip_depth == 0 => result.push(ch),
            _ => {}
        }
    }
    let trimmed = result.trim();
    if trimmed.is_empty() {
        title.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn request_url(
    client: &HttpClient,
    url: &str,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Option<String> {
    if cancelled() || Instant::now() >= deadline {
        return None;
    }
    let uri = windows::Foundation::Uri::CreateUri(&HSTRING::from(url)).ok()?;
    let response = await_until!(
        client.GetWithOptionAsync(&uri, HttpCompletionOption::ResponseHeadersRead),
        deadline
    )?;
    if !response.IsSuccessStatusCode().ok()? {
        return None;
    }
    let content = response.Content().ok()?;
    let mut declared_length = 0u64;
    if matches!(content.TryComputeLength(&mut declared_length), Ok(true))
        && declared_length > MAX_LYRICS_BYTES as u64
    {
        return None;
    }
    let stream = await_until!(content.ReadAsInputStreamAsync(), deadline)?;
    let body = read_stream_bounded(&stream, MAX_LYRICS_BYTES, deadline, cancelled)?;
    String::from_utf8(body).ok()
}

fn lookup_urls(title: &str, artist: &str, duration_ms: u64) -> Vec<String> {
    let clean_title = clean_track_title(title);
    let track = url_encode(&clean_title);
    let artist_name = url_encode(artist);
    let seconds = duration_ms / 1000;
    let mut urls = Vec::with_capacity(4);
    if seconds > 0 {
        urls.push(format!(
            "https://lrclib.net/api/get?track_name={track}&artist_name={artist_name}&duration={seconds}"
        ));
    }
    urls.push(format!(
        "https://lrclib.net/api/get?track_name={track}&artist_name={artist_name}"
    ));
    urls.push(format!(
        "https://lrclib.net/api/search?track_name={track}&artist_name={artist_name}"
    ));
    if clean_title != title {
        urls.push(format!(
            "https://lrclib.net/api/search?q={}",
            url_encode(&format!("{title} {artist}"))
        ));
    }
    urls
}

fn fetch_lyrics_online(
    title: &str,
    artist: &str,
    duration_ms: u64,
    cancelled: &dyn Fn() -> bool,
) -> LyricsLookup {
    if title.trim().is_empty() {
        return LyricsLookup::NotFound;
    }
    let Ok(client) = HttpClient::new() else {
        return LyricsLookup::Failed;
    };
    if let Ok(headers) = client.DefaultRequestHeaders() {
        let _ = headers.TryAppendWithoutValidation(
            &HSTRING::from("User-Agent"),
            &HSTRING::from("OttoOS/0.1.0 (https://github.com/otto-os)"),
        );
    }
    let deadline = Instant::now() + LYRICS_REQUEST_TIMEOUT;
    let mut answered = false;
    for url in lookup_urls(title, artist, duration_ms) {
        let Some(body) = request_url(&client, &url, deadline, cancelled) else {
            continue;
        };
        answered = true;
        if let Some(lyrics) = extract_synced_lyrics_from_json(&body) {
            return LyricsLookup::Found {
                lrc: lyrics,
                source: LyricsSource::Network,
            };
        }
    }
    if answered {
        LyricsLookup::NotFound
    } else {
        LyricsLookup::Failed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsSource {
    Cache,
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LyricsLookup {
    Found { lrc: String, source: LyricsSource },
    NotFound,
    Failed,
}

fn lookup_lyrics(
    request: &LyricsRequest,
    online: bool,
    cancelled: &dyn Fn() -> bool,
) -> LyricsLookup {
    if let Some(cached) = load_cached_lyrics(&request.title, &request.artist)
        && !cached.trim().is_empty()
    {
        return LyricsLookup::Found {
            lrc: cached,
            source: LyricsSource::Cache,
        };
    }
    if !online {
        return LyricsLookup::NotFound;
    }
    let found = fetch_lyrics_online(
        &request.title,
        &request.artist,
        request.duration_ms,
        cancelled,
    );
    if let LyricsLookup::Found { lrc, .. } = &found {
        save_cached_lyrics(&request.title, &request.artist, lrc);
    }
    found
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsRequest {
    pub generation: u64,
    pub title: String,
    pub artist: String,
    pub duration_ms: u64,
    pub allow_online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsOutcome {
    Found {
        sheet: LyricsSheet,
        source: LyricsSource,
    },
    NotFound,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsResponse {
    pub generation: u64,
    pub title: String,
    pub artist: String,
    pub outcome: LyricsOutcome,
}

struct LyricsShared {
    latest_generation: AtomicU64,
    stopping: AtomicBool,
}

impl LyricsShared {
    fn is_superseded(&self, generation: u64) -> bool {
        self.stopping.load(Ordering::Acquire)
            || self.latest_generation.load(Ordering::Acquire) != generation
    }
}

pub struct LyricsService {
    requests: Option<SyncSender<LyricsRequest>>,
    responses: Receiver<LyricsResponse>,
    shared: Arc<LyricsShared>,
    worker: Option<JoinHandle<()>>,
}

impl LyricsService {
    #[must_use]
    pub fn start() -> Self {
        let (requests, request_receiver) = sync_channel(LYRICS_QUEUE_DEPTH);
        let (response_sender, responses) = sync_channel(LYRICS_QUEUE_DEPTH);
        let shared = Arc::new(LyricsShared {
            latest_generation: AtomicU64::new(0),
            stopping: AtomicBool::new(false),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name(String::from("otto-lyrics"))
            .spawn(move || lyrics_worker(&request_receiver, &response_sender, &worker_shared))
            .ok();
        Self {
            requests: Some(requests),
            responses,
            shared,
            worker,
        }
    }

    #[must_use]
    pub fn request(&self, request: LyricsRequest) -> bool {
        let Some(requests) = self.requests.as_ref() else {
            return false;
        };
        self.shared
            .latest_generation
            .store(request.generation, Ordering::Release);
        requests.try_send(request).is_ok()
    }

    #[must_use]
    pub fn poll(&self) -> Option<LyricsResponse> {
        self.responses.try_recv().ok()
    }

    pub fn shutdown(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        self.requests = None;
        if let Some(worker) = self.worker.take() {
            let _ = crate::worker::join_within(worker, LYRICS_SHUTDOWN_TIMEOUT);
        }
    }
}

impl Drop for LyricsService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn lyrics_worker(
    requests: &Receiver<LyricsRequest>,
    responses: &SyncSender<LyricsResponse>,
    shared: &LyricsShared,
) {
    let com = crate::com::ComGuard::multithreaded();
    while let Ok(request) = requests.recv() {
        if shared.is_superseded(request.generation) {
            continue;
        }
        let generation = request.generation;
        let cancelled = || shared.is_superseded(generation);
        let online = request.allow_online && com.is_some();
        let outcome = match lookup_lyrics(&request, online, &cancelled) {
            LyricsLookup::Found { lrc, source } => LyricsOutcome::Found {
                sheet: parse_lrc(&lrc),
                source,
            },
            LyricsLookup::NotFound => LyricsOutcome::NotFound,
            LyricsLookup::Failed => LyricsOutcome::Failed,
        };
        if cancelled() {
            continue;
        }
        let _ = responses.try_send(LyricsResponse {
            generation,
            title: request.title,
            artist: request.artist,
            outcome,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(generation: u64) -> LyricsRequest {
        LyricsRequest {
            generation,
            title: format!("otto-test-track-{generation}-{}", std::process::id()),
            artist: String::from("otto-test-artist"),
            duration_ms: 0,
            allow_online: false,
        }
    }

    #[test]
    fn lookup_urls_start_with_the_most_specific_query() {
        let urls = lookup_urls("Song (Live)", "A & B", 61_000);
        assert_eq!(urls.len(), 4);
        assert!(urls[0].contains("duration=61"));
        assert!(urls[0].contains("track_name=Song&artist_name=A+%26+B"));
        assert!(urls[3].contains("/api/search?q=Song+%28Live%29+A+%26+B"));
        assert_eq!(lookup_urls("Song", "Artist", 0).len(), 2);
    }

    #[test]
    fn offline_lookup_reports_not_found_without_touching_the_network() {
        let never = || false;
        assert_eq!(
            lookup_lyrics(&request(1), false, &never),
            LyricsLookup::NotFound
        );
    }

    #[test]
    fn cancelled_online_lookup_fails_before_any_request_is_sent() {
        let always = || true;
        assert_eq!(
            fetch_lyrics_online("Song", "Artist", 0, &always),
            LyricsLookup::Failed
        );
    }

    #[test]
    fn service_answers_the_latest_request_and_skips_superseded_ones() {
        let mut service = LyricsService::start();
        assert!(service.request(request(1)));
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut answered = None;
        while answered.is_none() && Instant::now() < deadline {
            answered = service.poll();
            thread::sleep(Duration::from_millis(5));
        }
        let response = answered.expect("cache-only lookup must answer");
        assert_eq!(response.generation, 1);
        assert_eq!(response.outcome, LyricsOutcome::NotFound);

        let shared = Arc::clone(&service.shared);
        shared.latest_generation.store(9, Ordering::Release);
        assert!(shared.is_superseded(2));
        assert!(!shared.is_superseded(9));
        service.shutdown();
        assert!(shared.is_superseded(9));
        assert!(!service.request(request(10)));
    }

    #[test]
    fn request_queue_is_bounded() {
        let (sender, _receiver) = sync_channel::<LyricsRequest>(LYRICS_QUEUE_DEPTH);
        for generation in 0..LYRICS_QUEUE_DEPTH as u64 {
            assert!(sender.try_send(request(generation)).is_ok());
        }
        assert!(sender.try_send(request(99)).is_err());
    }

    #[test]
    fn url_encode_handles_special_characters() {
        assert_eq!(url_encode("Hello World"), "Hello+World");
        assert_eq!(url_encode("Song & Dance?"), "Song+%26+Dance%3F");
        assert_eq!(url_encode("Taylor Swift"), "Taylor+Swift");
    }

    #[test]
    fn extracts_synced_lyrics_from_json_string() {
        let json = r#"{"id":42,"trackName":"Song","artistName":"Band","syncedLyrics":"[00:10.00] Line one\n[00:15.50] Line two","plainLyrics":"Line one\nLine two"}"#;
        let lrc = extract_synced_lyrics_from_json(json).unwrap();
        assert_eq!(lrc, "[00:10.00] Line one\n[00:15.50] Line two");
    }

    #[test]
    fn returns_none_when_synced_lyrics_is_null_or_missing() {
        let json_null = r#"{"id":42,"trackName":"Song","syncedLyrics":null}"#;
        assert_eq!(extract_synced_lyrics_from_json(json_null), None);

        let json_empty = r#"{"id":42,"trackName":"Song","syncedLyrics":""}"#;
        assert_eq!(extract_synced_lyrics_from_json(json_empty), None);
    }

    #[test]
    fn clean_track_title_strips_metadata_suffixes_and_parentheticals() {
        assert_eq!(clean_track_title("Yellow"), "Yellow");
        assert_eq!(clean_track_title("Something (feat. Drake)"), "Something");
        assert_eq!(
            clean_track_title("Fix You - Live from Buenos Aires"),
            "Fix You"
        );
        assert_eq!(
            clean_track_title("Bohemian Rhapsody - 2011 Mix"),
            "Bohemian Rhapsody"
        );
        assert_eq!(clean_track_title("Stay [with Justin Bieber]"), "Stay");
    }

    #[test]
    fn cache_key_separates_titles_that_would_collide_after_sanitizing() {
        assert_ne!(lyrics_cache_key("AB", "C"), lyrics_cache_key("A", "BC"));
    }

    #[test]
    fn extracts_synced_lyrics_from_search_array_skipping_null() {
        let array_json =
            r#"[{"id":1,"syncedLyrics":null},{"id":2,"syncedLyrics":"[00:05.00] Hello world"}]"#;
        let lrc = extract_synced_lyrics_from_json(array_json).unwrap();
        assert_eq!(lrc, "[00:05.00] Hello world");
    }
}
