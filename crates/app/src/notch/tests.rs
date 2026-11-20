use super::{ALERT_HOLD_MS, NotchController, NotchEffect, NotchHotkey, NotchUpdate, unix_now_ms};
use otto_core::{
    ClockTime, FocusSubMode, HitTarget, IslandMode, MediaCommand, MotionPreference, NotchAction,
    NotchExpandedTab, NotchHudKind, NotificationStage, NotificationSummary, NowPlaying,
    PomodoroStatus, PomodoroSurface,
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
    while notch.pomodoro().status != PomodoroStatus::Alert && now < start + Duration::from_mins(30)
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
    let mut playing_notch = NotchController::new(Some(playing("Song")), NotchExpandedTab::Media);
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
        location_active: false,
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
        location_active: false,
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
        location_active: false,
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

#[test]
fn collapse_from_external_focus_change_collapses_pinned_and_hovered_unless_shelf_or_alert_active() {
    let mut pinned = idle_notch();
    let _ = pinned.handle_hotkey(NotchHotkey::ToggleNotch);
    let now = settle(&mut pinned, Instant::now());
    assert_eq!(pinned.island_mode(now), IslandMode::Expanded);
    assert!(pinned.collapse_from_external_focus_change());
    assert!(!pinned.collapse_from_external_focus_change());
    let now = settle(&mut pinned, now);
    assert_eq!(pinned.island_mode(now), IslandMode::Collapsed);

    let mut hovered = idle_notch();
    hovered.handle_pointer_move(true);
    let mut t = Instant::now();
    for _ in 0..40 {
        t += Duration::from_millis(8);
        let _ = hovered.advance(t, true, true, MotionPreference::Full);
    }
    assert_eq!(hovered.island_mode(t), IslandMode::Expanded);
    assert!(hovered.collapse_from_external_focus_change());
    t = settle(&mut hovered, t);
    assert_eq!(hovered.island_mode(t), IslandMode::Collapsed);

    let mut shelf_held = idle_notch();
    let _ = shelf_held.handle_hotkey(NotchHotkey::ToggleNotch);
    assert!(shelf_held.begin_shelf_modal_picker());
    assert!(!shelf_held.collapse_from_external_focus_change());

    let mut alerted = visible_running_notch();
    let _ = alerted.handle_hotkey(NotchHotkey::ToggleNotch);
    run_until_alert(&mut alerted);
    assert!(alerted.has_alert());
    assert!(!alerted.collapse_from_external_focus_change());
}

#[test]
fn notch_panel_spring_preserves_momentum_on_mid_flight_reversal() {
    let mut notch = idle_notch();
    let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
    let mut now = Instant::now();
    for _ in 0..8 {
        now += Duration::from_millis(12);
        let _ = notch.advance(now, false, false, MotionPreference::Full);
    }
    let mid_expansion = notch.panel_expansion;
    let mid_velocity = notch.panel_velocity;
    assert!(mid_expansion > 0.0);
    assert!(mid_velocity > 0.0);

    let _ = notch.handle_hotkey(NotchHotkey::ToggleNotch);
    now += Duration::from_millis(4);
    let _ = notch.advance(now, false, false, MotionPreference::Full);
    assert!(notch.panel_expansion > mid_expansion);
    assert!(notch.panel_velocity < mid_velocity);

    now = settle(&mut notch, now);
    assert!(notch.panel_expansion.abs() < f32::EPSILON);
    assert!(notch.panel_velocity.abs() < f32::EPSILON);
    let _ = now;
}

#[test]
fn set_notch_pressed_drives_tactile_compression_and_releases() {
    let mut notch = idle_notch();
    assert!(notch.set_notch_pressed(true));
    assert!(!notch.set_notch_pressed(true));

    let mut now = Instant::now();
    for _ in 0..12 {
        now += Duration::from_millis(16);
        let _ = notch.advance(now, true, false, MotionPreference::Full);
    }
    assert!(notch.layout_input(true).press_progress > 0.5);

    assert!(notch.handle_pointer_leave(false));
    assert!(!notch.press_active);
    now = settle(&mut notch, now);
    assert!(notch.layout_input(true).press_progress.abs() < f32::EPSILON);

    assert!(notch.set_notch_pressed(true));
    for _ in 0..8 {
        now += Duration::from_millis(16);
        let _ = notch.advance(now, true, false, MotionPreference::Full);
    }
    assert!(notch.layout_input(true).press_progress > 0.3);

    let _ = notch.handle_click(HitTarget::Notch);
    assert!(!notch.press_active);
    let _ = settle(&mut notch, now);
    assert!(notch.layout_input(true).press_progress.abs() < f32::EPSILON);
}

fn toast(id: u32, app: &str, title: &str, body: &str) -> NotificationSummary {
    NotificationSummary {
        id,
        app_id: format!("{app}.App"),
        app_name: String::from(app),
        title: String::from(title),
        body: String::from(body),
        timestamp_label: String::from("now"),
    }
}

fn notch_with_baseline() -> (NotchController, Instant) {
    let mut notch = idle_notch();
    let start = notch.epoch;
    assert!(!notch.ingest_notifications(&[], start));
    (notch, start)
}

#[test]
fn a_message_toast_opens_the_banner_and_a_call_toast_rings() {
    let (mut notch, start) = notch_with_baseline();
    let now = start + Duration::from_millis(100);
    assert!(notch.ingest_notifications(&[toast(1, "Mail", "Ayu", "Lunch?")], now));
    assert_eq!(
        notch.island_mode(now),
        otto_core::IslandMode::Notification(NotificationStage::Banner)
    );
    let banner = notch.layout_input(true).notification.expect("banner");
    assert!(!banner.ringing);
    let both = [
        toast(1, "Mail", "Ayu", "Lunch?"),
        toast(2, "Zoom", "Budi", "is calling you"),
    ];
    assert!(notch.ingest_notifications(&both, now));
    assert_eq!(
        notch.island_mode(now),
        otto_core::IslandMode::Notification(NotificationStage::Ringing)
    );
}

#[test]
fn clicking_the_card_opens_the_source_and_clicking_dismiss_only_removes_it() {
    let (mut notch, start) = notch_with_baseline();
    let now = start + Duration::from_millis(100);
    notch.ingest_notifications(&[toast(7, "Mail", "Ayu", "Lunch?")], now);
    let update = notch.handle_click(HitTarget::NotificationCard);
    assert!(matches!(
        update.effect,
        Some(NotchEffect::Platform(NotchAction::NotificationOpen(7)))
    ));
    assert_eq!(notch.island_mode(now), otto_core::IslandMode::Collapsed);
    notch.ingest_notifications(
        &[
            toast(7, "Mail", "Ayu", "Lunch?"),
            toast(8, "Mail", "Budi", "Hi"),
        ],
        now + Duration::from_secs(10),
    );
    let update = notch.handle_click(HitTarget::NotificationClose);
    assert!(matches!(
        update.effect,
        Some(NotchEffect::Platform(NotchAction::NotificationDismiss(8)))
    ));
}

#[test]
fn a_focus_session_holds_back_message_banners_but_not_ringing_calls() {
    let (mut notch, start) = notch_with_baseline();
    let _ = notch.handle_hotkey(NotchHotkey::TogglePomodoro);
    let now = start + Duration::from_millis(100);
    notch.ingest_notifications(&[toast(1, "Mail", "Ayu", "Lunch?")], now);
    assert!(notch.layout_input(true).notification.is_none());
    let both = [
        toast(1, "Mail", "Ayu", "Lunch?"),
        toast(2, "Zoom", "Budi", "is calling you"),
    ];
    notch.ingest_notifications(&both, now);
    assert!(
        notch
            .layout_input(true)
            .notification
            .is_some_and(|card| card.ringing)
    );
}

#[test]
fn hovering_the_banner_does_not_swap_it_for_the_full_panel() {
    let (mut notch, start) = notch_with_baseline();
    let now = start + Duration::from_millis(100);
    notch.ingest_notifications(&[toast(1, "Mail", "Ayu", "Lunch?")], now);
    notch.hover_since = Some(now);
    let later = now + Duration::from_millis(400);
    let _ = notch.advance(later, true, true, MotionPreference::Full);
    assert_eq!(
        notch.island_mode(later),
        otto_core::IslandMode::Notification(NotificationStage::Banner)
    );
}

fn on_a_call(app: &str, video: bool, muted: bool) -> otto_core::PrivacyState {
    otto_core::PrivacyState {
        mic: otto_core::MicState::Active { muted },
        camera_active: video,
        location_active: false,
        app_name: Some(String::from(app)),
    }
}

#[test]
fn a_call_in_the_browser_takes_the_notch_after_the_debounce() {
    let mut notch = idle_notch();
    let start = notch.epoch;
    assert!(notch.sync_privacy_state(on_a_call("chrome", false, false)));
    assert!(!matches!(
        notch.island_mode(start + Duration::from_millis(500)),
        IslandMode::Compact(otto_core::CompactContent::Call)
    ));
    let later = start + Duration::from_secs(5);
    assert_eq!(
        notch.island_mode(later),
        IslandMode::Compact(otto_core::CompactContent::Call)
    );
    let scene = notch.call_scene(later).expect("scene");
    assert!(!scene.video && !scene.muted);
    notch.last_anim_instant = later;
    assert_eq!(
        notch.layout_input(true).compact_primary,
        Some(otto_core::CompactSlotKind::Call)
    );
}

#[test]
fn muting_marks_the_call_scene_and_ending_the_call_clears_it() {
    let mut notch = idle_notch();
    let start = notch.epoch;
    notch.sync_privacy_state(on_a_call("Zoom", true, false));
    let later = start + Duration::from_secs(5);
    assert!(notch.call_scene(later).is_some_and(|scene| scene.video));
    notch.sync_privacy_state(on_a_call("Zoom", true, true));
    assert!(notch.call_scene(later).is_some_and(|scene| scene.muted));
    notch.sync_privacy_state(otto_core::PrivacyState::default());
    assert!(notch.call_scene(later).is_none());
}

#[test]
fn a_non_call_mic_user_never_takes_the_notch() {
    let mut notch = idle_notch();
    let start = notch.epoch;
    notch.sync_privacy_state(on_a_call("AudioRecorder", false, false));
    assert!(notch.call_scene(start + Duration::from_secs(30)).is_none());
}

fn notch_with_tasks(texts: &[&str]) -> NotchController {
    let mut notch = NotchController::new(None, NotchExpandedTab::Pomodoro);
    notch.set_tasks_enabled(true);
    for text in texts.iter().rev() {
        assert!(notch.add_task(text));
    }
    notch
}

#[test]
fn the_tasks_tab_is_off_until_enabled_and_falls_back_when_disabled() {
    let mut notch = NotchController::new(None, NotchExpandedTab::Pomodoro);
    assert!(!notch.is_tab_enabled(NotchExpandedTab::Tasks));
    assert!(!notch.layout_input(true).tasks_enabled);
    notch.set_tasks_enabled(true);
    assert!(notch.is_tab_enabled(NotchExpandedTab::Tasks));
    assert!(notch.layout_input(true).tasks_enabled);
    let update = notch.handle_click(HitTarget::NotchTabTasks);
    assert!(update.changed);
    assert_eq!(notch.active_tab(), NotchExpandedTab::Tasks);
    notch.set_tasks_enabled(false);
    assert_ne!(notch.active_tab(), NotchExpandedTab::Tasks);
    assert!(!notch.layout_input(true).tasks_enabled);
}

#[test]
fn task_clicks_mutate_the_list_and_request_a_save() {
    let mut notch = notch_with_tasks(&["first", "second"]);
    let layout = notch.layout_input(true).tasks;
    assert_eq!(layout.visible_count, 2);
    assert!(layout.has_items && !layout.has_done);

    let check = notch.handle_click(HitTarget::TaskCheck(0));
    assert!(check.changed);
    assert_eq!(check.effect, Some(NotchEffect::SaveTasks));
    assert!(notch.tasks_state().items[0].done);
    assert!(notch.layout_input(true).tasks.has_done);

    let focus = notch.handle_click(HitTarget::TaskItem(1));
    assert!(focus.changed);
    assert_eq!(focus.effect, Some(NotchEffect::SaveTasks));
    assert_eq!(
        notch
            .tasks_state()
            .active_task()
            .map(|task| task.text.as_str()),
        Some("second")
    );

    let done_row_focus = notch.handle_click(HitTarget::TaskItem(0));
    assert_eq!(done_row_focus, NotchUpdate::default());

    let cleared = notch.handle_click(HitTarget::TasksClearDone);
    assert_eq!(cleared.effect, Some(NotchEffect::SaveTasks));
    assert_eq!(notch.tasks_state().items.len(), 1);
    assert_eq!(
        notch.handle_click(HitTarget::TasksClearDone),
        NotchUpdate::default()
    );

    let removed = notch.handle_click(HitTarget::TaskRemove(0));
    assert_eq!(removed.effect, Some(NotchEffect::SaveTasks));
    assert!(notch.tasks_state().items.is_empty());
    assert_eq!(
        notch.handle_click(HitTarget::TaskRemove(0)),
        NotchUpdate::default()
    );
}

#[test]
fn add_opens_capture_and_blank_text_is_rejected() {
    let mut notch = notch_with_tasks(&[]);
    let update = notch.handle_click(HitTarget::TasksAdd);
    assert!(!update.changed);
    assert_eq!(update.effect, Some(NotchEffect::OpenQuickCapture));
    assert!(!notch.add_task("   	 "));
    assert!(notch.add_task("  write release notes  "));
    assert_eq!(notch.tasks_state().items[0].text, "write release notes");
}

#[test]
fn the_pager_steps_through_pages_without_saving() {
    let texts: Vec<String> = (0..6).map(|index| format!("task {index}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut notch = notch_with_tasks(&refs);
    assert!(notch.layout_input(true).tasks.has_pager);
    let update = notch.handle_click(HitTarget::TasksPager);
    assert!(update.changed);
    assert_eq!(update.effect, None);
    assert_eq!(notch.layout_input(true).tasks.visible_count, 2);
}
