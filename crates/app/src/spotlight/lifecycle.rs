use std::time::Instant;

use crate::host::RuntimeContext;
use crate::win_msg::SPOTLIGHT_TIMER;
use otto_core::{
    MotionMetrics, Rect, SPOTLIGHT_DEFAULT_HEIGHT_POINTS, SPOTLIGHT_DEFAULT_WIDTH_POINTS,
    SpotlightMode, SpotlightView, animate_value, compute_spotlight_layout, scale,
};
use otto_platform::{
    close_tray_overflow, monitor_work_area_at, motion_preference, primary_screen,
    release_capture_if_owned,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, SW_HIDE, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow,
};

use super::QueryEditorState;
use super::SPOTLIGHT_MAX_HEIGHT_POINTS;
use super::SPOTLIGHT_MAX_WIDTH_POINTS;
use super::SPOTLIGHT_TICK_MS;
use super::SpotlightSurface;
use super::interpolate_rect;

impl SpotlightSurface {
    pub(super) fn open(
        &mut self,
        runtime: &RuntimeContext,
        mode: SpotlightMode,
        trigger: Option<Rect>,
    ) {
        self.screen = primary_screen();
        if let Err(error) = self.ensure_renderer() {
            crate::host::log_present_error("spotlight renderer", Err(error));
            if self.visible {
                self.hide();
            }
            return;
        }
        self.motion = motion_preference();
        let trigger = trigger.unwrap_or_else(|| self.default_trigger());
        self.work_area = monitor_work_area_at(trigger.x, trigger.y);
        self.load_catalog();
        self.query.clear();
        self.query_state = QueryEditorState::default();
        self.math_result = None;
        self.pending_high_surrogate = None;
        self.mode = mode;
        self.view = match mode {
            SpotlightMode::Search => SpotlightView::List,
            SpotlightMode::Apps => SpotlightView::Grid,
        };
        self.rebuild_results();
        self.target_window = self.target_rect();
        self.source_window = self.source_rect(trigger);
        self.window = if self.motion.allows_motion() {
            self.source_window
        } else {
            self.target_window
        };
        self.morph_progress = self.motion.allows_motion().then_some(0.0);
        self.morph_target = 1.0;
        self.morph_tick_at = Instant::now();
        self.closing = false;
        self.blur_dismissed_at = None;
        self.restore_focus_on_close = false;
        if !self.visible {
            self.previous_foreground =
                unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0
                    as isize;
        }
        self.visible = true;
        self.pending_click = None;
        self.dragging = None;
        crate::menu::close(runtime);
        crate::control_center::close(runtime);
        let _ = close_tray_overflow();
        self.prime_window_backdrop(runtime, false);
        self.place();
        self.sync_backdrop(runtime, true);
        self.present(runtime);
        if self.morph_progress.is_some() {
            self.start_morph_timer();
        } else {
            self.stop_morph_timer();
        }
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNORMAL);
            let _ = SetForegroundWindow(HWND(self.hwnd as *mut _));
            let _ = SetFocus(Some(HWND(self.hwnd as *mut _)));
        }
    }

    pub(super) fn default_trigger(&self) -> Rect {
        let x = self.screen.width / 2;
        Rect {
            x: x - scale(24, self.screen.dpi),
            y: scale(16, self.screen.dpi),
            width: scale(48, self.screen.dpi),
            height: scale(28, self.screen.dpi),
        }
    }

    pub(super) fn target_rect(&self) -> Rect {
        let bounds = self.work_area;
        let minimum_width = scale(420, self.screen.dpi).min(bounds.width);
        let minimum_height = scale(300, self.screen.dpi).min(bounds.height);
        let width = scale(SPOTLIGHT_DEFAULT_WIDTH_POINTS, self.screen.dpi)
            .min(scale(SPOTLIGHT_MAX_WIDTH_POINTS, self.screen.dpi))
            .min((bounds.width - scale(32, self.screen.dpi)).max(minimum_width))
            .max(minimum_width);
        let height = scale(SPOTLIGHT_DEFAULT_HEIGHT_POINTS, self.screen.dpi)
            .min(scale(SPOTLIGHT_MAX_HEIGHT_POINTS, self.screen.dpi))
            .min((bounds.height - scale(24, self.screen.dpi)).max(minimum_height))
            .max(minimum_height);
        Rect {
            x: bounds.x + (bounds.width - width) / 2,
            y: (bounds.y + scale(20, self.screen.dpi))
                .clamp(bounds.y, (bounds.bottom() - height).max(bounds.y)),
            width,
            height,
        }
    }

    pub(super) fn source_rect(&self, trigger: Rect) -> Rect {
        let width = scale(420, self.screen.dpi).min(self.target_window.width);
        let height = scale(300, self.screen.dpi).min(self.target_window.height);
        let center = (
            trigger.x + trigger.width / 2,
            trigger.y + trigger.height / 2,
        );
        Rect {
            x: center.0 - width / 2,
            y: center.1 - height / 2,
            width,
            height,
        }
    }

    pub(super) fn clamp_window(&self, mut rect: Rect, bounds: Rect) -> Rect {
        let minimum_width = scale(420, self.screen.dpi).min(bounds.width);
        let minimum_height = scale(300, self.screen.dpi).min(bounds.height);
        rect.width = rect.width.clamp(
            minimum_width,
            scale(SPOTLIGHT_MAX_WIDTH_POINTS, self.screen.dpi).min(bounds.width),
        );
        rect.height = rect.height.clamp(
            minimum_height,
            scale(SPOTLIGHT_MAX_HEIGHT_POINTS, self.screen.dpi).min(bounds.height),
        );
        rect.x = rect
            .x
            .clamp(bounds.x, (bounds.right() - rect.width).max(bounds.x));
        rect.y = rect
            .y
            .clamp(bounds.y, (bounds.bottom() - rect.height).max(bounds.y));
        rect
    }

    pub(super) fn request_close(&mut self, restore_focus: bool) {
        if !self.visible || self.closing {
            return;
        }
        self.dragging = None;
        self.pending_click = None;
        self.restore_focus_on_close = restore_focus;
        if self.motion.allows_motion() && self.morph_progress.is_some() {
            self.closing = true;
            self.morph_target = 0.0;
            self.morph_tick_at = Instant::now();
            self.start_morph_timer();
        } else if self.motion.allows_motion() {
            self.closing = true;
            self.morph_progress = Some(1.0);
            self.morph_target = 0.0;
            self.morph_tick_at = Instant::now();
            self.start_morph_timer();
        } else {
            self.hide();
        }
    }

    pub(super) fn start_morph_timer(&self) {
        unsafe {
            let _ = SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                SPOTLIGHT_TIMER,
                SPOTLIGHT_TICK_MS,
                None,
            );
        }
    }

    pub(super) fn stop_morph_timer(&self) {
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), SPOTLIGHT_TIMER);
        }
    }

    pub(super) fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.closing = false;
        self.morph_progress = None;
        self.pending_click = None;
        self.dragging = None;
        self.backdrop = None;
        let _ = release_capture_if_owned(self.hwnd);
        unsafe {
            let hwnd = HWND(self.hwnd as *mut _);
            let _ = KillTimer(Some(hwnd), SPOTLIGHT_TIMER);
            let _ = ShowWindow(hwnd, SW_HIDE);
            if self.restore_focus_on_close && self.previous_foreground != 0 {
                let _ = SetForegroundWindow(HWND(self.previous_foreground as *mut _));
            }
        }
        self.restore_focus_on_close = false;
        self.renderer = None;
        self.query.clear();
        self.math_result = None;
        self.filtered.clear();
        self.layout = None;
        otto_platform::trim_working_set();
    }

    pub(super) fn advance_morph(&mut self, runtime: &RuntimeContext) -> bool {
        let Some(progress) = self.morph_progress else {
            self.stop_morph_timer();
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
            self.morph_target,
            MotionMetrics::default().spotlight_morph_rate,
            dt,
            self.motion,
        );
        self.morph_progress = Some(next);
        self.window = interpolate_rect(self.source_window, self.target_window, next);
        self.place();
        if self.closing && next <= 0.005 {
            self.morph_progress = None;
            self.hide();
            return false;
        }
        if !self.closing && next >= 0.995 {
            self.window = self.target_window;
            self.morph_progress = None;
            self.stop_morph_timer();
            self.prime_window_backdrop(runtime, true);
            self.sync_backdrop(runtime, true);
        }
        self.present(runtime);
        true
    }

    pub(super) fn place(&self) {
        unsafe {
            let _ = SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                self.window.x,
                self.window.y,
                self.window.width,
                self.window.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    pub(super) fn sync_layout(&mut self) {
        self.layout = Some(compute_spotlight_layout(
            (self.window.x, self.window.y),
            self.window.width,
            self.window.height,
            self.screen.dpi,
            self.view,
            self.filtered.len(),
            self.first_visible,
        ));
    }
}
