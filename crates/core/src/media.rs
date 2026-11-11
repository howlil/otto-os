#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub thumbnail: Option<std::sync::Arc<[u8]>>,
}

impl NowPlaying {
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
        };
        assert!(!playing_no_title.is_active());

        let paused = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            thumbnail: None,
        };
        assert!(!paused.is_active());

        let active = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 30_000,
            duration_ms: 120_000,
            thumbnail: None,
        };
        assert!(active.is_active());
        assert_eq!(active.display_artist(), "Artist");
        assert!((active.progress_ratio() - 0.25).abs() < f32::EPSILON);
        assert_eq!(active.elapsed_formatted(), "00:30");
        assert_eq!(active.remaining_formatted(), "-01:30");
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
        };
        assert!((over.progress_ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn notch_expanded_tab_defaults_and_labels() {
        assert_eq!(NotchExpandedTab::default(), NotchExpandedTab::Pomodoro);
        assert_eq!(NotchExpandedTab::Pomodoro.label(), "Focus");
        assert_eq!(NotchExpandedTab::Media.label(), "Now Playing");
    }
}
