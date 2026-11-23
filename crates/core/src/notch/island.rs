use crate::notch::media::TrackKey;
use crate::notch::notch_notification::NotificationStage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompactSlotKind {
    #[default]
    Focus,
    Media,
    Call,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactContent {
    Focus,
    Media(TrackKey),
    Call,
}

impl CompactContent {
    #[must_use]
    pub const fn kind(&self) -> CompactSlotKind {
        match self {
            Self::Focus => CompactSlotKind::Focus,
            Self::Media(_) => CompactSlotKind::Media,
            Self::Call => CompactSlotKind::Call,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IslandMode {
    Collapsed,
    Compact(CompactContent),
    Hud,
    Expanded,
    Alert,
    Notification(NotificationStage),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct IslandInputs {
    pub alert: bool,
    pub panel_requested: bool,
    pub hud: bool,
    pub notification: Option<NotificationStage>,
    pub focus_active: bool,
    pub call_active: bool,
    pub media: Option<TrackKey>,
    pub media_paused_hold: Option<TrackKey>,
    pub media_primary: bool,
    pub dismissed: Option<CompactContent>,
    pub dismissed_secondary: Option<CompactContent>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IslandTargets {
    pub panel: f32,
    pub hud: f32,
    pub compact: f32,
    pub secondary: f32,
}

impl IslandInputs {
    #[must_use]
    pub fn compact_content(&self) -> Option<CompactContent> {
        self.active_call().or_else(|| self.regular_primary())
    }

    fn regular_primary(&self) -> Option<CompactContent> {
        let focus = self.active_focus();
        let media = self.active_media();
        if self.media_primary {
            media.or(focus)
        } else {
            focus.or(media)
        }
    }

    fn active_call(&self) -> Option<CompactContent> {
        self.call_active
            .then_some(CompactContent::Call)
            .filter(|content| self.is_not_dismissed(content))
    }

    #[must_use]
    pub fn secondary_compact_content(&self) -> Option<CompactContent> {
        if self.active_call().is_some() {
            return self.regular_primary();
        }
        let focus = self.active_focus()?;
        let media = self.active_media()?;
        if self.media_primary {
            Some(focus)
        } else {
            Some(media)
        }
    }

    fn active_focus(&self) -> Option<CompactContent> {
        self.focus_active
            .then_some(CompactContent::Focus)
            .filter(|content| self.is_not_dismissed(content))
    }

    fn active_media(&self) -> Option<CompactContent> {
        self.media
            .as_ref()
            .or(self.media_paused_hold.as_ref())
            .map(|track| CompactContent::Media(track.clone()))
            .filter(|content| self.is_not_dismissed(content))
    }

    fn is_not_dismissed(&self, content: &CompactContent) -> bool {
        self.dismissed.as_ref() != Some(content)
            && self.dismissed_secondary.as_ref() != Some(content)
    }
}

#[must_use]
pub fn resolve_island(inputs: &IslandInputs) -> IslandMode {
    if inputs.alert {
        IslandMode::Alert
    } else if inputs.notification == Some(NotificationStage::Ringing) {
        IslandMode::Notification(NotificationStage::Ringing)
    } else if inputs.panel_requested {
        IslandMode::Expanded
    } else if let Some(stage) = inputs.notification {
        IslandMode::Notification(stage)
    } else if inputs.hud {
        IslandMode::Hud
    } else if let Some(content) = inputs.compact_content() {
        IslandMode::Compact(content)
    } else {
        IslandMode::Collapsed
    }
}

#[must_use]
pub fn island_targets(inputs: &IslandInputs) -> IslandTargets {
    let mode = resolve_island(inputs);
    let flag = |on: bool| if on { 1.0 } else { 0.0 };
    IslandTargets {
        panel: flag(matches!(
            mode,
            IslandMode::Expanded
                | IslandMode::Alert
                | IslandMode::Notification(NotificationStage::Banner | NotificationStage::Ringing)
        )),
        hud: flag(mode == IslandMode::Hud),
        compact: flag(
            inputs.compact_content().is_some()
                || mode == IslandMode::Notification(NotificationStage::Peek),
        ),
        secondary: flag(
            matches!(mode, IslandMode::Compact(_)) && inputs.secondary_compact_content().is_some(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{CompactContent, IslandInputs, IslandMode, island_targets, resolve_island};
    use crate::notch::media::TrackKey;
    use crate::notch::notch_notification::NotificationStage;

    fn track(title: &str) -> TrackKey {
        TrackKey {
            source_app: String::from("Spotify.exe"),
            title: String::from(title),
            artist: String::from("Artist"),
        }
    }

    fn everything() -> IslandInputs {
        IslandInputs {
            alert: true,
            panel_requested: true,
            hud: true,
            notification: None,
            focus_active: true,
            call_active: false,
            media: Some(track("Song")),
            media_paused_hold: None,
            media_primary: false,
            dismissed: None,
            dismissed_secondary: None,
        }
    }

    #[test]
    fn simultaneous_events_resolve_in_priority_order() {
        let mut inputs = everything();
        assert_eq!(resolve_island(&inputs), IslandMode::Alert);
        inputs.alert = false;
        assert_eq!(resolve_island(&inputs), IslandMode::Expanded);
        inputs.panel_requested = false;
        assert_eq!(resolve_island(&inputs), IslandMode::Hud);
        inputs.hud = false;
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Focus)
        );
        inputs.focus_active = false;
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Media(track("Song")))
        );
        inputs.media = None;
        assert_eq!(resolve_island(&inputs), IslandMode::Collapsed);
    }

    #[test]
    fn a_paused_track_stays_compact_only_while_its_hold_lasts() {
        let held = IslandInputs {
            media_paused_hold: Some(track("Song")),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&held),
            IslandMode::Compact(CompactContent::Media(track("Song")))
        );
        assert!((island_targets(&held).compact - 1.0).abs() < f32::EPSILON);
        let expired = IslandInputs::default();
        assert_eq!(resolve_island(&expired), IslandMode::Collapsed);
    }

    #[test]
    fn a_playing_track_wins_over_a_held_one_and_focus_wins_over_both() {
        let mut inputs = IslandInputs {
            media: Some(track("Now")),
            media_paused_hold: Some(track("Before")),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Media(track("Now")))
        );
        inputs.focus_active = true;
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Focus)
        );
    }

    #[test]
    fn dismissing_a_held_track_collapses_the_island() {
        let inputs = IslandInputs {
            media_paused_hold: Some(track("Song")),
            dismissed: Some(CompactContent::Media(track("Song"))),
            ..IslandInputs::default()
        };
        assert_eq!(resolve_island(&inputs), IslandMode::Collapsed);
    }

    #[test]
    fn a_volume_burst_never_replaces_an_open_panel_or_alert() {
        let expanded = IslandInputs {
            alert: false,
            ..everything()
        };
        let targets = island_targets(&expanded);
        assert!((targets.panel - 1.0).abs() < f32::EPSILON);
        assert!(targets.hud.abs() < f32::EPSILON);
        let alert = island_targets(&everything());
        assert!(alert.hud.abs() < f32::EPSILON);
    }

    #[test]
    fn the_hud_overlays_compact_content_without_collapsing_it() {
        let inputs = IslandInputs {
            hud: true,
            media: Some(track("Song")),
            ..IslandInputs::default()
        };
        let targets = island_targets(&inputs);
        assert!((targets.hud - 1.0).abs() < f32::EPSILON);
        assert!((targets.compact - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn dismissal_hides_only_the_dismissed_content() {
        let mut inputs = IslandInputs {
            media: Some(track("Song")),
            dismissed: Some(CompactContent::Media(track("Song"))),
            ..IslandInputs::default()
        };
        assert_eq!(resolve_island(&inputs), IslandMode::Collapsed);
        inputs.media = Some(track("Next song"));
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Media(track("Next song")))
        );
        inputs.focus_active = true;
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Focus)
        );
    }

    #[test]
    fn dismissing_focus_reveals_the_media_underneath() {
        let mut inputs = IslandInputs {
            focus_active: true,
            media: Some(track("Song")),
            dismissed: Some(CompactContent::Focus),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Media(track("Song")))
        );
        inputs.media = None;
        assert_eq!(resolve_island(&inputs), IslandMode::Collapsed);
    }

    #[test]
    fn a_dismissed_island_still_expands_on_request_and_alerts() {
        let inputs = IslandInputs {
            panel_requested: true,
            media: Some(track("Song")),
            dismissed: Some(CompactContent::Media(track("Song"))),
            ..IslandInputs::default()
        };
        assert_eq!(resolve_island(&inputs), IslandMode::Expanded);
        let alert = IslandInputs {
            alert: true,
            ..inputs
        };
        assert_eq!(resolve_island(&alert), IslandMode::Alert);
    }

    #[test]
    fn dual_active_modules_place_focus_primary_and_media_secondary_until_swapped() {
        let mut inputs = IslandInputs {
            focus_active: true,
            media: Some(track("Song")),
            ..IslandInputs::default()
        };
        assert_eq!(inputs.compact_content(), Some(CompactContent::Focus));
        assert_eq!(
            inputs.secondary_compact_content(),
            Some(CompactContent::Media(track("Song")))
        );
        let targets = island_targets(&inputs);
        assert!((targets.compact - 1.0).abs() < f32::EPSILON);
        assert!((targets.secondary - 1.0).abs() < f32::EPSILON);

        inputs.media_primary = true;
        assert_eq!(
            inputs.compact_content(),
            Some(CompactContent::Media(track("Song")))
        );
        assert_eq!(
            inputs.secondary_compact_content(),
            Some(CompactContent::Focus)
        );

        inputs.hud = true;
        assert!(island_targets(&inputs).secondary.abs() < f32::EPSILON);
        inputs.hud = false;
        inputs.panel_requested = true;
        assert!(island_targets(&inputs).secondary.abs() < f32::EPSILON);
    }

    #[test]
    fn a_ringing_call_beats_the_open_panel_but_a_message_banner_does_not() {
        let mut inputs = IslandInputs {
            panel_requested: true,
            hud: true,
            notification: Some(NotificationStage::Banner),
            ..IslandInputs::default()
        };
        assert_eq!(resolve_island(&inputs), IslandMode::Expanded);
        inputs.notification = Some(NotificationStage::Ringing);
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Notification(NotificationStage::Ringing)
        );
        inputs.alert = true;
        assert_eq!(resolve_island(&inputs), IslandMode::Alert);
    }

    #[test]
    fn a_message_banner_beats_hud_and_compact_and_opens_the_panel_channel() {
        let inputs = IslandInputs {
            hud: true,
            focus_active: true,
            notification: Some(NotificationStage::Banner),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Notification(NotificationStage::Banner)
        );
        let targets = island_targets(&inputs);
        assert!((targets.panel - 1.0).abs() < f32::EPSILON);
        assert!(targets.hud.abs() < f32::EPSILON);
    }

    #[test]
    fn a_message_peek_uses_the_compact_bar_channel_until_it_becomes_a_banner() {
        let inputs = IslandInputs {
            notification: Some(NotificationStage::Peek),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Notification(NotificationStage::Peek)
        );
        let targets = island_targets(&inputs);
        assert!(targets.panel.abs() < f32::EPSILON);
        assert!((targets.compact - 1.0).abs() < f32::EPSILON);
        assert!(targets.secondary.abs() < f32::EPSILON);
    }

    #[test]
    fn a_call_takes_the_main_notch_and_pushes_focus_or_media_to_the_circle() {
        let mut inputs = IslandInputs {
            call_active: true,
            focus_active: true,
            media: Some(track("Song")),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Call)
        );
        assert_eq!(
            inputs.secondary_compact_content(),
            Some(CompactContent::Focus)
        );
        inputs.media_primary = true;
        assert_eq!(
            inputs.secondary_compact_content(),
            Some(CompactContent::Media(track("Song")))
        );
        inputs.focus_active = false;
        inputs.media = None;
        assert_eq!(inputs.secondary_compact_content(), None);
        assert!((island_targets(&inputs).compact - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_dismissed_call_falls_back_to_the_regular_compact_content() {
        let inputs = IslandInputs {
            call_active: true,
            focus_active: true,
            dismissed: Some(CompactContent::Call),
            ..IslandInputs::default()
        };
        assert_eq!(
            resolve_island(&inputs),
            IslandMode::Compact(CompactContent::Focus)
        );
    }
}
