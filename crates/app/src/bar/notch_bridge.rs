use std::time::Instant;

use crate::host::RuntimeContext;
use crate::notch::{NotchEffect, TaskSubmitResult};
use otto_core::{HitTarget, NotchAction, NotchHudKind, NotchLayoutInput, NotchPort};
use otto_platform::release_capture_if_owned;
use otto_platform::{
    CaptureEvent, cursor_position, save_tasks, signal_command_failure,
    unregister_bluetooth_hud_target,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXDRAG, SM_CYDRAG};

use super::Bar;
use super::modal::DeferredModalAction;
use super::should_refresh_media;
use super::should_refresh_system_status;

impl Bar {
    pub(super) fn ensure_shelf_drop_targets(&mut self) {
        self.io.register_shelf_targets(self.hwnd, self.panel_hwnd);
    }

    pub(super) fn request_lyrics_for_current_track(&mut self) {
        let media_enabled = self.notch.media_enabled();
        if self.io.request_lyrics(media_enabled, self.notch.media()) {
            self.notch.begin_lyrics_request();
        }
    }

    pub(super) fn notch_input(&self) -> NotchLayoutInput {
        NotchLayoutInput {
            top_bar_enabled: self.top_bar_enabled,
            notch_enabled: self.notch_enabled,
            hidden_items: self.menu_prefs.hidden_items,
            ..self.notch.layout_input(self.control_center_enabled)
        }
    }

    pub(super) fn sync_bar_shelf_region(&self) {
        self.io
            .sync_shelf_region(self.layout.notch, self.layout.bar.height, self.screen.dpi);
    }

    pub(super) fn revoke_drop_targets(&mut self) {
        unregister_bluetooth_hud_target();
        self.io.release_shelf_targets();
        self.io.stop_clipboard();
        self.power_listener = None;
    }

    pub(super) fn on_bluetooth_hud(&mut self, runtime: &RuntimeContext, kind: NotchHudKind) {
        if !self.notch_enabled || !self.notch.hud_enabled() {
            return;
        }
        self.notch.trigger_hud(kind);
        self.ensure_anim_timer();
        self.present(runtime);
        self.schedule_deadline();
    }

    pub(super) fn open_inline_task_capture(&mut self, runtime: &RuntimeContext) {
        if !self.notch_enabled || !self.notch.tasks_enabled() || !self.shown() {
            return;
        }
        let update = self.notch.open_quick_capture_add();
        self.apply_notch_update(runtime, update);
        self.schedule_deadline();
    }

    pub(super) fn show_inline_task_editor(&self, initial_text: &str, allow_keep_open: bool) {
        if self.panel_visible
            && self.layout.panel_expansion > 0.55
            && !self.layout.tasks.editor_input.is_empty()
        {
            self.io.show_inline_quick_capture(
                self.layout.tasks.editor_input,
                initial_text,
                allow_keep_open,
                self.screen.dpi,
            );
        }
    }

    pub(super) fn sync_quick_capture_overlay(&self) {
        if self.notch.is_task_editor_open()
            && self.panel_visible
            && self.layout.panel_expansion > 0.55
            && !self.layout.tasks.editor_input.is_empty()
        {
            self.io
                .reposition_quick_capture(self.layout.tasks.editor_input, self.screen.dpi);
        } else if self.io.is_quick_capture_visible() {
            self.io.hide_quick_capture(false);
        }
    }

    pub(super) fn on_quick_capture_event(&mut self, runtime: &RuntimeContext) {
        let Some(event) = self.io.take_capture_event() else {
            return;
        };
        match event {
            CaptureEvent::Submitted(submission) => {
                match self
                    .notch
                    .submit_task_editor(&submission.text, submission.keep_open)
                {
                    TaskSubmitResult::Saved => {
                        if !save_tasks(self.notch.tasks_state()) {
                            signal_command_failure();
                        }
                    }
                    TaskSubmitResult::Unchanged => {}
                    TaskSubmitResult::Rejected => {
                        signal_command_failure();
                    }
                }
                self.recompute_layout();
                self.sync_panel_visibility();
                self.present(runtime);
                self.sync_quick_capture_overlay();
                self.ensure_anim_timer();
                self.schedule_deadline();
            }
            CaptureEvent::Cancelled => {
                if self.notch.cancel_task_editor() {
                    self.recompute_layout();
                    self.sync_panel_visibility();
                    self.present(runtime);
                    self.sync_quick_capture_overlay();
                    self.ensure_anim_timer();
                    self.schedule_deadline();
                }
            }
        }
    }

    pub(super) fn on_clipboard_update(&mut self, runtime: &RuntimeContext) {
        if !self.notch_enabled || !self.notch.clipboard_enabled() {
            return;
        }
        let paused = self.notch.clipboard_state().paused;
        if let Some(payload) = self.io.poll_clipboard(paused)
            && self.notch.sync_clipboard_capture(payload)
        {
            self.recompute_layout();
            if self.panel_visible {
                self.present(runtime);
            }
        }
    }

    pub(super) fn maybe_start_shelf_drag(
        &mut self,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) -> Option<DeferredModalAction> {
        self.io.pending_shelf_drag.as_ref()?;
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        let slop = (
            unsafe { GetSystemMetrics(SM_CXDRAG) }.max(4),
            unsafe { GetSystemMetrics(SM_CYDRAG) }.max(4),
        );
        self.io.take_shelf_drag_if_moved(x, y, slop)
    }

    pub(super) fn seek_ratio_at(&self, x: i32) -> Option<f32> {
        let seekbar = self.layout.media.seekbar;
        (seekbar.width > 0).then(|| ((x - seekbar.x) as f32 / seekbar.width as f32).clamp(0.0, 1.0))
    }

    pub(super) fn drag_seek(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        let (x, _) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        if let Some(ratio) = self.seek_ratio_at(x)
            && self.notch.update_seek_drag(ratio)
        {
            self.present(runtime);
        }
    }

    pub(super) fn release_seek(&mut self, runtime: &RuntimeContext, commit: bool) {
        let Some(hwnd) = self.io.seek_drag_hwnd.take() else {
            return;
        };
        let update = if commit {
            self.notch.finish_seek_drag(crate::notch::unix_now_ms())
        } else {
            self.notch.cancel_seek_drag();
            crate::notch::NotchUpdate {
                changed: true,
                effect: None,
            }
        };
        let _ = release_capture_if_owned(hwnd);
        self.apply_notch_effect(update.effect);
        self.present(runtime);
    }

    pub(super) fn refresh_media(&mut self, force_media: bool) -> bool {
        let mut dirty = false;
        if (self.top_bar_enabled || self.notch_enabled)
            && should_refresh_system_status(
                force_media,
                self.cadence.is_idle(),
                self.maintain_ticks,
            )
            && self.refresh_system_status()
        {
            dirty = true;
        }
        if !self.notch_enabled {
            return dirty;
        }
        if self.notch.privacy_enabled()
            && self
                .notch
                .sync_privacy_state(self.notch_port.read_privacy_state())
        {
            self.recompute_layout();
            dirty = true;
        }
        if self.notch.media_enabled() && should_refresh_media(force_media, self.maintain_ticks) {
            let latest_media = self.notch_port.read_now_playing();
            if self.notch.set_media(latest_media) {
                self.recompute_layout();
                dirty = true;
            }
            self.request_lyrics_for_current_track();
        }
        dirty
    }

    pub(super) fn apply_notch_effect(&mut self, effect: Option<NotchEffect>) -> bool {
        let Some(effect) = effect else {
            return false;
        };
        match effect {
            NotchEffect::Platform(action) => {
                self.notch_port.execute(action.clone());
                if matches!(action, NotchAction::ToggleMicMute) {
                    let _ = self
                        .notch
                        .sync_privacy_state(self.notch_port.read_privacy_state());
                    true
                } else {
                    false
                }
            }
            NotchEffect::PickShelfFiles | NotchEffect::OpenTaskEditor { .. } => false,
            NotchEffect::SaveTasks => {
                if !save_tasks(self.notch.tasks_state()) {
                    signal_command_failure();
                }
                false
            }
            NotchEffect::CloseTaskEditor { restore_focus } => {
                self.io.hide_quick_capture(restore_focus);
                false
            }
            NotchEffect::CopyClipboard(payload) => {
                if let Some(seq) = self.notch_port.copy_clipboard(&payload) {
                    self.io.suppress_own_copy(seq);
                    false
                } else {
                    self.notch.clear_clipboard_copied_feedback()
                }
            }
        }
    }

    pub(super) fn apply_notch_update(
        &mut self,
        runtime: &RuntimeContext,
        update: crate::notch::NotchUpdate,
    ) {
        let open_editor = match &update.effect {
            Some(NotchEffect::OpenTaskEditor {
                initial_text,
                allow_keep_open,
            }) => Some((initial_text.clone(), *allow_keep_open)),
            _ => None,
        };
        let effect_changed = self.apply_notch_effect(update.effect);
        if update.changed || effect_changed {
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
            self.ensure_anim_timer();
        }
        if let Some((initial_text, allow_keep_open)) = open_editor {
            self.show_inline_task_editor(&initial_text, allow_keep_open);
        } else {
            self.sync_quick_capture_overlay();
        }
    }

    pub(super) fn step_hud_animation(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let (cx, cy) = cursor_position();
        let update = self.notch.advance(
            now,
            self.layout.notch.contains(cx, cy),
            self.panel_visible,
            self.motion,
        );
        let effect_changed = self.apply_notch_effect(update.effect);
        if update.changed || effect_changed {
            self.settle_ticks = 10;
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
        } else if self.settle_ticks > 0 {
            self.settle_ticks -= 1;
        } else {
            self.sync_panel_visibility();
            self.stop_anim_timer();
        }
        self.sync_quick_capture_overlay();
        self.schedule_deadline();
    }

    pub(super) fn seek_from_keyboard(&mut self, runtime: &RuntimeContext, delta_ms: i64) {
        let update = self
            .notch
            .handle_seek_step(delta_ms, crate::notch::unix_now_ms());
        let effect_changed = self.apply_notch_effect(update.effect);
        if update.changed || effect_changed {
            self.present(runtime);
        }
    }

    pub(super) fn run_notch_target(
        &mut self,
        runtime: &RuntimeContext,
        target: HitTarget,
    ) -> Option<DeferredModalAction> {
        if target == HitTarget::PrivacyIndicator {
            let _ = self
                .notch
                .sync_privacy_state(self.notch_port.read_privacy_state());
        }
        let update = self.notch.handle_click(target);
        if matches!(update.effect, Some(NotchEffect::PickShelfFiles)) {
            return Some(DeferredModalAction::PickShelfFiles);
        }
        self.apply_notch_update(runtime, update);
        None
    }
}
