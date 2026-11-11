use std::time::{Duration, Instant};

use otto_core::{
    HitTarget, MediaCommand, MotionPreference, NotchAction, NotchExpandedTab, NotchHudKind,
    NotchHudState, NotchLayoutInput, NowPlaying, PomodoroClock, PomodoroState, PomodoroStatus,
    PomodoroTick, animate_value,
};

const HUD_HOLD_MS: u64 = 1_650;
const ALERT_HOLD_MS: u64 = 10_000;
const HOVER_DEBOUNCE_MS: u64 = 80;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PanelTrigger {
    #[default]
    Closed,
    Hovered,
    Pinned,
    PinnedHovered,
}

impl PanelTrigger {
    const fn is_open(self) -> bool {
        !matches!(self, Self::Closed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotchEffect {
    Platform(NotchAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotchHotkey {
    ToggleNotch,
    TogglePomodoro,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotchUpdate {
    pub changed: bool,
    pub effect: Option<NotchEffect>,
}

pub(crate) struct NotchController {
    hud_kind: Option<NotchHudKind>,
    hud_expansion: f32,
    hud_level: f32,
    hud_until: Option<Instant>,
    pomodoro: PomodoroState,
    compact_expansion: f32,
    panel_expansion: f32,
    hover_since: Option<Instant>,
    panel_trigger: PanelTrigger,
    alert_until: Option<Instant>,
    clock: PomodoroClock,
    epoch: Instant,
    last_anim_instant: Instant,
    media: Option<NowPlaying>,
    notch_tab: NotchExpandedTab,
    anim_phase: f32,
    lyrics_expanded: bool,
    lyrics: Option<otto_core::LyricsSheet>,
}

impl NotchController {
    pub(crate) fn new(media: Option<NowPlaying>, notch_tab: NotchExpandedTab) -> Self {
        Self {
            hud_kind: None,
            hud_expansion: 0.0,
            hud_level: 0.0,
            hud_until: None,
            pomodoro: PomodoroState::new(),
            compact_expansion: 0.0,
            panel_expansion: 0.0,
            hover_since: None,
            panel_trigger: PanelTrigger::Closed,
            alert_until: None,
            clock: PomodoroClock::default(),
            epoch: Instant::now(),
            last_anim_instant: Instant::now(),
            media,
            notch_tab,
            anim_phase: 0.0,
            lyrics_expanded: false,
            lyrics: None,
        }
    }

    fn millis(&self, moment: Instant) -> u64 {
        u64::try_from(moment.saturating_duration_since(self.epoch).as_millis()).unwrap_or(u64::MAX)
    }

    fn restart_clock(&mut self) {
        let now_ms = self.millis(Instant::now());
        self.clock.restart(now_ms);
    }

    pub(crate) fn sync_clock(&mut self, now: Instant) -> NotchUpdate {
        let (changed, effect) = self.step_pomodoro_clock(now);
        NotchUpdate { changed, effect }
    }

    pub(crate) fn suspend(&mut self, now: Instant) -> NotchUpdate {
        let now_ms = self.millis(now);
        let tick = self.clock.suspend(&mut self.pomodoro, now_ms);
        let (changed, effect) = self.apply_tick(tick, now);
        NotchUpdate { changed, effect }
    }

    pub(crate) fn resume(&mut self, now: Instant) {
        let now_ms = self.millis(now);
        self.clock.resume(now_ms);
    }

    pub(crate) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let now_ms = self.millis(now);
        let pomodoro = self
            .clock
            .next_wakeup_ms(&self.pomodoro, now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let alert = self
            .alert_until
            .filter(|_| self.pomodoro.status == PomodoroStatus::Alert);
        [pomodoro, alert, self.hud_until]
            .into_iter()
            .flatten()
            .min()
    }

    pub(crate) fn layout_input(&self, control_center_enabled: bool) -> NotchLayoutInput {
        NotchLayoutInput {
            hud_expansion: self.hud_expansion,
            compact_expansion: self.compact_expansion,
            panel_expansion: self.panel_expansion,
            alert_mode: self.pomodoro.status == PomodoroStatus::Alert,
            active_tab: self.notch_tab,
            control_center_enabled,
            lyrics_expanded: self.lyrics_expanded,
        }
    }

    pub(crate) fn panel_expansion(&self) -> f32 {
        self.panel_expansion
    }

    pub(crate) fn pomodoro(&self) -> PomodoroState {
        self.pomodoro
    }

    pub(crate) fn media(&self) -> Option<&NowPlaying> {
        self.media.as_ref()
    }

    pub(crate) fn hud_state(&self) -> Option<NotchHudState> {
        self.hud_kind.map(|kind| NotchHudState {
            kind,
            expansion: self.hud_expansion,
            level: self.hud_level,
        })
    }

    pub(crate) fn anim_phase(&self) -> f32 {
        self.anim_phase
    }

    pub(crate) fn is_panel_target_open(&self, now: Instant) -> bool {
        let alert_active = self.pomodoro.status == PomodoroStatus::Alert
            && self.alert_until.is_none_or(|until| now < until);
        self.panel_trigger.is_open() || alert_active
    }

    pub(crate) fn set_media(&mut self, latest_media: Option<NowPlaying>) -> bool {
        if latest_media == self.media {
            return false;
        }
        let track_changed = match (&self.media, &latest_media) {
            (Some(prev), Some(next)) => prev.title != next.title || prev.artist != next.artist,
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };
        if track_changed {
            self.lyrics = None;
        }
        if latest_media.as_ref().is_some_and(NowPlaying::is_active)
            && !self.pomodoro.is_compact_active()
            && self.notch_tab == NotchExpandedTab::Pomodoro
        {
            self.notch_tab = NotchExpandedTab::Media;
        }
        self.media = latest_media;
        true
    }

    pub(crate) fn lyrics(&self) -> Option<&otto_core::LyricsSheet> {
        self.lyrics.as_ref()
    }

    #[allow(dead_code)]
    pub(crate) fn set_lyrics(&mut self, lyrics: Option<otto_core::LyricsSheet>) {
        self.lyrics = lyrics;
    }

    #[allow(dead_code)]
    pub(crate) fn lyrics_expanded(&self) -> bool {
        self.lyrics_expanded
    }

    #[allow(dead_code)]
    pub(crate) fn toggle_lyrics(&mut self) -> NotchUpdate {
        self.lyrics_expanded = !self.lyrics_expanded;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn trigger_hud(&mut self, kind: NotchHudKind) {
        if self.hud_expansion < 0.05 {
            self.hud_level = kind.target_level();
        }
        self.hud_kind = Some(kind);
        self.hud_until = Some(Instant::now() + Duration::from_millis(HUD_HOLD_MS));
    }

    pub(crate) fn handle_hotkey(&mut self, hotkey: NotchHotkey) -> NotchUpdate {
        match hotkey {
            NotchHotkey::ToggleNotch => {
                let now = Instant::now();
                if self.is_panel_target_open(now) {
                    self.panel_trigger = PanelTrigger::Closed;
                    self.hover_since = None;
                    self.alert_until = None;
                    self.pomodoro.dismiss_alert();
                } else {
                    self.panel_trigger = PanelTrigger::Pinned;
                }
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            NotchHotkey::TogglePomodoro => {
                self.pomodoro.toggle_run();
                self.restart_clock();
                self.alert_until = None;
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
        }
    }

    pub(crate) fn handle_pointer_move(&mut self, inside_notch: bool) {
        if inside_notch {
            if self.panel_expansion > 0.55 && self.panel_trigger == PanelTrigger::Pinned {
                self.panel_trigger = PanelTrigger::PinnedHovered;
            }
            if self.hover_since.is_none() && self.panel_trigger == PanelTrigger::Closed {
                self.hover_since = Some(Instant::now());
            }
        } else {
            self.hover_since = None;
        }
    }

    pub(crate) fn handle_pointer_leave(&mut self, cursor_inside_notch: bool) -> bool {
        if cursor_inside_notch {
            return false;
        }
        self.hover_since = None;
        if matches!(
            self.panel_trigger,
            PanelTrigger::Hovered | PanelTrigger::PinnedHovered
        ) {
            self.panel_trigger = PanelTrigger::Closed;
            return true;
        }
        false
    }

    pub(crate) fn handle_seekbar_click(&mut self, ratio: f32) -> NotchUpdate {
        let Some(media) = self.media.as_mut() else {
            return NotchUpdate::default();
        };
        let target_ms = ((ratio * media.duration_ms as f32).round() as u64).min(media.duration_ms);
        media.position_ms = target_ms;
        NotchUpdate {
            changed: true,
            effect: Some(NotchEffect::Platform(NotchAction::Media(
                MediaCommand::Seek(target_ms),
            ))),
        }
    }

    pub(crate) fn handle_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroPhase
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive => self.handle_pomodoro_click(target),
            HitTarget::NotchTabPomodoro | HitTarget::NotchTabMedia => self.handle_tab_click(target),
            HitTarget::MediaPlayPause | HitTarget::MediaPrevious | HitTarget::MediaNext => {
                self.handle_media_click(target)
            }
            HitTarget::MediaLyricsToggle => self.toggle_lyrics(),
            HitTarget::Notch => self.handle_notch_click(),
            _ => NotchUpdate::default(),
        }
    }

    fn handle_pomodoro_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::PomodoroPlayPause => self.pomodoro.toggle_run(),
            HitTarget::PomodoroReset => self.pomodoro.reset(),
            HitTarget::PomodoroSkip => self.pomodoro.skip_phase(),
            HitTarget::PomodoroPhase => self.pomodoro.cycle_mode(),
            HitTarget::PomodoroStartNext => {
                self.pomodoro.start_next_from_alert();
                self.alert_until = None;
            }
            HitTarget::PomodoroExtendFive => {
                self.pomodoro.extend_five_minutes();
                self.alert_until = None;
            }
            _ => return NotchUpdate::default(),
        }
        self.restart_clock();
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn handle_tab_click(&mut self, target: HitTarget) -> NotchUpdate {
        self.notch_tab = match target {
            HitTarget::NotchTabPomodoro => NotchExpandedTab::Pomodoro,
            HitTarget::NotchTabMedia => NotchExpandedTab::Media,
            _ => return NotchUpdate::default(),
        };
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn handle_media_click(&mut self, target: HitTarget) -> NotchUpdate {
        let command = match target {
            HitTarget::MediaPlayPause => {
                if let Some(media) = self.media.as_mut() {
                    media.playing = !media.playing;
                }
                MediaCommand::TogglePlayPause
            }
            HitTarget::MediaPrevious => MediaCommand::Previous,
            HitTarget::MediaNext => MediaCommand::Next,
            _ => return NotchUpdate::default(),
        };
        NotchUpdate {
            changed: matches!(target, HitTarget::MediaPlayPause),
            effect: Some(NotchEffect::Platform(NotchAction::Media(command))),
        }
    }

    fn handle_notch_click(&mut self) -> NotchUpdate {
        if self.pomodoro.status == PomodoroStatus::Alert {
            self.pomodoro.dismiss_alert();
            self.alert_until = None;
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        if self.panel_expansion < 0.25 {
            return NotchUpdate {
                changed: false,
                effect: Some(NotchEffect::Platform(NotchAction::OpenTaskView)),
            };
        }
        NotchUpdate::default()
    }

    pub(crate) fn handle_wheel(&mut self, target: HitTarget, delta: i16) -> NotchUpdate {
        if self.panel_expansion <= 0.55
            || !matches!(
                target,
                HitTarget::Notch
                    | HitTarget::PomodoroPhase
                    | HitTarget::PomodoroPlayPause
                    | HitTarget::PomodoroReset
                    | HitTarget::PomodoroSkip
            )
        {
            return NotchUpdate::default();
        }
        let steps = i32::from(delta / 120).clamp(-10, 10);
        let step = if steps == 0 {
            if delta > 0 { 1 } else { -1 }
        } else {
            steps
        };
        NotchUpdate {
            changed: self.pomodoro.adjust_minutes(step),
            effect: None,
        }
    }

    pub(crate) fn advance(
        &mut self,
        now: Instant,
        cursor_inside_notch: bool,
        panel_visible: bool,
        motion: MotionPreference,
    ) -> NotchUpdate {
        let dt = now
            .saturating_duration_since(self.last_anim_instant)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_anim_instant = now;
        self.anim_phase = (self.anim_phase + dt) % 1000.0;

        let (mut changed, effect) = self.step_pomodoro_clock(now);
        self.step_hover_tracking(now, cursor_inside_notch, panel_visible);

        let hud_active = self.hud_until.is_some_and(|until| now < until);
        let media_active = self.media.as_ref().is_some_and(NowPlaying::is_active);
        let target_hud = if hud_active { 1.0 } else { 0.0 };
        let target_compact = if self.pomodoro.is_compact_active() || media_active {
            1.0
        } else {
            0.0
        };
        let target_panel = if self.is_panel_target_open(now) {
            1.0
        } else {
            0.0
        };

        let next_hud = animate_value(self.hud_expansion, target_hud, 18.0, dt, motion);
        let next_compact = animate_value(self.compact_expansion, target_compact, 16.0, dt, motion);
        let next_panel = animate_value(self.panel_expansion, target_panel, 20.0, dt, motion);
        let target_level = self
            .hud_kind
            .map_or(self.hud_level, NotchHudKind::target_level);
        let next_level = animate_value(self.hud_level, target_level, 22.0, dt, motion);

        if !hud_active && next_hud <= 0.008 {
            if self.hud_expansion != 0.0 || self.hud_kind.is_some() {
                self.hud_expansion = 0.0;
                self.hud_kind = None;
                changed = true;
            }
        } else if (next_hud - self.hud_expansion).abs() > 0.0005 {
            self.hud_expansion = next_hud;
            changed = true;
        }

        if (next_level - self.hud_level).abs() > 0.0005 {
            self.hud_level = next_level;
            changed = true;
        }

        let resolved_compact = if next_compact <= 0.008 && target_compact < 0.5 {
            0.0
        } else if next_compact >= 0.995 && target_compact > 0.5 {
            1.0
        } else {
            next_compact
        };
        if (resolved_compact - self.compact_expansion).abs() > 0.0005 {
            self.compact_expansion = resolved_compact;
            changed = true;
        }

        let resolved_panel = if next_panel <= 0.008 && target_panel < 0.5 {
            0.0
        } else if next_panel >= 0.995 && target_panel > 0.5 {
            1.0
        } else {
            next_panel
        };
        if (resolved_panel - self.panel_expansion).abs() > 0.0005 {
            self.panel_expansion = resolved_panel;
            changed = true;
        }

        NotchUpdate { changed, effect }
    }

    fn apply_tick(&mut self, tick: PomodoroTick, now: Instant) -> (bool, Option<NotchEffect>) {
        if tick == PomodoroTick::Completed {
            self.alert_until = Some(now + Duration::from_millis(ALERT_HOLD_MS));
            return (
                true,
                Some(NotchEffect::Platform(NotchAction::PlayPomodoroChime)),
            );
        }
        (tick != PomodoroTick::Unchanged, None)
    }

    fn step_pomodoro_clock(&mut self, now: Instant) -> (bool, Option<NotchEffect>) {
        let now_ms = self.millis(now);
        let tick = self.clock.sync(&mut self.pomodoro, now_ms);
        let (mut changed, effect) = self.apply_tick(tick, now);
        if self.hud_until.is_some_and(|until| now >= until) {
            self.hud_until = None;
            changed = true;
        }
        if self.pomodoro.status == PomodoroStatus::Alert
            && let Some(until) = self.alert_until
            && now >= until
            && self.panel_trigger == PanelTrigger::Closed
        {
            self.pomodoro.dismiss_alert();
            self.alert_until = None;
            changed = true;
        }
        (changed, effect)
    }

    fn step_hover_tracking(&mut self, now: Instant, inside_notch: bool, panel_visible: bool) {
        if self.hover_since.is_none()
            && self.panel_trigger == PanelTrigger::Closed
            && !panel_visible
        {
            return;
        }
        if inside_notch {
            if self.panel_expansion > 0.55 && self.panel_trigger == PanelTrigger::Pinned {
                self.panel_trigger = PanelTrigger::PinnedHovered;
            }
            if self.panel_trigger == PanelTrigger::Closed
                && let Some(since) = self.hover_since
                && now.duration_since(since) >= Duration::from_millis(HOVER_DEBOUNCE_MS)
            {
                self.panel_trigger = PanelTrigger::Hovered;
            }
        } else {
            self.hover_since = None;
            if matches!(
                self.panel_trigger,
                PanelTrigger::Hovered | PanelTrigger::PinnedHovered
            ) {
                self.panel_trigger = PanelTrigger::Closed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ALERT_HOLD_MS, HUD_HOLD_MS, NotchController, NotchEffect, NotchHotkey};
    use otto_core::{
        HitTarget, MediaCommand, MotionPreference, NotchAction, NotchExpandedTab, NotchHudKind,
        NowPlaying, PomodoroStatus,
    };
    use std::time::{Duration, Instant};

    fn idle_notch() -> NotchController {
        NotchController::new(None, NotchExpandedTab::Pomodoro)
    }

    #[test]
    fn pomodoro_advances_from_clock_syncs_without_animation_frames() {
        let mut notch = idle_notch();
        let start = notch.epoch;
        let _ = notch.sync_clock(start);
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch.clock.restart(0);
        let update = notch.sync_clock(start + Duration::from_secs(3));
        assert!(update.changed);
        assert_eq!(notch.pomodoro().remaining_seconds, 25 * 60 - 3);
    }

    #[test]
    fn running_pomodoro_schedules_a_wakeup_at_the_next_second() {
        let mut notch = idle_notch();
        let start = notch.epoch;
        assert_eq!(notch.next_deadline(start), None);
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch.clock.restart(0);
        assert_eq!(
            notch.next_deadline(start),
            Some(start + Duration::from_secs(1))
        );
    }

    #[test]
    fn hud_hold_expires_through_its_deadline_and_collapses_fully() {
        let mut notch = idle_notch();
        notch.trigger_hud(NotchHudKind::Volume {
            percent: 40,
            muted: false,
        });
        let deadline = notch.next_deadline(Instant::now()).expect("hud deadline");
        let mut now = deadline
            .checked_sub(Duration::from_millis(HUD_HOLD_MS))
            .expect("hud deadline follows its hold");
        for _ in 0..40 {
            now += Duration::from_millis(16);
            let _ = notch.advance(now, false, false, MotionPreference::Full);
        }
        assert!(notch.hud_state().is_some_and(|hud| hud.expansion > 0.9));
        let update = notch.sync_clock(deadline);
        assert!(update.changed);
        assert_eq!(notch.next_deadline(deadline), None);
        let mut now = deadline;
        for _ in 0..200 {
            now += Duration::from_millis(16);
            let _ = notch.advance(now, false, false, MotionPreference::Full);
        }
        assert!(notch.hud_state().is_none());
        assert!(
            !notch
                .advance(now, false, false, MotionPreference::Full)
                .changed
        );
    }

    #[test]
    fn completed_session_alert_dismisses_itself_at_its_deadline() {
        let mut notch = idle_notch();
        let start = notch.epoch;
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch.clock.restart(0);
        let mut now = start;
        let mut chimes = 0;
        while chimes == 0 && now < start + Duration::from_mins(30) {
            now += Duration::from_secs(20);
            if notch.sync_clock(now).effect.is_some() {
                chimes += 1;
            }
        }
        assert_eq!(chimes, 1);
        assert_eq!(notch.pomodoro().status, PomodoroStatus::Alert);
        let deadline = notch.next_deadline(now).expect("alert deadline");
        assert!(deadline <= now + Duration::from_millis(ALERT_HOLD_MS));
        assert!(notch.is_panel_target_open(now));
        let update = notch.sync_clock(deadline);
        assert!(update.changed);
        assert_eq!(notch.pomodoro().status, PomodoroStatus::Idle);
        assert!(!notch.is_panel_target_open(deadline));
        assert_eq!(notch.next_deadline(deadline), None);
    }

    #[test]
    fn panel_animation_reaches_exact_endpoints_and_then_stops_changing() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        let mut now = Instant::now();
        for _ in 0..400 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, false, true, MotionPreference::Full);
        }
        assert!((notch.panel_expansion() - 1.0).abs() < f32::EPSILON);
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        for _ in 0..400 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, false, true, MotionPreference::Full);
        }
        assert!(notch.panel_expansion().abs() < f32::EPSILON);
        assert!(
            !notch
                .advance(now, false, false, MotionPreference::Full)
                .changed
        );
    }

    #[test]
    fn reduced_motion_jumps_to_the_target_in_one_step() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        let now = Instant::now() + Duration::from_millis(8);
        let update = notch.advance(now, false, true, MotionPreference::Reduced);
        assert!(update.changed);
        assert!((notch.panel_expansion() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sleep_does_not_consume_a_running_session() {
        let mut notch = idle_notch();
        let start = notch.epoch;
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch.clock.restart(0);
        let _ = notch.suspend(start + Duration::from_secs(2));
        notch.resume(start + Duration::from_hours(2));
        let _ = notch.sync_clock(start + Duration::from_hours(2) + Duration::from_secs(1));
        assert_eq!(notch.pomodoro().remaining_seconds, 25 * 60 - 3);
    }

    #[test]
    fn media_play_pause_updates_optimistic_state_and_returns_effect() {
        let media = NowPlaying {
            title: String::from("Track"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 10_000,
            duration_ms: 60_000,
            thumbnail: None,
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);

        let update = notch.handle_click(HitTarget::MediaPlayPause);

        assert!(update.changed);
        assert_eq!(notch.media().map(|item| item.playing), Some(false));
        assert_eq!(
            update.effect,
            Some(NotchEffect::Platform(NotchAction::Media(
                MediaCommand::TogglePlayPause
            )))
        );
    }

    #[test]
    fn media_seekbar_click_updates_position_and_returns_seek_effect() {
        let media = NowPlaying {
            title: String::from("Track"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 0,
            duration_ms: 200_000,
            thumbnail: None,
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);

        let update = notch.handle_seekbar_click(0.5);

        assert!(update.changed);
        assert_eq!(notch.media().map(|item| item.position_ms), Some(100_000));
        assert_eq!(
            update.effect,
            Some(NotchEffect::Platform(NotchAction::Media(
                MediaCommand::Seek(100_000)
            )))
        );
    }

    #[test]
    fn hotkeys_toggle_the_notch_without_platform_access() {
        let mut notch = NotchController::new(None, NotchExpandedTab::Pomodoro);

        let update = notch.handle_hotkey(NotchHotkey::ToggleNotch);

        assert!(update.changed);
        assert!(notch.is_panel_target_open(std::time::Instant::now()));
    }

    #[test]
    fn media_snapshot_activates_media_tab_when_pomodoro_is_idle() {
        let mut notch = NotchController::new(None, NotchExpandedTab::Pomodoro);
        let media = NowPlaying {
            title: String::from("Track"),
            artist: String::new(),
            playing: true,
            position_ms: 0,
            duration_ms: 0,
            thumbnail: None,
        };

        assert!(notch.set_media(Some(media)));
        assert_eq!(notch.layout_input(true).active_tab, NotchExpandedTab::Media);
    }

    #[test]
    fn settled_media_does_not_force_animation_repaints() {
        let media = NowPlaying {
            title: String::from("Track"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 0,
            duration_ms: 200_000,
            thumbnail: None,
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        let now = std::time::Instant::now();
        notch.compact_expansion = 1.0;
        notch.last_anim_instant = now;

        let update = notch.advance(now, false, false, MotionPreference::Full);

        assert!(!update.changed);
    }

    #[test]
    fn media_lyrics_toggle_updates_lyrics_expanded_and_layout_input() {
        let media = NowPlaying {
            title: String::from("Track"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 0,
            duration_ms: 200_000,
            thumbnail: None,
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);

        assert!(!notch.lyrics_expanded());
        assert!(!notch.layout_input(true).lyrics_expanded);

        let update = notch.handle_click(HitTarget::MediaLyricsToggle);
        assert!(update.changed);
        assert_eq!(update.effect, None);
        assert!(notch.lyrics_expanded());
        assert!(notch.layout_input(true).lyrics_expanded);

        let update2 = notch.handle_click(HitTarget::MediaLyricsToggle);
        assert!(update2.changed);
        assert!(!notch.lyrics_expanded());
        assert!(!notch.layout_input(true).lyrics_expanded);
    }

    #[test]
    fn track_change_resets_loaded_lyrics() {
        let media = NowPlaying {
            title: String::from("Track 1"),
            artist: String::from("Artist 1"),
            playing: true,
            position_ms: 0,
            duration_ms: 200_000,
            thumbnail: None,
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        let sheet = otto_core::parse_lrc("[00:01.00]Hello world");
        notch.set_lyrics(Some(sheet));
        assert!(notch.lyrics().is_some());

        let same_track_updated_pos = NowPlaying {
            title: String::from("Track 1"),
            artist: String::from("Artist 1"),
            playing: true,
            position_ms: 50_000,
            duration_ms: 200_000,
            thumbnail: None,
        };
        notch.set_media(Some(same_track_updated_pos));
        assert!(notch.lyrics().is_some());

        let different_track = NowPlaying {
            title: String::from("Track 2"),
            artist: String::from("Artist 2"),
            playing: true,
            position_ms: 0,
            duration_ms: 180_000,
            thumbnail: None,
        };
        notch.set_media(Some(different_track));
        assert!(notch.lyrics().is_none());
    }
}
