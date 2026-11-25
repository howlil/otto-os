use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::{DOCK_ANIM_TIMER, WM_APP_DOCK_BACKDROP};
use otto_core::{
    BACKDROP_PADDING_PT, CaptureRegion, DockEntry, DockInput, DockItemKind, DockLayout, DockPort,
    Rect, bounce_offset, capture_interval_for, capture_wanted, compute_dock_layout, dock_capacity,
    dock_entry_badge, dock_entry_progress, dock_rest_zone_in_bounds, dock_should_hide, scale,
    split_dock_overflow,
};
use otto_platform::{
    DisplayMonitor, DockReservation, assert_window_topmost, capture_backdrop_now, cursor_position,
    enable_drag_accept_files, hide_window, is_running_on_battery, is_surface_visible,
    kill_window_timer, place, release_capture_if_owned, secondary_monitors, set_window_timer,
    watch_bottom_edges,
};
use otto_render::{DockRenderItem, DockRenderer};

use super::actions::{count_entries, sync_icons};
use super::visibility::should_assert_topmost;
use super::{
    AnimState, DOCK_HIDDEN_PROGRESS, DOCK_REVEAL_BAND, Dock, DragState, entry_notification_count,
};

#[derive(Clone, Copy, Default)]
pub(super) struct SecondaryPollState {
    pub(super) dirty: bool,
    pub(super) cover_geometry_changed: bool,
    pub(super) geometry_changed: bool,
}

#[derive(Default)]
pub(super) struct SecondaryDockCover {
    pub(super) fullscreen_covered: bool,
    pub(super) shell_full_screen: bool,
    pub(super) window_overlaps: bool,
}

impl SecondaryDockCover {
    pub(super) const fn is_fullscreen_covered(&self) -> bool {
        self.fullscreen_covered || self.shell_full_screen
    }
}

pub(super) struct SecondaryDock {
    pub(super) hwnd: isize,
    pub(super) monitor: DisplayMonitor,
    pub(super) renderer: DockRenderer,
    pub(super) layout: DockLayout,
    pub(super) hovering: bool,
    pub(super) last_cursor_x: Option<f32>,
    pub(super) tracking_mouse: bool,
    pub(super) anim: AnimState,
    pub(super) drag: DragState,
    pub(super) cover: SecondaryDockCover,
    pub(super) reservation: Option<DockReservation>,
    pub(super) backdrop: Option<BackdropFeed>,
}

impl SecondaryDock {
    pub(super) fn window_origin(&self) -> (i32, i32) {
        (
            self.monitor.bounds.x,
            self.monitor.bounds.bottom() - self.layout.window_height,
        )
    }

    pub(super) fn place(&self) {
        let (ox, oy) = self.window_origin();
        place(
            self.hwnd,
            Rect {
                x: ox,
                y: oy,
                width: self.layout.window_width,
                height: self.layout.window_height,
            },
        );
    }

    pub(super) fn ensure_topmost(&self, runtime: &RuntimeContext) {
        if !should_assert_topmost(self.anim.should_hide, Dock::higher_overlay_open(runtime)) {
            return;
        }
        assert_window_topmost(self.hwnd);
    }

    pub(super) fn ensure_animating(&mut self, tick_ms: u32) {
        if !self.anim.timer_running && set_window_timer(self.hwnd, DOCK_ANIM_TIMER, tick_ms) {
            otto_platform::begin_fine_timing();
            self.anim.timer_running = true;
            self.anim.last_tick = Instant::now();
        }
    }

    pub(super) fn stop_animating(&mut self) {
        if self.anim.timer_running {
            self.anim.timer_running = false;
            kill_window_timer(self.hwnd, DOCK_ANIM_TIMER);
            otto_platform::end_fine_timing();
        }
    }

    pub(super) fn teardown(mut self, runtime: &RuntimeContext) {
        self.stop_animating();
        let _ = release_capture_if_owned(self.hwnd);
        self.backdrop = None;
        self.reservation = None;
        hide_window(self.hwnd);
        runtime.queue_destroy_hwnd(self.hwnd);
    }

    pub(super) fn item_screen_rect(&self, index: usize) -> Option<Rect> {
        let item = self.layout.items.get(index)?;
        let (origin_x, origin_y) = self.window_origin();
        let size = item.size.round().max(1.0) as i32;
        Some(Rect {
            x: origin_x + item.x.round() as i32,
            y: (origin_y + item.y.round() as i32).min(self.monitor.bounds.bottom() - 4),
            width: size,
            height: size,
        })
    }

    pub(super) fn target_for_window(&self, dock: &Dock, hwnd: isize) -> Option<Rect> {
        let (visible, hidden) = self.displayed_entries(dock);
        if let Some(index) = visible.iter().position(|entry| entry.hwnds.contains(&hwnd)) {
            return self.item_screen_rect(index);
        }
        if hidden.iter().any(|entry| entry.hwnds.contains(&hwnd))
            && let Some(overflow_index) = visible
                .iter()
                .position(|entry| matches!(entry.kind, DockItemKind::Overflow { .. }))
        {
            return self.item_screen_rect(overflow_index);
        }
        None
    }

    pub(super) fn fallback_target(&self, dock: &Dock) -> Rect {
        let size = scale(dock.metrics.base_icon_size, self.monitor.dpi).max(24);
        let (origin_x, origin_y) = self.window_origin();
        let x = origin_x + self.layout.pill.x + (self.layout.pill.width - size) / 2;
        let y = (origin_y + self.layout.pill.y + (self.layout.pill.height - size) / 2)
            .min(self.monitor.bounds.bottom() - 4);
        Rect {
            x,
            y,
            width: size,
            height: size,
        }
    }

    pub(super) fn base_displayed_entries(&self, dock: &Dock) -> (Vec<DockEntry>, Vec<DockEntry>) {
        let mut all = dock.all_entries();
        let right_items = all
            .iter()
            .rev()
            .take_while(|entry| {
                matches!(
                    entry.kind,
                    DockItemKind::Downloads | DockItemKind::Trash { .. }
                )
            })
            .count();
        let app_count = all.len() - right_items;
        let capacity = dock_capacity(
            &dock.metrics,
            self.monitor.bounds.width,
            self.monitor.dpi,
            right_items,
        );
        let split = split_dock_overflow(app_count, capacity);
        if split.hidden_apps == 0 {
            return (all, Vec::new());
        }
        let right = all.split_off(app_count);
        let hidden = all.split_off(split.visible_apps);
        all.push(dock.port.overflow_entry(hidden.len()));
        all.extend(right);
        (all, hidden)
    }

    pub(super) fn displayed_entries(&self, dock: &Dock) -> (Vec<DockEntry>, Vec<DockEntry>) {
        let (mut visible, hidden) = self.base_displayed_entries(dock);
        if self.drag.is_dragging
            && let (Some(from), Some(to)) = (self.drag.start_idx, self.drag.hover_idx)
        {
            let (app_count, _, _) = count_entries(&visible);
            if from < app_count && to < app_count && from != to {
                let order =
                    DockLayout::preview_reordered_indices(visible.len(), Some(from), Some(to));
                visible = order.into_iter().map(|i| visible[i].clone()).collect();
            }
        }
        (visible, hidden)
    }

    pub(super) fn combined_entries(&self, dock: &Dock) -> Vec<DockEntry> {
        self.displayed_entries(dock).0
    }
}

impl Dock {
    pub(super) fn has_secondary_hwnd(&self, hwnd: isize) -> bool {
        self.secondary_docks.iter().any(|dock| dock.hwnd == hwnd)
    }

    pub(super) fn secondary_index(&self, hwnd: isize) -> Option<usize> {
        self.secondary_docks
            .iter()
            .position(|dock| dock.hwnd == hwnd)
    }

    pub(super) fn secondary_for_monitor(&self, monitor_id: isize) -> Option<&SecondaryDock> {
        self.secondary_docks
            .iter()
            .find(|dock| dock.monitor.id == monitor_id)
    }

    fn compute_secondary_layout_for(
        &self,
        monitor: DisplayMonitor,
        all: &[DockEntry],
        hovering: bool,
        last_cursor_x: Option<f32>,
        anim: &AnimState,
    ) -> DockLayout {
        let (app_count, right_count, has_trash) = count_entries(all);
        let bounce_offsets: Vec<f32> = all
            .iter()
            .map(|entry| {
                if anim.bounce_phase > 0.0 && anim.bounce_key.as_deref() == Some(entry.key.as_str())
                {
                    bounce_offset(anim.bounce_phase)
                } else if self.anim.bounce_phase > 0.0
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
        let active_cursor = if hovering || anim.mag_strength > 0.001 {
            last_cursor_x
        } else {
            None
        };
        let mut layout = compute_dock_layout(
            &self.metrics,
            DockInput {
                screen_width: monitor.bounds.width,
                dpi: monitor.dpi,
                app_count,
                right_count,
                has_trash,
                cursor_x: active_cursor,
                mag_strength: anim.mag_strength,
                bounce_offsets: &bounce_offsets,
                item_presence: &item_presence,
                hide_progress: anim.hide_progress,
            },
        );
        if !hovering {
            layout.hovered_index = None;
        }
        layout
    }

    fn create_secondary_dock(
        &self,
        runtime: &RuntimeContext,
        monitor: DisplayMonitor,
    ) -> Option<SecondaryDock> {
        let hwnd = crate::host::create_secondary_dock_window()?;
        enable_drag_accept_files(hwnd);
        let mut all = self.all_entries();
        let right_items = all
            .iter()
            .rev()
            .take_while(|entry| {
                matches!(
                    entry.kind,
                    DockItemKind::Downloads | DockItemKind::Trash { .. }
                )
            })
            .count();
        let app_count = all.len() - right_items;
        let capacity = dock_capacity(
            &self.metrics,
            monitor.bounds.width,
            monitor.dpi,
            right_items,
        );
        let split = split_dock_overflow(app_count, capacity);
        if split.hidden_apps > 0 {
            let right = all.split_off(app_count);
            let hidden = all.split_off(split.visible_apps);
            all.push(self.port.overflow_entry(hidden.len()));
            all.extend(right);
        }
        let anim = AnimState::default();
        let layout = self.compute_secondary_layout_for(monitor, &all, false, None, &anim);
        let Ok(mut renderer) =
            DockRenderer::new(layout.window_width, layout.window_height, monitor.dpi)
        else {
            hide_window(hwnd);
            runtime.queue_destroy_hwnd(hwnd);
            return None;
        };
        sync_icons(&mut renderer, &all);
        let backdrop = runtime.glass_enabled.get().then(|| {
            BackdropFeed::start(
                hwnd,
                WM_APP_DOCK_BACKDROP,
                vec![hwnd],
                runtime.capture_visible.get(),
            )
        });
        Some(SecondaryDock {
            hwnd,
            monitor,
            renderer,
            layout,
            hovering: false,
            last_cursor_x: None,
            tracking_mouse: false,
            anim,
            drag: DragState::default(),
            cover: SecondaryDockCover::default(),
            reservation: None,
            backdrop,
        })
    }

    pub(super) fn sync_secondary_docks(&mut self, runtime: &RuntimeContext) {
        let monitors = secondary_monitors();
        let mut existing = std::mem::take(&mut self.secondary_docks);
        let mut next = Vec::with_capacity(monitors.len());
        for monitor in monitors {
            if let Some(pos) = existing.iter().position(|d| d.monitor.id == monitor.id) {
                let mut sec = existing.remove(pos);
                let prev = sec.monitor;
                let old_origin = sec.window_origin();
                let carried = sec.renderer.take_backdrop();
                sec.monitor = monitor;
                let all = sec.combined_entries(self);
                sec.layout = self.compute_secondary_layout_for(
                    monitor,
                    &all,
                    sec.hovering,
                    sec.last_cursor_x,
                    &sec.anim,
                );
                if let Ok(mut renderer) = DockRenderer::new(
                    sec.layout.window_width,
                    sec.layout.window_height,
                    monitor.dpi,
                ) {
                    sync_icons(&mut renderer, &all);
                    if prev.bounds == monitor.bounds
                        && let Some((pixels, local)) = carried
                    {
                        let new_origin = sec.window_origin();
                        let screen_origin = (old_origin.0 + local.0, old_origin.1 + local.1);
                        renderer.set_backdrop(
                            pixels,
                            (
                                screen_origin.0 - new_origin.0,
                                screen_origin.1 - new_origin.1,
                            ),
                        );
                    }
                    sec.renderer = renderer;
                    if let Some(feed) = sec.backdrop.as_mut() {
                        feed.reset();
                    }
                }
                next.push(sec);
            } else if let Some(sec) = self.create_secondary_dock(runtime, monitor) {
                next.push(sec);
            }
        }
        for stale in existing {
            stale.teardown(runtime);
        }
        self.secondary_docks = next;
        runtime.set_secondary_dock_hwnds(self.secondary_docks.iter().map(|d| d.hwnd).collect());
        for index in 0..self.secondary_docks.len() {
            self.refresh_secondary_cover(index);
            let _ = self.sync_secondary_reservation(index);
            self.sync_secondary_backdrop(runtime, index, true);
            self.secondary_docks[index].place();
            self.present_secondary(runtime, index);
        }
        self.sync_bottom_edge_watches();
    }

    pub(super) fn teardown_secondary_docks(&mut self, runtime: &RuntimeContext) {
        for sec in self.secondary_docks.drain(..) {
            sec.teardown(runtime);
        }
        runtime.set_secondary_dock_hwnds(Vec::new());
    }

    pub(super) fn recompute_secondary_dock_layout(&mut self, index: usize) {
        let all = self.secondary_docks[index].combined_entries(self);
        let monitor = self.secondary_docks[index].monitor;
        let hovering = self.secondary_docks[index].hovering;
        let last_cursor_x = self.secondary_docks[index].last_cursor_x;
        let layout = self.compute_secondary_layout_for(
            monitor,
            &all,
            hovering,
            last_cursor_x,
            &self.secondary_docks[index].anim,
        );
        self.secondary_docks[index].layout = layout;
    }

    pub(super) fn sync_secondary_entries(&mut self, animate: bool) {
        let tick_ms = self.motion_metrics.animation_tick_ms;
        for index in 0..self.secondary_docks.len() {
            let all = self.secondary_docks[index].combined_entries(self);
            sync_icons(&mut self.secondary_docks[index].renderer, &all);
            self.recompute_secondary_dock_layout(index);
            if animate {
                self.secondary_docks[index].ensure_animating(tick_ms);
            }
        }
    }

    pub(super) fn refresh_secondary_cover(&mut self, index: usize) {
        let bounds = self.secondary_docks[index].monitor.bounds;
        self.secondary_docks[index].cover.fullscreen_covered =
            self.port.fullscreen_on_monitor(bounds, self.own_pid);
        self.secondary_docks[index].cover.window_overlaps = self.preferences.autohide
            && dock_rest_zone_in_bounds(&self.secondary_docks[index].layout, bounds)
                .is_some_and(|zone| self.port.window_overlaps_zone(zone, self.own_pid));
    }

    pub(super) fn sync_secondary_reservation(&mut self, index: usize) -> bool {
        if !(self.preferences.reserve_space && self.preferences.hide_taskbar) {
            self.secondary_docks[index].reservation = None;
            self.secondary_docks[index].cover.shell_full_screen = false;
            return false;
        }
        let sec = &mut self.secondary_docks[index];
        let mut updated = if sec.reservation.is_none() {
            sec.reservation = DockReservation::register(sec.hwnd);
            sec.reservation.is_some()
        } else {
            false
        };
        if let Some(reservation) = sec.reservation.as_mut() {
            updated |= reservation.reserve_in_bounds(sec.monitor.bounds, sec.layout.rest_height);
        }
        updated
    }

    pub(super) fn sync_secondary_hide_from_pointer(
        &mut self,
        runtime: &RuntimeContext,
        index: usize,
    ) -> bool {
        let (gx, gy) = cursor_position();
        let tick_ms = self.motion_metrics.animation_tick_ms;
        let prefs = self.preferences;
        let sec = &mut self.secondary_docks[index];
        let (win_left, win_top) = sec.window_origin();
        let local_x = gx - win_left;
        let local_y = gy - win_top;
        let on_monitor = sec.monitor.bounds.contains(gx, gy);
        let should_auto_hide = dock_should_hide(
            prefs,
            sec.cover.is_fullscreen_covered(),
            sec.cover.window_overlaps,
        );
        let at_bottom_edge = on_monitor && gy >= sec.monitor.bounds.bottom() - DOCK_REVEAL_BAND;
        let in_interactive_zone = on_monitor
            && sec.anim.hide_progress < 0.85
            && local_x >= sec.layout.pill.x - 20
            && local_x <= sec.layout.pill.right() + 20
            && local_y >= sec.layout.pill.y - 36;
        let target_hide = should_auto_hide && !at_bottom_edge && !in_interactive_zone;
        if target_hide != sec.anim.should_hide {
            sec.anim.should_hide = target_hide;
            if sec.anim.should_hide {
                sec.hovering = false;
            } else {
                sec.ensure_topmost(runtime);
            }
            sec.ensure_animating(tick_ms);
        }
        if sec.hovering && !in_interactive_zone {
            sec.hovering = false;
            sec.ensure_animating(tick_ms);
        } else if !sec.anim.should_hide && in_interactive_zone && sec.anim.hide_progress < 0.25 {
            let new_cx = local_x as f32;
            if !sec.hovering
                || sec
                    .last_cursor_x
                    .is_none_or(|old| (old - new_cx).abs() > 0.5)
            {
                let started_hover = !sec.hovering;
                sec.hovering = true;
                sec.last_cursor_x = Some(new_cx);
                if started_hover {
                    sec.ensure_topmost(runtime);
                }
                sec.ensure_animating(tick_ms);
            }
        }
        should_auto_hide
    }

    pub(super) fn sync_bottom_edge_watches(&self) {
        let mut watched = Vec::new();
        if self.anim.should_hide {
            watched.push((
                Rect {
                    x: 0,
                    y: 0,
                    width: self.screen.width,
                    height: self.screen.height,
                },
                DOCK_REVEAL_BAND,
            ));
        }
        for sec in &self.secondary_docks {
            if sec.anim.should_hide {
                watched.push((sec.monitor.bounds, DOCK_REVEAL_BAND));
            }
        }
        watch_bottom_edges(&watched);
    }

    pub(super) fn poll_secondary_docks(
        &mut self,
        runtime: &RuntimeContext,
        state: SecondaryPollState,
        has_generations: bool,
    ) -> bool {
        let mut any_busy = false;
        for index in 0..self.secondary_docks.len() {
            if state.cover_geometry_changed {
                self.refresh_secondary_cover(index);
            }
            let reservation_changed = self.sync_secondary_reservation(index);
            let should_auto_hide = self.sync_secondary_hide_from_pointer(runtime, index);
            if state.cover_geometry_changed || reservation_changed {
                self.secondary_docks[index].ensure_topmost(runtime);
            }
            if state.dirty && !self.secondary_docks[index].anim.timer_running {
                self.present_secondary(runtime, index);
            }
            self.sync_secondary_backdrop(runtime, index, state.geometry_changed);
            let sec = &self.secondary_docks[index];
            let reveal_needs_polling =
                should_auto_hide && (!sec.anim.should_hide || !has_generations);
            any_busy |= reveal_needs_polling
                || sec.hovering
                || sec.anim.timer_running
                || sec.drag.start_idx.is_some();
        }
        self.sync_bottom_edge_watches();
        any_busy
    }

    pub(super) fn sync_secondary_backdrop(
        &mut self,
        runtime: &RuntimeContext,
        index: usize,
        force: bool,
    ) {
        let appearance = crate::theme_state::appearance(runtime);
        let sec = &mut self.secondary_docks[index];
        let wanted = capture_wanted(&appearance, sec.backdrop.is_some());
        let hidden = sec.anim.hide_progress >= DOCK_HIDDEN_PROGRESS;
        let (origin_x, origin_y) = sec.window_origin();
        let window = Rect {
            x: origin_x,
            y: origin_y,
            width: sec.layout.window_width,
            height: sec.layout.window_height,
        };
        let pad = scale(BACKDROP_PADDING_PT, sec.monitor.dpi);
        let region = CaptureRegion::padded_to_bounds(window, pad, sec.monitor.bounds);
        let Some(feed) = sec.backdrop.as_mut() else {
            return;
        };
        if !wanted {
            if feed.is_active() {
                feed.reset();
                sec.renderer.clear_backdrop();
                self.present_secondary(runtime, index);
            }
            return;
        }
        if hidden {
            return;
        }
        if !force && sec.anim.timer_running && sec.renderer.has_backdrop() {
            return;
        }
        if let Some(region) = region {
            let capture_visible = runtime.capture_visible.get();
            if !sec.renderer.has_backdrop() {
                let can_prime = !capture_visible || !is_surface_visible(sec.hwnd);
                if can_prime
                    && let Some(pixels) =
                        capture_backdrop_now(region.origin(), region.width, region.height)
                {
                    sec.renderer
                        .set_backdrop(pixels, (region.x - origin_x, region.y - origin_y));
                }
            }
            feed.request(
                region,
                capture_interval_for(
                    is_running_on_battery(),
                    sec.hovering && feed.recently_changed(),
                ),
                force,
            );
        }
    }

    pub(super) fn present_secondary_docks(&mut self, runtime: &RuntimeContext) {
        for index in 0..self.secondary_docks.len() {
            self.present_secondary(runtime, index);
        }
    }

    pub(super) fn present_secondary(&mut self, runtime: &RuntimeContext, index: usize) {
        let all = self.secondary_docks[index].combined_entries(self);
        let focus_blocklist = runtime
            .focus_active
            .get()
            .then(|| {
                runtime
                    .focus_activity
                    .try_borrow()
                    .ok()
                    .map(|fa| fa.blocklist.clone())
            })
            .flatten();
        let render_items: Vec<DockRenderItem<'_>> = all
            .iter()
            .map(|e| {
                let suppress_badge = focus_blocklist.as_ref().is_some_and(|bl| {
                    bl.block_notifications || bl.is_app_blocked(&e.key, &e.title)
                });
                let notif_count = if suppress_badge {
                    None
                } else {
                    entry_notification_count(&self.notification_counts, e)
                };
                let badge_count = if suppress_badge {
                    None
                } else {
                    dock_entry_badge(&e.window_titles, notif_count)
                };
                let progress = if matches!(e.kind, DockItemKind::Downloads) {
                    self.downloads_progress
                } else {
                    dock_entry_progress(&e.window_titles, None)
                };
                DockRenderItem {
                    key: &e.key,
                    title: &e.title,
                    apple_icon: e.apple_icon,
                    running: e.running,
                    active: e.active,
                    hidden: e.hidden,
                    focused: false,
                    badge_count,
                    progress,
                }
            })
            .collect();
        let appearance = crate::theme_state::appearance(runtime);
        let sec = &mut self.secondary_docks[index];
        crate::host::log_present_error(
            "secondary-dock",
            sec.renderer.present(
                sec.hwnd,
                sec.window_origin(),
                &sec.layout,
                &render_items,
                appearance,
            ),
        );
    }

    pub(super) fn set_secondary_glass_enabled(&mut self, runtime: &RuntimeContext, enabled: bool) {
        for index in 0..self.secondary_docks.len() {
            if enabled {
                if self.secondary_docks[index].backdrop.is_none() {
                    let hwnd = self.secondary_docks[index].hwnd;
                    self.secondary_docks[index].backdrop = Some(BackdropFeed::start(
                        hwnd,
                        WM_APP_DOCK_BACKDROP,
                        vec![hwnd],
                        runtime.capture_visible.get(),
                    ));
                }
            } else {
                self.secondary_docks[index].backdrop = None;
                self.secondary_docks[index].renderer.clear_backdrop();
                self.present_secondary(runtime, index);
            }
            self.sync_secondary_backdrop(runtime, index, true);
        }
    }

    pub(super) fn set_secondary_capture_visible(
        &mut self,
        runtime: &RuntimeContext,
        visible: bool,
    ) {
        for index in 0..self.secondary_docks.len() {
            if let Some(feed) = self.secondary_docks[index].backdrop.as_mut() {
                feed.set_capture_visible(visible);
            }
            if visible {
                self.secondary_docks[index].renderer.clear_backdrop();
            }
            self.sync_secondary_backdrop(runtime, index, true);
            self.present_secondary(runtime, index);
        }
    }

    pub(super) fn on_secondary_taskbar_created(&mut self, runtime: &RuntimeContext) {
        for index in 0..self.secondary_docks.len() {
            self.secondary_docks[index].reservation = None;
            self.secondary_docks[index].cover.shell_full_screen = false;
            let _ = self.sync_secondary_reservation(index);
            if should_assert_topmost(
                self.secondary_docks[index].anim.should_hide,
                Self::higher_overlay_open(runtime),
            ) {
                self.secondary_docks[index].place();
            }
        }
    }
}
