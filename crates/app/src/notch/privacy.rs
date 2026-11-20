use std::time::Instant;

use otto_core::{NotchAction, NotchExpandedTab, PrivacyState};

use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;
use super::PanelTrigger;

impl NotchController {
    pub(crate) fn privacy_state(&self) -> &PrivacyState {
        &self.privacy
    }

    pub(crate) fn sync_privacy_state(&mut self, next: PrivacyState) -> bool {
        let resolved = if self.privacy_enabled {
            next
        } else {
            PrivacyState::default()
        };
        if self.privacy == resolved {
            return false;
        }
        self.privacy = resolved;
        self.call.update(&self.privacy, self.millis(Instant::now()));
        true
    }

    pub(super) fn handle_privacy_indicator_click(&mut self) -> NotchUpdate {
        if self.has_alert() {
            self.dismiss_active_alert();
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        self.panel_trigger = PanelTrigger::Pinned;
        self.hover_blocked = false;
        self.hover_since = None;
        if self.notch_tab == NotchExpandedTab::Pomodoro
            || (self.notch_tab == NotchExpandedTab::Media && self.media.is_none())
        {
            let _ = self.switch_tab(NotchExpandedTab::Shelf);
        }
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(super) fn handle_privacy_mic_click(&mut self) -> NotchUpdate {
        if !self.privacy.mic.is_available() {
            return NotchUpdate::default();
        }
        self.privacy.mic = self.privacy.mic.with_toggled_mute();
        NotchUpdate {
            changed: true,
            effect: Some(NotchEffect::Platform(NotchAction::ToggleMicMute)),
        }
    }
}
