use crate::layout::HitTarget;
use crate::media::{
    MediaCapabilities, MediaCommand, MediaSubtitle, NowPlaying, expanded_media_subtitle,
    unsupported_reason,
};

const SOURCE_LABEL_MAX_CHARS: usize = 16;
const BROWSER_PREFIXES: [&str; 8] = [
    "chrome",
    "msedge",
    "microsoftedge",
    "firefox",
    "brave",
    "opera",
    "vivaldi",
    "iexplore",
];
const BROWSER_EXACT: [&str; 2] = ["edge", "arc"];
const KNOWN_SOURCES: &[(&str, &str)] = &[
    ("applemusic", "Apple Music"),
    ("apple music", "Apple Music"),
    ("itunes", "iTunes"),
    ("youtubemusic", "YouTube Music"),
    ("youtube music", "YouTube Music"),
    ("spotify", "Spotify"),
    ("amazonmusic", "Amazon Music"),
    ("amazon music", "Amazon Music"),
    ("tidal", "TIDAL"),
    ("deezer", "Deezer"),
    ("qobuz", "Qobuz"),
    ("soundcloud", "SoundCloud"),
    ("pandora", "Pandora"),
    ("audible", "Audible"),
    ("pocketcasts", "Pocket Casts"),
    ("plexamp", "Plexamp"),
    ("plex", "Plex"),
    ("jellyfin", "Jellyfin"),
    ("kodi", "Kodi"),
    ("foobar2000", "foobar2000"),
    ("musicbee", "MusicBee"),
    ("mediamonkey", "MediaMonkey"),
    ("winamp", "Winamp"),
    ("aimp", "AIMP"),
    ("potplayer", "PotPlayer"),
    ("mpc-hc", "MPC-HC"),
    ("mpc-be", "MPC-BE"),
    ("vlc", "VLC"),
    ("netflix", "Netflix"),
    ("disneyplus", "Disney+"),
    ("twitch", "Twitch"),
    ("zunevideo", "Movies & TV"),
    ("zunemusic", "Media Player"),
    ("mediaplayer", "Media Player"),
];
pub const LIVE_LABEL: &str = "LIVE";

#[must_use]
pub fn source_label(source_app: &str) -> String {
    let identity = source_app.rsplit('!').next().unwrap_or(source_app).trim();
    let identity = identity
        .strip_suffix(".exe")
        .or_else(|| identity.strip_suffix(".EXE"))
        .unwrap_or(identity);
    let lowered = identity.to_ascii_lowercase();
    let is_browser = BROWSER_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
        || BROWSER_EXACT.contains(&lowered.as_str());
    if is_browser {
        return String::from("Browser");
    }
    let package = source_app.to_ascii_lowercase();
    if let Some((_, label)) = KNOWN_SOURCES.iter().find(|(key, _)| package.contains(key)) {
        return (*label).to_owned();
    }
    let name = identity
        .rsplit(['.', '\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or("");
    let name = name.split('_').next().unwrap_or(name);
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars)
            .take(SOURCE_LABEL_MAX_CHARS)
            .collect(),
        None => String::from("Media"),
    }
}

#[must_use]
pub const fn unsupported_command(
    target: HitTarget,
    capabilities: MediaCapabilities,
) -> Option<MediaCommand> {
    match target {
        HitTarget::MediaPlayPause if !capabilities.play_pause => {
            Some(MediaCommand::TogglePlayPause)
        }
        HitTarget::MediaPrevious if !capabilities.previous => Some(MediaCommand::Previous),
        HitTarget::MediaNext if !capabilities.next => Some(MediaCommand::Next),
        HitTarget::MediaSeekbar if !capabilities.seek => Some(MediaCommand::Seek(0)),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct MediaView {
    pub source: String,
    pub title: String,
    pub subtitle_kind: MediaSubtitle,
    pub subtitle: String,
    pub compact_text: String,
    pub compact_is_lyric: bool,
    pub paused: bool,
    pub live: bool,
    pub seekable: bool,
    pub progress: f32,
    pub elapsed: String,
    pub remaining: String,
}

impl MediaView {
    #[must_use]
    pub fn resolve(
        media: &NowPlaying,
        engaged_unsupported: Option<MediaCommand>,
        lyric_line: Option<&str>,
    ) -> Self {
        let subtitle_kind = expanded_media_subtitle(engaged_unsupported, lyric_line.is_some());
        let paused = !media.playing;
        let subtitle = match (subtitle_kind, lyric_line) {
            (MediaSubtitle::Unsupported(command), _) => unsupported_reason(command).to_owned(),
            (MediaSubtitle::Lyric, Some(line)) => line.to_owned(),
            _ if paused => format!("Paused \u{b7} {}", media.display_artist()),
            _ => media.display_artist().to_owned(),
        };
        let live = !media.has_known_duration();
        Self {
            source: source_label(&media.source_app),
            title: media.title.clone(),
            subtitle_kind,
            subtitle,
            compact_text: lyric_line.map_or_else(|| media.title.clone(), str::to_owned),
            compact_is_lyric: lyric_line.is_some(),
            paused,
            live,
            seekable: media.is_seekable(),
            progress: media.progress_ratio(),
            elapsed: media.elapsed_formatted(),
            remaining: if live {
                String::from(LIVE_LABEL)
            } else {
                media.remaining_formatted()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPeek {
    NowPlaying,
    Paused,
    Playing,
    Skipped,
}

impl MediaPeek {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NowPlaying => "Now playing",
            Self::Paused => "Paused",
            Self::Playing => "Playing",
            Self::Skipped => "Skipped",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaPeekFrame {
    pub label: &'static str,
    pub title: String,
    pub alpha: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaScene {
    pub media: NowPlaying,
    pub lyric_line: Option<String>,
    pub dim: f32,
    pub live_phase: f32,
    pub peek: Option<MediaPeekFrame>,
}

impl MediaScene {
    #[must_use]
    pub fn steady(media: NowPlaying, lyric_line: Option<String>) -> Self {
        let dim = if media.playing { 0.0 } else { 1.0 };
        Self {
            media,
            lyric_line,
            dim,
            live_phase: 0.5,
            peek: None,
        }
    }

    #[must_use]
    pub fn view(&self, engaged_unsupported: Option<MediaCommand>) -> MediaView {
        MediaView::resolve(&self.media, engaged_unsupported, self.lyric_line.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn track() -> NowPlaying {
        NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from("Midnight Static"),
            artist: String::from("Lumen Avenue"),
            playing: true,
            position_ms: 98_000,
            duration_ms: 234_000,
            ..NowPlaying::default()
        }
    }

    #[test]
    fn known_sources_get_friendly_labels() {
        assert_eq!(source_label("Spotify.exe"), "Spotify");
        assert_eq!(source_label("chrome.exe"), "Browser");
        assert_eq!(source_label("MSEdge"), "Browser");
        assert_eq!(source_label("firefox.exe"), "Browser");
        assert_eq!(source_label("vlc.exe"), "VLC");
        assert_eq!(
            source_label("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "Media Player"
        );
    }

    #[test]
    fn popular_music_and_media_apps_get_their_brand_names() {
        let cases = [
            ("AppleMusic.exe", "Apple Music"),
            ("AppleInc.AppleMusicWin_nzyj5cx40ttqa!App", "Apple Music"),
            ("AppleInc.iTunes_nzyj5cx40ttqa!iTunes", "iTunes"),
            ("YouTube Music.exe", "YouTube Music"),
            ("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", "Spotify"),
            ("Amazon Music.exe", "Amazon Music"),
            ("tidal.exe", "TIDAL"),
            ("Deezer.exe", "Deezer"),
            ("Qobuz.exe", "Qobuz"),
            ("Plexamp.exe", "Plexamp"),
            ("Plex.exe", "Plex"),
            ("foobar2000.exe", "foobar2000"),
            ("AIMP.exe", "AIMP"),
            ("winamp.exe", "Winamp"),
            ("PotPlayerMini64.exe", "PotPlayer"),
            (
                "Microsoft.ZuneVideo_8wekyb3d8bbwe!Microsoft.ZuneVideo",
                "Movies & TV",
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(source_label(source), expected, "{source}");
        }
    }

    #[test]
    fn every_known_source_label_fits_the_label_budget() {
        for (key, label) in KNOWN_SOURCES {
            assert!(label.chars().count() <= SOURCE_LABEL_MAX_CHARS, "{label}");
            assert_eq!(*key, key.to_ascii_lowercase(), "{key} must be lowercase");
        }
    }

    #[test]
    fn unknown_sources_are_cleaned_and_capped() {
        assert_eq!(source_label("soundbox.exe"), "Soundbox");
        assert_eq!(source_label("Search.exe"), "Search");
        assert_eq!(source_label("arc.exe"), "Browser");
        assert_eq!(source_label("Acme.SoundBox_abc!App"), "App");
        assert_eq!(source_label(""), "Media");
        assert_eq!(source_label("  "), "Media");
        assert!(
            source_label("averyveryverylongapplicationname.exe")
                .chars()
                .count()
                <= 16
        );
    }

    #[test]
    fn a_playing_track_shows_its_artist_and_remaining_time() {
        let view = MediaView::resolve(&track(), None, None);
        assert_eq!(view.source, "Spotify");
        assert_eq!(view.subtitle_kind, MediaSubtitle::Artist);
        assert_eq!(view.subtitle, "Lumen Avenue");
        assert_eq!(view.compact_text, "Midnight Static");
        assert!(!view.paused && !view.live && view.seekable);
        assert_eq!(view.elapsed, "1:38");
        assert_eq!(view.remaining, "-2:16");
        assert!((view.progress - 98.0 / 234.0).abs() < 0.001);
    }

    #[test]
    fn a_paused_track_names_the_pause_in_the_subtitle() {
        let paused = NowPlaying {
            playing: false,
            ..track()
        };
        let view = MediaView::resolve(&paused, None, None);
        assert!(view.paused);
        assert_eq!(view.subtitle, "Paused \u{b7} Lumen Avenue");
    }

    #[test]
    fn a_missing_artist_reads_as_unknown() {
        let anonymous = NowPlaying {
            artist: String::new(),
            ..track()
        };
        assert_eq!(
            MediaView::resolve(&anonymous, None, None).subtitle,
            "Unknown Artist"
        );
    }

    #[test]
    fn unsupported_reason_beats_lyric_beats_artist() {
        let media = track();
        let reason = MediaView::resolve(&media, Some(MediaCommand::Next), Some("a line"));
        assert_eq!(
            reason.subtitle,
            "This source doesn't support the next track"
        );
        let lyric = MediaView::resolve(&media, None, Some("And the radio hums"));
        assert_eq!(lyric.subtitle_kind, MediaSubtitle::Lyric);
        assert_eq!(lyric.subtitle, "And the radio hums");
        assert!(lyric.compact_is_lyric);
        assert_eq!(lyric.compact_text, "And the radio hums");
        let paused_lyric = MediaView::resolve(
            &NowPlaying {
                playing: false,
                ..media
            },
            None,
            Some("a line"),
        );
        assert_eq!(paused_lyric.subtitle, "a line");
    }

    #[test]
    fn an_unknown_length_is_live_and_not_seekable() {
        let stream = NowPlaying {
            duration_ms: 0,
            position_ms: 760_000,
            ..track()
        };
        let view = MediaView::resolve(&stream, None, None);
        assert!(view.live && !view.seekable);
        assert_eq!(view.remaining, "LIVE");
        assert!(view.progress.abs() < f32::EPSILON);
    }

    #[test]
    fn unsupported_command_maps_each_control_to_its_capability() {
        let none = MediaCapabilities {
            play_pause: false,
            next: false,
            previous: false,
            seek: false,
        };
        assert_eq!(
            unsupported_command(HitTarget::MediaNext, none),
            Some(MediaCommand::Next)
        );
        assert_eq!(
            unsupported_command(HitTarget::MediaSeekbar, none),
            Some(MediaCommand::Seek(0))
        );
        assert_eq!(
            unsupported_command(HitTarget::MediaNext, MediaCapabilities::ALL),
            None
        );
        assert_eq!(unsupported_command(HitTarget::Notch, none), None);
    }

    #[test]
    fn peeks_have_short_labels() {
        assert_eq!(MediaPeek::NowPlaying.label(), "Now playing");
        assert_eq!(MediaPeek::Skipped.label(), "Skipped");
    }

    #[test]
    fn a_steady_scene_dims_only_when_the_track_is_not_playing() {
        let playing = MediaScene::steady(track(), None);
        assert!(playing.dim.abs() < f32::EPSILON && playing.peek.is_none());
        let paused = MediaScene::steady(
            NowPlaying {
                playing: false,
                thumbnail: Some(Arc::from(vec![1u8, 2, 3])),
                ..track()
            },
            Some(String::from("line")),
        );
        assert!((paused.dim - 1.0).abs() < f32::EPSILON);
        assert!(paused.view(None).paused);
        assert_eq!(paused.view(None).subtitle, "line");
    }
}
