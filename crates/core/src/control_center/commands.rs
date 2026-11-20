use crate::bar::bar_focus::BarKey;
use crate::notch::media::MediaCommand;
use crate::ports::ControlCenterAction;
use crate::settings_window::SettingsPage;
use crate::theme::ThemeMode;

use super::layout_types::ControlCenterHitTarget;
use super::model::ControlCenterPage;
use super::model::SLIDER_KEY_STEP_PERCENT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlCenterCommand {
    OpenPage(ControlCenterPage),
    Back,
    OpenSettingsWindow(SettingsPage),
    ToggleWifi,
    ConnectWifi(String),
    ToggleBluetooth,
    ToggleBluetoothDevice(String, bool),
    ToggleFocus,
    CycleTheme,
    SetThemeMode(ThemeMode),
    SetBrightness(u8),
    SetVolume(u8),
    ToggleMute,
    SelectAudioOutput(String),
    Media(MediaCommand),
    OpenNotchMedia,
    OpenSettings(ControlCenterAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterFocusOutcome {
    Moved(ControlCenterHitTarget),
    Activate(ControlCenterHitTarget),
    AdjustSlider {
        target: ControlCenterHitTarget,
        delta: i8,
    },
    Exited,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControlCenterFocus {
    current: Option<ControlCenterHitTarget>,
}

impl ControlCenterFocus {
    #[must_use]
    pub const fn current(self) -> Option<ControlCenterHitTarget> {
        self.current
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        self.current.is_some()
    }

    pub fn enter(&mut self, targets: &[ControlCenterHitTarget]) -> Option<ControlCenterHitTarget> {
        self.current = targets.first().copied();
        self.current
    }

    pub fn enter_target(
        &mut self,
        targets: &[ControlCenterHitTarget],
        preferred: ControlCenterHitTarget,
    ) -> Option<ControlCenterHitTarget> {
        self.current = targets
            .iter()
            .copied()
            .find(|target| *target == preferred)
            .or_else(|| targets.first().copied());
        self.current
    }

    pub fn exit(&mut self) -> bool {
        self.current.take().is_some()
    }

    pub fn revalidate(&mut self, targets: &[ControlCenterHitTarget]) -> bool {
        let Some(current) = self.current else {
            return false;
        };
        if targets.contains(&current) {
            return false;
        }
        self.current = targets.first().copied();
        true
    }

    pub fn handle(
        &mut self,
        key: BarKey,
        targets: &[ControlCenterHitTarget],
    ) -> ControlCenterFocusOutcome {
        let Some(current) = self.current else {
            return ControlCenterFocusOutcome::Unchanged;
        };
        if key == BarKey::Exit || targets.is_empty() {
            self.current = None;
            return ControlCenterFocusOutcome::Exited;
        }
        let Some(index) = targets.iter().position(|target| *target == current) else {
            let first = targets[0];
            self.current = Some(first);
            return ControlCenterFocusOutcome::Moved(first);
        };
        if matches!(
            current,
            ControlCenterHitTarget::DisplaySlider | ControlCenterHitTarget::SoundSlider
        ) && matches!(key, BarKey::Previous | BarKey::Next)
        {
            let delta = if key == BarKey::Next {
                SLIDER_KEY_STEP_PERCENT
            } else {
                -SLIDER_KEY_STEP_PERCENT
            };
            return ControlCenterFocusOutcome::AdjustSlider {
                target: current,
                delta,
            };
        }
        if key == BarKey::Activate {
            return ControlCenterFocusOutcome::Activate(current);
        }
        let last = targets.len() - 1;
        let next = match key {
            BarKey::First => 0,
            BarKey::Last => last,
            BarKey::Next | BarKey::NextTab | BarKey::Down => (index + 1) % targets.len(),
            BarKey::Previous | BarKey::PreviousTab | BarKey::Up => {
                index.checked_sub(1).unwrap_or(last)
            }
            BarKey::Activate | BarKey::Exit => return ControlCenterFocusOutcome::Unchanged,
        };
        let target = targets[next];
        self.current = Some(target);
        if target == current {
            ControlCenterFocusOutcome::Unchanged
        } else {
            ControlCenterFocusOutcome::Moved(target)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCenterClickOutcome {
    pub command: ControlCenterCommand,
    pub dismiss: bool,
}
