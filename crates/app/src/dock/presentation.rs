use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use otto_core::{
    BACKDROP_PADDING_PT, CaptureRegion, DockInput, DockItemKind, Rect, bounce_offset,
    capture_interval_for, capture_wanted, compute_dock_layout, dock_entry_badge,
    dock_entry_progress, scale,
};
use otto_platform::{is_running_on_battery, primary_screen};
use otto_render::{DockRenderItem, DockRenderer};

use super::DOCK_HIDDEN_PROGRESS;
use super::Dock;
use super::actions::count_entries;
use super::actions::sync_icons;
use super::entry_notification_count;

impl Dock {
    pub(super) fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.screen = primary_screen();
        self.recompute_layout();
        self.refresh_cover();
        self.sync_reservation();
        let all = self.combined_entries();
        if let Ok(mut renderer) = DockRenderer::new(
            self.layout.window_width,
            self.layout.window_height,
            self.screen.dpi,
        ) {
            sync_icons(&mut renderer, &all);
            self.renderer = renderer;
            if let Some(feed) = self.backdrop.as_mut() {
                feed.reset();
            }
        }
        self.sync_backdrop(runtime, true);
        self.place();
        self.present(runtime);
    }

    pub(super) fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = capture_wanted(&appearance, self.backdrop.is_some());
        let hidden = self.anim.hide_progress >= DOCK_HIDDEN_PROGRESS;
        let (origin_x, origin_y) = self.window_origin();
        let window = Rect {
            x: origin_x,
            y: origin_y,
            width: self.layout.window_width,
            height: self.layout.window_height,
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let region =
            CaptureRegion::padded_to_screen(window, pad, self.screen.width, self.screen.height);
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        if !wanted {
            if feed.is_active() {
                feed.reset();
                self.renderer.clear_backdrop();
                self.present(runtime);
            }
            return;
        }
        if hidden {
            return;
        }
        if !force && self.anim.timer_running && self.renderer.has_backdrop() {
            return;
        }
        if let Some(region) = region {
            if !self.renderer.has_backdrop()
                && let Some(pixels) = otto_platform::capture_backdrop_now(
                    region.origin(),
                    region.width,
                    region.height,
                )
            {
                self.renderer
                    .set_backdrop(pixels, (region.x - origin_x, region.y - origin_y));
            }
            feed.request(
                region,
                capture_interval_for(
                    is_running_on_battery(),
                    self.hovering && feed.recently_changed(),
                ),
                force,
            );
        }
    }

    pub(super) fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let (origin_x, origin_y) = self.window_origin();
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (update.origin.0 - origin_x, update.origin.1 - origin_y);
        self.renderer.set_backdrop(update.pixels, local);
        self.present(runtime);
    }

    pub(super) fn recompute_layout(&mut self) {
        let all = self.combined_entries();
        let (app_count, right_count, has_trash) = count_entries(&all);
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
                right_count,
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

    pub(super) fn present(&mut self, runtime: &RuntimeContext) {
        let all = self.combined_entries();
        let render_items: Vec<DockRenderItem<'_>> = all
            .iter()
            .enumerate()
            .map(|(index, e)| {
                let notif_count = entry_notification_count(&self.notification_counts, e);
                let badge_count = dock_entry_badge(&e.window_titles, notif_count);
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
                    focused: self.focus.current() == Some(index),
                    badge_count,
                    progress,
                }
            })
            .collect();
        let appearance = crate::theme_state::appearance(runtime);
        crate::host::log_present_error(
            "dock",
            self.renderer.present(
                self.hwnd,
                self.window_origin(),
                &self.layout,
                &render_items,
                appearance,
            ),
        );
    }
}
