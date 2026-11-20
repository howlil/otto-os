pub const PRIVACY_COMPACT_EXTRA_POINTS: i32 = 16;
pub const PRIVACY_DOT_POINTS: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MicState {
    #[default]
    Unavailable,
    Idle {
        muted: bool,
    },
    Active {
        muted: bool,
    },
}

impl MicState {
    #[must_use]
    pub const fn is_available(self) -> bool {
        !matches!(self, Self::Unavailable)
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }

    #[must_use]
    pub const fn is_muted(self) -> bool {
        matches!(
            self,
            Self::Idle { muted: true } | Self::Active { muted: true }
        )
    }

    #[must_use]
    pub const fn with_toggled_mute(self) -> Self {
        match self {
            Self::Unavailable => Self::Unavailable,
            Self::Idle { muted } => Self::Idle { muted: !muted },
            Self::Active { muted } => Self::Active { muted: !muted },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrivacyState {
    pub mic: MicState,
    pub camera_active: bool,
    pub location_active: bool,
    pub app_name: Option<String>,
}

impl PrivacyState {
    #[must_use]
    pub const fn any_active(&self) -> bool {
        self.mic.is_active() || self.camera_active
    }

    #[must_use]
    pub const fn show_in_bar(&self) -> bool {
        self.any_active() || self.location_active
    }

    #[must_use]
    pub const fn show_panel_control(&self) -> bool {
        self.any_active() || self.mic.is_muted()
    }

    #[must_use]
    pub fn compact_label(&self) -> &'static str {
        if self.mic.is_muted() {
            "Muted"
        } else if self.mic.is_active() {
            "Mic On"
        } else if self.mic.is_available() {
            "Mic"
        } else {
            "Cam"
        }
    }

    #[must_use]
    pub fn pill_label(&self) -> String {
        let prefix = match (
            self.mic.is_active(),
            self.mic.is_muted(),
            self.camera_active,
        ) {
            (_, true, false) => "Muted",
            (true, true, true) => "Muted · Cam",
            (true, false, true) => "Mic · Cam",
            (true, false, false) => "Mic On",
            (false, true, true) => "Cam · Muted",
            (false, false, true) => "Camera",
            (false, false, false) => "Mic Ready",
        };
        match self.app_name.as_deref() {
            Some(app) if !app.is_empty() && !self.mic.is_muted() => format!("{prefix} · {app}"),
            Some(app) if !app.is_empty() && self.mic.is_muted() && self.mic.is_active() => {
                format!("Muted · {app}")
            }
            _ => String::from(prefix),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MicState, PrivacyState};

    #[test]
    fn mic_state_tracks_availability_activity_and_mute_toggle() {
        assert!(!MicState::Unavailable.is_available());
        assert_eq!(
            MicState::Unavailable.with_toggled_mute(),
            MicState::Unavailable
        );

        let active = MicState::Active { muted: false };
        assert!(active.is_available());
        assert!(active.is_active());
        assert!(!active.is_muted());

        let muted = active.with_toggled_mute();
        assert_eq!(muted, MicState::Active { muted: true });
        assert!(muted.is_active());
        assert!(muted.is_muted());
    }

    #[test]
    fn privacy_state_formats_bar_and_panel_labels() {
        let off = PrivacyState::default();
        assert!(!off.any_active());
        assert!(!off.show_in_bar());
        assert!(!off.show_panel_control());

        let muted_idle = PrivacyState {
            mic: MicState::Idle { muted: true },
            camera_active: false,
            location_active: false,
            app_name: None,
        };
        assert!(!muted_idle.show_in_bar());
        assert!(muted_idle.show_panel_control());
        assert_eq!(muted_idle.compact_label(), "Muted");
        assert_eq!(muted_idle.pill_label(), "Muted");

        let zoom_call = PrivacyState {
            mic: MicState::Active { muted: false },
            camera_active: true,
            location_active: false,
            app_name: Some(String::from("Zoom")),
        };
        assert!(zoom_call.show_in_bar());
        assert_eq!(zoom_call.pill_label(), "Mic · Cam · Zoom");
    }

    #[test]
    fn location_alone_shows_in_the_bar_without_a_mic_control() {
        let maps = PrivacyState {
            location_active: true,
            ..PrivacyState::default()
        };
        assert!(maps.show_in_bar());
        assert!(!maps.any_active());
        assert!(!maps.show_panel_control());
    }
}
