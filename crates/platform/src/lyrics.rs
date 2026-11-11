use std::path::PathBuf;

fn cache_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let mut path = PathBuf::from(local);
    path.push("otto-os");
    path.push("lyrics_cache");
    let _ = std::fs::create_dir_all(&path);
    Some(path)
}

fn lyrics_cache_key(title: &str, artist: &str) -> String {
    let clean_title: String = title.chars().filter(|c| c.is_alphanumeric()).collect();
    let clean_artist: String = artist.chars().filter(|c| c.is_alphanumeric()).collect();
    format!(
        "{}_{}.lrc",
        clean_artist.to_lowercase(),
        clean_title.to_lowercase()
    )
}

#[must_use]
pub fn load_cached_lyrics(title: &str, artist: &str) -> Option<String> {
    let mut path = cache_dir()?;
    path.push(lyrics_cache_key(title, artist));
    std::fs::read_to_string(path).ok()
}

pub fn save_cached_lyrics(title: &str, artist: &str, lrc: &str) {
    if let Some(mut path) = cache_dir() {
        path.push(lyrics_cache_key(title, artist));
        let _ = std::fs::write(path, lrc);
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

pub fn extract_synced_lyrics_from_json(json: &str) -> Option<String> {
    let marker = "\"syncedLyrics\":";
    let pos = json.find(marker)?;
    let after = json[pos + marker.len()..].trim_start();
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

#[must_use]
pub fn fetch_lyrics_online(title: &str, artist: &str, duration_ms: u64) -> Option<String> {
    if title.trim().is_empty() {
        return None;
    }

    let dur_sec = duration_ms / 1000;
    let url = if dur_sec > 0 {
        format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}&duration={}",
            url_encode(title),
            url_encode(artist),
            dur_sec
        )
    } else {
        format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            url_encode(title),
            url_encode(artist)
        )
    };

    let client = windows::Web::Http::HttpClient::new().ok()?;
    let uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(&url)).ok()?;
    let response = client.GetAsync(&uri).ok()?.join().ok()?;
    if !response.IsSuccessStatusCode().ok()? {
        return None;
    }
    let body_hstring = response
        .Content()
        .ok()?
        .ReadAsStringAsync()
        .ok()?
        .join()
        .ok()?;
    let body_json = body_hstring.to_string();

    extract_synced_lyrics_from_json(&body_json)
}

#[must_use]
pub fn get_or_fetch_lyrics(title: &str, artist: &str, duration_ms: u64) -> Option<String> {
    if let Some(cached) = load_cached_lyrics(title, artist)
        && !cached.trim().is_empty()
    {
        return Some(cached);
    }
    if let Some(fetched) = fetch_lyrics_online(title, artist, duration_ms) {
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
}
