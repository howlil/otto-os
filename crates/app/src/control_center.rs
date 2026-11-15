use std::time::{Duration, Instant};

use crate::backdrop_feed::BackdropFeed;
use crate::host::{PluginId, RuntimeContext};
use otto_core::{
    BACKDROP_PADDING_PT, BarKey, CaptureRegion, ControlCenterAction, ControlCenterCommand,
    ControlCenterFocus, ControlCenterFocusOutcome, ControlCenterHitTarget, ControlCenterLayout,
    ControlCenterMetrics, ControlCenterPage, ControlCenterPlacement, ControlCenterPort,
    ControlCenterSnapshot, OttoSettingKind, PluginSettingsState, SystemSettingsState,
    UPDATE_CHECKING_LABEL, build_control_center_accessible_tree, capture_interval_for,
    capture_wanted, compute_control_center_layout, control_center_action,
    control_center_focus_targets, decode_control_center_target, next_dock_icon_size, scale,
    scroll_offset_for_item, step_slider_percent, subpanel_opener_target,
};
use otto_platform::{
    MenuNavEvent, PrimaryScreen, WM_APP_CONTROL_CENTER_REFRESH, WM_APP_UIA_INVOKE,
    WindowsControlCenter, cursor_position, handle_wm_getobject, is_running_on_battery,
    monitor_work_area_at, poll_menu_events, primary_screen, release_capture_if_owned,
    route_bar_keys, routes_keys_to, stop_routing_if_targets,
};
use otto_render::ControlCenterRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::SetCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer,
    SetWindowPos, ShowWindow, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP,
    WM_TIMER,
};
use windows::core::Result;

const CC_POLL_TIMER: usize = 31;
const WM_GETOBJECT: u32 = 0x003D;
const CC_POLL_MS: u32 = 16;
const OPEN_CLICK_GRACE: Duration = Duration::from_millis(60);
const WHEEL_SCROLL_STEP: i32 = 36;
const WM_APP_CONTROL_CENTER_KEY: u32 = 0x8000 + 26;
const WM_APP_CONTROL_CENTER_BACKDROP: u32 = 0x8000 + 27;

pub(crate) struct ControlCenterSurface {
    hwnd: isize,
    port: Box<dyn ControlCenterPort>,
    renderer: Option<ControlCenterRenderer>,
    screen: PrimaryScreen,
    metrics: ControlCenterMetrics,
    snapshot: ControlCenterSnapshot,
    layout: ControlCenterLayout,
    anchor_right: i32,
    anchor_y: i32,
    scroll_offset: i32,
    hovered: Option<ControlCenterHitTarget>,
    focus: ControlCenterFocus,
    dragging: Option<ControlCenterHitTarget>,
    visible: bool,
    opened_at: Instant,
    was_mouse_down: bool,
    last_cursor: (i32, i32),
    last_specular_cursor: (i32, i32),
    backdrop: Option<BackdropFeed>,
}

enum PostClickAction {
    None,
    Close,
    Command(ControlCenterCommand, bool),
}

impl ControlCenterSurface {
    fn new(hwnd: isize) -> Self {
        Self {
            hwnd,
            port: Box::new(WindowsControlCenter),
            renderer: None,
            screen: primary_screen(),
            metrics: ControlCenterMetrics::DEFAULT,
            snapshot: ControlCenterSnapshot::default(),
            layout: ControlCenterLayout::default(),
            anchor_right: 0,
            anchor_y: 0,
            scroll_offset: 0,
            hovered: None,
            focus: ControlCenterFocus::default(),
            dragging: None,
            visible: false,
            opened_at: Instant::now(),
            was_mouse_down: false,
            last_cursor: (0, 0),
            last_specular_cursor: (0, 0),
            backdrop: None,
        }
    }

    fn ensure_renderer(&mut self) -> Result<()> {
        if self.renderer.is_none() {
            self.renderer = Some(ControlCenterRenderer::new(self.screen.dpi)?);
        }
        Ok(())
    }

    fn sync_snapshot(&mut self, runtime: &RuntimeContext) {
        self.snapshot.status = self.port.read_system_status();
        self.snapshot.focus_active =
            runtime.focus_module_enabled.get() && runtime.focus_active.get();
        self.snapshot.theme_mode = crate::theme_state::theme_mode(runtime);
        self.snapshot.media = if runtime.media_module_enabled.get() {
            self.port.read_now_playing()
        } else {
            None
        };
        self.snapshot.wifi = self.port.read_wifi_state();
        self.snapshot.bluetooth = self.port.read_bluetooth_state();
        self.snapshot.audio_outputs = self.port.read_audio_outputs();
        self.snapshot.settings.online_lyrics = runtime.online_lyrics_enabled.get();
        self.snapshot.settings.dock = runtime.dock_preferences.get();
        self.snapshot.settings.plugins = PluginSettingsState {
            dock: runtime.dock_enabled.get(),
            window_controls: runtime.window_controls_enabled.get(),
            genie: runtime.genie_enabled.get(),
        };
        self.snapshot.settings.system = SystemSettingsState {
            launch_at_login: self.port.read_launch_at_login(),
            hotkeys_enabled: runtime.hotkeys_enabled.get(),
            capture_visible: runtime.capture_visible.get(),
            classic_logo: runtime.classic_logo_enabled.get(),
            dynamic_island: runtime.dynamic_island_enabled.get(),
            privacy_indicators: runtime.privacy_indicators_enabled.get(),
            liquid_glass: runtime.glass_enabled.get(),
            clipboard_history: runtime.clipboard_history_enabled.get(),
            drop_shelf: runtime.drop_shelf_enabled.get(),
            notch_hud: runtime.notch_hud_enabled.get(),
            media_module: runtime.media_module_enabled.get(),
            focus_module: runtime.focus_module_enabled.get(),
        };
        if let Some(status) = self.port.poll_update_status() {
            self.snapshot.update_status = Some(status);
        }
    }

    fn relayout(&mut self) {
        let page = self.snapshot.page;
        let bounds = monitor_work_area_at(self.anchor_right, self.anchor_y);
        self.layout = compute_control_center_layout(
            &self.metrics,
            ControlCenterPlacement {
                bounds,
                dpi: self.screen.dpi,
                anchor_right: self.anchor_right,
                anchor_y: self.anchor_y,
                page,
                item_count: self.snapshot.subpanel_item_count(page),
                scroll_offset: self.scroll_offset,
            },
        );
        self.scroll_offset = self.layout.scroll_offset;
        if self.focus.is_active() {
            let targets = control_center_focus_targets(&self.layout, &self.snapshot);
            let _ = self.focus.revalidate(&targets);
        }
    }

    fn place(&self) {
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                self.layout.window.x,
                self.layout.window.y,
                self.layout.window.width,
                self.layout.window.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = self.visible
            && self.hwnd != 0
            && !runtime.capture_visible.get()
            && capture_wanted(&appearance, runtime.glass_enabled.get());
        if !wanted {
            if let Some(feed) = self.backdrop.as_mut()
                && feed.is_active()
            {
                feed.reset();
            }
            if let Some(renderer) = self.renderer.as_ref() {
                renderer.clear_backdrop();
            }
            return;
        }
        if self.backdrop.is_none() {
            self.backdrop = Some(BackdropFeed::start(
                self.hwnd,
                WM_APP_CONTROL_CENTER_BACKDROP,
                vec![self.hwnd],
            ));
        }
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.layout.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        let interval = capture_interval_for(is_running_on_battery(), self.visible);
        feed.request(region, interval, force);
    }

    fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (
            update.origin.0 - self.layout.window.x,
            update.origin.1 - self.layout.window.y,
        );
        if self.ensure_renderer().is_ok()
            && let Some(renderer) = self.renderer.as_ref()
        {
            renderer.set_backdrop(update.pixels, local);
        }
        self.present(runtime);
    }

    fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible || self.hwnd == 0 {
            return;
        }
        let appearance = crate::theme_state::appearance(runtime);
        if self.ensure_renderer().is_err() {
            return;
        }
        if let Some(renderer) = self.renderer.as_mut() {
            crate::host::log_present_error(
                "control-center",
                renderer.present(
                    self.hwnd,
                    &self.layout,
                    &self.snapshot,
                    self.hovered,
                    self.focus.current(),
                    &appearance,
                ),
            );
        }
    }

    fn open(
        &mut self,
        runtime: &RuntimeContext,
        page: ControlCenterPage,
        anchor_right: i32,
        anchor_y: i32,
        focused: bool,
    ) {
        let _ = self.leave_keyboard_focus();
        self.screen = primary_screen();
        self.anchor_right = anchor_right;
        self.anchor_y = anchor_y;
        self.snapshot.page = page;
        self.snapshot.audio_switch_failed = false;
        self.scroll_offset = 0;
        self.hovered = None;
        self.dragging = None;
        self.sync_snapshot(runtime);
        self.relayout();
        if focused {
            let _ = self.enter_keyboard_focus();
        }
        self.opened_at = Instant::now();
        self.was_mouse_down = poll_menu_events().0;
        self.last_cursor = cursor_position();
        self.visible = true;
        self.place();
        self.sync_backdrop(runtime, true);
        self.present(runtime);
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                CC_POLL_TIMER,
                CC_POLL_MS,
                None,
            );
        }
    }

    fn hide(&mut self) {
        if !self.visible {
            return;
        }
        let _ = self.leave_keyboard_focus();
        self.visible = false;
        self.hovered = None;
        self.dragging = None;
        self.backdrop = None;
        let _ = release_capture_if_owned(self.hwnd);
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), CC_POLL_TIMER);
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
        }
        self.renderer = None;
    }

    fn enter_keyboard_focus(&mut self) -> bool {
        let targets = control_center_focus_targets(&self.layout, &self.snapshot);
        if self.focus.enter(&targets).is_some()
            && route_bar_keys(self.hwnd, WM_APP_CONTROL_CENTER_KEY)
        {
            self.ensure_focused_visible();
            true
        } else {
            self.focus.exit();
            false
        }
    }

    fn leave_keyboard_focus(&mut self) -> bool {
        stop_routing_if_targets(self.hwnd);
        self.focus.exit()
    }

    fn ensure_focused_visible(&mut self) {
        let Some(target) = self.focus.current() else {
            return;
        };
        if let Some(next_scroll) = scroll_offset_for_item(&self.layout, target) {
            self.scroll_offset = next_scroll;
            self.relayout();
        }
    }

    fn begin_drag(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> bool {
        let Some(target) = self.layout.hit_test(x, y) else {
            return false;
        };
        let is_slider = matches!(
            target,
            ControlCenterHitTarget::DisplaySlider if self.snapshot.status.brightness.is_some()
        ) || target == ControlCenterHitTarget::SoundSlider;
        if !is_slider {
            return false;
        }
        let _ = self.leave_keyboard_focus();
        self.dragging = Some(target);
        unsafe {
            let _ = SetCapture(HWND(self.hwnd as *mut _));
        }
        self.apply_slider(runtime, target, x);
        true
    }

    fn end_drag(&mut self) {
        if self.dragging.take().is_some() {
            let _ = release_capture_if_owned(self.hwnd);
        }
    }

    fn commit_slider_value(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        value: u8,
    ) {
        match target {
            ControlCenterHitTarget::DisplaySlider => {
                if let Some(pct) = self.port.set_brightness(value) {
                    self.snapshot.status.brightness = Some(pct);
                }
            }
            ControlCenterHitTarget::SoundSlider => {
                if let Some((vol, muted)) = self.port.set_volume(value) {
                    self.snapshot.status.volume = vol;
                    self.snapshot.status.muted = muted;
                }
            }
            _ => return,
        }
        self.present(runtime);
    }

    fn apply_slider(&mut self, runtime: &RuntimeContext, target: ControlCenterHitTarget, x: i32) {
        let Some(value) = self.layout.slider_value_at(target, x) else {
            return;
        };
        self.commit_slider_value(runtime, target, value);
    }

    fn step_slider(&mut self, runtime: &RuntimeContext, target: ControlCenterHitTarget, delta: i8) {
        let current = match target {
            ControlCenterHitTarget::DisplaySlider => match self.snapshot.status.brightness {
                Some(pct) => pct,
                None => return,
            },
            ControlCenterHitTarget::SoundSlider => {
                if self.snapshot.status.muted {
                    0
                } else {
                    self.snapshot.status.volume
                }
            }
            _ => return,
        };
        let next = step_slider_percent(current, delta);
        self.commit_slider_value(runtime, target, next);
    }

    fn on_mouse_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.set_pointer(Some((x as f32, y as f32)));
        }
        if let Some(target) = self.dragging {
            self.apply_slider(runtime, target, x);
            return;
        }
        let next = self.layout.hit_test(x, y);
        let cleared_focus = next.is_some() && self.leave_keyboard_focus();
        let specular_moved =
            (x - self.last_specular_cursor.0).abs() + (y - self.last_specular_cursor.1).abs() >= 18;
        if next != self.hovered || cleared_focus || specular_moved {
            self.hovered = next;
            self.last_specular_cursor = (x, y);
            self.present(runtime);
        }
    }

    fn on_wheel(&mut self, runtime: &RuntimeContext, delta: i16) {
        if !self.visible || self.layout.max_scroll <= 0 {
            return;
        }
        let cleared_focus = self.leave_keyboard_focus();
        let step = if delta > 0 {
            -scale(WHEEL_SCROLL_STEP, self.screen.dpi)
        } else {
            scale(WHEEL_SCROLL_STEP, self.screen.dpi)
        };
        let next = (self.scroll_offset + step).clamp(0, self.layout.max_scroll);
        if next != self.scroll_offset || cleared_focus {
            self.scroll_offset = next;
            self.relayout();
            self.present(runtime);
        }
    }

    fn on_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> PostClickAction {
        if self.leave_keyboard_focus() {
            self.present(runtime);
        }
        let Some(target) = self.layout.hit_test(x, y) else {
            return PostClickAction::None;
        };
        let Some(outcome) = control_center_action(&self.snapshot, target) else {
            return PostClickAction::None;
        };
        PostClickAction::Command(outcome.command, outcome.dismiss)
    }

    fn on_key(&mut self, runtime: &RuntimeContext, key: BarKey) -> PostClickAction {
        if !self.visible {
            let _ = self.leave_keyboard_focus();
            return PostClickAction::None;
        }
        let targets = control_center_focus_targets(&self.layout, &self.snapshot);
        match self.focus.handle(key, &targets) {
            ControlCenterFocusOutcome::Moved(_) => {
                self.ensure_focused_visible();
                self.present(runtime);
                PostClickAction::None
            }
            ControlCenterFocusOutcome::Exited => PostClickAction::Close,
            ControlCenterFocusOutcome::AdjustSlider { target, delta } => {
                self.step_slider(runtime, target, delta);
                PostClickAction::None
            }
            ControlCenterFocusOutcome::Activate(target) => {
                let Some(outcome) = control_center_action(&self.snapshot, target) else {
                    return PostClickAction::None;
                };
                PostClickAction::Command(outcome.command, outcome.dismiss)
            }
            ControlCenterFocusOutcome::Unchanged => PostClickAction::None,
        }
    }

    fn on_poll_timer(&mut self, runtime: &RuntimeContext) -> PostClickAction {
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
            return PostClickAction::Close;
        }
        if self.dragging.is_some() {
            return PostClickAction::None;
        }
        self.sync_backdrop(runtime, false);
        if cursor != self.last_cursor {
            self.last_cursor = cursor;
            if inside {
                self.on_mouse_move(
                    runtime,
                    cursor.0 - self.layout.window.x,
                    cursor.1 - self.layout.window.y,
                );
            } else {
                if let Some(renderer) = self.renderer.as_ref() {
                    renderer.set_pointer(None);
                }
                if self.hovered.take().is_some() {
                    self.present(runtime);
                }
            }
        }
        PostClickAction::None
    }

    fn rebuild(&mut self, runtime: &RuntimeContext) {
        if !self.visible {
            return;
        }
        let prev_dpi = self.screen.dpi;
        self.screen = primary_screen();
        if self.screen.dpi != prev_dpi {
            self.renderer = None;
        }
        self.sync_snapshot(runtime);
        self.relayout();
        self.place();
        self.sync_backdrop(runtime, true);
        self.present(runtime);
    }
}

pub fn start(runtime: &RuntimeContext, hwnd: isize) {
    *runtime.control_center.borrow_mut() = Some(ControlCenterSurface::new(hwnd));
}

pub fn stop(runtime: &RuntimeContext) {
    if let Ok(mut guard) = runtime.control_center.try_borrow_mut()
        && let Some(mut surface) = guard.take()
    {
        surface.hide();
    }
}

pub fn close(runtime: &RuntimeContext) {
    with_cc(runtime, ControlCenterSurface::hide);
}

#[must_use]
pub fn is_open(runtime: &RuntimeContext) -> bool {
    runtime
        .control_center
        .try_borrow()
        .ok()
        .and_then(|guard| guard.as_ref().map(|surface| surface.visible))
        .unwrap_or(false)
}

#[must_use]
pub fn active_page(runtime: &RuntimeContext) -> Option<ControlCenterPage> {
    runtime.control_center.try_borrow().ok().and_then(|guard| {
        guard
            .as_ref()
            .filter(|surface| surface.visible)
            .map(|surface| surface.snapshot.page)
    })
}

pub fn open_at_page(
    runtime: &RuntimeContext,
    page: ControlCenterPage,
    anchor_right: i32,
    anchor_y: i32,
) {
    crate::menu::close(runtime);
    with_cc(runtime, |surface| {
        surface.open(runtime, page, anchor_right, anchor_y, false);
    });
}

pub fn open_at_page_focused(
    runtime: &RuntimeContext,
    page: ControlCenterPage,
    anchor_right: i32,
    anchor_y: i32,
) {
    crate::menu::close(runtime);
    with_cc(runtime, |surface| {
        surface.open(runtime, page, anchor_right, anchor_y, true);
    });
}

pub fn repaint(runtime: &RuntimeContext) {
    with_cc(runtime, |surface| {
        if surface.visible {
            surface.sync_snapshot(runtime);
            surface.relayout();
            surface.place();
            surface.sync_backdrop(runtime, true);
            surface.present(runtime);
        }
    });
}

pub fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_cc(runtime, |surface| {
        if !enabled {
            surface.backdrop = None;
            if let Some(renderer) = surface.renderer.as_ref() {
                renderer.clear_backdrop();
            }
        }
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub fn set_capture_visible(runtime: &RuntimeContext, _visible: bool) {
    with_cc(runtime, |surface| {
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_GETOBJECT => runtime
            .control_center
            .try_borrow()
            .ok()
            .and_then(|guard| {
                let surface = guard.as_ref().filter(|surface| surface.visible)?;
                handle_wm_getobject(hwnd, wparam.0, lparam.0, WM_APP_UIA_INVOKE, || {
                    build_control_center_accessible_tree(&surface.layout, &surface.snapshot)
                })
            })
            .map(LRESULT),
        WM_APP_UIA_INVOKE => {
            if let Some(target) = i32::try_from(wparam.0)
                .ok()
                .and_then(decode_control_center_target)
            {
                let mut post = PostClickAction::None;
                with_cc(runtime, |surface| {
                    if surface.visible
                        && let Some(outcome) = control_center_action(&surface.snapshot, target)
                    {
                        post = PostClickAction::Command(outcome.command, outcome.dismiss);
                    }
                });
                apply_post_action(runtime, post);
            }
            Some(LRESULT(0))
        }
        WM_APP_CONTROL_CENTER_KEY => {
            if let Some(key) = BarKey::from_code(wparam.0) {
                let mut post = PostClickAction::None;
                with_cc(runtime, |surface| post = surface.on_key(runtime, key));
                apply_post_action(runtime, post);
            }
            Some(LRESULT(0))
        }
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_TIMER if wparam.0 == CC_POLL_TIMER => {
            let mut post = PostClickAction::None;
            with_cc(runtime, |surface| post = surface.on_poll_timer(runtime));
            apply_post_action(runtime, post);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_cc(runtime, |surface| surface.on_mouse_move(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            with_cc(runtime, |surface| surface.on_wheel(runtime, delta));
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let mut end_drag = false;
            with_cc(runtime, |surface| {
                end_drag = surface.dragging.is_some();
                surface.end_drag();
            });
            if !end_drag {
                let (x, y) = unpack_point(lparam);
                let mut post = PostClickAction::None;
                with_cc(runtime, |surface| post = surface.on_click(runtime, x, y));
                apply_post_action(runtime, post);
            }
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_cc(runtime, |surface| surface.dragging = None);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_cc(runtime, |surface| {
                let _ = surface.begin_drag(runtime, x, y);
            });
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => Some(LRESULT(0)),
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_cc(runtime, |surface| surface.rebuild(runtime));
            Some(LRESULT(0))
        }
        WM_APP_CONTROL_CENTER_REFRESH => {
            repaint(runtime);
            Some(LRESULT(0))
        }
        WM_APP_CONTROL_CENTER_BACKDROP => {
            with_cc(runtime, |surface| surface.on_backdrop(runtime));
            Some(LRESULT(0))
        }
        WM_CLOSE => {
            with_cc(runtime, ControlCenterSurface::hide);
            Some(LRESULT(0))
        }
        _ => None,
    }
}

fn apply_post_action(runtime: &RuntimeContext, action: PostClickAction) {
    match action {
        PostClickAction::None => {}
        PostClickAction::Close => close(runtime),
        PostClickAction::Command(command, dismiss) => {
            execute_command(runtime, command);
            if dismiss {
                close(runtime);
            } else {
                refresh_and_present(runtime);
            }
        }
    }
}

fn execute_command(runtime: &RuntimeContext, command: ControlCenterCommand) {
    match command {
        ControlCenterCommand::OpenPage(page) => navigate_open_page(runtime, page),
        ControlCenterCommand::Back => navigate_back(runtime),
        ControlCenterCommand::SelectOttoCategory(category) => {
            select_otto_category(runtime, category);
        }
        ControlCenterCommand::ToggleWifi
        | ControlCenterCommand::ConnectWifi(_)
        | ControlCenterCommand::ToggleBluetooth
        | ControlCenterCommand::ToggleBluetoothDevice(_, _) => {
            execute_connectivity_command(runtime, command);
        }
        ControlCenterCommand::ToggleFocus => {
            if runtime.focus_module_enabled.get() {
                crate::bar::toggle_focus_mode(runtime);
            }
        }
        ControlCenterCommand::CycleTheme => {
            let _ = crate::theme_state::cycle_theme_mode(runtime);
        }
        ControlCenterCommand::SetThemeMode(mode) => {
            crate::theme_state::set_theme_mode(runtime, mode);
        }
        ControlCenterCommand::SetBrightness(pct) => {
            with_cc(runtime, |surface| {
                if let Some(next) = surface.port.set_brightness(pct) {
                    surface.snapshot.status.brightness = Some(next);
                }
            });
        }
        ControlCenterCommand::SetVolume(pct) => {
            with_cc(runtime, |surface| {
                if let Some((vol, muted)) = surface.port.set_volume(pct) {
                    surface.snapshot.status.volume = vol;
                    surface.snapshot.status.muted = muted;
                }
            });
        }
        ControlCenterCommand::ToggleMute => {
            with_cc(runtime, |surface| {
                let next = !surface.snapshot.status.muted;
                if let Some((vol, muted)) = surface.port.set_muted(next) {
                    surface.snapshot.status.volume = vol;
                    surface.snapshot.status.muted = muted;
                }
            });
        }
        ControlCenterCommand::SelectAudioOutput(id) => {
            with_cc(runtime, |surface| {
                surface.snapshot.audio_switch_failed = !surface.port.set_audio_output(&id);
            });
        }
        ControlCenterCommand::Media(cmd) => {
            if runtime.media_module_enabled.get() {
                with_cc(runtime, |surface| {
                    surface.port.execute(ControlCenterAction::Media(cmd));
                });
            }
        }
        ControlCenterCommand::OpenNotchMedia => {
            if runtime.media_module_enabled.get() {
                crate::bar::open_notch_media(runtime);
            }
        }
        ControlCenterCommand::ToggleSetting(kind) => toggle_setting(runtime, kind),
        ControlCenterCommand::CycleDockIconSize => {
            let mut prefs = runtime.dock_preferences.get();
            prefs.icon_size = next_dock_icon_size(prefs.icon_size);
            runtime.pending_dock_preferences.set(Some(prefs));
        }
        ControlCenterCommand::OpenSettings(action) => {
            with_cc(runtime, |surface| surface.port.execute(action));
        }
        ControlCenterCommand::OpenLogsFolder => {
            with_cc(runtime, |surface| surface.port.open_logs_folder());
        }
        ControlCenterCommand::CheckForUpdates => {
            with_cc(runtime, |surface| {
                if surface.port.start_update_check() {
                    surface.snapshot.update_status = Some(String::from(UPDATE_CHECKING_LABEL));
                } else if let Some(cached) = surface.port.poll_update_status() {
                    surface.snapshot.update_status = Some(cached);
                } else {
                    surface.snapshot.update_status = surface.port.check_for_updates();
                }
            });
        }
        ControlCenterCommand::OpenReleasesPage => {
            with_cc(runtime, |surface| surface.port.open_releases_page());
        }
    }
}

fn navigate_open_page(runtime: &RuntimeContext, page: ControlCenterPage) {
    with_cc(runtime, |surface| {
        surface.snapshot.page = page;
        surface.snapshot.audio_switch_failed = false;
        surface.scroll_offset = 0;
        surface.hovered = None;
        surface.sync_snapshot(runtime);
        surface.relayout();
        if surface.focus.is_active() {
            let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
            let _ = surface.focus.enter(&targets);
            surface.ensure_focused_visible();
        }
        surface.place();
        surface.present(runtime);
    });
}

fn navigate_back(runtime: &RuntimeContext) {
    with_cc(runtime, |surface| {
        let prev_page = surface.snapshot.page;
        surface.snapshot.page = ControlCenterPage::Main;
        surface.scroll_offset = 0;
        surface.hovered = None;
        surface.sync_snapshot(runtime);
        surface.relayout();
        if surface.focus.is_active() {
            let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
            if let Some(preferred) = subpanel_opener_target(prev_page) {
                let _ = surface.focus.enter_target(&targets, preferred);
            } else {
                let _ = surface.focus.enter(&targets);
            }
            surface.ensure_focused_visible();
        }
        surface.place();
        surface.present(runtime);
    });
}

fn select_otto_category(runtime: &RuntimeContext, category: otto_core::OttoSettingsCategory) {
    with_cc(runtime, |surface| {
        if surface.snapshot.otto_category != category {
            surface.snapshot.otto_category = category;
            surface.scroll_offset = 0;
            surface.hovered = None;
            surface.relayout();
            surface.place();
            surface.present(runtime);
        }
    });
}

fn execute_connectivity_command(runtime: &RuntimeContext, command: ControlCenterCommand) {
    match command {
        ControlCenterCommand::ToggleWifi => {
            with_cc(runtime, |surface| {
                let next = !surface.snapshot.wifi.enabled;
                if surface.port.set_wifi_enabled(next) {
                    surface.snapshot.wifi.enabled = next;
                }
            });
        }
        ControlCenterCommand::ConnectWifi(ssid) => {
            with_cc(runtime, |surface| {
                if !surface.port.connect_wifi(&ssid) {
                    surface.port.execute(ControlCenterAction::OpenWifi);
                }
            });
        }
        ControlCenterCommand::ToggleBluetooth => {
            with_cc(runtime, |surface| {
                let next = !surface.snapshot.bluetooth.enabled;
                if surface.port.set_bluetooth_enabled(next) {
                    surface.snapshot.bluetooth.enabled = next;
                }
            });
        }
        ControlCenterCommand::ToggleBluetoothDevice(id, connect) => {
            with_cc(runtime, |surface| {
                let _ = surface.port.toggle_bluetooth_device(&id, connect);
            });
        }
        _ => {}
    }
}

fn toggle_flag(flag: &std::cell::Cell<bool>, pending: &std::cell::Cell<Option<bool>>) {
    pending.set(Some(!flag.get()));
}

fn toggle_dock_setting(runtime: &RuntimeContext, kind: OttoSettingKind) {
    let mut prefs = runtime.dock_preferences.get();
    match kind {
        OttoSettingKind::DockHideTaskbar => prefs.hide_taskbar = !prefs.hide_taskbar,
        OttoSettingKind::DockShowDownloads => prefs.show_downloads = !prefs.show_downloads,
        OttoSettingKind::DockShowTrash => prefs.show_trash = !prefs.show_trash,
        OttoSettingKind::DockAutohide => prefs.autohide = !prefs.autohide,
        OttoSettingKind::DockMagnification => prefs.magnification = !prefs.magnification,
        _ => return,
    }
    runtime.pending_dock_preferences.set(Some(prefs));
}

fn toggle_setting(runtime: &RuntimeContext, kind: OttoSettingKind) {
    match kind {
        OttoSettingKind::DynamicIsland => {
            toggle_flag(
                &runtime.dynamic_island_enabled,
                &runtime.pending_dynamic_island,
            );
        }
        OttoSettingKind::Dock => {
            runtime
                .pending_plugin_toggle
                .set(Some((PluginId::Dock, !runtime.dock_enabled.get())));
        }
        OttoSettingKind::WindowControls => {
            runtime.pending_plugin_toggle.set(Some((
                PluginId::WindowControls,
                !runtime.window_controls_enabled.get(),
            )));
        }
        OttoSettingKind::Genie => {
            runtime
                .pending_plugin_toggle
                .set(Some((PluginId::Genie, !runtime.genie_enabled.get())));
        }
        OttoSettingKind::LiquidGlass => {
            toggle_flag(&runtime.glass_enabled, &runtime.pending_liquid_glass);
        }
        OttoSettingKind::PrivacyIndicators => {
            toggle_flag(
                &runtime.privacy_indicators_enabled,
                &runtime.pending_privacy_indicators,
            );
        }
        OttoSettingKind::ClipboardHistory => {
            toggle_flag(
                &runtime.clipboard_history_enabled,
                &runtime.pending_clipboard_history,
            );
        }
        OttoSettingKind::DropShelf => {
            toggle_flag(&runtime.drop_shelf_enabled, &runtime.pending_drop_shelf);
        }
        OttoSettingKind::NotchHud => {
            toggle_flag(&runtime.notch_hud_enabled, &runtime.pending_notch_hud);
        }
        OttoSettingKind::MediaModule => {
            toggle_flag(&runtime.media_module_enabled, &runtime.pending_media_module);
        }
        OttoSettingKind::FocusModule => {
            toggle_flag(&runtime.focus_module_enabled, &runtime.pending_focus_module);
        }
        OttoSettingKind::DockHideTaskbar
        | OttoSettingKind::DockShowDownloads
        | OttoSettingKind::DockShowTrash
        | OttoSettingKind::DockAutohide
        | OttoSettingKind::DockMagnification => toggle_dock_setting(runtime, kind),
        OttoSettingKind::CaptureVisible => {
            toggle_flag(&runtime.capture_visible, &runtime.pending_capture_visible);
        }
        OttoSettingKind::OnlineLyrics => {
            toggle_flag(
                &runtime.online_lyrics_enabled,
                &runtime.pending_online_lyrics,
            );
        }
        OttoSettingKind::LaunchAtLogin => {
            with_cc(runtime, |surface| {
                let next = !surface.port.read_launch_at_login();
                let _ = surface.port.set_launch_at_login(next);
            });
        }
        OttoSettingKind::Hotkeys => {
            toggle_flag(&runtime.hotkeys_enabled, &runtime.pending_hotkeys_enabled);
        }
        OttoSettingKind::ClassicLogo => {
            toggle_flag(&runtime.classic_logo_enabled, &runtime.pending_classic_logo);
        }
    }
}

fn refresh_and_present(runtime: &RuntimeContext) {
    with_cc(runtime, |surface| {
        if surface.visible {
            surface.sync_snapshot(runtime);
            surface.relayout();
            surface.place();
            surface.present(runtime);
        }
    });
}

fn with_cc(runtime: &RuntimeContext, action: impl FnOnce(&mut ControlCenterSurface)) {
    if let Ok(mut guard) = runtime.control_center.try_borrow_mut()
        && let Some(surface) = guard.as_mut()
    {
        action(surface);
    }
}

fn poll_dismiss_action(
    nav_event: Option<MenuNavEvent>,
    mouse_down: bool,
    was_mouse_down: &mut bool,
    grace_elapsed: bool,
    inside_window: bool,
) -> bool {
    let pressed_now = mouse_down && !*was_mouse_down;
    *was_mouse_down = mouse_down;
    if matches!(nav_event, Some(MenuNavEvent::Escape)) {
        return true;
    }
    grace_elapsed && pressed_now && !inside_window
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use otto_core::{
        BarKey, ControlCenterAction, ControlCenterCommand, ControlCenterHitTarget,
        ControlCenterPage, ControlCenterPort, NowPlaying, OttoSettingKind, OttoSettingsSnapshot,
        SystemStatus, UPDATE_CHECKING_LABEL, control_center_focus_targets,
    };
    use otto_platform::MenuNavEvent;

    use crate::host::RuntimeContext;

    use super::{
        ControlCenterSurface, PostClickAction, execute_command, poll_dismiss_action, toggle_setting,
    };

    #[derive(Default)]
    struct MockControlCenterState {
        brightness: Option<u8>,
        volume: u8,
        muted: bool,
        update_starts: usize,
        update_status: Option<String>,
        opened_releases: bool,
    }

    struct MockControlCenterPort(Arc<Mutex<MockControlCenterState>>);

    impl ControlCenterPort for MockControlCenterPort {
        fn read_system_status(&self) -> SystemStatus {
            let state = self.0.lock().unwrap();
            SystemStatus {
                brightness: state.brightness,
                volume: state.volume,
                muted: state.muted,
                ..SystemStatus::default()
            }
        }

        fn read_now_playing(&self) -> Option<NowPlaying> {
            None
        }

        fn set_volume(&self, percent: u8) -> Option<(u8, bool)> {
            let mut state = self.0.lock().unwrap();
            state.volume = percent;
            state.muted = percent == 0;
            Some((state.volume, state.muted))
        }

        fn set_muted(&self, muted: bool) -> Option<(u8, bool)> {
            let mut state = self.0.lock().unwrap();
            state.muted = muted;
            Some((state.volume, state.muted))
        }

        fn set_brightness(&self, percent: u8) -> Option<u8> {
            let mut state = self.0.lock().unwrap();
            state.brightness = Some(percent);
            state.brightness
        }

        fn execute(&self, _action: ControlCenterAction) {}

        fn start_update_check(&self) -> bool {
            let mut state = self.0.lock().unwrap();
            state.update_starts += 1;
            state.update_status = Some(String::from(UPDATE_CHECKING_LABEL));
            true
        }

        fn poll_update_status(&self) -> Option<String> {
            self.0.lock().unwrap().update_status.clone()
        }

        fn open_releases_page(&self) {
            self.0.lock().unwrap().opened_releases = true;
        }
    }

    #[test]
    fn inside_click_and_held_drag_stay_open_while_escape_and_outside_click_dismiss() {
        let mut was_down = true;
        assert!(!poll_dismiss_action(None, true, &mut was_down, true, false));
        assert!(was_down);

        assert!(!poll_dismiss_action(None, false, &mut was_down, true, true));
        assert!(!was_down);

        assert!(!poll_dismiss_action(None, true, &mut was_down, true, true));
        assert!(was_down);

        assert!(!poll_dismiss_action(
            None,
            false,
            &mut was_down,
            true,
            false
        ));
        assert!(!poll_dismiss_action(
            None,
            true,
            &mut was_down,
            false,
            false
        ));
        assert!(!poll_dismiss_action(
            None,
            false,
            &mut was_down,
            true,
            false
        ));
        assert!(poll_dismiss_action(None, true, &mut was_down, true, false));

        assert!(poll_dismiss_action(
            Some(MenuNavEvent::Escape),
            false,
            &mut was_down,
            false,
            true,
        ));
    }

    #[test]
    fn async_update_check_syncs_status_and_opens_releases_when_available() {
        let state = Arc::new(Mutex::new(MockControlCenterState {
            brightness: Some(60),
            volume: 50,
            ..MockControlCenterState::default()
        }));
        let runtime = RuntimeContext::new(false);
        let mut surface = ControlCenterSurface::new(0);
        surface.port = Box::new(MockControlCenterPort(Arc::clone(&state)));
        surface.snapshot.page = ControlCenterPage::OttoSettings;
        surface.visible = true;
        surface.sync_snapshot(&runtime);
        surface.relayout();
        *runtime.control_center.borrow_mut() = Some(surface);

        execute_command(&runtime, ControlCenterCommand::CheckForUpdates);
        assert_eq!(state.lock().unwrap().update_starts, 1);
        assert_eq!(
            runtime
                .control_center
                .borrow()
                .as_ref()
                .unwrap()
                .snapshot
                .update_status
                .as_deref(),
            Some(UPDATE_CHECKING_LABEL)
        );

        state.lock().unwrap().update_status = Some(String::from("Update v0.4.0"));
        {
            let mut guard = runtime.control_center.borrow_mut();
            let surface = guard.as_mut().unwrap();
            surface.sync_snapshot(&runtime);
            assert_eq!(
                surface.snapshot.update_status.as_deref(),
                Some("Update v0.4.0")
            );
            let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
            surface
                .focus
                .enter_target(&targets, ControlCenterHitTarget::FooterSecondary);
            match surface.on_key(&runtime, BarKey::Activate) {
                PostClickAction::Command(ControlCenterCommand::OpenReleasesPage, true) => {}
                _ => panic!("expected OpenReleasesPage dismiss action"),
            }
        }
        execute_command(&runtime, ControlCenterCommand::OpenReleasesPage);
        assert!(state.lock().unwrap().opened_releases);
    }

    #[test]
    fn keyboard_navigation_steps_sliders_scrolls_list_and_clears_on_pointer_hover() {
        let state = Arc::new(Mutex::new(MockControlCenterState {
            brightness: Some(60),
            volume: 50,
            ..MockControlCenterState::default()
        }));
        let runtime = RuntimeContext::new(false);
        let mut surface = ControlCenterSurface::new(0);
        surface.port = Box::new(MockControlCenterPort(Arc::clone(&state)));
        surface.snapshot.page = ControlCenterPage::Main;
        surface.visible = true;
        surface.sync_snapshot(&runtime);
        surface.relayout();

        let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
        assert_eq!(
            surface
                .focus
                .enter_target(&targets, ControlCenterHitTarget::DisplaySlider),
            Some(ControlCenterHitTarget::DisplaySlider)
        );
        assert!(matches!(
            surface.on_key(&runtime, BarKey::Next),
            PostClickAction::None
        ));
        assert_eq!(surface.snapshot.status.brightness, Some(65));
        assert_eq!(
            surface.focus.current(),
            Some(ControlCenterHitTarget::DisplaySlider)
        );
        assert!(matches!(
            surface.on_key(&runtime, BarKey::Previous),
            PostClickAction::None
        ));
        assert_eq!(surface.snapshot.status.brightness, Some(60));

        surface.snapshot.page = ControlCenterPage::OttoSettings;
        surface.scroll_offset = 0;
        surface.relayout();
        let settings_targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
        let target_idx = OttoSettingsSnapshot::row_count() - 2;
        assert_eq!(
            surface
                .focus
                .enter_target(&settings_targets, ControlCenterHitTarget::FooterSecondary),
            Some(ControlCenterHitTarget::FooterSecondary)
        );
        assert!(matches!(
            surface.on_key(&runtime, BarKey::Up),
            PostClickAction::None
        ));
        assert!(matches!(
            surface.on_key(&runtime, BarKey::Up),
            PostClickAction::None
        ));
        assert_eq!(
            surface.focus.current(),
            Some(ControlCenterHitTarget::SubPanelItem(
                OttoSettingsSnapshot::row_count() - 1
            ))
        );
        assert!(surface.scroll_offset > 0);
        let item_rect = surface.layout.items[target_idx];
        surface.on_mouse_move(&runtime, item_rect.x + 8, item_rect.y + 8);
        assert!(!surface.focus.is_active());
    }

    #[test]
    fn category_switching_and_new_module_toggles_queue_expected_runtime_updates() {
        let runtime = RuntimeContext::new(false);
        runtime.glass_enabled.set(true);
        let mut surface = ControlCenterSurface::new(0);
        surface.port = Box::new(MockControlCenterPort(Arc::new(Mutex::new(
            MockControlCenterState::default(),
        ))));
        surface.snapshot.page = ControlCenterPage::OttoSettings;
        surface.visible = true;
        surface.sync_snapshot(&runtime);
        surface.relayout();
        let all_height = surface.layout.window.height;
        *runtime.control_center.borrow_mut() = Some(surface);

        execute_command(
            &runtime,
            ControlCenterCommand::SelectOttoCategory(otto_core::OttoSettingsCategory::System),
        );
        {
            let guard = runtime.control_center.borrow();
            let cc = guard.as_ref().unwrap();
            assert_eq!(
                cc.snapshot.otto_category,
                otto_core::OttoSettingsCategory::System
            );
            assert_eq!(cc.scroll_offset, 0);
            assert!(cc.layout.window.height < all_height);
        }

        toggle_setting(&runtime, OttoSettingKind::LiquidGlass);
        toggle_setting(&runtime, OttoSettingKind::PrivacyIndicators);
        toggle_setting(&runtime, OttoSettingKind::ClipboardHistory);
        toggle_setting(&runtime, OttoSettingKind::DropShelf);
        toggle_setting(&runtime, OttoSettingKind::NotchHud);
        toggle_setting(&runtime, OttoSettingKind::MediaModule);
        toggle_setting(&runtime, OttoSettingKind::FocusModule);
        toggle_setting(&runtime, OttoSettingKind::DockHideTaskbar);

        assert_eq!(runtime.pending_liquid_glass.take(), Some(false));
        assert_eq!(runtime.pending_privacy_indicators.take(), Some(false));
        assert_eq!(runtime.pending_clipboard_history.take(), Some(false));
        assert_eq!(runtime.pending_drop_shelf.take(), Some(false));
        assert_eq!(runtime.pending_notch_hud.take(), Some(false));
        assert_eq!(runtime.pending_media_module.take(), Some(false));
        assert_eq!(runtime.pending_focus_module.take(), Some(false));
        let dock_prefs = runtime.pending_dock_preferences.take().unwrap();
        assert!(!dock_prefs.hide_taskbar);

        runtime.media_module_enabled.set(false);
        runtime.focus_module_enabled.set(false);
        runtime.focus_active.set(true);
        {
            let mut guard = runtime.control_center.borrow_mut();
            let cc = guard.as_mut().unwrap();
            cc.sync_snapshot(&runtime);
            assert!(cc.snapshot.media.is_none());
            assert!(!cc.snapshot.focus_active);
            assert!(!cc.snapshot.settings.system.media_module);
            assert!(!cc.snapshot.settings.system.focus_module);
        }
    }
}
