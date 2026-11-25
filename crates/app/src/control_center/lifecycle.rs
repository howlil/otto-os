use std::time::Instant;

use crate::host::RuntimeContext;
use crate::win_msg::CC_POLL_TIMER;
use otto_core::{
    BluetoothState, ControlCenterCommand, ControlCenterPage, MotionMetrics, MotionPreference,
    WifiState, animate_value,
};
use otto_platform::{
    MenuNavEvent, cursor_position, hide_window, kill_window_timer, monitor_at, poll_menu_events,
    release_capture_if_owned, routes_keys_to, set_window_timer,
};

use super::CC_POLL_MS;
use super::ControlCenterSurface;
use super::OPEN_CLICK_GRACE;
use super::PostClickAction;
use super::poll_dismiss_action;

impl ControlCenterSurface {
    pub(super) fn open(
        &mut self,
        runtime: &RuntimeContext,
        page: ControlCenterPage,
        anchor_right: i32,
        anchor_y: i32,
        focused: bool,
        motion: MotionPreference,
    ) {
        let _ = self.leave_keyboard_focus();
        let next_monitor = monitor_at(anchor_right - 1, anchor_y);
        if next_monitor.dpi != self.monitor.dpi {
            self.renderer = None;
        }
        self.monitor = next_monitor;
        self.anchor_right = anchor_right;
        self.anchor_y = anchor_y;
        self.motion = motion;
        self.start_morph();
        self.snapshot.page = page;
        self.snapshot.editing = false;
        self.snapshot.audio_switch_failed = false;
        self.scroll_offset = 0;
        self.hovered = None;
        self.pressed = None;
        self.drag = None;
        self.sync_snapshot(runtime);
        self.relayout();
        if focused {
            let _ = self.enter_keyboard_focus();
        }
        self.opened_at = Instant::now();
        self.dismissed_outside_at = None;
        self.was_mouse_down = poll_menu_events().0;
        self.last_cursor = cursor_position();
        self.visible = true;
        self.prime_backdrop(runtime);
        self.place();
        self.present(runtime);
        let _ = set_window_timer(self.hwnd, CC_POLL_TIMER, CC_POLL_MS);
    }

    pub(super) fn hide(&mut self) {
        if !self.visible {
            return;
        }
        let _ = self.leave_keyboard_focus();
        self.visible = false;
        self.snapshot.editing = false;
        self.hovered = None;
        self.pressed = None;
        self.drag = None;
        self.morph_progress = None;
        self.backdrop = None;
        let _ = release_capture_if_owned(self.hwnd);
        kill_window_timer(self.hwnd, CC_POLL_TIMER);
        hide_window(self.hwnd);
        self.renderer = None;
        self.snapshot.media = None;
        self.snapshot.wifi = WifiState::default();
        self.snapshot.bluetooth = BluetoothState::default();
        self.snapshot.audio_outputs.clear();
        otto_platform::trim_working_set();
    }

    pub(super) fn start_morph(&mut self) {
        let progress = self.morph_progress.unwrap_or(0.0);
        self.morph_tick_at = Instant::now();
        self.morph_progress = self.motion.allows_motion().then_some(progress);
    }

    pub(super) fn advance_morph(&mut self) -> bool {
        let Some(progress) = self.morph_progress else {
            return false;
        };
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.morph_tick_at)
            .as_secs_f32()
            .clamp(0.001, 0.1);
        self.morph_tick_at = now;
        let next = animate_value(
            progress,
            1.0,
            MotionMetrics::default().control_center_morph_rate,
            dt,
            self.motion,
        );
        if next >= 0.995 {
            self.morph_progress = None;
        } else {
            self.morph_progress = Some(next);
        }
        true
    }

    pub(super) fn on_poll_timer(&mut self, runtime: &RuntimeContext) -> PostClickAction {
        if !self.visible {
            return PostClickAction::None;
        }
        if self.focus.is_active() && !routes_keys_to(self.hwnd) {
            self.focus.exit();
            self.present(runtime);
        }
        let cursor = cursor_position();
        let inside = self.layout.window.contains(cursor.0, cursor.1);
        let (mouse_down, nav_event) = poll_menu_events();
        if poll_dismiss_action(
            nav_event,
            mouse_down,
            &mut self.was_mouse_down,
            self.opened_at.elapsed() >= OPEN_CLICK_GRACE,
            inside,
        ) {
            if matches!(nav_event, Some(MenuNavEvent::Escape)) {
                if self.snapshot.page != ControlCenterPage::Main {
                    return PostClickAction::Command(ControlCenterCommand::Back, false);
                }
                if self.snapshot.editing {
                    return PostClickAction::Command(ControlCenterCommand::ExitEditMode, false);
                }
            } else {
                self.dismissed_outside_at = Some(Instant::now());
            }
            return PostClickAction::Close;
        }
        if self.advance_morph() {
            self.present(runtime);
        }
        if self.drag.is_some() {
            return PostClickAction::None;
        }
        if cursor != self.last_cursor {
            self.last_cursor = cursor;
            if inside {
                self.on_mouse_move(
                    runtime,
                    cursor.0 - self.layout.window.x,
                    cursor.1 - self.layout.window.y,
                );
            } else if self.hovered.take().is_some() {
                self.present(runtime);
            }
        }
        PostClickAction::None
    }
}
