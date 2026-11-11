use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::size_of;
use std::time::Instant;

use otto_core::{
    CMD_DOCK_EMPTY_TRASH, CMD_DOCK_NEW_WINDOW, CMD_DOCK_PIN, CMD_DOCK_QUIT, CMD_DOCK_TOGGLE,
    CMD_DOCK_WINDOW_BASE, DockInput, DockLayout, DockMetrics, MotionMetrics, MotionPreference,
    Rect, advance_bounce_phase, animate_value, bounce_offset, compute_dock_layout, scale,
};
use otto_platform::{
    DockEntry, DockItemKind, PrimaryScreen, activate_or_launch, close_entry_windows,
    collect_dock_entries, current_process_id, cursor_position, empty_recycle_bin,
    extract_dock_icon, focus_window_hwnd, has_maximized_or_fullscreen_window, is_window_foreground,
    is_window_minimized, launch_new_instance, motion_preference, primary_screen,
    shutdown_recycle_bin_task, toggle_pin_entry,
};
use otto_render::{DockRenderItem, DockRenderer};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer, SetWindowPos,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP,
    WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const DOCK_POLL_TIMER: usize = 10;
const DOCK_ANIM_TIMER: usize = 11;
const DOCK_POLL_MS: u32 = 150;

thread_local! {
    static DOCK: RefCell<Option<Dock>> = const { RefCell::new(None) };
}

struct AnimState {
    mag_strength: f32,
    hide_progress: f32,
    should_hide: bool,
    bounce_key: Option<String>,
    bounce_phase: f32,
    timer_running: bool,
    last_tick: Instant,
}

impl Default for AnimState {
    fn default() -> Self {
        Self {
            mag_strength: 0.0,
            hide_progress: 0.0,
            should_hide: false,
            bounce_key: None,
            bounce_phase: 0.0,
            timer_running: false,
            last_tick: Instant::now(),
        }
    }
}

struct Dock {
    hwnd: isize,
    own_pid: u32,
    renderer: DockRenderer,
    screen: PrimaryScreen,
    metrics: DockMetrics,
    entries: Vec<DockEntry>,
    closing_entries: Vec<DockEntry>,
    presence: HashMap<String, f32>,
    layout: DockLayout,
    hovering: bool,
    last_cursor_x: Option<f32>,
    tracking_mouse: bool,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    anim: AnimState,
}

pub fn start(hwnd: isize) -> Result<()> {
    let own_pid = current_process_id();
    let screen = primary_screen();
    let metrics = DockMetrics::default();
    let motion = motion_preference();
    let motion_metrics = MotionMetrics::default();
    let entries = collect_dock_entries(own_pid);
    let mut presence = HashMap::new();
    for entry in &entries {
        presence.insert(entry.key.clone(), 1.0);
    }
    let (app_count, has_trash) = count_entries(&entries);
    let layout = compute_dock_layout(
        &metrics,
        DockInput {
            screen_width: screen.width,
            dpi: screen.dpi,
            app_count,
            has_trash,
            cursor_x: None,
            mag_strength: 0.0,
            bounce_offsets: &[],
            item_presence: &[],
            hide_progress: 0.0,
        },
    );
    let mut renderer = DockRenderer::new(layout.window_width, layout.window_height, screen.dpi)?;
    sync_icons(&mut renderer, &entries);

    let dock = Dock {
        hwnd,
        own_pid,
        renderer,
        screen,
        metrics,
        entries,
        closing_entries: Vec::new(),
        presence,
        layout,
        hovering: false,
        last_cursor_x: None,
        tracking_mouse: false,
        motion,
        motion_metrics,
        anim: AnimState::default(),
    };
    dock.place();
    dock.present();

    unsafe {
        SetTimer(
            Some(HWND(hwnd as *mut _)),
            DOCK_POLL_TIMER,
            DOCK_POLL_MS,
            None,
        );
    }

    DOCK.with(|cell| *cell.borrow_mut() = Some(dock));
    Ok(())
}

pub fn stop() {
    shutdown_recycle_bin_task();
    DOCK.with(|cell| {
        if let Some(dock) = cell.borrow_mut().take() {
            unsafe {
                let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_POLL_TIMER);
                let _ = KillTimer(Some(HWND(dock.hwnd as *mut _)), DOCK_ANIM_TIMER);
            }
        }
    });
}

pub fn handle_message(
    _hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_TIMER if wparam.0 == DOCK_POLL_TIMER => {
            with_dock(Dock::on_poll_timer);
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == DOCK_ANIM_TIMER => {
            with_dock(Dock::on_anim_timer);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_dock(|dock| dock.on_mouse_move(x, y));
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_dock(Dock::on_mouse_leave);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_dock(|dock| dock.on_left_click(x, y));
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_dock(|dock| dock.on_right_click(x, y));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_dock(Dock::rebuild);
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

fn with_dock(action: impl FnOnce(&mut Dock)) {
    DOCK.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut()
            && let Some(dock) = guard.as_mut()
        {
            action(dock);
        }
    });
}

pub fn repaint() {
    with_dock(|dock| dock.present());
}

#[must_use]
pub fn dock_target_for_window(hwnd: isize) -> Option<Rect> {
    DOCK.with(|cell| {
        let guard = cell.try_borrow().ok()?;
        let dock = guard.as_ref()?;
        let entries = dock.combined_entries();
        let index = entries
            .iter()
            .position(|entry| entry.hwnds.contains(&hwnd))?;
        let item = dock.layout.items.get(index)?;
        let (origin_x, origin_y) = dock.window_origin();
        let size = item.size.round().max(1.0) as i32;
        Some(Rect {
            x: origin_x + item.x.round() as i32,
            y: origin_y + item.y.round() as i32,
            width: size,
            height: size,
        })
    })
}

fn activate_entry(entry: &DockEntry, target: Option<Rect>) {
    if entry.hwnds.len() == 1
        && let Some(&hwnd) = entry.hwnds.first()
    {
        if is_window_minimized(hwnd) {
            if crate::genie::restore_window_to(hwnd, target) {
                return;
            }
        } else if is_window_foreground(hwnd) && crate::genie::minimize_window_to(hwnd, target) {
            return;
        }
    } else if entry.hwnds.len() > 1
        && !entry.hwnds.iter().copied().any(is_window_foreground)
        && let Some(&hwnd) = entry.hwnds.first()
        && is_window_minimized(hwnd)
        && crate::genie::restore_window_to(hwnd, target)
    {
        return;
    }
    activate_or_launch(entry);
}

pub fn execute_dock_menu_command(cmd: usize, entry: &DockEntry, target: Option<Rect>) {
    if cmd >= CMD_DOCK_WINDOW_BASE {
        let idx = cmd - CMD_DOCK_WINDOW_BASE;
        if let Some(&hwnd) = entry.hwnds.get(idx) {
            if is_window_minimized(hwnd) {
                if !crate::genie::restore_window_to(hwnd, target) {
                    focus_window_hwnd(hwnd);
                }
            } else {
                focus_window_hwnd(hwnd);
            }
        }
        return;
    }
    match cmd {
        CMD_DOCK_PIN => {
            toggle_pin_entry(entry);
            with_dock(|dock| {
                dock.entries = collect_dock_entries(dock.own_pid);
                let updated = dock.combined_entries();
                sync_icons(&mut dock.renderer, &updated);
                dock.recompute_layout();
                dock.present();
            });
        }
        CMD_DOCK_TOGGLE => activate_entry(entry, target),
        CMD_DOCK_NEW_WINDOW => launch_new_instance(entry),
        CMD_DOCK_EMPTY_TRASH => empty_recycle_bin(),
        CMD_DOCK_QUIT => close_entry_windows(entry),
        _ => {}
    }
}

fn count_entries(entries: &[DockEntry]) -> (usize, bool) {
    let has_trash = entries
        .last()
        .is_some_and(|e| matches!(e.kind, DockItemKind::Trash { .. }));
    let app_count = entries.len().saturating_sub(usize::from(has_trash));
    (app_count, has_trash)
}

fn sync_icons(renderer: &mut DockRenderer, entries: &[DockEntry]) {
    for entry in entries {
        if entry.apple_icon.is_none()
            && !renderer.has_icon(&entry.key)
            && let Some(bmp) = extract_dock_icon(entry)
        {
            let _ = renderer.set_icon(entry.key.clone(), bmp.width, bmp.height, &bmp.pixels);
        }
    }
    let alive: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    renderer.retain_icons(&alive);
}

impl Dock {
    fn window_origin(&self) -> (i32, i32) {
        (0, self.screen.height - self.layout.window_height)
    }

    fn combined_entries(&self) -> Vec<DockEntry> {
        if self.closing_entries.is_empty() {
            return self.entries.clone();
        }
        let mut list = Vec::with_capacity(self.entries.len() + self.closing_entries.len());
        let has_trash = self
            .entries
            .last()
            .is_some_and(|e| matches!(e.kind, DockItemKind::Trash { .. }));
        let app_slice = if has_trash {
            &self.entries[..self.entries.len() - 1]
        } else {
            &self.entries[..]
        };
        list.extend_from_slice(app_slice);
        for closing in &self.closing_entries {
            if !list.iter().any(|e| e.key == closing.key) {
                list.push(closing.clone());
            }
        }
        if has_trash && let Some(trash) = self.entries.last() {
            list.push(trash.clone());
        }
        list
    }

    fn place(&self) {
        let (ox, oy) = self.window_origin();
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                ox,
                oy,
                self.layout.window_width,
                self.layout.window_height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    fn rebuild(&mut self) {
        self.screen = primary_screen();
        self.recompute_layout();
        let all = self.combined_entries();
        if let Ok(mut renderer) = DockRenderer::new(
            self.layout.window_width,
            self.layout.window_height,
            self.screen.dpi,
        ) {
            sync_icons(&mut renderer, &all);
            self.renderer = renderer;
        }
        self.place();
        self.present();
    }

    fn on_poll_timer(&mut self) {
        let mut dirty = false;
        let next_motion = motion_preference();
        if next_motion != self.motion {
            self.motion = next_motion;
            self.ensure_animating();
        }
        let latest = collect_dock_entries(self.own_pid);
        if latest != self.entries {
            let mut need_anim = false;
            for new_e in &latest {
                if !self.entries.iter().any(|old_e| old_e.key == new_e.key) {
                    self.presence.insert(new_e.key.clone(), 0.08);
                    need_anim = true;
                }
                if new_e.running
                    && !matches!(new_e.kind, DockItemKind::Trash { .. })
                    && !self
                        .entries
                        .iter()
                        .any(|old_e| old_e.key == new_e.key && old_e.running)
                {
                    self.anim.bounce_key = Some(new_e.key.clone());
                    self.anim.bounce_phase = 0.01;
                    need_anim = true;
                }
            }

            for old_e in &self.entries {
                if !matches!(
                    old_e.kind,
                    DockItemKind::Finder
                        | DockItemKind::Launchpad
                        | DockItemKind::Downloads
                        | DockItemKind::Trash { .. }
                ) && !latest.iter().any(|new_e| new_e.key == old_e.key)
                    && !self.closing_entries.iter().any(|c| c.key == old_e.key)
                {
                    let mut ghost = old_e.clone();
                    ghost.running = false;
                    ghost.active = false;
                    self.closing_entries.push(ghost);
                    need_anim = true;
                }
            }
            if need_anim {
                self.ensure_animating();
            }

            self.closing_entries
                .retain(|c| !latest.iter().any(|n| n.key == c.key));
            self.entries = latest;
            let all = self.combined_entries();
            sync_icons(&mut self.renderer, &all);
            self.recompute_layout();
            dirty = true;
        }

        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;

        let fullscreen_or_max =
            has_maximized_or_fullscreen_window(self.screen.width, self.screen.height, self.own_pid);

        let at_bottom_edge = gy >= self.screen.height - 6;
        let in_interactive_zone = self.anim.hide_progress < 0.85
            && local_x >= self.layout.pill.x - 20
            && local_x <= self.layout.pill.right() + 20
            && local_y >= self.layout.pill.y - 36;

        let target_hide = fullscreen_or_max && !at_bottom_edge && !in_interactive_zone;
        if target_hide != self.anim.should_hide {
            self.anim.should_hide = target_hide;
            if self.anim.should_hide {
                self.hovering = false;
            }
            self.ensure_animating();
        }

        if self.hovering && !in_interactive_zone {
            self.hovering = false;
            self.ensure_animating();
        } else if !self.anim.should_hide && in_interactive_zone && self.anim.hide_progress < 0.25 {
            let new_cx = local_x as f32;
            if !self.hovering
                || self
                    .last_cursor_x
                    .is_none_or(|old| (old - new_cx).abs() > 0.5)
            {
                self.hovering = true;
                self.last_cursor_x = Some(new_cx);
                self.ensure_animating();
            }
        }

        if dirty && !self.anim.timer_running {
            self.present();
        }
    }

    fn on_anim_timer(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.anim.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.anim.last_tick = now;

        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;

        if self.hovering {
            let still_inside = !self.anim.should_hide
                && local_x >= self.layout.pill.x - 20
                && local_x <= self.layout.pill.right() + 20
                && local_y >= self.layout.pill.y - 36;
            if still_inside {
                self.last_cursor_x = Some(local_x as f32);
            } else {
                self.hovering = false;
            }
        }

        let target_mag = if self.hovering && !self.anim.should_hide {
            1.0
        } else {
            0.0
        };
        let target_hide = if self.anim.should_hide { 1.0 } else { 0.0 };

        self.anim.mag_strength = animate_value(
            self.anim.mag_strength,
            target_mag,
            self.motion_metrics.dock_magnify_rate,
            dt,
            self.motion,
        );
        self.anim.hide_progress = animate_value(
            self.anim.hide_progress,
            target_hide,
            self.motion_metrics.dock_hide_rate,
            dt,
            self.motion,
        );

        if self.anim.bounce_phase > 0.0 {
            self.anim.bounce_phase = advance_bounce_phase(
                self.anim.bounce_phase,
                dt,
                self.motion_metrics.bounce_duration_seconds,
                self.motion,
            );
            if self.anim.bounce_phase >= 1.0 {
                self.anim.bounce_phase = 0.0;
                self.anim.bounce_key = None;
            }
        }

        let mut presence_animating = false;
        for entry in &self.entries {
            let cur = self.presence.get(&entry.key).copied().unwrap_or(1.0);
            let next = animate_value(
                cur,
                1.0,
                self.motion_metrics.dock_presence_enter_rate,
                dt,
                self.motion,
            );
            self.presence.insert(entry.key.clone(), next);
            if (next - 1.0).abs() > 0.002 {
                presence_animating = true;
            }
        }
        let mut still_closing = Vec::new();
        for closing in self.closing_entries.drain(..) {
            let cur = self.presence.get(&closing.key).copied().unwrap_or(1.0);
            let next = animate_value(
                cur,
                0.0,
                self.motion_metrics.dock_presence_exit_rate,
                dt,
                self.motion,
            );
            if next > 0.03 {
                self.presence.insert(closing.key.clone(), next);
                still_closing.push(closing);
                presence_animating = true;
            } else {
                self.presence.remove(&closing.key);
            }
        }
        self.closing_entries = still_closing;

        self.recompute_layout();
        self.present();

        let mag_done = (self.anim.mag_strength - target_mag).abs() < 0.001;
        let hide_done = (self.anim.hide_progress - target_hide).abs() < 0.001;
        let bounce_done = self.anim.bounce_phase == 0.0;

        if mag_done && hide_done && bounce_done && !presence_animating {
            if !self.hovering {
                self.last_cursor_x = None;
            }
            let all = self.combined_entries();
            sync_icons(&mut self.renderer, &all);
            self.stop_animating();
        }
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        if !self.tracking_mouse {
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: HWND(self.hwnd as *mut _),
                dwHoverTime: 0,
            };
            if unsafe { TrackMouseEvent(&mut track) }.is_ok() {
                self.tracking_mouse = true;
            }
        }

        if self.anim.should_hide {
            return;
        }

        let near_pill = x >= self.layout.pill.x - 16
            && x <= self.layout.pill.right() + 16
            && y >= self.layout.pill.y - 36;

        if near_pill {
            self.hovering = true;
            self.last_cursor_x = Some(x as f32);
            self.ensure_animating();
        } else if self.hovering {
            self.hovering = false;
            self.ensure_animating();
        }
    }

    fn on_mouse_leave(&mut self) {
        self.tracking_mouse = false;
        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;
        let still_over_dock = !self.anim.should_hide
            && local_x >= self.layout.pill.x - 12
            && local_x <= self.layout.pill.right() + 12
            && local_y >= self.layout.pill.y - 28
            && local_y <= self.layout.pill.bottom();
        if !still_over_dock && self.hovering {
            self.hovering = false;
            self.ensure_animating();
        }
    }

    fn on_left_click(&mut self, x: i32, y: i32) {
        crate::menu::close();
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(x, y, gap_pad) else {
            return;
        };
        let all = self.combined_entries();
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        if self.motion.allows_motion()
            && (!entry.running
                || matches!(
                    entry.kind,
                    DockItemKind::Finder
                        | DockItemKind::Launchpad
                        | DockItemKind::Downloads
                        | DockItemKind::Trash { .. }
                ))
        {
            self.anim.bounce_key = Some(entry.key.clone());
            self.anim.bounce_phase = 0.01;
            self.ensure_animating();
        } else if self.motion.allows_motion() {
            self.anim.bounce_key = Some(entry.key.clone());
            self.anim.bounce_phase = 0.45;
            self.ensure_animating();
        }
        let target = self.target_for_entry(&entry);
        activate_entry(&entry, target);
    }

    fn on_right_click(&mut self, x: i32, y: i32) {
        let gap_pad = scale(self.metrics.icon_gap, self.screen.dpi) as f32 * 0.5;
        let Some(idx) = self.layout.hit_test(x, y, gap_pad) else {
            return;
        };
        let all = self.combined_entries();
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        let Some(slot) = self.layout.items.get(idx).copied() else {
            return;
        };
        self.hovering = false;
        self.ensure_animating();

        let (origin_x, origin_y) = self.window_origin();
        let center_x = origin_x + (slot.x + slot.size * 0.5).round() as i32;
        let bottom_y = origin_y + slot.y.round() as i32 - scale(10, self.screen.dpi);
        let target = self.target_for_entry(&entry);
        crate::menu::open_dock_menu(entry, center_x, bottom_y, target);
    }

    fn ensure_animating(&mut self) {
        if !self.anim.timer_running {
            self.anim.timer_running = true;
            self.anim.last_tick = Instant::now();
            unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    DOCK_ANIM_TIMER,
                    self.motion_metrics.animation_tick_ms,
                    None,
                );
            }
        }
    }

    fn stop_animating(&mut self) {
        if self.anim.timer_running {
            self.anim.timer_running = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), DOCK_ANIM_TIMER);
            }
        }
    }

    fn recompute_layout(&mut self) {
        let all = self.combined_entries();
        let (app_count, has_trash) = count_entries(&all);
        let bounce_offsets: Vec<f32> = all
            .iter()
            .map(|entry| {
                if self.anim.bounce_phase > 0.0
                    && self.anim.bounce_key.as_deref() == Some(entry.key.as_str())
                {
                    bounce_offset(self.anim.bounce_phase)
                } else {
                    0.0
                }
            })
            .collect();
        let item_presence: Vec<f32> = all
            .iter()
            .map(|entry| self.presence.get(&entry.key).copied().unwrap_or(1.0))
            .collect();

        let active_cursor = if self.hovering || self.anim.mag_strength > 0.001 {
            self.last_cursor_x
        } else {
            None
        };

        let mut layout = compute_dock_layout(
            &self.metrics,
            DockInput {
                screen_width: self.screen.width,
                dpi: self.screen.dpi,
                app_count,
                has_trash,
                cursor_x: active_cursor,
                mag_strength: self.anim.mag_strength,
                bounce_offsets: &bounce_offsets,
                item_presence: &item_presence,
                hide_progress: self.anim.hide_progress,
            },
        );
        if !self.hovering {
            layout.hovered_index = None;
        }
        self.layout = layout;
    }

    fn target_for_entry(&self, entry: &DockEntry) -> Option<Rect> {
        let index = self
            .combined_entries()
            .iter()
            .position(|candidate| candidate.key == entry.key)?;
        let item = self.layout.items.get(index)?;
        let (origin_x, origin_y) = self.window_origin();
        let size = item.size.round().max(1.0) as i32;
        Some(Rect {
            x: origin_x + item.x.round() as i32,
            y: origin_y + item.y.round() as i32,
            width: size,
            height: size,
        })
    }

    fn present(&self) {
        let all = self.combined_entries();
        let render_items: Vec<DockRenderItem<'_>> = all
            .iter()
            .map(|e| DockRenderItem {
                key: &e.key,
                title: &e.title,
                apple_icon: e.apple_icon,
                running: e.running,
                active: e.active,
            })
            .collect();
        let theme = crate::theme_state::resolved_theme();
        let _ = self.renderer.present(
            self.hwnd,
            self.window_origin(),
            &self.layout,
            &render_items,
            theme,
        );
    }
}
