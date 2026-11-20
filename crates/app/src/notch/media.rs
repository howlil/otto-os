use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use otto_core::{
    HitTarget, LYRIC_DRIFT_TOLERANCE_MS, LyricsStatus, LyricsView, MediaCommand, MediaPeek,
    MediaPeekFrame, MediaScene, MotionMetrics, NotchAction, NotchExpandedTab, NowPlaying,
    POSITION_DRIFT_TOLERANCE_MS, PendingSeek, SeekReadback, reconcile_seek, seek_target_ms,
};

use super::GLOW_SCENE_VISIBLE;
use super::MediaPeekState;
use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;
use super::PanelTrigger;
use super::unix_now_ms;

impl NotchController {
    pub(super) fn shows_live_media_progress(&self) -> bool {
        let panel_shows_media =
            self.panel_expansion > 0.05 && self.notch_tab == NotchExpandedTab::Media;
        let secondary_shows_media = self.secondary_expansion > 0.05;
        self.presented
            && (panel_shows_media || secondary_shows_media)
            && self.media.as_ref().is_some_and(|media| media.playing)
    }

    pub(super) fn shows_live_lyrics(&self) -> bool {
        self.lyrics_visible_now() && self.media.as_ref().is_some_and(|media| media.playing)
    }

    pub(super) fn lyrics_visible_now(&self) -> bool {
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

    pub(super) fn media_repaint_in(&self, now_unix_ms: i64) -> Option<Duration> {
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

    pub(super) fn show_media_peek(&mut self, kind: MediaPeek, title: String, now: Instant) {
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

    pub(super) fn note_media_transition(
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

    pub(super) fn compact_lyric_line(&self, media: &NowPlaying) -> Option<String> {
        if !self.lyrics_in_compact {
            return None;
        }
        self.lyrics
            .as_ref()
            .and_then(|sheet| sheet.active_line(media.position_ms))
            .map(str::to_owned)
    }

    pub(crate) fn media(&self) -> Option<&NowPlaying> {
        self.media.as_ref()
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

    pub(super) fn set_media_at(
        &mut self,
        mut latest_media: Option<NowPlaying>,
        now_unix_ms: i64,
    ) -> bool {
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
                NotchExpandedTab::Tasks,
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
                self.panel_velocity = 0.0;
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

    pub(super) fn handle_media_click(&mut self, target: HitTarget) -> NotchUpdate {
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

    pub(super) fn lyrics_is_live(&self) -> bool {
        self.presented
            && self.panel_expansion > GLOW_SCENE_VISIBLE
            && self.notch_tab == NotchExpandedTab::Media
            && self.motion.allows_motion()
            && self.lyrics_progress > 0.5
            && self.media.as_ref().is_some_and(|media| media.playing)
            && self.lyrics.as_ref().is_some_and(|sheet| !sheet.is_empty())
    }
}
