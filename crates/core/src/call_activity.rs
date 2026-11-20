use crate::privacy::PrivacyState;

pub const CALL_DEBOUNCE_MS: u64 = 3_000;

const CALL_MIC_APPS: [&str; 15] = [
    "teams", "zoom", "skype", "whatsapp", "discord", "telegram", "slack", "webex", "meet",
    "chrome", "msedge", "edge", "firefox", "brave", "opera",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallActivity {
    pub video: bool,
    pub app_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallScene {
    pub video: bool,
    pub muted: bool,
    pub elapsed: String,
}

#[must_use]
pub fn detect_call_activity(privacy: &PrivacyState) -> Option<CallActivity> {
    let app_name = privacy.app_name.clone().unwrap_or_default();
    if privacy.camera_active {
        return Some(CallActivity {
            video: true,
            app_name,
        });
    }
    let lowered = app_name.to_lowercase();
    let known = CALL_MIC_APPS.iter().any(|app| lowered.contains(app));
    (privacy.mic.is_active() && known).then_some(CallActivity {
        video: false,
        app_name,
    })
}

#[must_use]
pub fn format_call_duration(elapsed_secs: u64) -> String {
    let hours = elapsed_secs / 3_600;
    let minutes = (elapsed_secs % 3_600) / 60;
    let seconds = elapsed_secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[derive(Debug, Clone, Default)]
pub struct CallTracker {
    since_ms: Option<u64>,
    activity: Option<CallActivity>,
    reported: bool,
}

impl CallTracker {
    pub fn update(&mut self, privacy: &PrivacyState, now_ms: u64) -> bool {
        if let Some(activity) = detect_call_activity(privacy) {
            self.since_ms.get_or_insert(now_ms);
            self.activity = Some(activity);
        } else {
            self.since_ms = None;
            self.activity = None;
        }
        let active = self.is_active(now_ms);
        let changed = active != self.reported;
        self.reported = active;
        changed
    }

    #[must_use]
    pub fn is_active(&self, now_ms: u64) -> bool {
        self.since_ms
            .is_some_and(|since| now_ms >= since + CALL_DEBOUNCE_MS)
    }

    #[must_use]
    pub fn scene(&self, muted: bool, now_ms: u64) -> Option<CallScene> {
        if !self.is_active(now_ms) {
            return None;
        }
        let since = self.since_ms?;
        Some(CallScene {
            video: self
                .activity
                .as_ref()
                .is_some_and(|activity| activity.video),
            muted,
            elapsed: format_call_duration((now_ms - since) / 1_000),
        })
    }

    #[must_use]
    pub fn next_wake_ms(&self, now_ms: u64) -> Option<u64> {
        let since = self.since_ms?;
        let confirm = since + CALL_DEBOUNCE_MS;
        if now_ms < confirm {
            return Some(confirm);
        }
        Some((now_ms / 1_000 + 1) * 1_000)
    }
}

#[cfg(test)]
mod tests {
    use super::{CALL_DEBOUNCE_MS, CallTracker, detect_call_activity, format_call_duration};
    use crate::privacy::{MicState, PrivacyState};

    fn mic_in(app: &str) -> PrivacyState {
        PrivacyState {
            mic: MicState::Active { muted: false },
            camera_active: false,
            location_active: false,
            app_name: Some(String::from(app)),
        }
    }

    #[test]
    fn a_camera_or_a_known_apps_microphone_is_a_call_but_other_mic_use_is_not() {
        let video = PrivacyState {
            camera_active: true,
            app_name: Some(String::from("chrome")),
            ..PrivacyState::default()
        };
        assert_eq!(
            detect_call_activity(&video).map(|call| call.video),
            Some(true)
        );
        assert_eq!(
            detect_call_activity(&mic_in("Chrome")).map(|call| call.video),
            Some(false)
        );
        assert_eq!(detect_call_activity(&mic_in("AudioRecorder")), None);
        assert_eq!(detect_call_activity(&PrivacyState::default()), None);
    }

    #[test]
    fn durations_read_as_mm_ss_then_h_mm_ss() {
        assert_eq!(format_call_duration(0), "00:00");
        assert_eq!(format_call_duration(75), "01:15");
        assert_eq!(format_call_duration(3_725), "1:02:05");
    }

    #[test]
    fn a_call_shows_only_after_the_debounce_and_times_from_its_first_sign() {
        let mut tracker = CallTracker::default();
        assert!(!tracker.update(&mic_in("Zoom"), 1_000));
        assert!(!tracker.is_active(1_000 + CALL_DEBOUNCE_MS - 1));
        assert!(tracker.update(&mic_in("Zoom"), 1_000 + CALL_DEBOUNCE_MS));
        let scene = tracker
            .scene(false, 1_000 + CALL_DEBOUNCE_MS + 5_000)
            .expect("scene");
        assert_eq!(scene.elapsed, "00:08");
        assert!(!scene.video && !scene.muted);
    }

    #[test]
    fn a_short_mic_blip_never_becomes_a_call_and_ending_clears_it() {
        let mut tracker = CallTracker::default();
        tracker.update(&mic_in("Chrome"), 0);
        assert!(!tracker.update(&PrivacyState::default(), 1_000));
        assert!(!tracker.is_active(10_000));
        tracker.update(&mic_in("Chrome"), 20_000);
        assert!(tracker.update(&mic_in("Chrome"), 24_000));
        assert!(tracker.update(&PrivacyState::default(), 40_000));
        assert!(tracker.scene(false, 40_000).is_none());
    }

    #[test]
    fn the_next_wake_is_the_confirmation_then_each_whole_second() {
        let mut tracker = CallTracker::default();
        assert_eq!(tracker.next_wake_ms(0), None);
        tracker.update(&mic_in("Zoom"), 500);
        assert_eq!(tracker.next_wake_ms(600), Some(500 + CALL_DEBOUNCE_MS));
        assert_eq!(tracker.next_wake_ms(4_200), Some(5_000));
    }
}
