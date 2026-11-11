use std::mem::size_of;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::bar_controls::{BarControlAction, BarControlsController};
use crate::host::RuntimeContext;
use crate::notch::{NotchController, NotchEffect, NotchHotkey};
use otto_core::{
    HitTarget, Layout, LyricsSheet, Metrics, NotchExpandedTab, NotchHudKind, NotchLayoutInput,
    NotchPort, NowPlaying, compute_layout_with_notch, detect_hud_event, is_top_bar_menu_target,
    parse_lrc, scale,
};
use otto_platform::{
    AppBar, PrimaryScreen, WindowsNotchPort, adjust_volume, current_process_id, cursor_position,
    enable_backdrop_blur, foreground_app_snapshot, get_or_fetch_lyrics, load_cached_lyrics,
    open_quick_settings, open_spotlight_search, primary_screen, read_system_status,
    register_notch_hotkeys, step_display_brightness, suppress_native_osd, toggle_tray_overflow,
    unregister_notch_hotkeys,
};
use otto_render::Renderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer,
    SetWindowPos, ShowWindow, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_HOTKEY, WM_LBUTTONDOWN,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const BLUR_TINT_ALPHA: f32 = 0.28;
const STATUS_TIMER: usize = 1;
const HUD_ANIM_TIMER: usize = 2;
const STATUS_POLL_MS: u32 = 120;
const HUD_ANIM_MS: u32 = 8;

struct LyricsRequest {
    generation: u64,
    title: String,
    artist: String,
    duration_ms: u64,
    allow_online: bool,
}

struct LyricsResponse {
    generation: u64,
    title: String,
    artist: String,
    sheet: Option<LyricsSheet>,
}

struct ComApartmentGuard;

impl ComApartmentGuard {
    fn new() -> Option<Self> {
        if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
            return None;
        }
        Some(Self)
    }
}

impl Drop for ComApartmentGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn lyrics_worker_loop(receiver: &Receiver<LyricsRequest>, sender: &Sender<LyricsResponse>) {
    let com = ComApartmentGuard::new();
    while let Ok(mut request) = receiver.recv() {
        while let Ok(newer) = receiver.try_recv() {
            request = newer;
        }
        let text = if com.is_some() {
            get_or_fetch_lyrics(
                &request.title,
                &request.artist,
                request.duration_ms,
                request.allow_online,
            )
        } else {
            load_cached_lyrics(&request.title, &request.artist)
        };
        let sheet = text.map(|s| parse_lrc(&s));
        let _ = sender.send(LyricsResponse {
            generation: request.generation,
            title: request.title,
            artist: request.artist,
            sheet,
        });
    }
}

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
    layout: Layout,
    controls: BarControlsController,
    notch_port: WindowsNotchPort,
    notch: NotchController,
    hovered: Option<HitTarget>,
    tracking_bar_mouse: bool,
    tracking_panel_mouse: bool,
    maintain_ticks: u8,
    lyrics_receiver: Receiver<LyricsResponse>,
    lyrics_sender: Sender<LyricsRequest>,
    lyrics_track: Option<(String, String)>,
    lyrics_generation: u64,
    lyrics_worker: Option<JoinHandle<()>>,
    online_lyrics_enabled: bool,
    animating: bool,
    settle_ticks: u32,
    control_center_enabled: bool,
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
    let (active_app, active_hwnd) =
        foreground_app_snapshot(own_pid).unwrap_or_else(|| (String::from("Finder"), 0));
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
    let controls = BarControlsController::new(status, active_app.clone(), active_hwnd);
    let layout = compute_layout_with_notch(
        &metrics,
        screen.width,
        screen.dpi,
        &active_app,
        notch.layout_input(control_center_enabled),
    );
    let tint = if blur && enable_backdrop_blur(hwnd) {
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

    let (req_sender, req_receiver) = channel();
    let (resp_sender, resp_receiver) = channel();
    let lyrics_worker = thread::spawn(move || {
        lyrics_worker_loop(&req_receiver, &resp_sender);
    });
    let mut lyrics_generation = 0u64;
    let lyrics_track = notch.media().map(|m| (m.title.clone(), m.artist.clone()));
    if let Some(m) = notch.media() {
        lyrics_generation = 1;
        let _ = req_sender.send(LyricsRequest {
            generation: 1,
            title: m.title.clone(),
            artist: m.artist.clone(),
            duration_ms: m.duration_ms,
            allow_online: runtime.online_lyrics_enabled.get(),
        });
    }

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
        layout,
        controls,
        notch_port,
        notch,
        hovered: None,
        tracking_bar_mouse: false,
        tracking_panel_mouse: false,
        maintain_ticks: 0,
        lyrics_receiver: resp_receiver,
        lyrics_sender: req_sender,
        lyrics_track,
        lyrics_generation,
        lyrics_worker: Some(lyrics_worker),
        online_lyrics_enabled: runtime.online_lyrics_enabled.get(),
        animating: false,
        settle_ticks: 0,
        control_center_enabled,
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

    *runtime.bar.borrow_mut() = Some(bar);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    if let Some(mut bar) = runtime.bar.borrow_mut().take() {
        unsafe {
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), STATUS_TIMER);
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), HUD_ANIM_TIMER);
        }
        unregister_notch_hotkeys(bar.hwnd);
        let (dummy_sender, _) = channel();
        bar.lyrics_sender = dummy_sender;
        if let Some(handle) = bar.lyrics_worker.take() {
            let _ = handle.join();
        }
    }
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_TIMER if wparam.0 == STATUS_TIMER => {
            with_bar(runtime, |bar| bar.tick(runtime));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == HUD_ANIM_TIMER => {
            with_bar(runtime, |bar| bar.step_hud_animation(runtime));
            Some(LRESULT(0))
        }
        WM_HOTKEY => {
            with_bar(runtime, |bar| bar.on_hotkey(runtime, wparam.0 as i32));
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_mouse_move(runtime, hwnd, x, y));
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
        _ => None,
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
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

pub fn repaint(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| bar.present(runtime));
}

pub fn set_online_lyrics_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.online_lyrics_enabled = enabled;
        bar.lyrics_generation = bar.lyrics_generation.wrapping_add(1);
        if let Some(media) = bar.notch.media() {
            let _ = bar.lyrics_sender.send(LyricsRequest {
                generation: bar.lyrics_generation,
                title: media.title.clone(),
                artist: media.artist.clone(),
                duration_ms: media.duration_ms,
                allow_online: enabled,
            });
        }
    });
}

impl Bar {
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

    fn place(&self) {
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
        let should_show = self.notch.panel_expansion() > 0.002;
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
        self.appbar.reserve_top(
            self.screen.width,
            self.screen.height,
            self.layout.bar.height,
        );
        self.place();
        self.present(runtime);
        self.ensure_anim_timer();
    }

    fn tick(&mut self, runtime: &RuntimeContext) {
        self.maintain_ticks = self.maintain_ticks.wrapping_add(1);
        if self.maintain_ticks.is_multiple_of(8) {
            suppress_native_osd();
            self.appbar.maintain(
                self.screen.width,
                self.screen.height,
                self.layout.bar.height,
            );
            if self.panel_visible {
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
        while let Ok(response) = self.lyrics_receiver.try_recv() {
            if response.generation == self.lyrics_generation
                && let Some(media) = self.notch.media()
                && media.title == response.title
                && media.artist == response.artist
            {
                self.notch.set_lyrics(response.sheet);
                dirty = true;
            }
        }
        if let Some((app_name, app_hwnd)) = foreground_app_snapshot(self.own_pid)
            && self.controls.apply_foreground(app_name, app_hwnd)
        {
            self.recompute_layout();
            dirty = true;
        }
        let latest = read_system_status();
        if let Some(event) = detect_hud_event(self.controls.status(), latest) {
            self.notch.trigger_hud(event);
            dirty = true;
        }
        if self.controls.apply_status(latest) {
            dirty = true;
        }
        if self.maintain_ticks.is_multiple_of(4) {
            let latest_media = self.notch_port.read_now_playing();
            if self.notch.set_media(latest_media) {
                self.recompute_layout();
                dirty = true;
            }
            if let Some(media) = self.notch.media() {
                let track_key = (media.title.clone(), media.artist.clone());
                if !track_key.0.trim().is_empty() && self.lyrics_track.as_ref() != Some(&track_key)
                {
                    self.lyrics_track = Some(track_key.clone());
                    self.lyrics_generation = self.lyrics_generation.wrapping_add(1);
                    let _ = self.lyrics_sender.send(LyricsRequest {
                        generation: self.lyrics_generation,
                        title: track_key.0,
                        artist: track_key.1,
                        duration_ms: media.duration_ms,
                        allow_online: self.online_lyrics_enabled,
                    });
                }
            } else {
                self.lyrics_track = None;
            }
        }
        let pomodoro_active = self.notch.pomodoro().is_compact_active();
        if dirty || pomodoro_active {
            self.ensure_anim_timer();
        }
        if dirty {
            self.present(runtime);
        }
    }

    fn step_hud_animation(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let (cx, cy) = cursor_position();
        let update =
            self.notch
                .advance(now, self.layout.notch.contains(cx, cy), self.panel_visible);
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
            self.stop_anim_timer();
        }
    }

    fn on_hotkey(&mut self, runtime: &RuntimeContext, hotkey_id: i32) {
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
            && let Some(slot) = self.layout.slot_rect(hit)
        {
            crate::menu::open_top_bar_menu(
                runtime,
                self.hwnd,
                hit,
                slot.x,
                self.layout.bar.height + 4,
                self.controls.active_app(),
                self.controls.active_hwnd(),
            );
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

    #[allow(clippy::too_many_lines)]
    fn on_left_click(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        self.ensure_anim_timer();
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        let Some(target) = self.layout.hit_test(x, y) else {
            return;
        };

        if target == HitTarget::MediaSeekbar {
            let seekbar = self.layout.media_seekbar;
            if seekbar.width > 0 {
                let ratio = ((x - seekbar.x) as f32 / seekbar.width as f32).clamp(0.0, 1.0);
                let update = self.notch.handle_seekbar_click(ratio);
                if let Some(NotchEffect::Platform(action)) = update.effect {
                    self.notch_port.execute(action);
                }
                if update.changed {
                    self.recompute_layout();
                    self.present(runtime);
                }
                return;
            }
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

        let Some(action) = BarControlsController::click_action(target) else {
            return;
        };
        match action {
            BarControlAction::TopBarMenu(target) => {
                if crate::menu::active_top_bar_target(runtime) == Some(target) {
                    crate::menu::close(runtime);
                } else if let Some(slot) = self.layout.slot_rect(target) {
                    crate::menu::open_top_bar_menu(
                        runtime,
                        self.hwnd,
                        target,
                        slot.x,
                        self.layout.bar.height + 4,
                        self.controls.active_app(),
                        self.controls.active_hwnd(),
                    );
                }
                self.present(runtime);
            }
            BarControlAction::Tray => {
                let anchor_x = self.layout.tray_slot.x + self.layout.tray_slot.width / 2;
                toggle_tray_overflow(anchor_x, self.layout.bar.height, self.screen.width);
            }
            BarControlAction::ControlCenter => {
                if !self.control_center_enabled {
                    return;
                }
                if crate::menu::is_control_center_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    let right = self
                        .layout
                        .slot_rect(HitTarget::ControlCenter)
                        .map_or(self.screen.width, |slot| slot.right());
                    crate::menu::open_control_center(runtime, right, self.layout.bar.height + 4);
                }
            }
            BarControlAction::QuickSettings => open_quick_settings(),
            BarControlAction::Search => open_spotlight_search(),
            BarControlAction::Clock => {
                if crate::menu::is_calendar_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    let right = self
                        .layout
                        .slot_rect(HitTarget::Clock)
                        .map_or(self.screen.width, |slot| slot.right());
                    crate::menu::open_calendar(runtime, right, self.layout.bar.height + 4);
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
        let anchor_x = if self.layout.hit_test(x, y) == Some(HitTarget::Start) {
            self.layout.start_slot.x
        } else {
            x
        };
        crate::menu::open_top_bar_menu(
            runtime,
            self.hwnd,
            HitTarget::Start,
            anchor_x,
            self.layout.bar.height + 4,
            self.controls.active_app(),
            self.controls.active_hwnd(),
        );
        self.present(runtime);
    }

    fn present(&self, runtime: &RuntimeContext) {
        let hovered = self
            .hovered
            .or_else(|| crate::menu::active_top_bar_target(runtime));
        let theme = crate::theme_state::resolved_theme(runtime);
        let _ = self.renderer.present(
            self.hwnd,
            (0, 0),
            &self.layout,
            self.controls.status(),
            self.controls.active_app(),
            hovered,
            self.notch.hud_state(),
            self.notch.pomodoro(),
            self.notch.media(),
            self.notch.anim_phase(),
            theme,
        );
        if self.panel_visible {
            let _ = self.renderer.present_panel(
                self.panel_hwnd,
                &self.layout,
                hovered,
                self.notch.pomodoro(),
                self.notch.media(),
                self.notch.anim_phase(),
                self.notch.lyrics(),
            );
        }
    }
}
