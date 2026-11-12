pub const NORMAL_PLAYBACK_RATE_PERMILLE: u32 = 1_000;
const MAX_PLAYBACK_RATE_PERMILLE: u32 = 16_000;
pub const POSITION_DRIFT_TOLERANCE_MS: u64 = 750;
pub const KEYBOARD_SEEK_STEP_MS: i64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct MediaCapabilities {
    pub play_pause: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
}

impl MediaCapabilities {
    pub const ALL: Self = Self {
        play_pause: true,
        next: true,
        previous: true,
        seek: true,
    };

    #[must_use]
    pub const fn allows(self, command: MediaCommand) -> bool {
        match command {
            MediaCommand::TogglePlayPause => self.play_pause,
            MediaCommand::Next => self.next,
            MediaCommand::Previous => self.previous,
            MediaCommand::Seek(_) => self.seek,
        }
    }
}

#[must_use]
pub const fn unsupported_reason(command: MediaCommand) -> &'static str {
    match command {
        MediaCommand::TogglePlayPause => "Sumber ini tidak bisa diputar atau dijeda",
        MediaCommand::Next => "Sumber ini tidak mendukung lagu berikutnya",
        MediaCommand::Previous => "Sumber ini tidak mendukung lagu sebelumnya",
        MediaCommand::Seek(_) => "Sumber ini tidak mendukung pencarian posisi",
    }
}

impl Default for MediaCapabilities {
    fn default() -> Self {
        Self::ALL
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackKey {
    pub source_app: String,
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    pub source_app: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub position_updated_unix_ms: Option<i64>,
    pub playback_rate_permille: u32,
    pub capabilities: MediaCapabilities,
    pub thumbnail: Option<std::sync::Arc<[u8]>>,
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self {
            source_app: String::new(),
            title: String::new(),
            artist: String::new(),
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            position_updated_unix_ms: None,
            playback_rate_permille: NORMAL_PLAYBACK_RATE_PERMILLE,
            capabilities: MediaCapabilities::ALL,
            thumbnail: None,
        }
    }
}

impl NowPlaying {
    #[must_use]
    pub fn track_key(&self) -> TrackKey {
        TrackKey {
            source_app: self.source_app.clone(),
            title: self.title.clone(),
            artist: self.artist.clone(),
        }
    }

    #[must_use]
    pub fn is_same_track(&self, other: &Self) -> bool {
        self.source_app == other.source_app
            && self.title == other.title
            && self.artist == other.artist
    }

    #[must_use]
    pub const fn has_known_duration(&self) -> bool {
        self.duration_ms > 0
    }

    #[must_use]
    pub const fn is_seekable(&self) -> bool {
        self.capabilities.seek && self.has_known_duration()
    }

    #[must_use]
    pub fn position_at(&self, now_unix_ms: i64) -> u64 {
        let Some(updated) = self.position_updated_unix_ms.filter(|_| self.playing) else {
            return self.clamped_position(self.position_ms);
        };
        let elapsed = u64::try_from(now_unix_ms.saturating_sub(updated)).unwrap_or(0);
        let rate = u64::from(self.playback_rate_permille.min(MAX_PLAYBACK_RATE_PERMILLE));
        let advanced = elapsed.saturating_mul(rate) / u64::from(NORMAL_PLAYBACK_RATE_PERMILLE);
        self.clamped_position(self.position_ms.saturating_add(advanced))
    }

    #[must_use]
    pub fn presents_like(&self, other: &Self, now_unix_ms: i64) -> bool {
        self.is_same_track(other)
            && self.playing == other.playing
            && self.duration_ms == other.duration_ms
            && self.playback_rate_permille == other.playback_rate_permille
            && self.capabilities == other.capabilities
            && self.thumbnail == other.thumbnail
            && self
                .position_at(now_unix_ms)
                .abs_diff(other.position_at(now_unix_ms))
                <= POSITION_DRIFT_TOLERANCE_MS
    }

    #[must_use]
    pub fn at(&self, now_unix_ms: i64) -> Self {
        Self {
            position_ms: self.position_at(now_unix_ms),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn next_whole_second_in(&self, now_unix_ms: i64) -> Option<u64> {
        if !self.playing || self.position_updated_unix_ms.is_none() {
            return None;
        }
        let rate = u64::from(
            self.playback_rate_permille
                .clamp(1, MAX_PLAYBACK_RATE_PERMILLE),
        );
        let position = self.position_at(now_unix_ms);
        let media_wait = 1_000 - position % 1_000;
        Some((media_wait * u64::from(NORMAL_PLAYBACK_RATE_PERMILLE)).div_ceil(rate))
    }

    const fn clamped_position(&self, position_ms: u64) -> u64 {
        if self.has_known_duration() && position_ms > self.duration_ms {
            self.duration_ms
        } else {
            position_ms
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.playing && !self.title.trim().is_empty()
    }

    #[must_use]
    pub fn display_artist(&self) -> &str {
        if self.artist.trim().is_empty() {
            "Unknown Artist"
        } else {
            self.artist.as_str()
        }
    }

    #[must_use]
    pub fn progress_ratio(&self) -> f32 {
        if self.duration_ms == 0 {
            0.0
        } else {
            ((self.position_ms as f64) / (self.duration_ms as f64)).clamp(0.0, 1.0) as f32
        }
    }

    #[must_use]
    pub fn elapsed_formatted(&self) -> String {
        format_duration(self.position_ms)
    }

    #[must_use]
    pub fn remaining_formatted(&self) -> String {
        let remaining = self.duration_ms.saturating_sub(self.position_ms);
        format!("-{}", format_duration(remaining))
    }
}

pub const SEEK_CONFIRM_TOLERANCE_MS: u64 = 1_500;
pub const SEEK_READBACK_TIMEOUT_MS: i64 = 2_500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingSeek {
    pub target_ms: u64,
    pub issued_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekReadback {
    Confirmed,
    Hold,
    Expired,
}

#[must_use]
pub fn seek_target_ms(ratio: f32, duration_ms: u64) -> u64 {
    ((ratio.clamp(0.0, 1.0) * duration_ms as f32).round() as u64).min(duration_ms)
}

#[must_use]
pub fn reconcile_seek(pending: PendingSeek, latest: &NowPlaying, now_unix_ms: i64) -> SeekReadback {
    let expected = NowPlaying {
        position_ms: pending.target_ms,
        position_updated_unix_ms: Some(pending.issued_unix_ms),
        ..latest.clone()
    }
    .position_at(now_unix_ms);
    let reported = latest.position_at(now_unix_ms);
    if reported.abs_diff(expected) <= SEEK_CONFIRM_TOLERANCE_MS {
        SeekReadback::Confirmed
    } else if now_unix_ms.saturating_sub(pending.issued_unix_ms) >= SEEK_READBACK_TIMEOUT_MS {
        SeekReadback::Expired
    } else {
        SeekReadback::Hold
    }
}

#[must_use]
pub fn format_duration(ms: u64) -> String {
    let total_seconds = ms / 1_000;
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    TogglePlayPause,
    Next,
    Previous,
    Seek(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotchExpandedTab {
    #[default]
    Pomodoro,
    Media,
}

impl NotchExpandedTab {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pomodoro => "Focus",
            Self::Media => "Now Playing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_playing_activity_requires_playing_and_non_empty_title() {
        let empty = NowPlaying::default();
        assert!(!empty.is_active());

        let playing_no_title = NowPlaying {
            title: String::from("   "),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 0,
            duration_ms: 0,
            thumbnail: None,
            ..NowPlaying::default()
        };
        assert!(!playing_no_title.is_active());

        let paused = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            thumbnail: None,
            ..NowPlaying::default()
        };
        assert!(!paused.is_active());

        let active = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 30_000,
            duration_ms: 120_000,
            thumbnail: None,
            ..NowPlaying::default()
        };
        assert!(active.is_active());
        assert_eq!(active.display_artist(), "Artist");
        assert!((active.progress_ratio() - 0.25).abs() < f32::EPSILON);
        assert_eq!(active.elapsed_formatted(), "00:30");
        assert_eq!(active.remaining_formatted(), "-01:30");
    }

    fn playing_at(position_ms: u64, updated_unix_ms: i64) -> NowPlaying {
        NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms,
            duration_ms: 200_000,
            position_updated_unix_ms: Some(updated_unix_ms),
            ..NowPlaying::default()
        }
    }

    #[test]
    fn a_readback_that_matches_the_extrapolated_position_presents_alike() {
        let shown = playing_at(10_000, 1_000_000);
        let readback = playing_at(14_900, 1_005_000);
        assert!(shown.presents_like(&readback, 1_005_000));
        assert_ne!(shown, readback);
    }

    #[test]
    fn a_seek_beyond_the_drift_tolerance_presents_differently() {
        let shown = playing_at(10_000, 1_000_000);
        let sought = playing_at(60_000, 1_005_000);
        assert!(!shown.presents_like(&sought, 1_005_000));
    }

    #[test]
    fn play_state_or_track_changes_present_differently() {
        let shown = playing_at(10_000, 1_000_000);
        let paused = NowPlaying {
            playing: false,
            ..shown.clone()
        };
        let other = NowPlaying {
            title: String::from("Other"),
            ..shown.clone()
        };
        assert!(!shown.presents_like(&paused, 1_000_000));
        assert!(!shown.presents_like(&other, 1_000_000));
    }

    #[test]
    fn fallback_artist_when_empty() {
        let item = NowPlaying {
            title: String::from("Song"),
            artist: String::new(),
            playing: true,
            position_ms: 0,
            duration_ms: 0,
            thumbnail: None,
            ..NowPlaying::default()
        };
        assert_eq!(item.display_artist(), "Unknown Artist");
    }

    #[test]
    fn format_duration_handles_zero_seconds_minutes_and_hours() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(5_000), "00:05");
        assert_eq!(format_duration(65_000), "01:05");
        assert_eq!(format_duration(3_665_000), "1:01:05");
    }

    #[test]
    fn progress_ratio_clamps_and_handles_zero_duration() {
        let zero = NowPlaying::default();
        assert!((zero.progress_ratio() - 0.0).abs() < f32::EPSILON);

        let over = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 200_000,
            duration_ms: 100_000,
            thumbnail: None,
            ..NowPlaying::default()
        };
        assert!((over.progress_ratio() - 1.0).abs() < f32::EPSILON);
    }

    fn playing_track(position_ms: u64, duration_ms: u64) -> NowPlaying {
        NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms,
            duration_ms,
            position_updated_unix_ms: Some(10_000),
            ..NowPlaying::default()
        }
    }

    #[test]
    fn position_advances_from_the_last_timeline_update_while_playing() {
        let track = playing_track(30_000, 120_000);
        assert_eq!(track.position_at(10_000), 30_000);
        assert_eq!(track.position_at(12_500), 32_500);
        assert_eq!(track.position_at(9_000), 30_000);
        assert_eq!(track.at(15_000).position_ms, 35_000);
    }

    #[test]
    fn paused_tracks_and_unknown_update_times_do_not_advance() {
        let paused = NowPlaying {
            playing: false,
            ..playing_track(30_000, 120_000)
        };
        assert_eq!(paused.position_at(60_000), 30_000);
        let unstamped = NowPlaying {
            position_updated_unix_ms: None,
            ..playing_track(30_000, 120_000)
        };
        assert_eq!(unstamped.position_at(60_000), 30_000);
        assert_eq!(unstamped.next_whole_second_in(60_000), None);
    }

    #[test]
    fn position_follows_the_playback_rate_and_stops_at_the_end() {
        let fast = NowPlaying {
            playback_rate_permille: 2_000,
            ..playing_track(30_000, 120_000)
        };
        assert_eq!(fast.position_at(11_000), 32_000);
        assert_eq!(fast.position_at(1_000_000), 120_000);
    }

    #[test]
    fn rate_extrapolation_is_capped_and_a_zero_rate_holds_the_position() {
        let huge = NowPlaying {
            playback_rate_permille: u32::MAX,
            ..playing_track(0, 10_000_000)
        };
        assert_eq!(huge.position_at(11_000), 16_000);
        let frozen = NowPlaying {
            playback_rate_permille: 0,
            ..playing_track(30_000, 120_000)
        };
        assert_eq!(frozen.position_at(60_000), 30_000);
    }

    #[test]
    fn a_timeline_stamp_in_the_future_holds_until_it_is_reached() {
        let skewed = NowPlaying {
            position_updated_unix_ms: Some(15_000),
            ..playing_track(30_000, 120_000)
        };
        assert_eq!(skewed.position_at(10_000), 30_000);
        assert_eq!(skewed.position_at(15_000), 30_000);
        assert_eq!(skewed.position_at(17_000), 32_000);
    }

    #[test]
    fn duration_and_capability_changes_present_differently() {
        let shown = playing_track(10_000, 120_000);
        let longer = NowPlaying {
            duration_ms: 240_000,
            ..shown.clone()
        };
        let restricted = NowPlaying {
            capabilities: MediaCapabilities {
                next: false,
                ..MediaCapabilities::ALL
            },
            ..shown.clone()
        };
        assert!(!shown.presents_like(&longer, 10_000));
        assert!(!shown.presents_like(&restricted, 10_000));
        assert!(shown.presents_like(&shown, 10_000));
    }

    #[test]
    fn live_streams_keep_counting_and_are_not_seekable() {
        let live = playing_track(5_000, 0);
        assert!(!live.has_known_duration());
        assert!(!live.is_seekable());
        assert_eq!(live.position_at(20_000), 15_000);
        let no_seek = NowPlaying {
            capabilities: MediaCapabilities {
                seek: false,
                ..MediaCapabilities::ALL
            },
            ..playing_track(5_000, 60_000)
        };
        assert!(!no_seek.is_seekable());
        assert!(playing_track(5_000, 60_000).is_seekable());
    }

    #[test]
    fn repaint_waits_until_the_next_whole_media_second() {
        let track = playing_track(30_250, 120_000);
        assert_eq!(track.next_whole_second_in(10_000), Some(750));
        let double = NowPlaying {
            playback_rate_permille: 2_000,
            ..track
        };
        assert_eq!(double.next_whole_second_in(10_000), Some(375));
    }

    #[test]
    fn track_identity_includes_the_source_application() {
        let spotify = playing_track(0, 1);
        let browser = NowPlaying {
            source_app: String::from("chrome.exe"),
            ..playing_track(0, 1)
        };
        assert!(!spotify.is_same_track(&browser));
        assert_ne!(spotify.track_key(), browser.track_key());
        let later = NowPlaying {
            playing: false,
            position_ms: 900,
            ..playing_track(0, 1)
        };
        assert!(spotify.is_same_track(&later));
    }

    #[test]
    fn seek_targets_clamp_to_the_track() {
        assert_eq!(seek_target_ms(0.5, 200_000), 100_000);
        assert_eq!(seek_target_ms(1.4, 200_000), 200_000);
        assert_eq!(seek_target_ms(-0.2, 200_000), 0);
        assert_eq!(seek_target_ms(f32::NAN, 200_000), 0);
    }

    #[test]
    fn seek_readback_confirms_holds_and_expires() {
        let pending = PendingSeek {
            target_ms: 90_000,
            issued_unix_ms: 10_000,
        };
        let stale = playing_track(30_000, 120_000);
        assert_eq!(reconcile_seek(pending, &stale, 10_500), SeekReadback::Hold);
        assert_eq!(
            reconcile_seek(pending, &stale, 10_000 + SEEK_READBACK_TIMEOUT_MS),
            SeekReadback::Expired
        );
        let moved = NowPlaying {
            position_ms: 90_200,
            position_updated_unix_ms: Some(10_300),
            ..playing_track(0, 120_000)
        };
        assert_eq!(
            reconcile_seek(pending, &moved, 11_000),
            SeekReadback::Confirmed
        );
    }

    #[test]
    fn capabilities_gate_each_transport_command() {
        let limited = MediaCapabilities {
            next: false,
            seek: false,
            ..MediaCapabilities::ALL
        };
        assert!(limited.allows(MediaCommand::TogglePlayPause));
        assert!(!limited.allows(MediaCommand::Next));
        assert!(limited.allows(MediaCommand::Previous));
        assert!(!limited.allows(MediaCommand::Seek(1)));
    }

    #[test]
    fn notch_expanded_tab_defaults_and_labels() {
        assert_eq!(NotchExpandedTab::default(), NotchExpandedTab::Pomodoro);
        assert_eq!(NotchExpandedTab::Pomodoro.label(), "Focus");
        assert_eq!(NotchExpandedTab::Media.label(), "Now Playing");
    }
}
