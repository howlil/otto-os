use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use otto_core::{NowPlaying, Rect, ShelfPayload, scale};
use otto_platform::{
    CaptureEvent, ClipboardListenerGuard, ClipboardRead, LyricsRequest, LyricsService,
    QuickCaptureInput, ShelfDropQueue, ShelfDropRegion, ShelfDropTargetGuard,
    clipboard_sequence_number, read_clipboard_payload, register_shelf_drop_target,
};

use crate::bar::DeferredModalAction;
use crate::win_msg::WM_APP_BAR_CAPTURE;

pub(crate) struct PendingShelfDrag {
    pub(crate) slot: u8,
    pub(crate) source_hwnd: isize,
    pub(crate) start_x: i32,
    pub(crate) start_y: i32,
    pub(crate) payload: ShelfPayload,
}

pub(crate) struct NotchIo {
    pub(crate) shelf_queue: ShelfDropQueue,
    pub(crate) shelf_local_drag: Arc<AtomicBool>,
    pub(crate) pending_shelf_drag: Option<PendingShelfDrag>,
    notch_shelf_region: Arc<ShelfDropRegion>,
    notch_drop_target: Option<ShelfDropTargetGuard>,
    panel_drop_target: Option<ShelfDropTargetGuard>,
    clipboard_listener: Option<ClipboardListenerGuard>,
    last_clipboard_seq: u32,
    suppressed_clipboard_seq: Option<u32>,
    pub(crate) quick_capture: Option<QuickCaptureInput>,
    pub(crate) seek_drag_hwnd: Option<isize>,
    lyrics: Option<LyricsService>,
    lyrics_track: Option<(String, String)>,
    lyrics_generation: u64,
    pub(crate) online_lyrics_enabled: bool,
}

impl NotchIo {
    pub(crate) fn new(
        notch: Rect,
        bar_height: i32,
        dpi: u32,
        online_lyrics_enabled: bool,
        quick_capture: Option<QuickCaptureInput>,
    ) -> Self {
        let pad = scale(12, dpi);
        Self {
            shelf_queue: ShelfDropQueue::new(),
            shelf_local_drag: Arc::new(AtomicBool::new(false)),
            pending_shelf_drag: None,
            notch_shelf_region: ShelfDropRegion::restricted(
                notch.x - pad,
                0,
                notch.right() + pad,
                bar_height + pad,
            ),
            notch_drop_target: None,
            panel_drop_target: None,
            clipboard_listener: None,
            last_clipboard_seq: clipboard_sequence_number(),
            suppressed_clipboard_seq: None,
            quick_capture,
            seek_drag_hwnd: None,
            lyrics: None,
            lyrics_track: None,
            lyrics_generation: 0,
            online_lyrics_enabled,
        }
    }

    pub(crate) fn register_shelf_targets(&mut self, hwnd: isize, panel_hwnd: isize) {
        if self.notch_drop_target.is_some() && self.panel_drop_target.is_some() {
            return;
        }
        self.notch_drop_target = register_shelf_drop_target(
            hwnd,
            self.shelf_queue.clone(),
            Arc::clone(&self.notch_shelf_region),
            Arc::clone(&self.shelf_local_drag),
        );
        self.panel_drop_target = register_shelf_drop_target(
            panel_hwnd,
            self.shelf_queue.clone(),
            ShelfDropRegion::full_window(),
            Arc::clone(&self.shelf_local_drag),
        );
    }

    pub(crate) fn release_shelf_targets(&mut self) {
        self.notch_drop_target = None;
        self.panel_drop_target = None;
    }

    pub(crate) fn release_shelf(&mut self) {
        self.release_shelf_targets();
        self.pending_shelf_drag = None;
        let _ = self.shelf_queue.drain();
    }

    pub(crate) fn sync_shelf_region(&self, notch: Rect, bar_height: i32, dpi: u32) {
        let pad = scale(12, dpi);
        self.notch_shelf_region
            .set_rect(notch.x - pad, 0, notch.right() + pad, bar_height + pad);
    }

    pub(crate) fn take_shelf_drag_if_moved(
        &mut self,
        x: i32,
        y: i32,
        drag_slop: (i32, i32),
    ) -> Option<DeferredModalAction> {
        let pending = self.pending_shelf_drag.as_ref()?;
        let dx = (x - pending.start_x).abs();
        let dy = (y - pending.start_y).abs();
        if dx < drag_slop.0 && dy < drag_slop.1 {
            return None;
        }
        let pending = self.pending_shelf_drag.take()?;
        Some(DeferredModalAction::StartShelfDrag {
            capture_hwnd: pending.source_hwnd,
            payload: pending.payload,
        })
    }

    pub(crate) fn listen_clipboard(&mut self, hwnd: isize) {
        if self.clipboard_listener.is_none() {
            self.clipboard_listener = ClipboardListenerGuard::register(hwnd);
            self.last_clipboard_seq = clipboard_sequence_number();
            self.suppressed_clipboard_seq = None;
        }
    }

    pub(crate) fn stop_clipboard(&mut self) {
        self.clipboard_listener = None;
    }

    pub(crate) fn suppress_own_copy(&mut self, seq: u32) {
        self.suppressed_clipboard_seq = Some(seq);
        self.last_clipboard_seq = seq;
    }

    pub(crate) fn poll_clipboard(&mut self, paused: bool) -> Option<otto_core::ClipboardPayload> {
        let seq = clipboard_sequence_number();
        if paused {
            self.last_clipboard_seq = seq;
            self.suppressed_clipboard_seq = None;
            return None;
        }
        if self.suppressed_clipboard_seq == Some(seq) {
            self.suppressed_clipboard_seq = None;
            self.last_clipboard_seq = seq;
            return None;
        }
        if seq == self.last_clipboard_seq {
            return None;
        }
        match read_clipboard_payload() {
            ClipboardRead::Captured { seq, payload } => {
                self.last_clipboard_seq = seq;
                self.suppressed_clipboard_seq = None;
                Some(payload)
            }
            ClipboardRead::SensitiveOrEmpty { seq } => {
                self.last_clipboard_seq = seq;
                self.suppressed_clipboard_seq = None;
                None
            }
            ClipboardRead::Busy => None,
        }
    }

    pub(crate) fn enable_quick_capture(&mut self, hwnd: isize) {
        if self.quick_capture.is_none() {
            self.quick_capture = QuickCaptureInput::create(hwnd, WM_APP_BAR_CAPTURE);
        }
    }

    pub(crate) fn disable_quick_capture(&mut self) {
        self.quick_capture = None;
    }

    pub(crate) fn show_inline_quick_capture(
        &self,
        rect: Rect,
        initial_text: &str,
        allow_keep_open: bool,
        dpi: u32,
    ) {
        if let Some(capture) = &self.quick_capture {
            capture.show_inline(rect, initial_text, allow_keep_open, dpi);
        }
    }

    pub(crate) fn reposition_quick_capture(&self, rect: Rect, dpi: u32) {
        if let Some(capture) = &self.quick_capture {
            capture.reposition(rect, dpi);
        }
    }

    pub(crate) fn raise_quick_capture(&self) {
        if let Some(capture) = &self.quick_capture {
            capture.raise();
        }
    }

    pub(crate) fn hide_quick_capture(&self, restore_focus: bool) {
        if let Some(capture) = &self.quick_capture {
            capture.hide(restore_focus);
        }
    }

    pub(crate) fn is_quick_capture_visible(&self) -> bool {
        self.quick_capture
            .as_ref()
            .is_some_and(QuickCaptureInput::is_visible)
    }

    pub(crate) fn take_capture_event(&self) -> Option<CaptureEvent> {
        self.quick_capture
            .as_ref()
            .and_then(QuickCaptureInput::take_event)
    }

    pub(crate) fn forget_lyrics_track(&mut self) {
        self.lyrics_track = None;
    }

    pub(crate) fn invalidate_lyrics(&mut self) {
        self.lyrics_track = None;
        self.lyrics_generation = self.lyrics_generation.wrapping_add(1);
    }

    pub(crate) fn stop_lyrics(&mut self) {
        self.invalidate_lyrics();
        if let Some(mut lyrics) = self.lyrics.take() {
            lyrics.shutdown();
        }
    }

    pub(crate) fn request_lyrics(
        &mut self,
        media_enabled: bool,
        media: Option<&NowPlaying>,
    ) -> bool {
        let Some(media) = media.filter(|_| media_enabled) else {
            self.lyrics_track = None;
            return false;
        };
        let track = (media.title.clone(), media.artist.clone());
        if track.0.trim().is_empty() || self.lyrics_track.as_ref() == Some(&track) {
            return false;
        }
        let generation = self.lyrics_generation.wrapping_add(1);
        let lyrics = self.lyrics.get_or_insert_with(LyricsService::start);
        let accepted = lyrics.request(LyricsRequest {
            generation,
            title: track.0.clone(),
            artist: track.1.clone(),
            duration_ms: media.duration_ms,
            allow_online: self.online_lyrics_enabled,
        });
        if accepted {
            self.lyrics_generation = generation;
            self.lyrics_track = Some(track);
        }
        accepted
    }

    pub(crate) fn poll_lyrics(&self) -> Option<otto_platform::LyricsResponse> {
        self.lyrics.as_ref().and_then(LyricsService::poll)
    }

    pub(crate) fn is_current_lyrics_generation(&self, generation: u64) -> bool {
        generation == self.lyrics_generation
    }

    pub(crate) fn shutdown(&mut self) {
        self.stop_lyrics();
    }
}
