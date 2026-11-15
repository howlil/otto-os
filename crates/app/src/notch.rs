use std::f32::consts::TAU;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use otto_core::{
    ClipboardPayload, ClipboardState, ClockTime, CompactContent, CompactSlotKind, FocusSubMode,
    HitTarget, IslandInputs, LYRIC_DRIFT_TOLERANCE_MS, LyricsStatus, LyricsView, MediaCommand,
    MediaPeek, MediaPeekFrame, MediaScene, MotionMetrics, MotionPreference, NotchAction,
    NotchExpandedTab, NotchHudKind, NotchHudState, NotchLayoutInput, NowPlaying,
    POMODORO_PRESETS_MINUTES, POSITION_DRIFT_TOLERANCE_MS, PendingSeek, PomodoroClock,
    PomodoroPeek, PomodoroPeekFrame, PomodoroScene, PomodoroState, PomodoroStatus, PomodoroSurface,
    PomodoroTick, PrivacyLayoutState, PrivacyState, QuickTimerState, SHELF_HOVER_LEAVE_MS,
    SeekReadback, ShelfLayoutInput, ShelfPayload, ShelfState, ShelfThumbnail, StopwatchClock,
    StopwatchState, TIMER_PRESETS_MINUTES, TrackKey, animate_value, hud_content_alpha,
    is_openable_shelf_url, is_valid_shelf_file_path, island_targets, reconcile_seek,
    seek_target_ms, warning_threshold_crossed,
};

const ALERT_HOLD_MS: u64 = 10_000;
const HOVER_DEBOUNCE_MS: u64 = 80;
const PEEK_FADE_SECONDS: f32 = 0.18;
const GLOW_BREATH_FLOOR: f32 = 0.7;
const GLOW_SCENE_VISIBLE: f32 = 0.05;
const CLIPBOARD_COPIED_BADGE_MS: u64 = 1_200;

struct ActivePeek {
    text: String,
    shown_at: Instant,
    until: Instant,
}

struct MediaPeekState {
    label: &'static str,
    title: String,
    shown_at: Instant,
    until: Instant,
}

pub(crate) fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotchEffect {
    Platform(NotchAction),
    PickShelfFiles,
    CopyClipboard(ClipboardPayload),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotchHotkey {
    ToggleNotch,
    TogglePomodoro,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct NotchUpdate {
    pub changed: bool,
    pub effect: Option<NotchEffect>,
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct NotchController {
    hud_kind: Option<NotchHudKind>,
    hud_expansion: f32,
    hud_level: f32,
    hud_until: Option<Instant>,
    pomodoro: PomodoroState,
    focus_mode: FocusSubMode,
    quick_timer: QuickTimerState,
    stopwatch: StopwatchState,
    privacy: PrivacyState,
    shelf: ShelfState,
    clipboard: ClipboardState,
    clipboard_copied_until: Option<Instant>,
    shelf_drag_leave_until: Option<Instant>,
    opened_by_shelf_drag: bool,
    modal_picker_open: bool,
    drag_out_active: bool,
    compact_expansion: f32,
    secondary_expansion: f32,
    compact_swap_blend: f32,
    panel_expansion: f32,
    hover_since: Option<Instant>,
    panel_trigger: PanelTrigger,
    alert_until: Option<Instant>,
    clock: PomodoroClock,
    timer_clock: PomodoroClock,
    stopwatch_clock: StopwatchClock,
    epoch: Instant,
    last_anim_instant: Instant,
    media: Option<NowPlaying>,
    notch_tab: NotchExpandedTab,
    previous_tab: NotchExpandedTab,
    anim_phase: f32,
    lyrics_expanded: bool,
    lyrics_progress: f32,
    tab_blend: f32,
    motion: MotionPreference,
    lyrics: Option<otto_core::LyricsSheet>,
    lyrics_status: LyricsStatus,
    pending_seek: Option<PendingSeek>,
    seek_drag: Option<f32>,
    media_primary: bool,
    dismissed: Option<CompactContent>,
    dismissed_secondary: Option<CompactContent>,
    last_primary_slot: Option<CompactSlotKind>,
    last_secondary_slot: Option<CompactSlotKind>,
    hover_blocked: bool,
    clock_time: ClockTime,
    presented: bool,
    glow_presence: f32,
    dim_presence: f32,
    pulse_presence: f32,
    scene_step_at: Instant,
    peek: Option<ActivePeek>,
    media_peek: Option<MediaPeekState>,
    media_paused_at: Option<Instant>,
    media_dim_presence: f32,
    lyrics_in_compact: bool,
    focus_enabled: bool,
    shelf_enabled: bool,
    clipboard_enabled: bool,
    media_enabled: bool,
    privacy_enabled: bool,
    hud_enabled: bool,
    compact_title_points: f32,
    compact_title_target: f32,
    compact_title_measured: Option<(String, f32)>,
}

impl NotchController {
    pub(crate) fn new(media: Option<NowPlaying>, notch_tab: NotchExpandedTab) -> Self {
        let initial_primary = media
            .as_ref()
            .is_some_and(NowPlaying::is_active)
            .then_some(CompactSlotKind::Media);
        Self {
            hud_kind: None,
            hud_expansion: 0.0,
            hud_level: 0.0,
            hud_until: None,
            pomodoro: PomodoroState::new(),
            focus_mode: FocusSubMode::Pomodoro,
            quick_timer: QuickTimerState::new(),
            stopwatch: StopwatchState::new(),
            privacy: PrivacyState::default(),
            shelf: ShelfState::default(),
            clipboard: ClipboardState::default(),
            clipboard_copied_until: None,
            shelf_drag_leave_until: None,
            opened_by_shelf_drag: false,
            modal_picker_open: false,
            drag_out_active: false,
            compact_expansion: 0.0,
            secondary_expansion: 0.0,
            compact_swap_blend: 1.0,
            panel_expansion: 0.0,
            hover_since: None,
            panel_trigger: PanelTrigger::Closed,
            alert_until: None,
            clock: PomodoroClock::default(),
            timer_clock: PomodoroClock::default(),
            stopwatch_clock: StopwatchClock::default(),
            epoch: Instant::now(),
            last_anim_instant: Instant::now(),
            media,
            notch_tab,
            previous_tab: notch_tab,
            anim_phase: 0.0,
            lyrics_expanded: false,
            lyrics_progress: 0.0,
            tab_blend: 1.0,
            motion: MotionPreference::Full,
            lyrics: None,
            lyrics_status: LyricsStatus::Idle,
            pending_seek: None,
            seek_drag: None,
            media_primary: false,
            dismissed: None,
            dismissed_secondary: None,
            last_primary_slot: initial_primary,
            last_secondary_slot: None,
            hover_blocked: false,
            clock_time: ClockTime::default(),
            presented: true,
            glow_presence: 0.0,
            dim_presence: 0.0,
            pulse_presence: 0.0,
            scene_step_at: Instant::now(),
            peek: None,
            media_peek: None,
            media_paused_at: None,
            media_dim_presence: 0.0,
            lyrics_in_compact: false,
            focus_enabled: true,
            shelf_enabled: true,
            clipboard_enabled: true,
            media_enabled: true,
            privacy_enabled: true,
            hud_enabled: true,
            compact_title_points: 0.0,
            compact_title_target: 0.0,
            compact_title_measured: None,
        }
    }

    pub(crate) fn set_lyrics_in_compact(&mut self, enabled: bool) {
        self.lyrics_in_compact = enabled;
    }

    pub(crate) const fn shelf_enabled(&self) -> bool {
        self.shelf_enabled
    }

    pub(crate) const fn clipboard_enabled(&self) -> bool {
        self.clipboard_enabled
    }

    pub(crate) const fn media_enabled(&self) -> bool {
        self.media_enabled
    }

    pub(crate) const fn privacy_enabled(&self) -> bool {
        self.privacy_enabled
    }

    pub(crate) const fn hud_enabled(&self) -> bool {
        self.hud_enabled
    }

    pub(crate) fn is_tab_enabled(&self, tab: NotchExpandedTab) -> bool {
        match tab {
            NotchExpandedTab::Pomodoro => self.focus_enabled,
            NotchExpandedTab::Shelf => self.shelf_enabled,
            NotchExpandedTab::Clipboard => self.clipboard_enabled,
            NotchExpandedTab::Media => self.media_enabled && self.media.is_some(),
        }
    }

    pub(crate) fn has_openable_panel(&self) -> bool {
        self.focus_enabled
            || self.shelf_enabled
            || self.clipboard_enabled
            || (self.media_enabled && self.media.is_some())
    }

    pub(crate) fn first_enabled_tab(&self) -> Option<NotchExpandedTab> {
        [
            NotchExpandedTab::Pomodoro,
            NotchExpandedTab::Shelf,
            NotchExpandedTab::Clipboard,
            NotchExpandedTab::Media,
        ]
        .into_iter()
        .find(|&tab| self.is_tab_enabled(tab))
    }

    fn ensure_valid_tab(&mut self) {
        if self.is_tab_enabled(self.notch_tab) {
            return;
        }
        if let Some(fallback) = self.first_enabled_tab() {
            self.notch_tab = fallback;
            self.previous_tab = fallback;
            self.tab_blend = 1.0;
        } else {
            self.panel_trigger = PanelTrigger::Closed;
            self.panel_expansion = 0.0;
            self.hover_since = None;
        }
    }

    fn clear_compact_slot(&mut self, slot: CompactSlotKind) {
        if self.last_primary_slot == Some(slot) {
            self.last_primary_slot = None;
            self.compact_expansion = 0.0;
        }
        if self.last_secondary_slot == Some(slot) {
            self.last_secondary_slot = None;
            self.secondary_expansion = 0.0;
        }
    }

    pub(crate) fn set_focus_enabled(&mut self, enabled: bool) {
        if self.focus_enabled == enabled {
            return;
        }
        self.focus_enabled = enabled;
        if !enabled {
            match self.pomodoro.status {
                PomodoroStatus::Running => self.pomodoro.toggle_run(),
                PomodoroStatus::Alert => self.pomodoro.dismiss_alert(),
                PomodoroStatus::Idle | PomodoroStatus::Paused => {}
            }
            match self.quick_timer.status {
                PomodoroStatus::Running => self.quick_timer.toggle_run(),
                PomodoroStatus::Alert => self.quick_timer.dismiss_alert(),
                PomodoroStatus::Idle | PomodoroStatus::Paused => {}
            }
            if self.stopwatch.running {
                self.stopwatch.toggle_run();
            }
            self.alert_until = None;
            self.peek = None;
            self.glow_presence = 0.0;
            self.dim_presence = 0.0;
            self.pulse_presence = 0.0;
            self.clear_compact_slot(CompactSlotKind::Focus);
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_shelf_enabled(&mut self, enabled: bool) {
        if self.shelf_enabled == enabled {
            return;
        }
        self.shelf_enabled = enabled;
        if !enabled {
            self.opened_by_shelf_drag = false;
            self.modal_picker_open = false;
            self.drag_out_active = false;
            self.shelf_drag_leave_until = None;
            self.shelf.drag_hover = false;
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_clipboard_enabled(&mut self, enabled: bool) {
        if self.clipboard_enabled == enabled {
            return;
        }
        self.clipboard_enabled = enabled;
        if !enabled {
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_media_enabled(&mut self, enabled: bool) {
        if self.media_enabled == enabled {
            return;
        }
        self.media_enabled = enabled;
        if !enabled {
            self.media = None;
            self.lyrics = None;
            self.lyrics_status = LyricsStatus::Idle;
            self.lyrics_expanded = false;
            self.lyrics_progress = 0.0;
            self.pending_seek = None;
            self.seek_drag = None;
            self.media_paused_at = None;
            self.media_peek = None;
            self.media_dim_presence = 0.0;
            self.clear_compact_slot(CompactSlotKind::Media);
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn set_privacy_enabled(&mut self, enabled: bool) {
        if self.privacy_enabled == enabled {
            return;
        }
        self.privacy_enabled = enabled;
        if !enabled {
            self.privacy = PrivacyState::default();
        }
    }

    pub(crate) fn set_hud_enabled(&mut self, enabled: bool) {
        if self.hud_enabled == enabled {
            return;
        }
        self.hud_enabled = enabled;
        if !enabled {
            self.hud_kind = None;
            self.hud_until = None;
            self.hud_expansion = 0.0;
            self.hud_level = 0.0;
        }
    }

    pub(crate) fn sync_compact_title_width(
        &mut self,
        now: Instant,
        now_unix_ms: i64,
        measure: impl FnOnce(&str) -> f32,
    ) -> bool {
        let mode = otto_core::resolve_island(&self.island_inputs(now));
        let compact_media = matches!(
            mode,
            otto_core::IslandMode::Compact(CompactContent::Media(_))
        );
        let holds_width = matches!(
            mode,
            otto_core::IslandMode::Expanded
                | otto_core::IslandMode::Alert
                | otto_core::IslandMode::Hud
        ) || (compact_media && self.media_peek.is_some());
        if holds_width {
            return false;
        }
        let lyric = if compact_media {
            self.display_media(now_unix_ms)
                .and_then(|media| self.compact_lyric_line(&media))
        } else {
            None
        };
        let target = match lyric {
            None => 0.0,
            Some(line) if line.trim().is_empty() => return false,
            Some(line) => match &self.compact_title_measured {
                Some((text, points)) if *text == line => *points,
                _ => {
                    let points = measure(&line);
                    self.compact_title_measured = Some((line, points));
                    points
                }
            },
        };
        let moved = (target - self.compact_title_target).abs() > f32::EPSILON;
        self.compact_title_target = target;
        moved
    }

    fn paused_hold_track(&self, now: Instant) -> Option<TrackKey> {
        let media = self
            .media
            .as_ref()
            .filter(|media| !media.playing && !media.title.trim().is_empty())?;
        let hold = Duration::from_millis(MotionMetrics::default().media_pause_hold_ms);
        let paused_at = self.media_paused_at?;
        (now < paused_at + hold).then(|| media.track_key())
    }

    pub(crate) fn set_clock(&mut self, clock: ClockTime) {
        self.clock_time = clock;
    }

    pub(crate) fn set_presented(&mut self, presented: bool) {
        self.presented = presented;
    }

    fn is_mode_compact_active(&self, mode: FocusSubMode) -> bool {
        match mode {
            FocusSubMode::Pomodoro => self.pomodoro.is_compact_active(),
            FocusSubMode::Timer => self.quick_timer.is_compact_active(),
            FocusSubMode::Stopwatch => self.stopwatch.is_compact_active(),
        }
    }

    fn compact_focus_mode(&self) -> Option<FocusSubMode> {
        if self.is_mode_compact_active(self.focus_mode) {
            return Some(self.focus_mode);
        }
        FocusSubMode::ALL
            .into_iter()
            .find(|mode| self.is_mode_compact_active(*mode))
    }

    fn has_compact_focus(&self) -> bool {
        self.focus_enabled && self.compact_focus_mode().is_some()
    }

    fn has_alert(&self) -> bool {
        self.focus_enabled
            && (self.pomodoro.status == PomodoroStatus::Alert
                || self.quick_timer.status == PomodoroStatus::Alert)
    }

    fn active_focus_status(&self) -> PomodoroStatus {
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.pomodoro.status,
            FocusSubMode::Timer => self.quick_timer.status,
            FocusSubMode::Stopwatch => self.stopwatch.status(),
        }
    }

    fn active_is_last_minute(&self) -> bool {
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.pomodoro.is_last_minute(),
            FocusSubMode::Timer => self.quick_timer.is_last_minute(),
            FocusSubMode::Stopwatch => false,
        }
    }

    fn is_stopwatch_visible(&self) -> bool {
        self.presented
            && ((self.panel_expansion > GLOW_SCENE_VISIBLE
                && self.notch_tab == NotchExpandedTab::Pomodoro
                && self.focus_mode == FocusSubMode::Stopwatch)
                || (self.compact_expansion > GLOW_SCENE_VISIBLE
                    && !self.media_primary
                    && self.compact_focus_mode() == Some(FocusSubMode::Stopwatch)))
    }

    fn active_surface(&self) -> PomodoroSurface {
        match self.focus_mode {
            FocusSubMode::Pomodoro => PomodoroSurface::of_state(&self.pomodoro),
            FocusSubMode::Timer => PomodoroSurface::of_timer_state(&self.quick_timer),
            FocusSubMode::Stopwatch => PomodoroSurface::of_stopwatch_state(&self.stopwatch),
        }
    }

    fn dismiss_active_alert(&mut self) {
        self.pomodoro.dismiss_alert();
        self.quick_timer.dismiss_alert();
        self.alert_until = None;
    }

    fn island_inputs(&self, now: Instant) -> IslandInputs {
        IslandInputs {
            alert: self.has_alert() && self.alert_until.is_none_or(|until| now < until),
            panel_requested: self.has_openable_panel()
                && (self.panel_trigger.is_open()
                    || (self.shelf_enabled && self.is_shelf_hold_open())),
            hud: self.hud_enabled && self.hud_until.is_some_and(|until| now < until),
            focus_active: self.has_compact_focus(),
            media: self
                .media
                .as_ref()
                .filter(|media| self.media_enabled && media.is_active())
                .map(NowPlaying::track_key),
            media_paused_hold: if self.media_enabled {
                self.paused_hold_track(now)
            } else {
                None
            },
            media_primary: self.media_primary,
            dismissed: self.dismissed.clone(),
            dismissed_secondary: self.dismissed_secondary.clone(),
        }
    }

    #[cfg(test)]
    fn island_mode(&self, now: Instant) -> otto_core::IslandMode {
        otto_core::resolve_island(&self.island_inputs(now))
    }

    fn record_dismissed(&mut self, content: CompactContent) {
        if self.dismissed.as_ref() == Some(&content)
            || self.dismissed_secondary.as_ref() == Some(&content)
        {
            return;
        }
        if self.dismissed.is_none() {
            self.dismissed = Some(content);
        } else {
            self.dismissed_secondary = Some(content);
        }
    }

    fn clear_focus_dismissed(&mut self) {
        if self.dismissed == Some(CompactContent::Focus) {
            self.dismissed = self.dismissed_secondary.take();
        } else if self.dismissed_secondary == Some(CompactContent::Focus) {
            self.dismissed_secondary = None;
        }
    }

    fn sync_tab_to_primary(&mut self, now: Instant) {
        let Some(primary) = self.island_inputs(now).compact_content() else {
            self.ensure_valid_tab();
            return;
        };
        let target = match primary.kind() {
            CompactSlotKind::Focus => NotchExpandedTab::Pomodoro,
            CompactSlotKind::Media => NotchExpandedTab::Media,
        };
        if self.is_tab_enabled(target) {
            self.notch_tab = target;
            self.previous_tab = target;
            self.tab_blend = 1.0;
        } else {
            self.ensure_valid_tab();
        }
    }

    pub(crate) fn dismiss_compact(&mut self, now: Instant) -> NotchUpdate {
        if matches!(
            self.panel_trigger,
            PanelTrigger::Pinned | PanelTrigger::PinnedHovered
        ) {
            return NotchUpdate::default();
        }
        let inputs = self.island_inputs(now);
        let Some(content) = inputs.compact_content() else {
            return NotchUpdate::default();
        };
        let had_secondary = inputs.secondary_compact_content().is_some();
        self.record_dismissed(content);
        if had_secondary {
            self.compact_swap_blend = 0.0;
            self.sync_tab_to_primary(now);
        }
        self.panel_trigger = PanelTrigger::Closed;
        self.hover_since = None;
        self.hover_blocked = true;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn dismiss_secondary_compact(&mut self, now: Instant) -> NotchUpdate {
        let Some(content) = self.island_inputs(now).secondary_compact_content() else {
            return NotchUpdate::default();
        };
        self.record_dismissed(content);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn millis(&self, moment: Instant) -> u64 {
        u64::try_from(moment.saturating_duration_since(self.epoch).as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn sync_clock(&mut self, now: Instant) -> NotchUpdate {
        let (mut changed, effect) = self.step_pomodoro_clock(now);
        if self.step_scene_at(now, self.motion) {
            changed = true;
        }
        NotchUpdate {
            changed: changed
                || self.shows_live_media_progress()
                || self.shows_live_lyrics()
                || self.scene_frame_active(),
            effect,
        }
    }

    fn shows_live_media_progress(&self) -> bool {
        let panel_shows_media =
            self.panel_expansion > 0.05 && self.notch_tab == NotchExpandedTab::Media;
        let secondary_shows_media = self.secondary_expansion > 0.05;
        self.presented
            && (panel_shows_media || secondary_shows_media)
            && self.media.as_ref().is_some_and(|media| media.playing)
    }

    fn shows_live_lyrics(&self) -> bool {
        self.lyrics_visible_now() && self.media.as_ref().is_some_and(|media| media.playing)
    }

    fn lyrics_visible_now(&self) -> bool {
        let Some(sheet) = self.lyrics.as_ref() else {
            return false;
        };
        if sheet.is_empty() || !self.presented {
            return false;
        }
        let panel_shows_lyrics = self.panel_expansion > 0.05
            && self.notch_tab == NotchExpandedTab::Media
            && (self.lyrics_expanded || self.lyrics_progress > 0.05);
        let compact_shows_lyrics = self.compact_expansion > 0.05
            && (!self.has_compact_focus() || self.media_primary)
            && self.lyrics_in_compact;
        panel_shows_lyrics || compact_shows_lyrics
    }

    fn media_repaint_in(&self, now_unix_ms: i64) -> Option<Duration> {
        if !self.presented {
            return None;
        }
        let media = self.media.as_ref().filter(|media| media.playing)?;
        let second_wait = self
            .shows_live_media_progress()
            .then(|| media.next_whole_second_in(now_unix_ms))
            .flatten();
        let lyric_wait = if self.lyrics_visible_now() && media.position_updated_unix_ms.is_some() {
            self.lyrics.as_ref().and_then(|sheet| {
                sheet.next_line_delay_ms(
                    media.position_at(now_unix_ms),
                    media.playback_rate_permille,
                )
            })
        } else {
            None
        };
        match (second_wait, lyric_wait) {
            (Some(a), Some(b)) => Some(Duration::from_millis(a.min(b))),
            (Some(a), None) | (None, Some(a)) => Some(Duration::from_millis(a)),
            (None, None) => None,
        }
    }

    pub(crate) fn suspend(&mut self, now: Instant) -> NotchUpdate {
        let now_ms = self.millis(now);
        let tick = self.clock.suspend(&mut self.pomodoro, now_ms);
        let timer_tick = self
            .timer_clock
            .suspend_timer(&mut self.quick_timer, now_ms);
        let sw_changed = self.stopwatch_clock.suspend(&mut self.stopwatch, now_ms);
        let (mut changed, mut effect) = self.apply_tick(tick, now);
        if tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Pomodoro;
        }
        let (timer_changed, timer_effect) = self.apply_tick(timer_tick, now);
        if timer_tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Timer;
        }
        changed |= timer_changed || sw_changed;
        if effect.is_none() {
            effect = timer_effect;
        }
        NotchUpdate { changed, effect }
    }

    pub(crate) fn resume(&mut self, now: Instant) {
        let now_ms = self.millis(now);
        self.clock.resume(now_ms);
        self.timer_clock.resume(now_ms);
        self.stopwatch_clock.resume(now_ms);
    }

    pub(crate) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let now_ms = self.millis(now);
        let pomodoro = self
            .clock
            .next_wakeup_ms(&self.pomodoro, now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let timer = self
            .timer_clock
            .next_timer_wakeup_ms(&self.quick_timer, now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let stopwatch = self
            .stopwatch_clock
            .next_wakeup_ms(&self.stopwatch, now_ms, self.is_stopwatch_visible())
            .map(|at| self.epoch + Duration::from_millis(at));
        let alert = self.alert_until.filter(|_| self.has_alert());
        let media = self.media_repaint_in(unix_now_ms()).map(|wait| now + wait);
        let metrics = MotionMetrics::default();
        let frame = self
            .scene_frame_active()
            .then(|| now + Duration::from_millis(metrics.pomodoro_glow_frame_ms));
        let peek = self.peek.as_ref().map(|peek| peek.until);
        let media_peek = self.media_peek.as_ref().map(|peek| peek.until);
        let paused_hold = self
            .media_paused_at
            .map(|at| at + Duration::from_millis(metrics.media_pause_hold_ms));
        [
            pomodoro,
            timer,
            stopwatch,
            alert,
            self.hud_until,
            media,
            frame,
            peek,
            media_peek,
            paused_hold,
            self.shelf_drag_leave_until,
            self.clipboard_copied_until,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(crate) fn layout_input(&self, control_center_enabled: bool) -> NotchLayoutInput {
        let inputs = self.island_inputs(self.last_anim_instant);
        let compact_primary = inputs
            .compact_content()
            .map(|content| content.kind())
            .or_else(|| {
                (self.compact_expansion > 0.0)
                    .then_some(self.last_primary_slot)
                    .flatten()
            });
        let compact_secondary = inputs
            .secondary_compact_content()
            .map(|content| content.kind())
            .or_else(|| {
                (self.secondary_expansion > 0.0)
                    .then_some(self.last_secondary_slot)
                    .flatten()
            });
        NotchLayoutInput {
            hud_expansion: self.hud_expansion,
            compact_expansion: self.compact_expansion,
            secondary_expansion: self.secondary_expansion,
            compact_swap_blend: self.compact_swap_blend,
            panel_expansion: self.panel_expansion,
            compact_primary,
            compact_secondary,
            focus_mode: self.focus_mode,
            pomodoro_surface: self.active_surface(),
            active_tab: self.notch_tab,
            previous_tab: self.previous_tab,
            control_center_enabled,
            focus_enabled: self.focus_enabled,
            shelf_enabled: self.shelf_enabled,
            clipboard_enabled: self.clipboard_enabled,
            media_present: self.media_enabled && self.media.is_some(),
            media_live: self
                .media
                .as_ref()
                .is_some_and(|media| self.media_enabled && !media.has_known_duration()),
            lyrics_progress: self.lyrics_progress,
            tab_blend: self.tab_blend,
            compact_title_points: self.compact_title_points.round() as i32,
            privacy: if self.privacy_enabled {
                PrivacyLayoutState {
                    compact_dots: self.privacy.show_in_bar(),
                    panel_mic_control: self.privacy.show_panel_control(),
                }
            } else {
                PrivacyLayoutState {
                    compact_dots: false,
                    panel_mic_control: false,
                }
            },
            shelf: ShelfLayoutInput {
                visible_count: u8::try_from(self.shelf.visible_items().len()).unwrap_or(0),
                has_pager: self.shelf.page_count() > 1,
                has_items: !self.shelf.items.is_empty(),
            },
            clipboard: self.clipboard.layout_input(),
        }
    }

    pub(crate) const fn clipboard_state(&self) -> &ClipboardState {
        &self.clipboard
    }

    pub(crate) fn sync_clipboard_capture(&mut self, payload: ClipboardPayload) -> bool {
        self.clipboard.push_payload(payload).is_some()
    }

    pub(crate) fn clear_clipboard_copied_feedback(&mut self) -> bool {
        self.clipboard_copied_until = None;
        self.clipboard.clear_copied_feedback()
    }

    pub(crate) const fn shelf_state(&self) -> &ShelfState {
        &self.shelf
    }

    pub(crate) fn shelf_item_payload(&self, slot: u8) -> Option<ShelfPayload> {
        self.shelf
            .visible_item_at(slot)
            .map(|item| item.payload.clone())
    }

    pub(crate) const fn is_shelf_hold_open(&self) -> bool {
        self.shelf.drag_hover
            || self.opened_by_shelf_drag
            || self.shelf_drag_leave_until.is_some()
            || self.modal_picker_open
            || self.drag_out_active
    }

    pub(crate) fn begin_shelf_modal_picker(&mut self) -> bool {
        let was_open = self.modal_picker_open;
        self.modal_picker_open = true;
        self.hover_blocked = false;
        !was_open
    }

    pub(crate) fn finish_shelf_modal_picker(
        &mut self,
        items: Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>,
    ) -> NotchUpdate {
        self.modal_picker_open = false;
        self.opened_by_shelf_drag = false;
        self.hover_blocked = false;
        let mut changed = true;
        for (payload, is_dir, thumb) in items {
            if let Some(valid) = sanitize_shelf_payload(payload)
                && self.shelf.push_payload(valid, is_dir, thumb).is_some()
            {
                changed = true;
            }
        }
        changed |= self.switch_tab(NotchExpandedTab::Shelf);
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn set_shelf_drag_out(&mut self, active: bool) -> bool {
        if self.drag_out_active == active {
            return false;
        }
        self.drag_out_active = active;
        if active {
            self.hover_blocked = false;
        }
        true
    }

    pub(crate) fn handle_shelf_drag_enter(&mut self) -> NotchUpdate {
        self.shelf_drag_leave_until = None;
        self.hover_blocked = false;
        let mut changed = self.shelf.set_drag_hover(true);
        if !self.panel_trigger.is_open() && !self.opened_by_shelf_drag {
            self.opened_by_shelf_drag = true;
            changed = true;
        }
        changed |= self.switch_tab(NotchExpandedTab::Shelf);
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn handle_shelf_drag_leave(&mut self, now: Instant) -> NotchUpdate {
        let changed = self.shelf.set_drag_hover(false);
        self.shelf_drag_leave_until = if self.opened_by_shelf_drag || self.panel_trigger.is_open() {
            Some(now + Duration::from_millis(SHELF_HOVER_LEAVE_MS))
        } else {
            None
        };
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn handle_shelf_drop(
        &mut self,
        items: Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>,
    ) -> NotchUpdate {
        self.shelf_drag_leave_until = None;
        self.hover_blocked = false;
        let mut changed = self.shelf.set_drag_hover(false);
        let was_drag_opened = self.opened_by_shelf_drag;
        self.opened_by_shelf_drag = false;
        if was_drag_opened && self.panel_trigger == PanelTrigger::Closed {
            self.panel_trigger = PanelTrigger::Hovered;
            changed = true;
        }
        let mut added = false;
        for (payload, is_dir, thumb) in items {
            if let Some(valid) = sanitize_shelf_payload(payload)
                && self.shelf.push_payload(valid, is_dir, thumb).is_some()
            {
                added = true;
            }
        }
        if added {
            let _ = self.switch_tab(NotchExpandedTab::Shelf);
            changed = true;
        }
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn handle_shelf_right_click(&self, slot: u8) -> NotchUpdate {
        let Some(item) = self.shelf.visible_item_at(slot) else {
            return NotchUpdate::default();
        };
        let action = match &item.payload {
            ShelfPayload::File(_) => NotchAction::ShelfReveal(item.payload.clone()),
            ShelfPayload::Url(_) | ShelfPayload::Text(_) => {
                NotchAction::ShelfCopy(item.payload.clone())
            }
        };
        NotchUpdate {
            changed: false,
            effect: Some(NotchEffect::Platform(action)),
        }
    }

    fn step_shelf_drag_leave(&mut self, now: Instant) -> bool {
        if self
            .shelf_drag_leave_until
            .is_some_and(|until| now >= until)
        {
            self.shelf_drag_leave_until = None;
            let hover_cleared = self.shelf.set_drag_hover(false);
            let drag_closed = self.opened_by_shelf_drag;
            self.opened_by_shelf_drag = false;
            return hover_cleared || drag_closed;
        }
        false
    }

    pub(crate) fn panel_expansion(&self) -> f32 {
        self.panel_expansion
    }

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
        true
    }

    pub(crate) fn focus_active(&self) -> bool {
        self.has_compact_focus()
    }

    pub(crate) fn open_media_tab(&mut self) {
        if !self.media_enabled || self.media.is_none() {
            return;
        }
        self.dismiss_active_alert();
        self.panel_trigger = PanelTrigger::Pinned;
        self.hover_blocked = false;
        self.hover_since = None;
        let _ = self.switch_tab(NotchExpandedTab::Media);
    }

    pub(crate) fn active_tab(&self) -> NotchExpandedTab {
        self.notch_tab
    }

    #[cfg(test)]
    pub(crate) fn pomodoro(&self) -> PomodoroState {
        self.pomodoro
    }

    pub(crate) fn pomodoro_scene(&self, now: Instant) -> PomodoroScene {
        let metrics = MotionMetrics::default();
        let animated = self.motion.allows_motion();
        let seconds = self.millis(now) as f32 / 1_000.0;
        let breath = if animated {
            0.5 - 0.5 * (TAU * seconds / metrics.pomodoro_glow_breathe_seconds).cos()
        } else {
            1.0
        };
        let wave = if animated {
            0.5 + 0.5 * (TAU * seconds / metrics.pomodoro_last_minute_pulse_seconds).sin()
        } else {
            1.0
        };
        PomodoroScene {
            state: self.pomodoro,
            timer: self.quick_timer,
            stopwatch: self.stopwatch,
            focus_mode: self.focus_mode,
            compact_mode: self.compact_focus_mode().unwrap_or(self.focus_mode),
            clock: self.clock_time,
            glow: self.glow_presence * (GLOW_BREATH_FLOOR + (1.0 - GLOW_BREATH_FLOOR) * breath),
            dim: self.dim_presence,
            pulse: self.pulse_presence * wave,
            peek: self.peek.as_ref().map(|peek| PomodoroPeekFrame {
                text: peek.text.clone(),
                alpha: self.peek_alpha(peek, now),
            }),
        }
    }

    fn peek_alpha(&self, peek: &ActivePeek, now: Instant) -> f32 {
        self.fade_alpha(peek.shown_at, peek.until, now)
    }

    fn fade_alpha(&self, shown_at: Instant, until: Instant, now: Instant) -> f32 {
        if !self.motion.allows_motion() {
            return 1.0;
        }
        let entered = now.saturating_duration_since(shown_at).as_secs_f32();
        let remaining = until.saturating_duration_since(now).as_secs_f32();
        (entered.min(remaining) / PEEK_FADE_SECONDS).clamp(0.0, 1.0)
    }

    fn show_media_peek(&mut self, kind: MediaPeek, title: String, now: Instant) {
        if self.has_compact_focus() && !self.media_primary {
            return;
        }
        let hold = Duration::from_millis(MotionMetrics::default().pomodoro_peek_ms);
        self.media_peek = Some(MediaPeekState {
            label: kind.label(),
            title,
            shown_at: now,
            until: now + hold,
        });
    }

    fn note_media_transition(
        &mut self,
        latest: Option<&NowPlaying>,
        track_changed: bool,
        now: Instant,
    ) {
        let Some(latest) = latest else {
            self.media_paused_at = None;
            self.media_peek = None;
            return;
        };
        if track_changed {
            self.media_paused_at = None;
            if latest.is_active() {
                self.show_media_peek(MediaPeek::NowPlaying, latest.title.clone(), now);
            }
            return;
        }
        match (
            self.media.as_ref().map(|media| media.playing),
            latest.playing,
        ) {
            (Some(true), false) => {
                self.media_paused_at = Some(now);
                self.show_media_peek(MediaPeek::Paused, latest.title.clone(), now);
            }
            (Some(false), true) => {
                self.media_paused_at = None;
                self.show_media_peek(MediaPeek::Playing, latest.title.clone(), now);
            }
            _ => {}
        }
    }

    fn compact_lyric_line(&self, media: &NowPlaying) -> Option<String> {
        if !self.lyrics_in_compact {
            return None;
        }
        self.lyrics
            .as_ref()
            .and_then(|sheet| sheet.active_line(media.position_ms))
            .map(str::to_owned)
    }

    fn show_peek(&mut self, kind: PomodoroPeek, now: Instant) {
        let hold = Duration::from_millis(MotionMetrics::default().pomodoro_peek_ms);
        self.peek = Some(ActivePeek {
            text: kind.text(&self.pomodoro),
            shown_at: now,
            until: now + hold,
        });
    }

    fn note_pomodoro_transition(&mut self, previous: PomodoroStatus, now: Instant) {
        self.pomodoro.mark_started(self.clock_time.minute_of_day());
        match (previous, self.pomodoro.status) {
            (
                PomodoroStatus::Idle | PomodoroStatus::Paused | PomodoroStatus::Alert,
                PomodoroStatus::Running,
            ) => self.show_peek(PomodoroPeek::Started, now),
            (PomodoroStatus::Running, PomodoroStatus::Paused) => {
                self.show_peek(PomodoroPeek::Paused, now);
            }
            _ => {}
        }
    }

    fn settle(value: f32, target: f32, dt: f32, motion: MotionPreference) -> f32 {
        let rate = MotionMetrics::default().pomodoro_tone_rate;
        Self::snap_to_target(animate_value(value, target, rate, dt, motion), target)
    }

    fn step_scene_at(&mut self, now: Instant, motion: MotionPreference) -> bool {
        let dt = now
            .saturating_duration_since(self.scene_step_at)
            .as_secs_f32()
            .clamp(0.001, 0.25);
        self.scene_step_at = now;
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let status = self.active_focus_status();
        let glow = Self::settle(
            self.glow_presence,
            flag(status == PomodoroStatus::Running),
            dt,
            motion,
        );
        let dim = Self::settle(
            self.dim_presence,
            flag(status == PomodoroStatus::Paused),
            dt,
            motion,
        );
        let pulse = Self::settle(
            self.pulse_presence,
            flag(self.active_is_last_minute()),
            dt,
            motion,
        );
        let media_dim = Self::settle(
            self.media_dim_presence,
            flag(self.media.as_ref().is_some_and(|media| !media.playing)),
            dt,
            motion,
        );
        let changed = (glow - self.glow_presence).abs() > f32::EPSILON
            || (dim - self.dim_presence).abs() > f32::EPSILON
            || (pulse - self.pulse_presence).abs() > f32::EPSILON
            || (media_dim - self.media_dim_presence).abs() > f32::EPSILON;
        self.glow_presence = glow;
        self.dim_presence = dim;
        self.pulse_presence = pulse;
        self.media_dim_presence = media_dim;
        changed
    }

    fn scene_frame_active(&self) -> bool {
        let panel_live_media = self.panel_expansion > GLOW_SCENE_VISIBLE
            && self.notch_tab == NotchExpandedTab::Media
            && self
                .media
                .as_ref()
                .is_some_and(|media| media.playing && !media.has_known_duration());
        let countdown_running = self.pomodoro.status == PomodoroStatus::Running
            || self.quick_timer.status == PomodoroStatus::Running;
        self.presented
            && self.motion.allows_motion()
            && (self.compact_expansion > GLOW_SCENE_VISIBLE
                || self.panel_expansion > GLOW_SCENE_VISIBLE)
            && (countdown_running
                || self.peek.is_some()
                || self.media_peek.is_some()
                || panel_live_media)
    }

    pub(crate) fn media(&self) -> Option<&NowPlaying> {
        self.media.as_ref()
    }

    pub(crate) fn hud_state(&self) -> Option<NotchHudState> {
        self.hud_kind.map(|kind| NotchHudState {
            kind,
            expansion: self.hud_expansion,
            content_alpha: hud_content_alpha(self.hud_expansion, &MotionMetrics::default()),
            level: self.hud_level,
        })
    }

    pub(crate) fn anim_phase(&self) -> f32 {
        self.anim_phase
    }

    pub(crate) fn is_panel_target_open(&self, now: Instant) -> bool {
        let alert_active = self.has_alert() && self.alert_until.is_none_or(|until| now < until);
        self.panel_trigger.is_open() || alert_active
    }

    pub(crate) fn display_media(&self, now_unix_ms: i64) -> Option<NowPlaying> {
        let media = self.media.as_ref()?;
        Some(match self.seek_drag {
            Some(ratio) => NowPlaying {
                position_ms: seek_target_ms(ratio, media.duration_ms),
                position_updated_unix_ms: None,
                ..media.clone()
            },
            None => media.at(now_unix_ms),
        })
    }

    pub(crate) fn media_scene(&self, now: Instant, now_unix_ms: i64) -> Option<MediaScene> {
        let media = self.display_media(now_unix_ms)?;
        let metrics = MotionMetrics::default();
        let live_phase = if self.motion.allows_motion() {
            let seconds = self.millis(now) as f32 / 1_000.0;
            0.5 - 0.5 * (TAU * seconds / metrics.media_live_drift_seconds).cos()
        } else {
            0.5
        };
        Some(MediaScene {
            dim: self.media_dim_presence,
            live_phase,
            lyric_line: self.compact_lyric_line(&media),
            peek: self.media_peek.as_ref().map(|peek| MediaPeekFrame {
                label: peek.label,
                title: peek.title.clone(),
                alpha: self.fade_alpha(peek.shown_at, peek.until, now),
            }),
            media,
        })
    }

    pub(crate) fn is_seek_dragging(&self) -> bool {
        self.seek_drag.is_some()
    }

    pub(crate) fn begin_seek_drag(&mut self, ratio: f32) -> bool {
        if !self.media.as_ref().is_some_and(NowPlaying::is_seekable) {
            return false;
        }
        self.seek_drag = Some(ratio.clamp(0.0, 1.0));
        true
    }

    pub(crate) fn update_seek_drag(&mut self, ratio: f32) -> bool {
        let Some(current) = self.seek_drag.as_mut() else {
            return false;
        };
        let next = ratio.clamp(0.0, 1.0);
        let changed = (*current - next).abs() > f32::EPSILON;
        *current = next;
        changed
    }

    pub(crate) fn finish_seek_drag(&mut self, now_unix_ms: i64) -> NotchUpdate {
        match self.seek_drag.take() {
            Some(ratio) => self.handle_seekbar_click(ratio, now_unix_ms),
            None => NotchUpdate::default(),
        }
    }

    pub(crate) fn cancel_seek_drag(&mut self) -> bool {
        self.seek_drag.take().is_some()
    }

    pub(crate) fn set_media(&mut self, latest_media: Option<NowPlaying>) -> bool {
        self.set_media_at(latest_media, unix_now_ms())
    }

    fn set_media_at(&mut self, mut latest_media: Option<NowPlaying>, now_unix_ms: i64) -> bool {
        if !self.media_enabled {
            latest_media = None;
        }
        if let (Some(pending), Some(latest)) = (self.pending_seek, latest_media.as_mut()) {
            let same_track = self
                .media
                .as_ref()
                .is_some_and(|current| current.is_same_track(latest));
            match reconcile_seek(pending, latest, now_unix_ms) {
                SeekReadback::Hold if same_track => {
                    latest.position_ms = pending.target_ms;
                    latest.position_updated_unix_ms = Some(pending.issued_unix_ms);
                }
                SeekReadback::Hold | SeekReadback::Confirmed | SeekReadback::Expired => {
                    self.pending_seek = None;
                }
            }
        }
        if latest_media == self.media {
            return false;
        }
        if let (Some(current), Some(latest)) = (self.media.as_ref(), latest_media.as_ref()) {
            let lyrics_visible = self.lyrics_visible_now();
            let tolerance = if lyrics_visible {
                LYRIC_DRIFT_TOLERANCE_MS
            } else {
                POSITION_DRIFT_TOLERANCE_MS
            };
            let lyric_changed = lyrics_visible
                && current.is_same_track(latest)
                && self.lyrics.as_ref().is_some_and(|sheet| {
                    sheet.active_line_index(current.position_at(now_unix_ms))
                        != sheet.active_line_index(latest.position_at(now_unix_ms))
                });
            if !lyric_changed && current.presents_like_within(latest, now_unix_ms, tolerance) {
                self.media = latest_media;
                return false;
            }
        }
        let track_changed = match (&self.media, &latest_media) {
            (Some(prev), Some(next)) => !prev.is_same_track(next),
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };
        if track_changed {
            self.lyrics = None;
            self.lyrics_status = LyricsStatus::Idle;
            self.pending_seek = None;
            self.seek_drag = None;
        }
        let media_ready =
            self.media_enabled && latest_media.as_ref().is_some_and(NowPlaying::is_active);
        if latest_media.is_none()
            && (self.notch_tab == NotchExpandedTab::Media
                || self.previous_tab == NotchExpandedTab::Media)
        {
            if let Some(fallback) = [
                NotchExpandedTab::Pomodoro,
                NotchExpandedTab::Shelf,
                NotchExpandedTab::Clipboard,
            ]
            .into_iter()
            .find(|&tab| self.is_tab_enabled(tab))
            {
                self.notch_tab = fallback;
                self.previous_tab = fallback;
                self.tab_blend = 1.0;
            } else {
                self.panel_trigger = PanelTrigger::Closed;
                self.panel_expansion = 0.0;
                self.hover_since = None;
            }
        }
        self.note_media_transition(latest_media.as_ref(), track_changed, Instant::now());
        self.media = latest_media;
        if media_ready && !self.has_compact_focus() && self.notch_tab == NotchExpandedTab::Pomodoro
        {
            if self.panel_expansion > 0.05 {
                let _ = self.switch_tab(NotchExpandedTab::Media);
            } else {
                self.notch_tab = NotchExpandedTab::Media;
                self.previous_tab = NotchExpandedTab::Media;
                self.tab_blend = 1.0;
            }
        }
        true
    }

    pub(crate) fn lyrics_view(&self) -> LyricsView<'_> {
        LyricsView {
            sheet: self.lyrics.as_ref(),
            status: self.lyrics_status,
            animated: self.motion.allows_motion(),
        }
    }

    pub(crate) fn begin_lyrics_request(&mut self) {
        self.lyrics = None;
        self.lyrics_status = LyricsStatus::Loading;
    }

    pub(crate) fn finish_lyrics(
        &mut self,
        sheet: Option<otto_core::LyricsSheet>,
        status: LyricsStatus,
    ) {
        self.lyrics_status = match (&sheet, status) {
            (Some(found), LyricsStatus::Ready) if found.is_empty() => LyricsStatus::NotFound {
                searched_online: true,
            },
            _ => status,
        };
        self.lyrics = sheet.filter(|found| !found.is_empty());
    }

    #[allow(dead_code)]
    pub(crate) fn set_lyrics(&mut self, lyrics: Option<otto_core::LyricsSheet>) {
        self.lyrics_status = if lyrics.is_some() {
            LyricsStatus::Ready
        } else {
            LyricsStatus::Idle
        };
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
        if !self.hud_enabled {
            return;
        }
        if self.hud_expansion < 0.05 {
            self.hud_level = kind.target_level();
        }
        self.hud_kind = Some(kind);
        self.hud_until =
            Some(Instant::now() + Duration::from_millis(MotionMetrics::default().hud_hold_ms));
    }

    pub(crate) fn handle_hotkey(&mut self, hotkey: NotchHotkey) -> NotchUpdate {
        let now = Instant::now();
        let now_ms = self.millis(now);
        match hotkey {
            NotchHotkey::ToggleNotch => {
                if self.is_panel_target_open(now) {
                    self.panel_trigger = PanelTrigger::Closed;
                    self.hover_since = None;
                    self.dismiss_active_alert();
                } else {
                    if !self.has_openable_panel() {
                        return NotchUpdate::default();
                    }
                    self.ensure_valid_tab();
                    self.panel_trigger = PanelTrigger::Pinned;
                }
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            NotchHotkey::TogglePomodoro => {
                if !self.focus_enabled {
                    return NotchUpdate::default();
                }
                self.clear_focus_dismissed();
                match self.focus_mode {
                    FocusSubMode::Pomodoro => {
                        let previous = self.pomodoro.status;
                        self.pomodoro.toggle_run();
                        self.note_pomodoro_transition(previous, now);
                        self.clock.restart(now_ms);
                        self.alert_until = None;
                        NotchUpdate {
                            changed: true,
                            effect: None,
                        }
                    }
                    FocusSubMode::Timer => {
                        self.quick_timer.toggle_run();
                        self.timer_clock.restart(now_ms);
                        self.alert_until = None;
                        NotchUpdate {
                            changed: true,
                            effect: None,
                        }
                    }
                    FocusSubMode::Stopwatch => {
                        self.stopwatch_clock.sync(&mut self.stopwatch, now_ms, true);
                        self.stopwatch.toggle_run();
                        self.stopwatch_clock.restart(now_ms);
                        NotchUpdate {
                            changed: true,
                            effect: None,
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn handle_pointer_move(&mut self, inside_notch: bool) {
        if inside_notch {
            if self.panel_expansion > 0.55 && self.panel_trigger == PanelTrigger::Pinned {
                self.panel_trigger = PanelTrigger::PinnedHovered;
            }
            if self.hover_since.is_none()
                && self.panel_trigger == PanelTrigger::Closed
                && !self.hover_blocked
                && self.has_openable_panel()
            {
                self.hover_since = Some(Instant::now());
            }
        } else {
            self.hover_since = None;
            self.hover_blocked = false;
        }
    }

    pub(crate) fn handle_pointer_leave(&mut self, cursor_inside_notch: bool) -> bool {
        if cursor_inside_notch || self.is_shelf_hold_open() {
            return false;
        }
        self.hover_since = None;
        self.hover_blocked = false;
        if matches!(
            self.panel_trigger,
            PanelTrigger::Hovered | PanelTrigger::PinnedHovered
        ) {
            self.panel_trigger = PanelTrigger::Closed;
            return true;
        }
        false
    }

    pub(crate) fn handle_seek_step(&mut self, delta_ms: i64, now_unix_ms: i64) -> NotchUpdate {
        let Some(media) = self
            .display_media(now_unix_ms)
            .filter(NowPlaying::is_seekable)
        else {
            return NotchUpdate::default();
        };
        let target_ms = (media.position_ms as i64 + delta_ms).clamp(0, media.duration_ms as i64);
        let ratio = target_ms as f32 / media.duration_ms as f32;
        self.handle_seekbar_click(ratio, now_unix_ms)
    }

    pub(crate) fn handle_seekbar_click(&mut self, ratio: f32, now_unix_ms: i64) -> NotchUpdate {
        let Some(media) = self.media.as_mut().filter(|media| media.is_seekable()) else {
            return NotchUpdate::default();
        };
        let target_ms = seek_target_ms(ratio, media.duration_ms);
        media.position_ms = target_ms;
        media.position_updated_unix_ms = Some(now_unix_ms);
        self.pending_seek = Some(PendingSeek {
            target_ms,
            issued_unix_ms: now_unix_ms,
        });
        NotchUpdate {
            changed: true,
            effect: Some(NotchEffect::Platform(NotchAction::Media(
                MediaCommand::Seek(target_ms),
            ))),
        }
    }

    pub(crate) fn handle_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::FocusMode(mode) => self.handle_focus_mode_click(mode),
            HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroPreset(_)
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive => self.handle_pomodoro_click(target),
            HitTarget::NotchTabPomodoro
            | HitTarget::NotchTabShelf
            | HitTarget::NotchTabClipboard
            | HitTarget::NotchTabMedia => self.handle_tab_click(target),
            HitTarget::ShelfAdd
            | HitTarget::ShelfClear
            | HitTarget::ShelfPager
            | HitTarget::ShelfItem(_)
            | HitTarget::ShelfCopyItem(_)
            | HitTarget::ShelfRemoveItem(_) => self.handle_shelf_click(target),
            HitTarget::ClipboardPauseToggle
            | HitTarget::ClipboardClear
            | HitTarget::ClipboardPager
            | HitTarget::ClipboardItem(_)
            | HitTarget::ClipboardRemoveItem(_) => self.handle_clipboard_click(target),
            HitTarget::PrivacyIndicator => self.handle_privacy_indicator_click(),
            HitTarget::PrivacyMicMute => self.handle_privacy_mic_click(),
            HitTarget::MediaPlayPause | HitTarget::MediaPrevious | HitTarget::MediaNext => {
                self.handle_media_click(target)
            }
            HitTarget::MediaLyricsToggle => self.toggle_lyrics(),
            HitTarget::Notch => self.handle_notch_click(),
            HitTarget::NotchSecondary => self.handle_secondary_notch_click(),
            _ => NotchUpdate::default(),
        }
    }

    fn handle_shelf_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::ShelfAdd => NotchUpdate {
                changed: false,
                effect: Some(NotchEffect::PickShelfFiles),
            },
            HitTarget::ShelfClear => NotchUpdate {
                changed: self.shelf.clear(),
                effect: None,
            },
            HitTarget::ShelfPager => NotchUpdate {
                changed: self.shelf.step_page(1),
                effect: None,
            },
            HitTarget::ShelfItem(slot) => {
                let Some(item) = self.shelf.visible_item_at(slot) else {
                    return NotchUpdate::default();
                };
                let action = match &item.payload {
                    ShelfPayload::File(_) | ShelfPayload::Url(_) => {
                        NotchAction::ShelfOpen(item.payload.clone())
                    }
                    ShelfPayload::Text(_) => NotchAction::ShelfCopy(item.payload.clone()),
                };
                NotchUpdate {
                    changed: false,
                    effect: Some(NotchEffect::Platform(action)),
                }
            }
            HitTarget::ShelfCopyItem(slot) => {
                let Some(item) = self.shelf.visible_item_at(slot) else {
                    return NotchUpdate::default();
                };
                NotchUpdate {
                    changed: false,
                    effect: Some(NotchEffect::Platform(NotchAction::ShelfCopy(
                        item.payload.clone(),
                    ))),
                }
            }
            HitTarget::ShelfRemoveItem(slot) => NotchUpdate {
                changed: self
                    .shelf
                    .visible_item_at(slot)
                    .map(|item| item.id)
                    .is_some_and(|id| self.shelf.remove(id)),
                effect: None,
            },
            _ => NotchUpdate::default(),
        }
    }

    fn handle_clipboard_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::ClipboardPauseToggle => {
                let changed = self.clipboard.toggle_pause();
                NotchUpdate {
                    changed,
                    effect: None,
                }
            }
            HitTarget::ClipboardClear => {
                let changed = self.clipboard.clear();
                if changed {
                    self.clipboard_copied_until = None;
                }
                NotchUpdate {
                    changed,
                    effect: None,
                }
            }
            HitTarget::ClipboardPager => NotchUpdate {
                changed: self.clipboard.step_page(1),
                effect: None,
            },
            HitTarget::ClipboardItem(slot) => {
                let Some(item) = self.clipboard.visible_item_at(slot).cloned() else {
                    return NotchUpdate::default();
                };
                self.clipboard.mark_copied(item.id);
                self.clipboard_copied_until =
                    Some(Instant::now() + Duration::from_millis(CLIPBOARD_COPIED_BADGE_MS));
                NotchUpdate {
                    changed: true,
                    effect: Some(NotchEffect::CopyClipboard(item.payload)),
                }
            }
            HitTarget::ClipboardRemoveItem(slot) => {
                let changed = self
                    .clipboard
                    .visible_item_at(slot)
                    .map(|item| item.id)
                    .is_some_and(|id| self.clipboard.remove(id));
                if self.clipboard.copied_feedback_id.is_none() {
                    self.clipboard_copied_until = None;
                }
                NotchUpdate {
                    changed,
                    effect: None,
                }
            }
            _ => NotchUpdate::default(),
        }
    }

    fn handle_privacy_indicator_click(&mut self) -> NotchUpdate {
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

    fn handle_privacy_mic_click(&mut self) -> NotchUpdate {
        if !self.privacy.mic.is_available() {
            return NotchUpdate::default();
        }
        self.privacy.mic = self.privacy.mic.with_toggled_mute();
        NotchUpdate {
            changed: true,
            effect: Some(NotchEffect::Platform(NotchAction::ToggleMicMute)),
        }
    }

    fn handle_focus_mode_click(&mut self, mode: FocusSubMode) -> NotchUpdate {
        if self.focus_mode == mode {
            return NotchUpdate::default();
        }
        let now_ms = self.millis(Instant::now());
        if mode == FocusSubMode::Stopwatch {
            self.stopwatch_clock.sync(&mut self.stopwatch, now_ms, true);
        }
        self.focus_mode = mode;
        self.peek = None;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn handle_pomodoro_click(&mut self, target: HitTarget) -> NotchUpdate {
        self.clear_focus_dismissed();
        let now = Instant::now();
        let now_ms = self.millis(now);
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.handle_pomodoro_mode_click(target, now, now_ms),
            FocusSubMode::Timer => self.handle_timer_mode_click(target, now_ms),
            FocusSubMode::Stopwatch => self.handle_stopwatch_mode_click(target, now_ms),
        }
    }

    fn handle_pomodoro_mode_click(
        &mut self,
        target: HitTarget,
        now: Instant,
        now_ms: u64,
    ) -> NotchUpdate {
        let previous = self.pomodoro.status;
        match target {
            HitTarget::PomodoroPlayPause => {
                self.pomodoro.toggle_run();
                self.note_pomodoro_transition(previous, now);
            }
            HitTarget::PomodoroReset => {
                self.pomodoro.reset();
                self.peek = None;
            }
            HitTarget::PomodoroSkip => {
                self.pomodoro.skip_phase();
                self.peek = None;
            }
            HitTarget::PomodoroPreset(index) => {
                let Some(minutes) = POMODORO_PRESETS_MINUTES.get(usize::from(index)) else {
                    return NotchUpdate::default();
                };
                if !self.pomodoro.set_preset(*minutes) {
                    return NotchUpdate::default();
                }
            }
            HitTarget::PomodoroStartNext => {
                self.pomodoro.start_next_from_alert();
                self.alert_until = None;
                self.note_pomodoro_transition(previous, now);
            }
            HitTarget::PomodoroExtendFive => {
                self.pomodoro.extend_five_minutes();
                self.alert_until = None;
                self.pomodoro.mark_started(self.clock_time.minute_of_day());
                self.show_peek(PomodoroPeek::Extended, now);
            }
            _ => return NotchUpdate::default(),
        }
        self.clock.restart(now_ms);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn handle_timer_mode_click(&mut self, target: HitTarget, now_ms: u64) -> NotchUpdate {
        match target {
            HitTarget::PomodoroPlayPause => self.quick_timer.toggle_run(),
            HitTarget::PomodoroReset => self.quick_timer.reset(),
            HitTarget::PomodoroPreset(index) => {
                let Some(minutes) = TIMER_PRESETS_MINUTES.get(usize::from(index)) else {
                    return NotchUpdate::default();
                };
                if !self.quick_timer.set_preset(*minutes) {
                    return NotchUpdate::default();
                }
            }
            HitTarget::PomodoroStartNext => {
                self.quick_timer.start_next_from_alert();
                self.alert_until = None;
            }
            HitTarget::PomodoroExtendFive => {
                self.quick_timer.extend_five_minutes();
                self.alert_until = None;
            }
            _ => return NotchUpdate::default(),
        }
        self.timer_clock.restart(now_ms);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn handle_stopwatch_mode_click(&mut self, target: HitTarget, now_ms: u64) -> NotchUpdate {
        self.stopwatch_clock.sync(&mut self.stopwatch, now_ms, true);
        match target {
            HitTarget::PomodoroPlayPause => self.stopwatch.toggle_run(),
            HitTarget::PomodoroReset => self.stopwatch.reset(),
            HitTarget::PomodoroSkip => {
                if !self.stopwatch.record_lap() {
                    return NotchUpdate::default();
                }
            }
            _ => return NotchUpdate::default(),
        }
        self.stopwatch_clock.restart(now_ms);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn switch_tab(&mut self, target: NotchExpandedTab) -> bool {
        if target == self.notch_tab || !self.is_tab_enabled(target) {
            return false;
        }
        if target == self.previous_tab && self.tab_blend < 1.0 {
            self.previous_tab = self.notch_tab;
            self.notch_tab = target;
            self.tab_blend = (1.0 - self.tab_blend).clamp(0.0, 1.0);
        } else {
            let dominant = if self.tab_blend >= 0.5 {
                self.notch_tab
            } else {
                self.previous_tab
            };
            self.previous_tab = dominant;
            self.notch_tab = target;
            self.tab_blend = 0.0;
        }
        true
    }

    fn handle_tab_click(&mut self, target: HitTarget) -> NotchUpdate {
        let tab = match target {
            HitTarget::NotchTabPomodoro => NotchExpandedTab::Pomodoro,
            HitTarget::NotchTabShelf => NotchExpandedTab::Shelf,
            HitTarget::NotchTabClipboard => NotchExpandedTab::Clipboard,
            HitTarget::NotchTabMedia => NotchExpandedTab::Media,
            _ => return NotchUpdate::default(),
        };
        NotchUpdate {
            changed: self.switch_tab(tab),
            effect: None,
        }
    }

    fn handle_media_click(&mut self, target: HitTarget) -> NotchUpdate {
        let capabilities = self
            .media
            .as_ref()
            .map_or(otto_core::MediaCapabilities::ALL, |media| {
                media.capabilities
            });
        let command = match target {
            HitTarget::MediaPlayPause => MediaCommand::TogglePlayPause,
            HitTarget::MediaPrevious => MediaCommand::Previous,
            HitTarget::MediaNext => MediaCommand::Next,
            _ => return NotchUpdate::default(),
        };
        if !capabilities.allows(command) {
            return NotchUpdate::default();
        }
        let now = Instant::now();
        match (command, self.media.as_mut()) {
            (MediaCommand::TogglePlayPause, Some(media)) => {
                media.playing = !media.playing;
                let (playing, title) = (media.playing, media.title.clone());
                if playing {
                    self.media_paused_at = None;
                    self.show_media_peek(MediaPeek::Playing, title, now);
                } else {
                    self.media_paused_at = Some(now);
                    self.show_media_peek(MediaPeek::Paused, title, now);
                }
            }
            (MediaCommand::Next | MediaCommand::Previous, Some(media)) => {
                let title = media.title.clone();
                self.show_media_peek(MediaPeek::Skipped, title, now);
            }
            _ => {}
        }
        NotchUpdate {
            changed: matches!(target, HitTarget::MediaPlayPause),
            effect: Some(NotchEffect::Platform(NotchAction::Media(command))),
        }
    }

    pub(crate) fn collapse_from_gesture(&mut self) -> NotchUpdate {
        if self.is_shelf_hold_open() || !self.panel_trigger.is_open() {
            return NotchUpdate::default();
        }
        self.panel_trigger = PanelTrigger::Closed;
        self.hover_since = None;
        self.hover_blocked = true;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    fn handle_notch_click(&mut self) -> NotchUpdate {
        if self.has_alert() {
            self.dismiss_active_alert();
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        if self.panel_expansion < 0.25 {
            if !self.has_openable_panel() {
                return NotchUpdate::default();
            }
            self.panel_trigger = PanelTrigger::Pinned;
            self.hover_blocked = false;
            self.hover_since = None;
            if matches!(
                self.notch_tab,
                NotchExpandedTab::Pomodoro | NotchExpandedTab::Media
            ) {
                self.sync_tab_to_primary(Instant::now());
            }
            self.ensure_valid_tab();
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        }
        self.collapse_from_gesture()
    }

    fn handle_secondary_notch_click(&mut self) -> NotchUpdate {
        let now = Instant::now();
        if self
            .island_inputs(now)
            .secondary_compact_content()
            .is_none()
        {
            return NotchUpdate::default();
        }
        self.media_primary = !self.media_primary;
        self.compact_swap_blend = 0.0;
        self.sync_tab_to_primary(now);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn handle_wheel(&mut self, target: HitTarget, delta: i16) -> NotchUpdate {
        if self.panel_expansion <= 0.55
            || !matches!(
                target,
                HitTarget::Notch
                    | HitTarget::FocusMode(_)
                    | HitTarget::PomodoroPreset(_)
                    | HitTarget::PomodoroPlayPause
                    | HitTarget::PomodoroReset
                    | HitTarget::PomodoroSkip
                    | HitTarget::NotchTabShelf
                    | HitTarget::NotchTabClipboard
                    | HitTarget::ShelfAdd
                    | HitTarget::ShelfClear
                    | HitTarget::ShelfPager
                    | HitTarget::ShelfItem(_)
                    | HitTarget::ShelfCopyItem(_)
                    | HitTarget::ShelfRemoveItem(_)
                    | HitTarget::ClipboardPauseToggle
                    | HitTarget::ClipboardClear
                    | HitTarget::ClipboardPager
                    | HitTarget::ClipboardItem(_)
                    | HitTarget::ClipboardRemoveItem(_)
                    | HitTarget::PrivacyMicMute
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
        let changed = match self.notch_tab {
            NotchExpandedTab::Pomodoro => match self.focus_mode {
                FocusSubMode::Pomodoro => self.pomodoro.adjust_minutes(step),
                FocusSubMode::Timer => self.quick_timer.adjust_minutes(step),
                FocusSubMode::Stopwatch => false,
            },
            NotchExpandedTab::Shelf => self.shelf.step_page(-step),
            NotchExpandedTab::Clipboard => self.clipboard.step_page((-step) as isize),
            NotchExpandedTab::Media => false,
        };
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    fn step_hud(
        &mut self,
        hud_active: bool,
        target_hud: f32,
        dt: f32,
        motion: MotionPreference,
    ) -> bool {
        let rates = MotionMetrics::default();
        let next_hud = animate_value(
            self.hud_expansion,
            target_hud,
            rates.hud_expand_rate,
            dt,
            motion,
        );
        let target_level = self
            .hud_kind
            .map_or(self.hud_level, NotchHudKind::target_level);
        let next_level = animate_value(
            self.hud_level,
            target_level,
            rates.hud_meter_rate,
            dt,
            motion,
        );
        let mut changed = false;
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
        changed
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
        self.motion = motion;
        self.anim_phase = (self.anim_phase + dt) % 1000.0;

        let (mut changed, effect) = self.step_pomodoro_clock(now);
        if self.step_scene_at(now, motion) {
            changed = true;
        }
        self.step_hover_tracking(now, cursor_inside_notch, panel_visible);

        let hud_active = self.hud_until.is_some_and(|until| now < until);
        let inputs = self.island_inputs(now);
        if let Some(primary) = inputs.compact_content() {
            self.last_primary_slot = Some(primary.kind());
        }
        if let Some(secondary) = inputs.secondary_compact_content() {
            self.last_secondary_slot = Some(secondary.kind());
        } else {
            self.media_primary = false;
        }
        let targets = island_targets(&inputs);
        let target_hud = targets.hud;

        if self.step_morph_blends(dt, motion) {
            changed = true;
        }

        if self.step_hud(hud_active, target_hud, dt, motion) {
            changed = true;
        }

        for (value, target, rate) in [
            (&mut self.compact_expansion, targets.compact, 16.0),
            (&mut self.secondary_expansion, targets.secondary, 18.0),
            (&mut self.panel_expansion, targets.panel, 20.0),
        ] {
            if Self::step_expansion(value, target, rate, dt, motion) {
                changed = true;
            }
        }

        NotchUpdate { changed, effect }
    }

    fn step_morph_blends(&mut self, dt: f32, motion: MotionPreference) -> bool {
        let mut changed = false;
        let morph = MotionMetrics::default();
        let next_swap_blend = animate_value(
            self.compact_swap_blend,
            1.0,
            morph.notch_tab_morph_rate,
            dt,
            motion,
        );
        let resolved_swap_blend = Self::snap_to_target(next_swap_blend, 1.0);
        if (resolved_swap_blend - self.compact_swap_blend).abs() > f32::EPSILON {
            self.compact_swap_blend = resolved_swap_blend;
            changed = true;
        }
        let lyrics_target = if self.lyrics_expanded && self.notch_tab == NotchExpandedTab::Media {
            1.0
        } else {
            0.0
        };
        let next_tab_blend =
            animate_value(self.tab_blend, 1.0, morph.notch_tab_morph_rate, dt, motion);
        let next_lyrics_progress = animate_value(
            self.lyrics_progress,
            lyrics_target,
            morph.notch_lyrics_morph_rate,
            dt,
            motion,
        );
        let resolved_tab_blend = Self::snap_to_target(next_tab_blend, 1.0);
        let resolved_lyrics_progress = Self::snap_to_target(next_lyrics_progress, lyrics_target);
        if (resolved_tab_blend - self.tab_blend).abs() > f32::EPSILON {
            self.tab_blend = resolved_tab_blend;
            changed = true;
        }
        if (resolved_tab_blend - 1.0).abs() <= f32::EPSILON && self.previous_tab != self.notch_tab {
            self.previous_tab = self.notch_tab;
            changed = true;
        }
        if (resolved_lyrics_progress - self.lyrics_progress).abs() > f32::EPSILON {
            self.lyrics_progress = resolved_lyrics_progress;
            changed = true;
        }
        if self.lyrics_is_live() {
            changed = true;
        }
        let next_title = animate_value(
            self.compact_title_points,
            self.compact_title_target,
            16.0,
            dt,
            motion,
        );
        let resolved_title = if (next_title - self.compact_title_target).abs() <= 0.25 {
            self.compact_title_target
        } else {
            next_title
        };
        if (resolved_title - self.compact_title_points).abs() > f32::EPSILON {
            self.compact_title_points = resolved_title;
            changed = true;
        }
        changed
    }

    fn step_expansion(
        value: &mut f32,
        target: f32,
        rate: f32,
        dt: f32,
        motion: MotionPreference,
    ) -> bool {
        let next = animate_value(*value, target, rate, dt, motion);
        let resolved = if next <= 0.008 && target < 0.5 {
            0.0
        } else if next >= 0.995 && target > 0.5 {
            1.0
        } else {
            next
        };
        if (resolved - *value).abs() > 0.0005 {
            *value = resolved;
            true
        } else {
            false
        }
    }

    fn lyrics_is_live(&self) -> bool {
        self.presented
            && self.panel_expansion > GLOW_SCENE_VISIBLE
            && self.notch_tab == NotchExpandedTab::Media
            && self.motion.allows_motion()
            && self.lyrics_progress > 0.5
            && self.media.as_ref().is_some_and(|media| media.playing)
            && self.lyrics.as_ref().is_some_and(|sheet| !sheet.is_empty())
    }

    fn snap_to_target(value: f32, target: f32) -> f32 {
        if (value - target).abs() <= 0.002 {
            target
        } else {
            value
        }
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
        let before = self.pomodoro;
        let tick = self.clock.sync(&mut self.pomodoro, now_ms);
        let (mut changed, mut effect) = self.apply_tick(tick, now);
        if tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Pomodoro;
        }
        let timer_tick = self.timer_clock.sync_timer(&mut self.quick_timer, now_ms);
        let (timer_changed, timer_effect) = self.apply_tick(timer_tick, now);
        if timer_tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Timer;
        }
        changed |= timer_changed;
        if effect.is_none() {
            effect = timer_effect;
        }
        let sw_visible = self.is_stopwatch_visible();
        if self
            .stopwatch_clock
            .sync(&mut self.stopwatch, now_ms, sw_visible)
            && sw_visible
        {
            changed = true;
        }
        if warning_threshold_crossed(before, self.pomodoro) {
            self.show_peek(PomodoroPeek::FiveMinutesLeft, now);
            changed = true;
        }
        if self.peek.as_ref().is_some_and(|peek| now >= peek.until) {
            self.peek = None;
            changed = true;
        }
        if self
            .media_peek
            .as_ref()
            .is_some_and(|peek| now >= peek.until)
        {
            self.media_peek = None;
            changed = true;
        }
        let pause_hold = Duration::from_millis(MotionMetrics::default().media_pause_hold_ms);
        if self
            .media_paused_at
            .is_some_and(|paused_at| now >= paused_at + pause_hold)
        {
            self.media_paused_at = None;
            changed = true;
        }
        if self.hud_until.is_some_and(|until| now >= until) {
            self.hud_until = None;
            changed = true;
        }
        if self.has_alert()
            && let Some(until) = self.alert_until
            && now >= until
            && self.panel_trigger == PanelTrigger::Closed
        {
            self.dismiss_active_alert();
            changed = true;
        }
        if self.step_shelf_drag_leave(now) {
            changed = true;
        }
        if self.step_clipboard_feedback(now) {
            changed = true;
        }
        if !self.has_compact_focus() {
            self.clear_focus_dismissed();
        }
        (changed, effect)
    }

    fn step_clipboard_feedback(&mut self, now: Instant) -> bool {
        if self
            .clipboard_copied_until
            .is_some_and(|until| now >= until)
        {
            self.clipboard_copied_until = None;
            return self.clipboard.clear_copied_feedback();
        }
        false
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
                && self.has_openable_panel()
                && let Some(since) = self.hover_since
                && now.duration_since(since) >= Duration::from_millis(HOVER_DEBOUNCE_MS)
            {
                if matches!(
                    self.notch_tab,
                    NotchExpandedTab::Pomodoro | NotchExpandedTab::Media
                ) {
                    self.sync_tab_to_primary(now);
                }
                self.ensure_valid_tab();
                self.panel_trigger = PanelTrigger::Hovered;
            }
        } else {
            if self.is_shelf_hold_open() {
                return;
            }
            self.hover_since = None;
            self.hover_blocked = false;
            if matches!(
                self.panel_trigger,
                PanelTrigger::Hovered | PanelTrigger::PinnedHovered
            ) {
                self.panel_trigger = PanelTrigger::Closed;
            }
        }
    }
}

fn sanitize_shelf_payload(payload: ShelfPayload) -> Option<ShelfPayload> {
    match payload {
        ShelfPayload::File(path) => {
            is_valid_shelf_file_path(&path).then_some(ShelfPayload::File(path))
        }
        ShelfPayload::Url(url) => is_openable_shelf_url(&url).then_some(ShelfPayload::Url(url)),
        ShelfPayload::Text(text) => (!text.trim().is_empty()).then_some(ShelfPayload::Text(text)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ALERT_HOLD_MS, NotchController, NotchEffect, NotchHotkey, NotchUpdate, unix_now_ms,
    };
    use otto_core::{
        ClockTime, FocusSubMode, HitTarget, IslandMode, MediaCommand, MotionPreference,
        NotchAction, NotchExpandedTab, NotchHudKind, NowPlaying, PomodoroStatus, PomodoroSurface,
    };
    use std::time::{Duration, Instant};

    fn at_clock(hour: u8, minute: u8) -> ClockTime {
        ClockTime {
            hour,
            minute,
            ..ClockTime::default()
        }
    }

    fn peek_text(notch: &NotchController) -> Option<String> {
        notch
            .pomodoro_scene(Instant::now())
            .peek
            .map(|frame| frame.text)
    }

    fn run_until_alert(notch: &mut NotchController) {
        notch.clock.restart(0);
        let start = notch.epoch;
        let mut now = start;
        while notch.pomodoro().status != PomodoroStatus::Alert
            && now < start + Duration::from_mins(30)
        {
            now += Duration::from_secs(20);
            let _ = notch.sync_clock(now);
        }
        assert_eq!(notch.pomodoro().status, PomodoroStatus::Alert);
    }

    fn visible_running_notch() -> NotchController {
        let mut notch = idle_notch();
        notch.compact_expansion = 1.0;
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch
    }

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
    fn starting_a_session_records_the_wall_clock_and_peeks_its_length() {
        let mut notch = idle_notch();
        notch.set_clock(at_clock(12, 23));
        let _ = notch.handle_click(HitTarget::PomodoroPlayPause);
        assert_eq!(notch.pomodoro().started_minute, Some(12 * 60 + 23));
        assert_eq!(peek_text(&notch).as_deref(), Some("Focus · 25 min"));
        notch.set_clock(at_clock(12, 40));
        let _ = notch.handle_click(HitTarget::PomodoroPlayPause);
        assert_eq!(peek_text(&notch).as_deref(), Some("Paused"));
        let _ = notch.handle_click(HitTarget::PomodoroPlayPause);
        assert_eq!(notch.pomodoro().started_minute, Some(12 * 60 + 23));
        assert_eq!(notch.pomodoro_scene(Instant::now()).clock, at_clock(12, 40));
    }

    #[test]
    fn reset_and_skip_clear_the_peek_and_the_start_time() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        assert!(peek_text(&notch).is_some());
        let _ = notch.handle_click(HitTarget::PomodoroReset);
        assert_eq!(peek_text(&notch), None);
        assert_eq!(notch.pomodoro().started_minute, None);
    }

    #[test]
    fn presets_change_the_session_length_without_a_peek() {
        let mut notch = idle_notch();
        let update = notch.handle_click(HitTarget::PomodoroPreset(2));
        assert!(update.changed);
        assert_eq!(notch.pomodoro().remaining_seconds, 45 * 60);
        assert_eq!(peek_text(&notch), None);
        assert!(!notch.handle_click(HitTarget::PomodoroPreset(2)).changed);
        assert!(!notch.handle_click(HitTarget::PomodoroPreset(9)).changed);
    }

    #[test]
    fn extending_from_the_alert_peeks_and_restarts_the_session_clock() {
        let mut notch = idle_notch();
        notch.set_clock(at_clock(9, 0));
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        run_until_alert(&mut notch);
        notch.set_clock(at_clock(9, 25));
        let _ = notch.handle_click(HitTarget::PomodoroExtendFive);
        assert_eq!(notch.pomodoro().status, PomodoroStatus::Running);
        assert_eq!(notch.pomodoro().started_minute, Some(9 * 60 + 25));
        assert_eq!(peek_text(&notch).as_deref(), Some("+5 min added"));
    }

    #[test]
    fn starting_the_next_phase_from_the_alert_peeks_the_break() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        run_until_alert(&mut notch);
        let _ = notch.handle_click(HitTarget::PomodoroStartNext);
        assert_eq!(peek_text(&notch).as_deref(), Some("Break · 5 min"));
    }

    #[test]
    fn five_minutes_left_peeks_exactly_once_and_the_peek_expires_by_deadline() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch.peek = None;
        notch.clock.restart(0);
        notch.pomodoro.remaining_seconds = 5 * 60 + 2;
        let start = notch.epoch;
        let _ = notch.sync_clock(start + Duration::from_secs(1));
        assert_eq!(peek_text(&notch), None);
        let shown_at = start + Duration::from_secs(3);
        let update = notch.sync_clock(shown_at);
        assert!(update.changed);
        assert_eq!(peek_text(&notch).as_deref(), Some("5 minutes left"));
        let until = notch.peek.as_ref().expect("peek").until;
        assert_eq!(
            notch.next_deadline(shown_at),
            Some(shown_at + Duration::from_secs(1)).min(Some(until))
        );
        let _ = notch.sync_clock(shown_at + Duration::from_secs(1));
        assert_eq!(peek_text(&notch).as_deref(), Some("5 minutes left"));
        let _ = notch.sync_clock(until);
        assert_eq!(peek_text(&notch), None);
    }

    #[test]
    fn glow_and_dim_follow_the_running_state_and_jump_under_reduced_motion() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        let now = notch.epoch + Duration::from_millis(16);
        let _ = notch.advance(now, false, false, MotionPreference::Full);
        let rising = notch.pomodoro_scene(now);
        assert!(rising.glow > 0.0 && rising.glow < 1.0 && rising.dim == 0.0);

        let mut reduced = idle_notch();
        let _ = reduced.handle_hotkey(NotchHotkey::TogglePomodoro);
        let _ = reduced.advance(now, false, false, MotionPreference::Reduced);
        assert!((reduced.pomodoro_scene(now).glow - 1.0).abs() < f32::EPSILON);
        let _ = reduced.handle_hotkey(NotchHotkey::TogglePomodoro);
        let later = now + Duration::from_millis(16);
        let _ = reduced.advance(later, false, false, MotionPreference::Reduced);
        let paused = reduced.pomodoro_scene(later);
        assert!(paused.glow < f32::EPSILON && (paused.dim - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn last_minute_pulse_is_steady_under_reduced_motion() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        notch.pomodoro.remaining_seconds = 30;
        let now = notch.epoch + Duration::from_millis(16);
        let _ = notch.advance(now, false, false, MotionPreference::Reduced);
        for offset in [0, 250, 500, 750] {
            let scene = notch.pomodoro_scene(now + Duration::from_millis(offset));
            assert!((scene.pulse - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn glow_frames_are_scheduled_only_while_running_visible_presented_and_animated() {
        let mut notch = visible_running_notch();
        notch.motion = MotionPreference::Full;
        let now = notch.epoch;
        notch.clock.restart(0);
        assert_eq!(
            notch.next_deadline(now),
            Some(now + Duration::from_millis(80))
        );
        assert!(notch.sync_clock(now + Duration::from_millis(80)).changed);
        notch.clock.restart(0);

        notch.set_presented(false);
        assert_eq!(notch.next_deadline(now), Some(now + Duration::from_secs(1)));
        notch.set_presented(true);

        notch.motion = MotionPreference::Reduced;
        assert_eq!(notch.next_deadline(now), Some(now + Duration::from_secs(1)));
        notch.motion = MotionPreference::Full;

        notch.compact_expansion = 0.0;
        assert_eq!(notch.next_deadline(now), Some(now + Duration::from_secs(1)));
        notch.compact_expansion = 1.0;

        let _ = notch.handle_click(HitTarget::PomodoroPlayPause);
        notch.peek = None;
        assert_eq!(notch.next_deadline(now), None);
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
            .checked_sub(Duration::from_millis(
                otto_core::MotionMetrics::default().hud_hold_ms,
            ))
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

    fn settle(notch: &mut NotchController, mut now: Instant) -> Instant {
        for _ in 0..400 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, false, true, MotionPreference::Full);
        }
        now
    }

    fn playing(title: &str) -> NowPlaying {
        NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from(title),
            artist: String::from("Artist"),
            playing: true,
            ..NowPlaying::default()
        }
    }

    fn media_peek_text(notch: &NotchController) -> Option<(String, String)> {
        notch
            .media_scene(Instant::now(), 0)
            .and_then(|scene| scene.peek)
            .map(|frame| (frame.label.to_owned(), frame.title))
    }

    fn with_media(title: &str) -> NotchController {
        let mut notch = idle_notch();
        assert!(notch.set_media_at(Some(playing(title)), 0));
        notch.media_peek = None;
        notch
    }

    #[test]
    fn a_new_track_peeks_now_playing_with_its_title() {
        let mut notch = with_media("Song A");
        assert!(notch.set_media_at(Some(playing("Song B")), 0));
        assert_eq!(
            media_peek_text(&notch),
            Some((String::from("Now playing"), String::from("Song B")))
        );
    }

    #[test]
    fn pausing_peeks_and_holds_the_compact_island_for_eight_seconds() {
        let mut notch = with_media("Song");
        let paused = NowPlaying {
            playing: false,
            ..playing("Song")
        };
        assert!(notch.set_media_at(Some(paused), 0));
        assert_eq!(
            media_peek_text(&notch),
            Some((String::from("Paused"), String::from("Song")))
        );
        let paused_at = notch.media_paused_at.expect("pause time");
        assert!(matches!(
            notch.island_mode(paused_at + Duration::from_secs(7)),
            IslandMode::Compact(_)
        ));
        assert_eq!(
            notch.island_mode(paused_at + Duration::from_secs(9)),
            IslandMode::Collapsed
        );
        let expiry = paused_at + Duration::from_secs(8);
        assert!(notch.next_deadline(paused_at).is_some_and(|d| d <= expiry));
        let update = notch.sync_clock(expiry);
        assert!(update.changed);
        assert_eq!(notch.media_paused_at, None);
        assert_eq!(notch.island_mode(expiry), IslandMode::Collapsed);
    }

    #[test]
    fn resuming_peeks_playing_and_ends_the_hold() {
        let mut notch = with_media("Song");
        let _ = notch.set_media_at(
            Some(NowPlaying {
                playing: false,
                ..playing("Song")
            }),
            0,
        );
        assert!(notch.set_media_at(Some(playing("Song")), 0));
        assert_eq!(
            media_peek_text(&notch),
            Some((String::from("Playing"), String::from("Song")))
        );
        assert_eq!(notch.media_paused_at, None);
    }

    #[test]
    fn next_and_previous_peek_skipped_and_play_pause_toggles_the_hold() {
        let mut notch = with_media("Song");
        let _ = notch.handle_click(HitTarget::MediaNext);
        assert_eq!(
            media_peek_text(&notch),
            Some((String::from("Skipped"), String::from("Song")))
        );
        let _ = notch.handle_click(HitTarget::MediaPlayPause);
        assert_eq!(
            media_peek_text(&notch),
            Some((String::from("Paused"), String::from("Song")))
        );
        assert!(notch.media_paused_at.is_some());
        let _ = notch.handle_click(HitTarget::MediaPlayPause);
        assert_eq!(
            media_peek_text(&notch),
            Some((String::from("Playing"), String::from("Song")))
        );
        assert!(notch.media_paused_at.is_none());
    }

    #[test]
    fn an_unsupported_control_neither_peeks_nor_holds() {
        let mut notch = idle_notch();
        let mut stream = playing("Song");
        stream.capabilities.next = false;
        let _ = notch.set_media_at(Some(stream), 0);
        notch.media_peek = None;
        let update = notch.handle_click(HitTarget::MediaNext);
        assert!(!update.changed && update.effect.is_none());
        assert_eq!(media_peek_text(&notch), None);
    }

    #[test]
    fn media_peeks_stay_quiet_while_the_pomodoro_owns_the_compact_island() {
        let mut notch = with_media("Song A");
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        let _ = notch.set_media_at(Some(playing("Song B")), 0);
        assert_eq!(media_peek_text(&notch), None);
    }

    #[test]
    fn losing_the_media_clears_its_peek_and_hold() {
        let mut notch = with_media("Song");
        let _ = notch.handle_click(HitTarget::MediaPlayPause);
        assert!(notch.set_media_at(None, 0));
        assert!(notch.media_peek.is_none() && notch.media_paused_at.is_none());
    }

    #[test]
    fn the_media_peek_expires_by_deadline() {
        let mut notch = with_media("Song A");
        let _ = notch.set_media_at(Some(playing("Song B")), 0);
        let until = notch.media_peek.as_ref().expect("peek").until;
        assert!(
            notch
                .next_deadline(until.checked_sub(Duration::from_secs(2)).expect("earlier"))
                .is_some()
        );
        let update = notch.sync_clock(until);
        assert!(update.changed);
        assert!(notch.media_peek.is_none());
    }

    #[test]
    fn media_progress_repaints_each_second_only_when_panel_or_secondary_is_visible_and_playing() {
        let mut notch = idle_notch();
        let _ = notch.set_media_at(
            Some(NowPlaying {
                position_updated_unix_ms: Some(unix_now_ms()),
                duration_ms: 240_000,
                ..playing("Song")
            }),
            0,
        );
        notch.media_peek = None;
        notch.compact_expansion = 1.0;
        notch.motion = MotionPreference::Reduced;
        let now = Instant::now();
        assert!(!notch.shows_live_media_progress());

        notch.panel_expansion = 1.0;
        notch.notch_tab = NotchExpandedTab::Media;
        assert!(notch.shows_live_media_progress());
        assert!(
            notch
                .next_deadline(now)
                .is_some_and(|d| d <= now + Duration::from_secs(1))
        );
        notch.set_presented(false);
        assert!(!notch.shows_live_media_progress());
        notch.set_presented(true);
        notch.notch_tab = NotchExpandedTab::Pomodoro;
        assert!(!notch.shows_live_media_progress());

        notch.panel_expansion = 0.0;
        notch.secondary_expansion = 1.0;
        assert!(notch.shows_live_media_progress());
    }

    #[test]
    fn live_stream_schedules_drift_frames_only_on_expanded_media_tab() {
        let mut notch = idle_notch();
        let stream = NowPlaying {
            duration_ms: 0,
            ..playing("Radio")
        };
        let _ = notch.set_media_at(Some(stream), 0);
        notch.media_peek = None;
        notch.compact_expansion = 1.0;
        notch.motion = MotionPreference::Full;
        let now = Instant::now();
        let frame = now + Duration::from_millis(80);
        assert_eq!(notch.next_deadline(now), None);

        notch.panel_expansion = 1.0;
        notch.notch_tab = NotchExpandedTab::Media;
        assert!(notch.next_deadline(now).is_some_and(|d| d <= frame));
        assert!(notch.sync_clock(frame).changed);
        notch.motion = MotionPreference::Reduced;
        let static_scene = notch.media_scene(now, 0).expect("scene");
        assert!((static_scene.live_phase - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn the_cover_dims_smoothly_when_paused_and_jumps_under_reduced_motion() {
        let mut notch = with_media("Song");
        let _ = notch.set_media_at(
            Some(NowPlaying {
                playing: false,
                ..playing("Song")
            }),
            0,
        );
        let now = notch.epoch + Duration::from_millis(16);
        let _ = notch.advance(now, false, false, MotionPreference::Full);
        let dim = notch.media_scene(now, 0).expect("scene").dim;
        assert!(dim > 0.0 && dim < 1.0);
        let _ = notch.advance(
            now + Duration::from_millis(16),
            false,
            false,
            MotionPreference::Reduced,
        );
        let settled = notch.media_scene(now, 0).expect("scene").dim;
        assert!((settled - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_compact_width_follows_the_active_lyric_and_relaxes_without_one() {
        let mut notch = with_media("Song");
        notch.set_lyrics(Some(otto_core::LyricsSheet::new(vec![
            otto_core::LyricLine {
                timestamp_ms: 0,
                text: String::from("A synced line"),
            },
        ])));
        notch.set_lyrics_in_compact(true);
        let now = Instant::now();
        assert!(notch.sync_compact_title_width(now, 0, |line| line.len() as f32 * 7.0));
        assert!(!notch.sync_compact_title_width(now, 0, |_| panic!("same line is measured once")));
        let _ = notch.advance(
            now + Duration::from_millis(16),
            false,
            false,
            MotionPreference::Reduced,
        );
        assert_eq!(notch.layout_input(true).compact_title_points, 91);

        notch.set_lyrics_in_compact(false);
        assert!(notch.sync_compact_title_width(now, 0, |_| 0.0));
        let _ = notch.advance(
            now + Duration::from_millis(32),
            false,
            false,
            MotionPreference::Reduced,
        );
        assert_eq!(notch.layout_input(true).compact_title_points, 0);
    }

    #[test]
    fn the_active_lyric_replaces_the_compact_title_only_after_the_opt_in() {
        let mut notch = with_media("Song");
        notch.set_lyrics(Some(otto_core::LyricsSheet::new(vec![
            otto_core::LyricLine {
                timestamp_ms: 0,
                text: String::from("A synced line"),
            },
        ])));
        assert_eq!(
            notch
                .media_scene(Instant::now(), 0)
                .expect("scene")
                .lyric_line,
            None
        );
        notch.set_lyrics_in_compact(true);
        assert_eq!(
            notch
                .media_scene(Instant::now(), 0)
                .expect("scene")
                .lyric_line
                .as_deref(),
            Some("A synced line")
        );
        notch.set_lyrics_in_compact(false);
        assert_eq!(
            notch
                .media_scene(Instant::now(), 0)
                .expect("scene")
                .lyric_line,
            None
        );
    }

    #[test]
    fn a_volume_burst_while_the_panel_is_open_keeps_the_panel_and_hides_the_hud() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        let now = settle(&mut notch, Instant::now());
        for _ in 0..20 {
            notch.trigger_hud(NotchHudKind::Volume {
                percent: 40,
                muted: false,
            });
        }
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);
        let _ = settle(&mut notch, now);
        assert!((notch.panel_expansion() - 1.0).abs() < f32::EPSILON);
        assert!(notch.hud_expansion.abs() < 0.01);
    }

    #[test]
    fn right_click_dismisses_compact_media_until_the_track_changes() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let now = settle(&mut notch, Instant::now());
        assert!((notch.compact_expansion - 1.0).abs() < f32::EPSILON);
        assert!(notch.dismiss_compact(now).changed);
        let now = settle(&mut notch, now);
        assert!(notch.compact_expansion.abs() < f32::EPSILON);
        assert_eq!(notch.island_mode(now), IslandMode::Collapsed);
        assert!(notch.set_media(Some(playing("Next song"))));
        let now = settle(&mut notch, now);
        assert!((notch.compact_expansion - 1.0).abs() < f32::EPSILON);
        assert!(matches!(notch.island_mode(now), IslandMode::Compact(_)));
    }

    #[test]
    fn dismissing_needs_compact_content_and_never_closes_a_pinned_panel() {
        let mut notch = idle_notch();
        let now = Instant::now();
        assert_eq!(notch.dismiss_compact(now), NotchUpdate::default());
        let mut playing_notch =
            NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let _ = playing_notch.handle_hotkey(NotchHotkey::ToggleNotch);
        assert_eq!(playing_notch.dismiss_compact(now), NotchUpdate::default());
    }

    #[test]
    fn dismissing_from_a_hover_panel_closes_it_until_the_pointer_leaves() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let mut now = Instant::now();
        notch.handle_pointer_move(true);
        for _ in 0..40 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, true, true, MotionPreference::Full);
        }
        assert_eq!(notch.island_mode(now), otto_core::IslandMode::Expanded);
        assert!(notch.dismiss_compact(now).changed);
        notch.handle_pointer_move(true);
        for _ in 0..80 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, true, true, MotionPreference::Full);
        }
        assert_eq!(notch.island_mode(now), otto_core::IslandMode::Collapsed);
        notch.handle_pointer_move(false);
        notch.handle_pointer_move(true);
        for _ in 0..40 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, true, true, MotionPreference::Full);
        }
        assert_eq!(notch.island_mode(now), otto_core::IslandMode::Expanded);
    }

    #[test]
    fn rapid_notch_toggles_settle_to_the_last_request() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let mut now = Instant::now();
        for _ in 0..25 {
            let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
            now += Duration::from_millis(8);
            let _ = notch.advance(now, false, true, MotionPreference::Full);
        }
        let now = settle(&mut notch, now);
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);
        assert!((notch.panel_expansion() - 1.0).abs() < f32::EPSILON);
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        let now = settle(&mut notch, now);
        assert!(notch.panel_expansion().abs() < f32::EPSILON);
        assert!(matches!(notch.island_mode(now), IslandMode::Compact(_)));
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
            ..NowPlaying::default()
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
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);

        let update = notch.handle_seekbar_click(0.5, 50_000);

        assert!(update.changed);
        assert_eq!(notch.media().map(|item| item.position_ms), Some(100_000));
        assert_eq!(
            notch.media().and_then(|item| item.position_updated_unix_ms),
            Some(50_000)
        );
        assert_eq!(
            update.effect,
            Some(NotchEffect::Platform(NotchAction::Media(
                MediaCommand::Seek(100_000)
            )))
        );
    }

    #[test]
    fn unsupported_transport_commands_are_ignored() {
        let media = NowPlaying {
            title: String::from("Live radio"),
            playing: true,
            capabilities: otto_core::MediaCapabilities {
                next: false,
                previous: false,
                seek: false,
                play_pause: true,
            },
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        assert_eq!(
            notch.handle_click(HitTarget::MediaNext),
            NotchUpdate::default()
        );
        assert_eq!(
            notch.handle_click(HitTarget::MediaPrevious),
            NotchUpdate::default()
        );
        assert_eq!(notch.handle_seekbar_click(0.5, 0), NotchUpdate::default());
        let toggle = notch.handle_click(HitTarget::MediaPlayPause);
        assert!(toggle.changed);
        assert_eq!(notch.media().map(|item| item.playing), Some(false));
    }

    #[test]
    fn a_track_from_another_application_counts_as_a_track_change() {
        let spotify = NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(spotify.clone()), NotchExpandedTab::Media);
        notch.set_lyrics(Some(otto_core::parse_lrc("[00:01.00]line")));
        let browser = NowPlaying {
            source_app: String::from("chrome.exe"),
            ..spotify
        };
        assert!(notch.set_media(Some(browser)));
        assert!(notch.lyrics_view().sheet.is_none());
    }

    #[test]
    fn playing_media_in_the_open_panel_wakes_once_per_media_second() {
        let media = NowPlaying {
            title: String::from("Song"),
            playing: true,
            position_ms: 10_400,
            duration_ms: 100_000,
            position_updated_unix_ms: Some(1_000),
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        assert_eq!(notch.media_repaint_in(1_000), None);
        notch.panel_expansion = 1.0;
        assert_eq!(
            notch.media_repaint_in(1_000),
            Some(Duration::from_millis(600))
        );
        notch.media.as_mut().unwrap().playing = false;
        assert_eq!(notch.media_repaint_in(1_000), None);
    }

    #[test]
    fn synced_lyrics_wake_at_exact_millisecond_timestamps_in_compact_and_drawer() {
        let media = NowPlaying {
            title: String::from("Song"),
            playing: true,
            position_ms: 10_000,
            duration_ms: 100_000,
            position_updated_unix_ms: Some(1_000),
            ..NowPlaying::default()
        };
        let sheet = otto_core::LyricsSheet::new(vec![
            otto_core::LyricLine {
                timestamp_ms: 9_000,
                text: String::from("First"),
            },
            otto_core::LyricLine {
                timestamp_ms: 10_150,
                text: String::from("Second"),
            },
        ]);
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        notch.set_lyrics(Some(sheet));
        notch.compact_expansion = 1.0;
        assert_eq!(notch.media_repaint_in(1_000), None);

        notch.set_lyrics_in_compact(true);
        assert_eq!(
            notch.media_repaint_in(1_000),
            Some(Duration::from_millis(150))
        );
        assert!(notch.sync_clock(Instant::now()).changed);

        notch.set_lyrics_in_compact(false);
        notch.compact_expansion = 0.0;
        notch.panel_expansion = 1.0;
        assert_eq!(notch.media_repaint_in(1_000), Some(Duration::from_secs(1)));

        let _ = notch.toggle_lyrics();
        assert_eq!(
            notch.media_repaint_in(1_000),
            Some(Duration::from_millis(150))
        );
    }

    #[test]
    fn synced_lyrics_trigger_repaint_on_lyric_drift_or_line_crossing() {
        let baseline = NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 11_950,
            duration_ms: 100_000,
            position_updated_unix_ms: Some(1_000),
            ..NowPlaying::default()
        };
        let sheet = otto_core::LyricsSheet::new(vec![
            otto_core::LyricLine {
                timestamp_ms: 9_000,
                text: String::from("Line one"),
            },
            otto_core::LyricLine {
                timestamp_ms: 12_000,
                text: String::from("Line two"),
            },
        ]);
        let mut notch = NotchController::new(Some(baseline.clone()), NotchExpandedTab::Media);
        notch.set_lyrics(Some(sheet));
        notch.compact_expansion = 1.0;
        notch.set_lyrics_in_compact(true);

        let tiny_same_line_drift = NowPlaying {
            position_ms: 11_900,
            ..baseline.clone()
        };
        assert!(!notch.set_media_at(Some(tiny_same_line_drift), 1_000));

        let crosses_line_within_tolerance = NowPlaying {
            position_ms: 12_010,
            ..baseline.clone()
        };
        assert!(notch.set_media_at(Some(crosses_line_within_tolerance), 1_000));

        notch.compact_expansion = 0.0;
        notch.panel_expansion = 1.0;
        let _ = notch.toggle_lyrics();
        let large_drift_without_line_flip = NowPlaying {
            position_ms: 12_250,
            ..baseline
        };
        assert!(notch.set_media_at(Some(large_drift_without_line_flip), 1_000));
    }

    fn seekable_track() -> NowPlaying {
        NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            position_ms: 30_000,
            duration_ms: 200_000,
            position_updated_unix_ms: Some(0),
            ..NowPlaying::default()
        }
    }

    #[test]
    fn dragging_previews_the_position_and_seeks_only_on_release() {
        let mut notch = NotchController::new(Some(seekable_track()), NotchExpandedTab::Media);
        assert!(notch.begin_seek_drag(0.25));
        assert!(notch.update_seek_drag(0.5));
        assert_eq!(
            notch.display_media(5_000).map(|media| media.position_ms),
            Some(100_000)
        );
        let update = notch.finish_seek_drag(5_000);
        assert_eq!(
            update.effect,
            Some(NotchEffect::Platform(NotchAction::Media(
                MediaCommand::Seek(100_000)
            )))
        );
        assert!(!notch.is_seek_dragging());
    }

    #[test]
    fn a_cancelled_drag_sends_nothing_and_restores_the_live_position() {
        let mut notch = NotchController::new(Some(seekable_track()), NotchExpandedTab::Media);
        assert!(notch.begin_seek_drag(0.9));
        assert!(notch.cancel_seek_drag());
        assert_eq!(notch.finish_seek_drag(1_000), NotchUpdate::default());
        assert_eq!(
            notch.display_media(1_000).map(|media| media.position_ms),
            Some(31_000)
        );
    }

    #[test]
    fn stale_readback_after_a_seek_is_held_until_confirmed_or_expired() {
        let mut notch = NotchController::new(Some(seekable_track()), NotchExpandedTab::Media);
        let _ = notch.handle_seekbar_click(0.5, 10_000);
        let stale = seekable_track();
        notch.set_media_at(Some(stale.clone()), 10_400);
        assert_eq!(
            notch.display_media(10_400).map(|media| media.position_ms),
            Some(100_400)
        );
        let confirmed = NowPlaying {
            position_ms: 100_500,
            position_updated_unix_ms: Some(10_500),
            ..seekable_track()
        };
        notch.set_media_at(Some(confirmed), 10_600);
        assert!(notch.pending_seek.is_none());

        let _ = notch.handle_seekbar_click(0.9, 20_000);
        notch.set_media_at(Some(stale), 20_000 + otto_core::SEEK_READBACK_TIMEOUT_MS);
        assert!(notch.pending_seek.is_none());
    }

    #[test]
    fn unseekable_media_cannot_start_a_drag() {
        let live = NowPlaying {
            duration_ms: 0,
            ..seekable_track()
        };
        let mut notch = NotchController::new(Some(live), NotchExpandedTab::Media);
        assert!(!notch.begin_seek_drag(0.5));
        assert!(!notch.is_seek_dragging());
    }

    #[test]
    fn lyrics_status_tracks_requests_results_and_track_changes() {
        let track = NowPlaying {
            title: String::from("Song"),
            artist: String::from("Artist"),
            playing: true,
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(track.clone()), NotchExpandedTab::Media);
        assert_eq!(notch.lyrics_view().status, otto_core::LyricsStatus::Idle);
        notch.begin_lyrics_request();
        assert_eq!(notch.lyrics_view().status, otto_core::LyricsStatus::Loading);
        notch.finish_lyrics(None, otto_core::LyricsStatus::Failed);
        assert_eq!(notch.lyrics_view().status, otto_core::LyricsStatus::Failed);
        notch.finish_lyrics(
            Some(otto_core::LyricsSheet::default()),
            otto_core::LyricsStatus::Ready,
        );
        assert_eq!(
            notch.lyrics_view().status,
            otto_core::LyricsStatus::NotFound {
                searched_online: true
            }
        );
        assert!(notch.lyrics_view().sheet.is_none());
        let next = NowPlaying {
            title: String::from("Next"),
            ..track
        };
        assert!(notch.set_media(Some(next)));
        assert_eq!(notch.lyrics_view().status, otto_core::LyricsStatus::Idle);
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
            ..NowPlaying::default()
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
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        let now = std::time::Instant::now();
        notch.compact_expansion = 1.0;
        notch.last_anim_instant = now;

        let update = notch.advance(now, false, false, MotionPreference::Full);

        assert!(!update.changed);
    }

    #[test]
    fn media_lyrics_toggle_morphs_the_drawer_open_and_closed() {
        let mut notch = NotchController::new(Some(playing("Track")), NotchExpandedTab::Media);
        assert!(notch.layout_input(true).lyrics_progress.abs() < f32::EPSILON);

        let update = notch.handle_click(HitTarget::MediaLyricsToggle);
        assert!(update.changed);
        assert_eq!(update.effect, None);
        assert!(notch.lyrics_expanded());
        assert!(notch.layout_input(true).lyrics_progress.abs() < f32::EPSILON);

        let now = Instant::now() + Duration::from_millis(8);
        let _ = notch.advance(now, false, true, MotionPreference::Full);
        let mid = notch.layout_input(true).lyrics_progress;
        assert!(mid > 0.0 && mid < 1.0);

        let _ = settle(&mut notch, now);
        assert!((notch.layout_input(true).lyrics_progress - 1.0).abs() < f32::EPSILON);

        let _ = notch.handle_click(HitTarget::MediaLyricsToggle);
        assert!(!notch.lyrics_expanded());
        let _ = settle(&mut notch, Instant::now());
        assert!(notch.layout_input(true).lyrics_progress.abs() < f32::EPSILON);
    }

    #[test]
    fn tab_switch_morphs_between_focus_and_now_playing() {
        let mut notch = NotchController::new(Some(playing("Track")), NotchExpandedTab::Pomodoro);
        assert_eq!(
            notch.layout_input(true).active_tab,
            NotchExpandedTab::Pomodoro
        );
        assert!((notch.layout_input(true).tab_blend - 1.0).abs() < f32::EPSILON);

        let _ = notch.handle_click(HitTarget::NotchTabMedia);
        assert_eq!(notch.layout_input(true).active_tab, NotchExpandedTab::Media);
        assert_eq!(
            notch.layout_input(true).previous_tab,
            NotchExpandedTab::Pomodoro
        );
        assert!(notch.layout_input(true).tab_blend.abs() < f32::EPSILON);

        let now = Instant::now() + Duration::from_millis(8);
        let _ = notch.advance(now, false, true, MotionPreference::Full);
        let mid = notch.layout_input(true).tab_blend;
        assert!(mid > 0.0 && mid < 1.0);

        let now = settle(&mut notch, now);
        assert!((notch.layout_input(true).tab_blend - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            notch.layout_input(true).previous_tab,
            NotchExpandedTab::Media
        );

        let _ = notch.handle_click(HitTarget::NotchTabPomodoro);
        assert_eq!(
            notch.layout_input(true).previous_tab,
            NotchExpandedTab::Media
        );
        assert!(notch.layout_input(true).tab_blend.abs() < f32::EPSILON);
        let _ = settle(&mut notch, now);
        assert!((notch.layout_input(true).tab_blend - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            notch.layout_input(true).previous_tab,
            NotchExpandedTab::Pomodoro
        );
    }

    #[test]
    fn switching_among_four_tabs_reverses_mid_flight_and_guards_missing_media() {
        let mut notch = NotchController::new(None, NotchExpandedTab::Pomodoro);
        assert!(!notch.handle_click(HitTarget::NotchTabMedia).changed);
        assert_eq!(
            notch.layout_input(true).active_tab,
            NotchExpandedTab::Pomodoro
        );

        assert!(notch.handle_click(HitTarget::NotchTabShelf).changed);
        let t1 = Instant::now() + Duration::from_millis(8);
        let _ = notch.advance(t1, false, true, MotionPreference::Full);
        let mid = notch.layout_input(true).tab_blend;
        assert!(mid > 0.0 && mid < 1.0);

        assert!(notch.handle_click(HitTarget::NotchTabPomodoro).changed);
        let reversed = notch.layout_input(true);
        assert_eq!(reversed.active_tab, NotchExpandedTab::Pomodoro);
        assert_eq!(reversed.previous_tab, NotchExpandedTab::Shelf);
        assert!((reversed.tab_blend - (1.0 - mid)).abs() < 0.001);

        let t2 = settle(&mut notch, t1);
        assert!(notch.handle_click(HitTarget::NotchTabClipboard).changed);
        let _ = settle(&mut notch, t2);
        assert_eq!(
            notch.layout_input(true).active_tab,
            NotchExpandedTab::Clipboard
        );

        assert!(notch.set_media(Some(playing("Track"))));
        assert!(notch.handle_click(HitTarget::NotchTabMedia).changed);
        let _ = settle(&mut notch, Instant::now());
        assert!(notch.set_media(None));
        assert_eq!(
            notch.layout_input(true).active_tab,
            NotchExpandedTab::Pomodoro
        );
    }

    #[test]
    fn leaving_the_media_tab_closes_the_lyrics_drawer_with_it() {
        let mut notch = NotchController::new(Some(playing("Track")), NotchExpandedTab::Media);
        let _ = notch.handle_click(HitTarget::MediaLyricsToggle);
        let now = settle(&mut notch, Instant::now());
        assert!((notch.layout_input(true).lyrics_progress - 1.0).abs() < f32::EPSILON);

        let _ = notch.handle_click(HitTarget::NotchTabPomodoro);
        let _ = settle(&mut notch, now);
        assert!(notch.layout_input(true).lyrics_progress.abs() < f32::EPSILON);
        assert!(notch.lyrics_expanded());
    }

    #[test]
    fn reduced_motion_jumps_tab_and_lyrics_to_their_targets() {
        let mut notch = NotchController::new(Some(playing("Track")), NotchExpandedTab::Pomodoro);
        let _ = notch.handle_click(HitTarget::NotchTabMedia);
        let _ = notch.handle_click(HitTarget::MediaLyricsToggle);
        let now = Instant::now() + Duration::from_millis(8);
        let update = notch.advance(now, false, true, MotionPreference::Reduced);
        assert!(update.changed);
        let layout = notch.layout_input(true);
        assert!((layout.tab_blend - 1.0).abs() < f32::EPSILON);
        assert!((layout.lyrics_progress - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn lyric_frames_run_only_while_the_open_drawer_plays_synced_lines() {
        let mut notch = NotchController::new(Some(playing("Track")), NotchExpandedTab::Media);
        notch.set_lyrics(Some(otto_core::LyricsSheet::new(vec![
            otto_core::LyricLine {
                timestamp_ms: 0,
                text: String::from("First line"),
            },
        ])));
        let mut now = settle(&mut notch, Instant::now());
        now += Duration::from_millis(8);
        assert!(
            !notch
                .advance(now, false, true, MotionPreference::Full)
                .changed
        );

        let _ = notch.handle_click(HitTarget::MediaLyricsToggle);
        now = settle(&mut notch, now);
        now += Duration::from_millis(8);
        assert!(
            !notch
                .advance(now, false, false, MotionPreference::Full)
                .changed
        );

        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        now = settle(&mut notch, now);
        now += Duration::from_millis(8);
        assert!(
            notch
                .advance(now, false, true, MotionPreference::Full)
                .changed
        );

        notch.set_presented(false);
        now += Duration::from_millis(8);
        assert!(
            !notch
                .advance(now, false, true, MotionPreference::Full)
                .changed
        );
        notch.set_presented(true);

        now += Duration::from_millis(8);
        assert!(
            !notch
                .advance(now, false, true, MotionPreference::Reduced)
                .changed
        );

        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        now = settle(&mut notch, now);
        now += Duration::from_millis(8);
        assert!(
            !notch
                .advance(now, false, false, MotionPreference::Full)
                .changed
        );

        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        if let Some(media) = notch.media.as_mut() {
            media.playing = false;
        }
        now = settle(&mut notch, now);
        now += Duration::from_millis(8);
        assert!(
            !notch
                .advance(now, false, true, MotionPreference::Full)
                .changed
        );
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
            ..NowPlaying::default()
        };
        let mut notch = NotchController::new(Some(media), NotchExpandedTab::Media);
        let sheet = otto_core::parse_lrc("[00:01.00]Hello world");
        notch.set_lyrics(Some(sheet));
        assert!(notch.lyrics_view().sheet.is_some());

        let same_track_updated_pos = NowPlaying {
            title: String::from("Track 1"),
            artist: String::from("Artist 1"),
            playing: true,
            position_ms: 50_000,
            duration_ms: 200_000,
            thumbnail: None,
            ..NowPlaying::default()
        };
        notch.set_media(Some(same_track_updated_pos));
        assert!(notch.lyrics_view().sheet.is_some());

        let different_track = NowPlaying {
            title: String::from("Track 2"),
            artist: String::from("Artist 2"),
            playing: true,
            position_ms: 0,
            duration_ms: 180_000,
            thumbnail: None,
            ..NowPlaying::default()
        };
        notch.set_media(Some(different_track));
        assert!(notch.lyrics_view().sheet.is_none());
    }

    #[test]
    fn quick_timer_presets_countdown_alert_and_extend_work_through_focus_sub_mode() {
        let mut notch = idle_notch();
        let start = notch.epoch;
        let _ = notch.handle_click(HitTarget::FocusMode(FocusSubMode::Timer));
        let _ = notch.handle_click(HitTarget::PomodoroPreset(0));
        assert_eq!(notch.quick_timer.remaining_seconds, 60);

        let _ = notch.handle_click(HitTarget::PomodoroPlayPause);
        notch.timer_clock.restart(0);
        assert_eq!(notch.quick_timer.status, PomodoroStatus::Running);

        let mut now = start;
        let mut alert_effect = None;
        for _ in 0..3 {
            now += Duration::from_secs(20);
            let update = notch.sync_clock(now);
            if update.effect.is_some() {
                alert_effect = update.effect;
            }
        }
        assert!(matches!(
            alert_effect,
            Some(NotchEffect::Platform(NotchAction::PlayPomodoroChime))
        ));
        assert_eq!(notch.quick_timer.status, PomodoroStatus::Alert);
        assert_eq!(
            notch.layout_input(true).pomodoro_surface,
            PomodoroSurface::Alert
        );

        let _ = notch.handle_click(HitTarget::PomodoroExtendFive);
        assert_eq!(notch.quick_timer.status, PomodoroStatus::Running);
        assert_eq!(notch.quick_timer.remaining_seconds, 5 * 60);
    }

    #[test]
    fn stopwatch_records_laps_resets_and_gates_one_hz_wakeups_on_visibility() {
        let mut notch = idle_notch();
        let start = notch.epoch;
        let _ = notch.handle_click(HitTarget::FocusMode(FocusSubMode::Stopwatch));
        let _ = notch.handle_click(HitTarget::PomodoroPlayPause);
        notch.stopwatch_clock.restart(0);

        assert_eq!(notch.next_deadline(start), None);

        notch.panel_expansion = 1.0;
        let revealed_at = start + Duration::from_secs(5);
        let deadline = notch
            .next_deadline(revealed_at)
            .expect("visible stopwatch should schedule a 1 Hz wakeup");
        assert!(deadline - revealed_at <= Duration::from_secs(1));

        let update = notch.sync_clock(revealed_at);
        assert!(update.changed);
        assert_eq!(notch.stopwatch.elapsed_ms, 5_000);

        let _ = notch.handle_click(HitTarget::PomodoroSkip);
        assert_eq!(notch.stopwatch.lap_count, 1);
        assert_eq!(notch.stopwatch.last_lap_ms, Some(5_000));

        let _ = notch.handle_click(HitTarget::PomodoroReset);
        assert_eq!(notch.stopwatch.elapsed_ms, 0);
        assert_eq!(notch.stopwatch.lap_count, 0);
        assert!(!notch.stopwatch.running);
    }

    #[test]
    fn privacy_mic_pill_toggles_mute_optimistically_and_hides_when_unavailable() {
        let mut notch = idle_notch();
        assert!(!notch.layout_input(true).privacy.compact_dots);
        assert!(!notch.layout_input(true).privacy.panel_mic_control);
        assert_eq!(
            notch.handle_click(HitTarget::PrivacyMicMute),
            NotchUpdate::default()
        );

        assert!(notch.sync_privacy_state(otto_core::PrivacyState {
            mic: otto_core::MicState::Active { muted: false },
            camera_active: false,
            app_name: Some(String::from("Zoom")),
        }));
        let layout = notch.layout_input(true);
        assert!(layout.privacy.compact_dots);
        assert!(layout.privacy.panel_mic_control);

        let muted = notch.handle_click(HitTarget::PrivacyMicMute);
        assert!(muted.changed);
        assert!(notch.privacy_state().mic.is_muted());
        assert_eq!(
            muted.effect,
            Some(NotchEffect::Platform(NotchAction::ToggleMicMute))
        );
    }

    #[test]
    fn clicking_privacy_indicator_opens_panel_showing_privacy_mic_control() {
        let mut notch = idle_notch();
        assert!(notch.sync_privacy_state(otto_core::PrivacyState {
            mic: otto_core::MicState::Idle { muted: false },
            camera_active: true,
            app_name: Some(String::from("Camera")),
        }));
        assert!(notch.layout_input(true).privacy.compact_dots);

        let update = notch.handle_click(HitTarget::PrivacyIndicator);
        assert!(update.changed);
        assert!(update.effect.is_none());
        assert_eq!(notch.active_tab(), NotchExpandedTab::Shelf);

        let mut media_notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        assert!(media_notch.sync_privacy_state(otto_core::PrivacyState {
            mic: otto_core::MicState::Active { muted: false },
            camera_active: false,
            app_name: Some(String::from("Meet")),
        }));
        let media_update = media_notch.handle_click(HitTarget::PrivacyIndicator);
        assert!(media_update.changed);
        assert_eq!(media_notch.active_tab(), NotchExpandedTab::Media);
    }

    #[test]
    fn shelf_drag_enter_opens_shelf_tab_and_leave_honors_grace_deadline() {
        let mut notch = idle_notch();
        let t0 = Instant::now();
        let enter = notch.handle_shelf_drag_enter();
        assert!(enter.changed);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Shelf);
        assert!(notch.shelf_state().drag_hover);
        assert_eq!(notch.island_mode(t0), IslandMode::Expanded);

        let leave = notch.handle_shelf_drag_leave(t0);
        assert!(leave.changed);
        assert!(!notch.shelf_state().drag_hover);
        assert_eq!(notch.island_mode(t0), IslandMode::Expanded);
        assert_eq!(
            notch.island_mode(t0 + Duration::from_millis(200)),
            IslandMode::Expanded
        );

        let _ = notch.sync_clock(t0 + Duration::from_millis(220));
        assert_eq!(
            notch.island_mode(t0 + Duration::from_millis(220)),
            IslandMode::Collapsed
        );
    }

    #[test]
    fn shelf_drop_adds_items_and_preserves_open_panel_for_pointer_inspection() {
        let mut notch = idle_notch();
        let t0 = Instant::now();
        let _ = notch.handle_shelf_drag_enter();
        let drop_update = notch.handle_shelf_drop(vec![
            (
                otto_core::ShelfPayload::File(String::from(r"C:\Temp\a.pdf")),
                false,
                None,
            ),
            (
                otto_core::ShelfPayload::Url(String::from("https://otto.dev")),
                false,
                None,
            ),
        ]);
        assert!(drop_update.changed);
        assert_eq!(notch.shelf_state().items.len(), 2);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Shelf);
        assert_eq!(notch.island_mode(t0), IslandMode::Expanded);

        assert!(notch.handle_pointer_leave(false));
        assert_eq!(notch.island_mode(t0), IslandMode::Collapsed);
    }

    #[test]
    fn shelf_click_right_click_wheel_and_clear_emit_expected_effects() {
        let mut notch = idle_notch();
        let mut items = Vec::new();
        for i in 0..8 {
            items.push((
                otto_core::ShelfPayload::File(format!(r"C:\Temp\f{i}.txt")),
                false,
                None,
            ));
        }
        let _ = notch.handle_shelf_drop(items);
        let _ = notch.handle_click(HitTarget::NotchTabShelf);
        let open = notch.handle_click(HitTarget::ShelfItem(0));
        assert_eq!(
            open.effect,
            Some(NotchEffect::Platform(NotchAction::ShelfOpen(
                otto_core::ShelfPayload::File(String::from(r"C:\Temp\f7.txt"))
            )))
        );
        let copy = notch.handle_click(HitTarget::ShelfCopyItem(0));
        assert_eq!(
            copy.effect,
            Some(NotchEffect::Platform(NotchAction::ShelfCopy(
                otto_core::ShelfPayload::File(String::from(r"C:\Temp\f7.txt"))
            )))
        );
        let reveal = notch.handle_shelf_right_click(0);
        assert_eq!(
            reveal.effect,
            Some(NotchEffect::Platform(NotchAction::ShelfReveal(
                otto_core::ShelfPayload::File(String::from(r"C:\Temp\f7.txt"))
            )))
        );
        let add = notch.handle_click(HitTarget::ShelfAdd);
        assert_eq!(add.effect, Some(NotchEffect::PickShelfFiles));

        let clear = notch.handle_click(HitTarget::ShelfClear);
        assert!(clear.changed);
        assert!(notch.shelf_state().items.is_empty());
    }

    #[test]
    fn shelf_modal_picker_and_drag_out_guard_pointer_leave_collapse() {
        let mut notch = idle_notch();
        let t0 = Instant::now();
        assert!(notch.begin_shelf_modal_picker());
        assert!(!notch.handle_pointer_leave(false));
        assert_eq!(notch.island_mode(t0), IslandMode::Expanded);

        let finished = notch.finish_shelf_modal_picker(vec![(
            otto_core::ShelfPayload::Text(String::from("picked")),
            false,
            None,
        )]);
        assert!(finished.changed);
        assert!(notch.set_shelf_drag_out(true));
        assert!(!notch.handle_pointer_leave(false));
        assert!(notch.set_shelf_drag_out(false));
    }

    #[test]
    fn clipboard_captures_toggles_pause_pages_recopies_with_expiry_and_clears() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        let _ = notch.advance(
            notch.epoch + Duration::from_millis(16),
            true,
            false,
            MotionPreference::Reduced,
        );
        assert!(notch.handle_click(HitTarget::NotchTabClipboard).changed);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Clipboard);

        assert!(
            notch.sync_clipboard_capture(otto_core::ClipboardPayload::Text(String::from(
                "first clip"
            )))
        );
        for index in 1..6 {
            assert!(
                notch.sync_clipboard_capture(otto_core::ClipboardPayload::Link(format!(
                    "https://example.com/{index}"
                )))
            );
        }
        assert_eq!(notch.clipboard_state().items.len(), 6);
        assert_eq!(notch.clipboard_state().page_count(), 2);

        let paged = notch.handle_click(HitTarget::ClipboardPager);
        assert!(paged.changed);
        assert_eq!(notch.clipboard_state().clamped_page(), 1);

        let wheel = notch.handle_wheel(HitTarget::NotchTabClipboard, 120);
        assert!(wheel.changed);
        assert_eq!(notch.clipboard_state().clamped_page(), 0);

        let clicked = notch.handle_click(HitTarget::ClipboardItem(0));
        assert!(clicked.changed);
        assert_eq!(
            clicked.effect,
            Some(NotchEffect::CopyClipboard(
                otto_core::ClipboardPayload::Link(String::from("https://example.com/5"))
            ))
        );
        let copied_id = notch.clipboard_state().copied_feedback_id;
        assert!(copied_id.is_some());

        let deadline = notch
            .next_deadline(Instant::now())
            .expect("copied badge deadline");
        let after = notch.sync_clock(deadline + Duration::from_millis(1));
        assert!(after.changed);
        assert_eq!(notch.clipboard_state().copied_feedback_id, None);

        assert!(notch.handle_click(HitTarget::ClipboardPauseToggle).changed);
        assert!(notch.clipboard_state().paused);
        assert!(
            !notch.sync_clipboard_capture(otto_core::ClipboardPayload::Text(String::from(
                "ignored while paused"
            )))
        );
        assert_eq!(notch.clipboard_state().items.len(), 6);

        assert!(
            notch
                .handle_click(HitTarget::ClipboardRemoveItem(0))
                .changed
        );
        assert_eq!(notch.clipboard_state().items.len(), 5);
        assert!(notch.handle_click(HitTarget::ClipboardClear).changed);
        assert!(notch.clipboard_state().items.is_empty());
    }

    #[test]
    fn dual_active_modules_split_into_main_notch_and_secondary_circle_and_swap_on_click() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        let now = settle(&mut notch, Instant::now());

        let layout = notch.layout_input(true);
        assert!((layout.compact_expansion - 1.0).abs() < f32::EPSILON);
        assert!((layout.secondary_expansion - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            layout.compact_primary,
            Some(otto_core::CompactSlotKind::Focus)
        );
        assert_eq!(
            layout.compact_secondary,
            Some(otto_core::CompactSlotKind::Media)
        );

        let swap = notch.handle_click(HitTarget::NotchSecondary);
        assert!(swap.changed);
        assert_eq!(swap.effect, None);
        assert!(notch.layout_input(true).compact_swap_blend.abs() < f32::EPSILON);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Media);

        let now = settle(&mut notch, now);
        let swapped = notch.layout_input(true);
        assert!((swapped.compact_swap_blend - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            swapped.compact_primary,
            Some(otto_core::CompactSlotKind::Media)
        );
        assert_eq!(
            swapped.compact_secondary,
            Some(otto_core::CompactSlotKind::Focus)
        );

        let swap_back = notch.handle_click(HitTarget::NotchSecondary);
        assert!(swap_back.changed);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Pomodoro);
        let _ = settle(&mut notch, now);
        let restored = notch.layout_input(true);
        assert_eq!(
            restored.compact_primary,
            Some(otto_core::CompactSlotKind::Focus)
        );
        assert_eq!(
            restored.compact_secondary,
            Some(otto_core::CompactSlotKind::Media)
        );
    }

    #[test]
    fn hovering_main_notch_opens_panel_to_active_primary_module_tab() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        let mut now = settle(&mut notch, Instant::now());

        notch.handle_pointer_move(true);
        for _ in 0..40 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, true, true, MotionPreference::Full);
        }
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Pomodoro);
        assert!(notch.layout_input(true).secondary_expansion < 0.25);

        assert!(notch.handle_pointer_leave(false));
        now = settle(&mut notch, now);

        assert!(notch.handle_click(HitTarget::NotchSecondary).changed);
        now = settle(&mut notch, now);
        notch.handle_pointer_move(true);
        for _ in 0..40 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, true, true, MotionPreference::Full);
        }
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Media);
    }

    #[test]
    fn dismissing_secondary_or_primary_updates_split_compact_slots_independently() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        let now = settle(&mut notch, Instant::now());

        assert!(notch.dismiss_secondary_compact(now).changed);
        let now = settle(&mut notch, now);
        let after_secondary = notch.layout_input(true);
        assert!((after_secondary.compact_expansion - 1.0).abs() < f32::EPSILON);
        assert!(after_secondary.secondary_expansion.abs() < f32::EPSILON);
        assert_eq!(
            after_secondary.compact_primary,
            Some(otto_core::CompactSlotKind::Focus)
        );
        assert_eq!(after_secondary.compact_secondary, None);

        assert!(notch.set_media(Some(playing("Next Song"))));
        let now = settle(&mut notch, now);
        assert!((notch.layout_input(true).secondary_expansion - 1.0).abs() < f32::EPSILON);

        assert!(notch.dismiss_compact(now).changed);
        let _ = settle(&mut notch, now);
        let promoted = notch.layout_input(true);
        assert!((promoted.compact_expansion - 1.0).abs() < f32::EPSILON);
        assert!(promoted.secondary_expansion.abs() < f32::EPSILON);
        assert_eq!(
            promoted.compact_primary,
            Some(otto_core::CompactSlotKind::Media)
        );
        assert_eq!(promoted.compact_secondary, None);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Media);
    }

    fn settle_inside(notch: &mut NotchController, mut now: Instant) -> Instant {
        for _ in 0..400 {
            now += Duration::from_millis(8);
            let _ = notch.advance(now, true, true, MotionPreference::Full);
        }
        now
    }

    #[test]
    fn clicking_collapsed_notch_expands_pins_syncs_primary_tab_and_header_click_collapses() {
        let mut notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Pomodoro);
        let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        let now = settle(&mut notch, Instant::now());
        assert!(notch.handle_click(HitTarget::NotchSecondary).changed);
        let mut now = settle(&mut notch, now);

        let expand = notch.handle_click(HitTarget::Notch);
        assert!(expand.changed);
        assert_eq!(expand.effect, None);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Media);
        now = settle_inside(&mut notch, now);
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);

        let collapse = notch.handle_click(HitTarget::Notch);
        assert!(collapse.changed);
        assert_eq!(collapse.effect, None);
        notch.handle_pointer_move(true);
        now = settle_inside(&mut notch, now);
        assert_ne!(notch.island_mode(now), IslandMode::Expanded);

        notch.handle_pointer_move(false);
        let _ = notch.advance(
            now + Duration::from_millis(8),
            false,
            false,
            MotionPreference::Full,
        );
        let reopen = notch.handle_click(HitTarget::Notch);
        assert!(reopen.changed);
        let now = settle(&mut notch, now + Duration::from_millis(8));
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);
    }

    #[test]
    fn collapse_from_gesture_blocks_hover_unless_held_open_by_shelf_or_modal() {
        let mut notch = idle_notch();
        let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
        let mut now = settle(&mut notch, Instant::now());
        assert_eq!(notch.island_mode(now), IslandMode::Expanded);

        let collapsed = notch.collapse_from_gesture();
        assert!(collapsed.changed);
        assert_eq!(collapsed.effect, None);
        notch.handle_pointer_move(true);
        now = settle_inside(&mut notch, now);
        assert_eq!(notch.island_mode(now), IslandMode::Collapsed);

        notch.handle_pointer_move(false);
        now += Duration::from_millis(8);
        let _ = notch.advance(now, false, false, MotionPreference::Full);
        assert!(notch.begin_shelf_modal_picker());
        let held = notch.collapse_from_gesture();
        assert!(!held.changed);
    }

    #[test]
    fn collapsing_expanded_notch_from_alert_dismisses_alert_first() {
        let mut notch = visible_running_notch();
        run_until_alert(&mut notch);
        assert!(notch.has_alert());

        let click = notch.handle_click(HitTarget::Notch);
        assert!(click.changed);
        assert_eq!(click.effect, None);
        assert!(!notch.has_alert());
        let now = settle(&mut notch, Instant::now());
        assert_ne!(notch.island_mode(now), IslandMode::Expanded);
    }

    #[test]
    fn disabling_modules_gates_tabs_hud_privacy_and_hotkeys_safely() {
        let mut notch = visible_running_notch();
        assert!(notch.focus_active());
        notch.set_hud_enabled(false);
        notch.trigger_hud(NotchHudKind::Volume {
            percent: 50,
            muted: false,
        });
        assert!(notch.hud_state().is_none());

        notch.set_focus_enabled(false);
        assert!(!notch.focus_active());
        assert_eq!(notch.active_tab(), NotchExpandedTab::Shelf);
        assert!(!notch.handle_hotkey(NotchHotkey::TogglePomodoro).changed);

        notch.set_privacy_enabled(false);
        assert!(!notch.layout_input(true).privacy.compact_dots);

        notch.set_shelf_enabled(false);
        assert_eq!(notch.active_tab(), NotchExpandedTab::Clipboard);
        notch.set_clipboard_enabled(false);
        notch.set_media_enabled(false);
        assert!(!notch.has_openable_panel());
        assert!(!notch.handle_hotkey(NotchHotkey::ToggleNotch).changed);
        assert!(!notch.handle_click(HitTarget::Notch).changed);
    }
}
