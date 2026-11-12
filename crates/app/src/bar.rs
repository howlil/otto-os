use std::mem::size_of;
use std::time::Instant;

use crate::bar_controls::{BarControlAction, BarControlsController};
use crate::host::RuntimeContext;
use crate::menu::TopBarMenuRequest;
use crate::notch::{NotchController, NotchEffect, NotchHotkey};
use otto_core::{
    Appearance, BarFocus, BarFocusOutcome, BarKey, BarPointer, BarVisibility, ChangeGate,
    HitTarget, Layout, LyricsStatus, Metrics, MotionPreference, NotchExpandedTab, NotchHudKind,
    NotchLayoutInput, NotchPort, NowPlaying, TickCadence, compute_layout_with_notch,
    detect_hud_event, is_top_bar_menu_target, scale,
};
use otto_platform::release_capture_if_owned;
use otto_platform::{
    AppBar, BackdropMaterial, LyricsOutcome, LyricsRequest, LyricsService, PrimaryScreen,
    WindowsNotchPort, acknowledge_idle_wake, adjust_volume, apply_backdrop_material,
    current_process_id, cursor_position, enable_backdrop_blur, foreground_application,
    is_escape_down, motion_preference, open_quick_settings, open_spotlight_search, primary_screen,
    read_system_status, register_idle_wake, register_notch_hotkeys, restore_shell_from_recovery,
    route_bar_keys, routes_keys_to, step_display_brightness, stop_routing_bar_keys,
    stop_routing_if_targets, suppress_native_osd, toggle_tray_overflow, unregister_idle_wake,
    unregister_notch_hotkeys, watch_top_edge, window_bounds, window_change_generations,
};
use otto_render::Renderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer,
    SetWindowPos, ShowWindow, WM_CAPTURECHANGED, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_DWMCOLORIZATIONCOLORCHANGED, WM_ENDSESSION, WM_HOTKEY, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUERYENDSESSION, WM_RBUTTONUP,
    WM_SETTINGCHANGE, WM_SYSCOLORCHANGE, WM_THEMECHANGED, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const BLUR_TINT_ALPHA: f32 = 0.28;
const STATUS_TIMER: usize = 1;
const HUD_ANIM_TIMER: usize = 2;
const DEADLINE_TIMER: usize = 3;
const WM_POWERBROADCAST: u32 = 0x0218;
const PBT_APMSUSPEND: usize = 0x0004;
const PBT_APMRESUMESUSPEND: usize = 0x0007;
const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
const MIN_DEADLINE_MS: u128 = 10;
const STATUS_POLL_MS: u32 = 120;
const FOREGROUND_FALLBACK_TICKS: u32 = 8;
const WM_APP_BAR_KEY: u32 = 0x8000 + 21;
const WM_APP_IDLE_WAKE: u32 = 0x8000 + 22;
const STATUS_IDLE_MS: u32 = 1000;
const STATUS_IDLE_AFTER_TICKS: u32 = 16;
const MEDIA_EVERY_TICKS: u8 = 4;
const MAINTAIN_EVERY_TICKS: u8 = 8;
const HUD_ANIM_MS: u32 = 8;

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Bar {
    hwnd: isize,
    panel_hwnd: isize,
    panel_visible: bool,
    own_pid: u32,
    appbar: AppBar,
    renderer: Renderer,
    screen: PrimaryScreen,
    metrics: Metrics,
    tint: f32,
    backdrop_requested: bool,
    layout: Layout,
    controls: BarControlsController,
    notch_port: WindowsNotchPort,
    notch: NotchController,
    hovered: Option<HitTarget>,
    tracking_bar_mouse: bool,
    tracking_panel_mouse: bool,
    maintain_ticks: u8,
    foreground_gate: ChangeGate,
    cadence: TickCadence,
    focus: BarFocus,
    seek_drag_hwnd: Option<isize>,
    lyrics: LyricsService,
    lyrics_track: Option<(String, String)>,
    lyrics_generation: u64,
    online_lyrics_enabled: bool,
    animating: bool,
    settle_ticks: u32,
    control_center_enabled: bool,
    motion: MotionPreference,
    visibility: BarVisibility,
    shell_full_screen: bool,
}

#[allow(clippy::too_many_lines)]
pub fn start(
    runtime: &RuntimeContext,
    hwnd: isize,
    panel_hwnd: isize,
    blur: bool,
    control_center_enabled: bool,
) -> Result<()> {
    let mut appbar = AppBar::register(hwnd);
    let own_pid = current_process_id();
    let (active_app, active_window) = foreground_application(own_pid).map_or_else(
        || (String::from("Finder"), None),
        |foreground| (foreground.name, foreground.window),
    );
    let metrics = Metrics::default();
    let screen = primary_screen();
    let notch_port = WindowsNotchPort;
    let media = notch_port.read_now_playing();
    let notch_tab = if media.as_ref().is_some_and(NowPlaying::is_active) {
        NotchExpandedTab::Media
    } else {
        NotchExpandedTab::Pomodoro
    };
    let notch = NotchController::new(media, notch_tab);
    let status = read_system_status();
    let controls = BarControlsController::new(status, active_app.clone(), active_window);
    let layout = compute_layout_with_notch(
        &metrics,
        screen.width,
        screen.dpi,
        &active_app,
        notch.layout_input(control_center_enabled),
    );
    let appearance = crate::theme_state::appearance(runtime);
    let tint = if blur && appearance.uses_backdrop() && enable_backdrop_blur(hwnd) {
        BLUR_TINT_ALPHA
    } else {
        0.0
    };
    let max_panel_h = scale(
        metrics
            .notch_expanded_height
            .max(metrics.notch_lyrics_height)
            + 4,
        screen.dpi,
    );
    let renderer = Renderer::new(
        layout.bar.width,
        layout.bar.height,
        layout.panel_window.width,
        max_panel_h,
        screen.dpi,
        tint,
    )?;

    appbar.hide_system_taskbar();
    appbar.reserve_top(screen.width, screen.height, layout.bar.height);
    register_notch_hotkeys(hwnd);

    let lyrics = LyricsService::start();

    let mut bar = Bar {
        hwnd,
        panel_hwnd,
        panel_visible: false,
        own_pid,
        appbar,
        renderer,
        screen,
        metrics,
        tint,
        backdrop_requested: blur,
        layout,
        controls,
        notch_port,
        notch,
        hovered: None,
        tracking_bar_mouse: false,
        tracking_panel_mouse: false,
        maintain_ticks: 0,
        foreground_gate: ChangeGate::new(FOREGROUND_FALLBACK_TICKS),
        cadence: TickCadence::new(STATUS_POLL_MS, STATUS_IDLE_MS, STATUS_IDLE_AFTER_TICKS),
        focus: BarFocus::default(),
        seek_drag_hwnd: None,
        lyrics,
        lyrics_track: None,
        lyrics_generation: 0,
        online_lyrics_enabled: runtime.online_lyrics_enabled.get(),
        animating: false,
        settle_ticks: 0,
        control_center_enabled,
        motion: motion_preference(),
        visibility: BarVisibility::default(),
        shell_full_screen: false,
    };
    bar.place();
    bar.present(runtime);

    unsafe {
        SetTimer(
            Some(HWND(hwnd as *mut _)),
            STATUS_TIMER,
            STATUS_POLL_MS,
            None,
        );
    }
    bar.ensure_anim_timer();
    register_idle_wake(hwnd, WM_APP_IDLE_WAKE);

    *runtime.bar.borrow_mut() = Some(bar);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    unregister_idle_wake();
    if let Some(mut bar) = runtime.bar.borrow_mut().take() {
        unsafe {
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), STATUS_TIMER);
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), HUD_ANIM_TIMER);
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), DEADLINE_TIMER);
        }
        unregister_notch_hotkeys(bar.hwnd);
        stop_routing_bar_keys();
        bar.lyrics.shutdown();
    }
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    wake_for_message(runtime, message);
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_QUERYENDSESSION => Some(LRESULT(1)),
        WM_APP_IDLE_WAKE => Some(LRESULT(0)),
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                let _ = otto_platform::restore_hidden_windows();
                let _ = restore_shell_from_recovery();
            }
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == STATUS_TIMER => {
            with_bar(runtime, |bar| bar.tick(runtime));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == HUD_ANIM_TIMER => {
            with_bar(runtime, |bar| bar.step_hud_animation(runtime));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == DEADLINE_TIMER => {
            with_bar(runtime, |bar| bar.on_deadline(runtime));
            Some(LRESULT(0))
        }
        otto_platform::APPBAR_CALLBACK_MESSAGE => {
            if let Some(notification) =
                otto_platform::classify_shell_notification(wparam.0, lparam.0)
            {
                with_bar(runtime, |bar| {
                    bar.on_shell_notification(runtime, notification);
                });
            }
            Some(LRESULT(0))
        }
        WM_POWERBROADCAST => {
            with_bar(runtime, |bar| bar.on_power_event(runtime, wparam.0));
            Some(LRESULT(1))
        }
        WM_HOTKEY => {
            with_bar(runtime, |bar| bar.on_hotkey(runtime, wparam.0 as i32));
            Some(LRESULT(0))
        }
        WM_APP_BAR_KEY => {
            if let Some(key) = BarKey::from_code(wparam.0) {
                with_bar(runtime, |bar| bar.on_bar_key(runtime, key));
            }
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| {
                if bar.notch.is_seek_dragging() {
                    bar.drag_seek(runtime, hwnd, x, y);
                } else {
                    bar.on_mouse_move(runtime, hwnd, x, y);
                }
            });
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            with_bar(runtime, |bar| bar.release_seek(runtime, true));
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_bar(runtime, |bar| bar.release_seek(runtime, false));
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_bar(runtime, |bar| bar.on_mouse_leave(runtime, hwnd));
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_mouse_wheel(runtime, x, y, delta));
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_left_click(runtime, hwnd, x, y));
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_right_click(runtime, hwnd, x, y));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_bar(runtime, |bar| bar.rebuild(runtime));
            Some(LRESULT(0))
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_DWMCOLORIZATIONCOLORCHANGED => {
            crate::theme_state::refresh_system_appearance(runtime);
            None
        }
        _ => None,
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

const fn wakes_status_tick(message: u32) -> bool {
    matches!(
        message,
        WM_APP_IDLE_WAKE
            | WM_HOTKEY
            | WM_APP_BAR_KEY
            | WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MOUSEWHEEL
            | WM_POWERBROADCAST
            | otto_platform::APPBAR_CALLBACK_MESSAGE
    )
}

fn wake_for_message(runtime: &RuntimeContext, message: u32) {
    if message == WM_APP_IDLE_WAKE {
        acknowledge_idle_wake();
    }
    if wakes_status_tick(message) {
        wake(runtime);
    }
}

pub fn wake(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| bar.wake(runtime));
    crate::dock::wake(runtime);
}

fn with_bar(runtime: &RuntimeContext, action: impl FnOnce(&mut Bar)) {
    if let Ok(mut guard) = runtime.bar.try_borrow_mut()
        && let Some(bar) = guard.as_mut()
    {
        action(bar);
    }
}

pub fn refresh_plugin_layout(runtime: &RuntimeContext, control_center_enabled: bool) {
    with_bar(runtime, |bar| {
        bar.control_center_enabled = control_center_enabled;
        bar.recompute_layout();
        bar.present(runtime);
    });
}

pub fn activate_hidden_target(runtime: &RuntimeContext, target: HitTarget) {
    with_bar(runtime, |bar| {
        if let Some(action) = BarControlsController::click_action(target) {
            bar.perform(runtime, action);
        }
    });
}

pub fn apply_appearance(runtime: &RuntimeContext, appearance: Appearance) {
    with_bar(runtime, |bar| {
        if bar.backdrop_requested {
            let material = if appearance.uses_backdrop() {
                BackdropMaterial::Acrylic
            } else {
                BackdropMaterial::None
            };
            let _ = apply_backdrop_material(bar.hwnd, material);
        }
        bar.present(runtime);
    });
}

pub fn set_online_lyrics_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.online_lyrics_enabled = enabled;
        bar.lyrics_track = None;
        bar.request_lyrics_for_current_track();
    });
}

impl Bar {
    fn request_lyrics_for_current_track(&mut self) {
        let Some(media) = self.notch.media() else {
            self.lyrics_track = None;
            return;
        };
        let track = (media.title.clone(), media.artist.clone());
        if track.0.trim().is_empty() || self.lyrics_track.as_ref() == Some(&track) {
            return;
        }
        let generation = self.lyrics_generation.wrapping_add(1);
        let accepted = self.lyrics.request(LyricsRequest {
            generation,
            title: track.0.clone(),
            artist: track.1.clone(),
            duration_ms: media.duration_ms,
            allow_online: self.online_lyrics_enabled,
        });
        self.lyrics_generation = generation;
        if accepted {
            self.lyrics_track = Some(track);
            self.notch.begin_lyrics_request();
        }
    }

    fn notch_input(&self) -> NotchLayoutInput {
        self.notch.layout_input(self.control_center_enabled)
    }

    fn recompute_layout(&mut self) {
        self.layout = compute_layout_with_notch(
            &self.metrics,
            self.screen.width,
            self.screen.dpi,
            self.controls.active_app(),
            self.notch_input(),
        );
    }

    fn seek_ratio_at(&self, x: i32) -> Option<f32> {
        let seekbar = self.layout.media_seekbar;
        (seekbar.width > 0).then(|| ((x - seekbar.x) as f32 / seekbar.width as f32).clamp(0.0, 1.0))
    }

    fn drag_seek(&mut self, runtime: &RuntimeContext, source_hwnd: isize, raw_x: i32, raw_y: i32) {
        let (x, _) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        if let Some(ratio) = self.seek_ratio_at(x)
            && self.notch.update_seek_drag(ratio)
        {
            self.present(runtime);
        }
    }

    fn release_seek(&mut self, runtime: &RuntimeContext, commit: bool) {
        let Some(hwnd) = self.seek_drag_hwnd.take() else {
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
        if let Some(NotchEffect::Platform(action)) = update.effect {
            self.notch_port.execute(action);
        }
        self.present(runtime);
    }

    fn translate_client_point(&self, source_hwnd: isize, x: i32, y: i32) -> (i32, i32) {
        if source_hwnd == self.panel_hwnd {
            (
                x + self.layout.panel_window.x,
                y + self.layout.panel_window.y,
            )
        } else {
            (x, y)
        }
    }

    fn on_shell_notification(
        &mut self,
        runtime: &RuntimeContext,
        notification: otto_platform::ShellNotification,
    ) {
        match notification {
            otto_platform::ShellNotification::PositionChanged => {
                self.appbar.reserve_top(
                    self.screen.width,
                    self.screen.height,
                    self.layout.bar.height,
                );
            }
            otto_platform::ShellNotification::FullScreenApp(active) => {
                self.shell_full_screen = active;
                self.sync_full_screen(runtime);
            }
        }
    }

    fn foreground_covers_primary(&self) -> bool {
        self.controls.active_window().is_some_and(|window| {
            window.full_screen
                && window_bounds(window.identity.hwnd).is_some_and(|frame| {
                    frame.x <= 0
                        && frame.y <= 0
                        && frame.right() >= self.screen.width
                        && frame.bottom() >= self.screen.height
                })
        })
    }

    fn sync_full_screen(&mut self, runtime: &RuntimeContext) {
        let active = self.shell_full_screen || self.foreground_covers_primary();
        if self.visibility.set_full_screen_app(active) {
            self.apply_visibility(runtime);
        }
    }

    fn track_reveal(&mut self, runtime: &RuntimeContext) {
        let (x, y) = cursor_position();
        let pointer = BarPointer {
            on_bar_monitor: x >= 0 && x < self.screen.width && y >= 0 && y < self.screen.height,
            y,
        };
        let popup_open = crate::menu::active_top_bar_target(runtime).is_some()
            || crate::menu::is_control_center_open(runtime)
            || crate::menu::is_calendar_open(runtime);
        if self
            .visibility
            .track_pointer(pointer, self.layout.bar.height, popup_open)
        {
            self.apply_visibility(runtime);
        }
    }

    fn apply_visibility(&mut self, runtime: &RuntimeContext) {
        if self.visibility.is_visible() {
            self.place();
            self.present(runtime);
            return;
        }
        crate::menu::close(runtime);
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            let _ = ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE);
        }
        self.panel_visible = false;
    }

    fn place(&self) {
        if !self.visibility.is_visible() {
            return;
        }
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                0,
                0,
                self.layout.bar.width,
                self.layout.bar.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    fn sync_panel_visibility(&mut self) {
        let should_show = self.notch.panel_expansion() > 0.005 && self.visibility.is_visible();
        if should_show {
            let _ = unsafe {
                SetWindowPos(
                    HWND(self.panel_hwnd as *mut _),
                    Some(HWND_TOPMOST),
                    self.layout.panel_window.x,
                    self.layout.panel_window.y,
                    self.layout.panel_window.width,
                    self.layout.panel_window.height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
            };
            self.panel_visible = true;
        } else if self.panel_visible {
            let _ = unsafe { ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE) };
            self.panel_visible = false;
        }
    }

    fn ensure_anim_timer(&mut self) {
        if !self.animating {
            let timer = unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    HUD_ANIM_TIMER,
                    HUD_ANIM_MS,
                    None,
                )
            };
            if timer != 0 {
                self.animating = true;
                self.settle_ticks = 10;
            }
        }
    }

    fn stop_anim_timer(&mut self) {
        if self.animating {
            self.animating = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), HUD_ANIM_TIMER);
            }
            otto_platform::trim_working_set();
        }
    }

    fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.screen = primary_screen();
        self.recompute_layout();
        let max_panel_h = scale(
            self.metrics
                .notch_expanded_height
                .max(self.metrics.notch_lyrics_height)
                + 4,
            self.screen.dpi,
        );
        if let Ok(renderer) = Renderer::new(
            self.layout.bar.width,
            self.layout.bar.height,
            self.layout.panel_window.width,
            max_panel_h,
            self.screen.dpi,
            self.tint,
        ) {
            self.renderer = renderer;
        }
        self.appbar.rebase(self.screen.width, self.screen.height);
        self.appbar.reserve_top(
            self.screen.width,
            self.screen.height,
            self.layout.bar.height,
        );
        self.place();
        self.present(runtime);
        self.ensure_anim_timer();
    }

    fn wake(&mut self, runtime: &RuntimeContext) {
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
            self.tick(runtime);
        }
    }

    fn set_status_interval(&self, interval: u32) {
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                STATUS_TIMER,
                interval,
                None,
            );
        }
    }

    fn periodic_due(&self, every_ticks: u8) -> bool {
        self.cadence.is_idle() || self.maintain_ticks.is_multiple_of(every_ticks)
    }

    fn hidden_for_full_screen(&self) -> bool {
        self.visibility.tracks_pointer() && !self.visibility.is_visible()
    }

    fn status_busy(&self, runtime: &RuntimeContext, dirty: bool) -> bool {
        let reveal_needs_polling = self.visibility.tracks_pointer()
            && (self.visibility.is_visible() || window_change_generations().is_none());
        dirty || reveal_needs_polling || self.needs_status_polling(runtime)
    }

    fn needs_status_polling(&self, runtime: &RuntimeContext) -> bool {
        self.animating
            || self.settle_ticks > 0
            || self.panel_visible
            || self.hovered.is_some()
            || self.focus.is_active()
            || self.notch.is_seek_dragging()
            || self.notch.hud_state().is_some()
            || self.notch.lyrics_view().status == LyricsStatus::Loading
            || crate::menu::active_top_bar_target(runtime).is_some()
            || crate::menu::is_control_center_open(runtime)
            || crate::menu::is_calendar_open(runtime)
    }

    fn tick(&mut self, runtime: &RuntimeContext) {
        self.maintain_ticks = self.maintain_ticks.wrapping_add(1);
        self.track_reveal(runtime);
        crate::window_controls::poll_foreground(runtime);
        if self.periodic_due(MAINTAIN_EVERY_TICKS) {
            suppress_native_osd();
            self.appbar.maintain(
                self.screen.width,
                self.screen.height,
                self.layout.bar.height,
            );
            if self.panel_visible && self.visibility.is_visible() {
                let _ = unsafe {
                    SetWindowPos(
                        HWND(self.panel_hwnd as *mut _),
                        Some(HWND_TOPMOST),
                        self.layout.panel_window.x,
                        self.layout.panel_window.y,
                        self.layout.panel_window.width,
                        self.layout.panel_window.height,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    )
                };
            }
        }
        let mut dirty = false;
        while let Some(response) = self.lyrics.poll() {
            if response.generation == self.lyrics_generation
                && let Some(media) = self.notch.media()
                && media.title == response.title
                && media.artist == response.artist
            {
                let (sheet, status) = match response.outcome {
                    LyricsOutcome::Found { sheet, .. } => (Some(sheet), LyricsStatus::Ready),
                    LyricsOutcome::NotFound => (
                        None,
                        LyricsStatus::NotFound {
                            searched_online: self.online_lyrics_enabled,
                        },
                    ),
                    LyricsOutcome::Failed => (None, LyricsStatus::Failed),
                };
                self.notch.finish_lyrics(sheet, status);
                dirty = true;
            }
        }
        let foreground_changed = self
            .foreground_gate
            .should_refresh(window_change_generations().map(|changes| changes.geometry));
        if foreground_changed && let Some(foreground) = foreground_application(self.own_pid) {
            let change = self
                .controls
                .apply_foreground(foreground.name, foreground.window);
            if change.app {
                self.recompute_layout();
                dirty = true;
            }
            if change.window && crate::menu::active_top_bar_target(runtime).is_some() {
                crate::menu::close(runtime);
                dirty = true;
            }
        }
        self.sync_full_screen(runtime);
        self.sync_keyboard_focus(runtime);
        if self.notch.is_seek_dragging() && is_escape_down() {
            self.release_seek(runtime, false);
        }
        let latest = read_system_status();
        if let Some(event) = detect_hud_event(self.controls.status(), latest) {
            self.notch.trigger_hud(event);
            dirty = true;
        }
        if self.controls.apply_status(latest) {
            dirty = true;
        }
        if self.periodic_due(MEDIA_EVERY_TICKS) {
            let latest_media = self.notch_port.read_now_playing();
            if self.notch.set_media(latest_media) {
                self.recompute_layout();
                dirty = true;
            }
            self.request_lyrics_for_current_track();
        }
        self.motion = motion_preference();
        if dirty {
            self.ensure_anim_timer();
            self.present(runtime);
        }
        self.schedule_deadline();
        watch_top_edge(
            self.hidden_for_full_screen(),
            self.screen.width,
            self.screen.height,
        );
        let busy = self.status_busy(runtime, dirty);
        if let Some(interval) = self.cadence.observe(busy) {
            self.set_status_interval(interval);
        }
    }

    fn schedule_deadline(&self) {
        let hwnd = Some(HWND(self.hwnd as *mut _));
        let now = Instant::now();
        match self.notch.next_deadline(now) {
            Some(deadline) => {
                let wait = deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .max(MIN_DEADLINE_MS);
                unsafe {
                    SetTimer(
                        hwnd,
                        DEADLINE_TIMER,
                        u32::try_from(wait).unwrap_or(u32::MAX),
                        None,
                    );
                }
            }
            None => unsafe {
                let _ = KillTimer(hwnd, DEADLINE_TIMER);
            },
        }
    }

    fn apply_notch_update(&mut self, runtime: &RuntimeContext, update: crate::notch::NotchUpdate) {
        if let Some(NotchEffect::Platform(action)) = update.effect {
            self.notch_port.execute(action);
        }
        if update.changed {
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
            self.ensure_anim_timer();
        }
    }

    fn on_deadline(&mut self, runtime: &RuntimeContext) {
        let update = self.notch.sync_clock(Instant::now());
        self.apply_notch_update(runtime, update);
        self.schedule_deadline();
    }

    fn on_power_event(&mut self, runtime: &RuntimeContext, event: usize) {
        let now = Instant::now();
        match event {
            PBT_APMSUSPEND => {
                let update = self.notch.suspend(now);
                self.apply_notch_update(runtime, update);
            }
            PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC => {
                self.notch.resume(now);
                self.schedule_deadline();
            }
            _ => {}
        }
    }

    fn step_hud_animation(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let (cx, cy) = cursor_position();
        let update = self.notch.advance(
            now,
            self.layout.notch.contains(cx, cy),
            self.panel_visible,
            self.motion,
        );
        if let Some(NotchEffect::Platform(action)) = update.effect {
            self.notch_port.execute(action);
        }
        if update.changed {
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
        self.schedule_deadline();
    }

    fn enter_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if !self.visibility.is_visible() {
            return;
        }
        crate::menu::close(runtime);
        let targets = self.layout.keyboard_targets();
        if self.focus.enter(&targets).is_some() && route_bar_keys(self.hwnd, WM_APP_BAR_KEY) {
            self.present(runtime);
        } else {
            self.focus.exit();
        }
    }

    fn enter_from_dock(&mut self, runtime: &RuntimeContext, from_end: bool) {
        if !self.visibility.is_visible() {
            stop_routing_if_targets(self.hwnd);
            return;
        }
        let targets = self.layout.keyboard_targets();
        let entered = if from_end {
            self.focus.enter_last(&targets)
        } else {
            self.focus.enter(&targets)
        };
        if entered.is_some() && route_bar_keys(self.hwnd, WM_APP_BAR_KEY) {
            self.present(runtime);
        } else {
            self.focus.exit();
            stop_routing_if_targets(self.hwnd);
        }
    }

    fn leave_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        stop_routing_if_targets(self.hwnd);
        self.focus.exit();
        self.present(runtime);
    }

    fn sync_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if !self.focus.is_active() {
            return;
        }
        if !routes_keys_to(self.hwnd) || !self.visibility.is_visible() {
            self.leave_keyboard_focus(runtime);
        } else if self.focus.revalidate(&self.layout.keyboard_targets()) {
            self.present(runtime);
        }
    }

    fn on_bar_key(&mut self, runtime: &RuntimeContext, key: BarKey) {
        let targets = self.layout.keyboard_targets();
        if let Some(from_end) = self.focus.dock_handoff(key, &targets)
            && crate::dock::keyboard_entry_count(runtime) > 0
        {
            self.focus.exit();
            crate::dock::enter_keyboard_focus(runtime, from_end);
            self.present(runtime);
            return;
        }
        match self.focus.handle(key, &targets) {
            BarFocusOutcome::Moved(_) => self.present(runtime),
            BarFocusOutcome::Exited => self.leave_keyboard_focus(runtime),
            BarFocusOutcome::Activate(target) => {
                self.leave_keyboard_focus(runtime);
                self.present(runtime);
                if let Some(action) = BarControlsController::click_action(target) {
                    self.perform(runtime, action);
                }
            }
            BarFocusOutcome::Unchanged => {}
        }
    }

    fn on_hotkey(&mut self, runtime: &RuntimeContext, hotkey_id: i32) {
        if hotkey_id == otto_platform::HOTKEY_FOCUS_BAR {
            if self.focus.is_active() {
                self.leave_keyboard_focus(runtime);
            } else {
                self.enter_keyboard_focus(runtime);
            }
            return;
        }
        self.ensure_anim_timer();
        let Some(hotkey) = (match hotkey_id {
            otto_platform::HOTKEY_TOGGLE_NOTCH => Some(NotchHotkey::ToggleNotch),
            otto_platform::HOTKEY_TOGGLE_POMODORO => Some(NotchHotkey::TogglePomodoro),
            _ => None,
        }) else {
            return;
        };
        let update = self.notch.handle_hotkey(hotkey);
        if update.changed {
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
        }
        self.schedule_deadline();
    }

    fn ensure_mouse_tracking(&mut self, source_hwnd: isize) {
        let already_tracking = if source_hwnd == self.panel_hwnd {
            self.tracking_panel_mouse
        } else {
            self.tracking_bar_mouse
        };
        if already_tracking {
            return;
        }
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: HWND(source_hwnd as *mut _),
            dwHoverTime: 0,
        };
        if unsafe { TrackMouseEvent(&mut track) }.is_ok() {
            if source_hwnd == self.panel_hwnd {
                self.tracking_panel_mouse = true;
            } else {
                self.tracking_bar_mouse = true;
            }
        }
    }

    fn on_mouse_move(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        self.ensure_mouse_tracking(source_hwnd);
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);

        self.notch
            .handle_pointer_move(self.layout.notch.contains(x, y));

        let target = self.layout.hit_test(x, y);
        if target != self.hovered || self.layout.notch.contains(x, y) {
            self.ensure_anim_timer();
        }
        if let Some(open_target) = crate::menu::active_top_bar_target(runtime)
            && let Some(hit) = target
            && is_top_bar_menu_target(hit)
            && hit != open_target
        {
            self.open_menu(runtime, hit, None);
        }
        if target != self.hovered {
            self.hovered = target;
            self.present(runtime);
        }
    }

    fn on_mouse_leave(&mut self, runtime: &RuntimeContext, source_hwnd: isize) {
        self.ensure_anim_timer();
        if source_hwnd == self.panel_hwnd {
            self.tracking_panel_mouse = false;
        } else {
            self.tracking_bar_mouse = false;
        }
        let (cx, cy) = cursor_position();
        let cursor_inside_notch = self.layout.notch.contains(cx, cy);
        let notch_changed = self.notch.handle_pointer_leave(cursor_inside_notch);
        if cursor_inside_notch {
            return;
        }
        if self.hovered.is_some() {
            self.hovered = None;
            self.present(runtime);
        } else if notch_changed {
            self.present(runtime);
        }
    }

    fn on_mouse_wheel(&mut self, runtime: &RuntimeContext, x: i32, y: i32, delta: i16) {
        self.ensure_anim_timer();
        let hit = self.hovered.or_else(|| self.layout.hit_test(x, y));
        if self.notch.panel_expansion() > 0.55
            && let Some(
                target @ (HitTarget::Notch
                | HitTarget::PomodoroPhase
                | HitTarget::PomodoroPlayPause
                | HitTarget::PomodoroReset
                | HitTarget::PomodoroSkip),
            ) = hit
        {
            let update = self.notch.handle_wheel(target, delta);
            if update.changed {
                self.present(runtime);
            }
            return;
        }
        if let Some(target) = hit
            && let Some(action) = BarControlsController::wheel_action(
                target,
                delta,
                self.notch.panel_expansion() > 0.55,
            )
        {
            match action {
                BarControlAction::AdjustVolume(delta) => {
                    adjust_volume(delta);
                    let latest = read_system_status();
                    self.controls.apply_status(latest);
                    self.notch.trigger_hud(NotchHudKind::Volume {
                        percent: if latest.muted { 0 } else { latest.volume },
                        muted: latest.muted,
                    });
                }
                BarControlAction::StepBrightness(delta) => {
                    let pct = step_display_brightness(delta);
                    let mut status = self.controls.status();
                    status.brightness = pct;
                    self.controls.apply_status(status);
                    self.notch
                        .trigger_hud(NotchHudKind::Brightness { percent: pct });
                }
                _ => return,
            }
            self.present(runtime);
        }
    }

    fn on_left_click(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        self.ensure_anim_timer();
        self.leave_keyboard_focus(runtime);
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        let Some(target) = self.layout.hit_test(x, y) else {
            return;
        };

        if target == HitTarget::MediaSeekbar {
            if let Some(ratio) = self.seek_ratio_at(x)
                && self.notch.begin_seek_drag(ratio)
            {
                unsafe {
                    let _ = SetCapture(HWND(source_hwnd as *mut _));
                }
                self.seek_drag_hwnd = Some(source_hwnd);
                self.present(runtime);
            }
            return;
        }

        let notch_target = matches!(
            target,
            HitTarget::Notch
                | HitTarget::PomodoroPhase
                | HitTarget::PomodoroPlayPause
                | HitTarget::PomodoroReset
                | HitTarget::PomodoroSkip
                | HitTarget::PomodoroStartNext
                | HitTarget::PomodoroExtendFive
                | HitTarget::NotchTabPomodoro
                | HitTarget::NotchTabMedia
                | HitTarget::MediaPlayPause
                | HitTarget::MediaPrevious
                | HitTarget::MediaNext
                | HitTarget::MediaLyricsToggle
        );
        if notch_target {
            let update = self.notch.handle_click(target);
            if let Some(NotchEffect::Platform(action)) = update.effect {
                self.notch_port.execute(action);
            }
            if update.changed {
                self.recompute_layout();
                self.sync_panel_visibility();
                self.present(runtime);
            }
            return;
        }

        if let Some(action) = BarControlsController::click_action(target) {
            self.perform(runtime, action);
        }
    }

    fn open_menu(&self, runtime: &RuntimeContext, target: HitTarget, anchor_x: Option<i32>) {
        let Some(slot) = self.layout.menu_anchor_rect(target) else {
            return;
        };
        crate::menu::open_top_bar_menu(
            runtime,
            &TopBarMenuRequest {
                bar_hwnd: self.hwnd,
                target,
                anchor_x: anchor_x.unwrap_or(slot.x),
                anchor_y: self.layout.bar.height + 4,
                app_name: self.controls.active_app(),
                window: self.controls.active_window(),
                hidden: &self.layout.hidden_targets,
            },
        );
    }

    fn anchor_right(&self, target: HitTarget) -> i32 {
        self.layout
            .menu_anchor_rect(target)
            .map_or(self.screen.width, |slot| slot.right())
    }

    fn perform(&mut self, runtime: &RuntimeContext, action: BarControlAction) {
        match action {
            BarControlAction::TopBarMenu(target) => {
                if crate::menu::active_top_bar_target(runtime) == Some(target) {
                    crate::menu::close(runtime);
                } else {
                    self.open_menu(runtime, target, None);
                }
                self.present(runtime);
            }
            BarControlAction::Tray => {
                let anchor = self
                    .layout
                    .menu_anchor_rect(HitTarget::Tray)
                    .unwrap_or(self.layout.tray_slot);
                let _ = toggle_tray_overflow(
                    anchor.x + anchor.width / 2,
                    self.layout.bar.height,
                    self.screen.width,
                );
            }
            BarControlAction::ControlCenter => {
                if !self.control_center_enabled {
                    return;
                }
                if crate::menu::is_control_center_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    crate::menu::open_control_center(
                        runtime,
                        self.anchor_right(HitTarget::ControlCenter),
                        self.layout.bar.height + 4,
                    );
                }
            }
            BarControlAction::QuickSettings => open_quick_settings(),
            BarControlAction::Search => open_spotlight_search(),
            BarControlAction::Clock => {
                if crate::menu::is_calendar_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    crate::menu::open_calendar(
                        runtime,
                        self.anchor_right(HitTarget::Clock),
                        self.layout.bar.height + 4,
                    );
                }
            }
            BarControlAction::AdjustVolume(_) | BarControlAction::StepBrightness(_) => {}
        }
    }

    fn on_right_click(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        self.ensure_anim_timer();
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        if self.layout.notch.contains(x, y) {
            let update = self.notch.dismiss_compact(Instant::now());
            if update.changed {
                self.recompute_layout();
                self.present(runtime);
                return;
            }
        }
        let anchor_x = if self.layout.hit_test(x, y) == Some(HitTarget::Start) {
            self.layout.start_slot.x
        } else {
            x
        };
        self.open_menu(runtime, HitTarget::Start, Some(anchor_x));
        self.present(runtime);
    }

    fn present(&self, runtime: &RuntimeContext) {
        let hovered = self
            .hovered
            .or_else(|| crate::menu::active_top_bar_target(runtime));
        let appearance = crate::theme_state::appearance(runtime);
        let media = self.notch.display_media(crate::notch::unix_now_ms());
        let _ = self.renderer.present(
            self.hwnd,
            (0, 0),
            &self.layout,
            self.controls.status(),
            self.controls.active_app(),
            hovered,
            self.focus.current(),
            self.notch.hud_state(),
            self.notch.pomodoro(),
            media.as_ref(),
            self.notch.anim_phase(),
            appearance,
        );
        if self.panel_visible {
            let _ = self.renderer.present_panel(
                self.panel_hwnd,
                &self.layout,
                hovered,
                self.notch.pomodoro(),
                media.as_ref(),
                self.notch.anim_phase(),
                self.notch.lyrics_view(),
            );
        }
    }
}

pub fn enter_keyboard_focus_from_dock(runtime: &RuntimeContext, from_end: bool) {
    with_bar(runtime, |bar| bar.enter_from_dock(runtime, from_end));
}
