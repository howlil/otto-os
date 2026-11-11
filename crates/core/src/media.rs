#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    TogglePlayPause,
    Next,
    Previous,
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
        };
        assert!(!playing_no_title.is_active());

        let paused = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: false,
        };
        assert!(!paused.is_active());

        let active = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
        };
        assert!(active.is_active());
        assert_eq!(active.display_artist(), "Artist");
    }

    #[test]
    fn fallback_artist_when_empty() {
        let item = NowPlaying {
            title: String::from("Song"),
            artist: String::new(),
            playing: true,
        };
        assert_eq!(item.display_artist(), "Unknown Artist");
    }

    #[test]
    fn notch_expanded_tab_defaults_and_labels() {
        assert_eq!(NotchExpandedTab::default(), NotchExpandedTab::Pomodoro);
        assert_eq!(NotchExpandedTab::Pomodoro.label(), "Focus");
        assert_eq!(NotchExpandedTab::Media.label(), "Now Playing");
    }
}
