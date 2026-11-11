use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

const MAX_LYRICS_BYTES: usize = 256 * 1024;
const LYRICS_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

fn cache_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let mut path = PathBuf::from(local);
    path.push("otto-os");
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

#[must_use]
pub fn load_cached_lyrics(title: &str, artist: &str) -> Option<String> {
    let path = cache_path(title, artist)?;
    if std::fs::metadata(&path).ok()?.len() > MAX_LYRICS_BYTES as u64 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub fn save_cached_lyrics(title: &str, artist: &str, lrc: &str) {
    if lrc.trim().is_empty() || lrc.len() > MAX_LYRICS_BYTES {
        return;
    }
    if let Some(path) = cache_path(title, artist) {
        let _ = crate::file::replace_file_atomically(&path, lrc.as_bytes());
    }
}

pub fn url_encode(input: &str) -> String {
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

pub fn extract_synced_lyrics_from_json(json: &str) -> Option<String> {
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
    client: &windows::Web::Http::HttpClient,
    url: &str,
    deadline: Instant,
) -> Option<String> {
    let uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url)).ok()?;
    let request = client.GetAsync(&uri).ok()?;
    let request_cancel = request.clone();
    let (request_sender, request_receiver) = sync_channel(1);
    request
        .when(move |result| {
            let _ = request_sender.send(result);
        })
        .ok()?;
    let Ok(Ok(response)) =
        request_receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    else {
        let _ = request_cancel.Cancel();
        return None;
    };
    if !response.IsSuccessStatusCode().ok()? {
        return None;
    }
    let content = response.Content().ok()?;
    let mut content_length = 0u64;
    if !matches!(content.TryComputeLength(&mut content_length), Ok(true))
        || content_length > MAX_LYRICS_BYTES as u64
    {
        return None;
    }
    let body = content.ReadAsStringAsync().ok()?;
    let body_cancel = body.clone();
    let (body_sender, body_receiver) = sync_channel(1);
    body.when(move |result| {
        let _ = body_sender.send(result);
    })
    .ok()?;
    let Ok(Ok(body)) =
        body_receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    else {
        let _ = body_cancel.Cancel();
        return None;
    };
    let body = body.to_string();
    (body.len() <= MAX_LYRICS_BYTES).then_some(body)
}

#[must_use]
pub fn fetch_lyrics_online(title: &str, artist: &str, duration_ms: u64) -> Option<String> {
    if title.trim().is_empty() {
        return None;
    }

    let client = windows::Web::Http::HttpClient::new().ok()?;
    if let Ok(headers) = client.DefaultRequestHeaders() {
        let _ = headers.TryAppendWithoutValidation(
            &windows::core::HSTRING::from("User-Agent"),
            &windows::core::HSTRING::from("OttoOS/0.1.0 (https://github.com/otto-os)"),
        );
    }

    let clean_title = clean_track_title(title);
    let dur_sec = duration_ms / 1000;
    let deadline = Instant::now() + LYRICS_REQUEST_TIMEOUT;

    if dur_sec > 0 {
        let url = format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}&duration={}",
            url_encode(&clean_title),
            url_encode(artist),
            dur_sec
        );
        if let Some(body) = request_url(&client, &url, deadline)
            && let Some(lyrics) = extract_synced_lyrics_from_json(&body)
        {
            return Some(lyrics);
        }
    }

    let url_no_dur = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}",
        url_encode(&clean_title),
        url_encode(artist)
    );
    if let Some(body) = request_url(&client, &url_no_dur, deadline)
        && let Some(lyrics) = extract_synced_lyrics_from_json(&body)
    {
        return Some(lyrics);
    }

    let url_search = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        url_encode(&clean_title),
        url_encode(artist)
    );
    if let Some(body) = request_url(&client, &url_search, deadline)
        && let Some(lyrics) = extract_synced_lyrics_from_json(&body)
    {
        return Some(lyrics);
    }

    if clean_title != title {
        let raw_query = format!("{title} {artist}");
        let url_raw_search = format!("https://lrclib.net/api/search?q={}", url_encode(&raw_query));
        if let Some(body) = request_url(&client, &url_raw_search, deadline)
            && let Some(lyrics) = extract_synced_lyrics_from_json(&body)
        {
            return Some(lyrics);
        }
    }

    None
}

#[must_use]
pub fn get_or_fetch_lyrics(
    title: &str,
    artist: &str,
    duration_ms: u64,
    allow_online: bool,
) -> Option<String> {
    if let Some(cached) = load_cached_lyrics(title, artist)
        && !cached.trim().is_empty()
    {
        return Some(cached);
    }
    if allow_online && let Some(fetched) = fetch_lyrics_online(title, artist, duration_ms) {
        save_cached_lyrics(title, artist, &fetched);
        return Some(fetched);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
