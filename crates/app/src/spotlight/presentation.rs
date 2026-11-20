use std::collections::HashSet;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_SPOTLIGHT_BACKDROP;
use otto_core::IconBitmap;
use otto_core::{
    BACKDROP_PADDING_PT, CaptureRegion, capture_interval_for, capture_wanted,
    compute_spotlight_layout, scale,
};
use otto_platform::{extract_dock_icon, is_running_on_battery};
use otto_render::{SpotlightPaintEntry, SpotlightPaintState, SpotlightRenderer};

use super::SpotlightSurface;

impl SpotlightSurface {
    pub(super) fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (
            update.origin.0 - self.window.x,
            update.origin.1 - self.window.y,
        );
        if let Err(error) = self.ensure_renderer() {
            crate::host::log_present_error("spotlight renderer", Err(error));
            self.hide();
            return;
        }
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.set_backdrop(update.pixels, local);
        }
        self.present(runtime);
    }

    pub(super) fn prime_window_backdrop(&self, runtime: &RuntimeContext, force: bool) {
        if !capture_wanted(
            &crate::theme_state::appearance(runtime),
            runtime.glass_enabled.get(),
        ) {
            return;
        }
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        if !force && renderer.has_backdrop() {
            return;
        }
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        if let Some(pixels) =
            otto_platform::capture_backdrop_now(region.origin(), region.width, region.height)
        {
            renderer.set_backdrop(pixels, (region.x - self.window.x, region.y - self.window.y));
        }
    }

    pub(super) fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = self.visible
            && self.hwnd != 0
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
        if !force
            && self.morph_progress.is_some()
            && self
                .renderer
                .as_ref()
                .is_some_and(SpotlightRenderer::has_backdrop)
        {
            return;
        }
        if self.backdrop.is_none() {
            self.backdrop = Some(BackdropFeed::start(
                self.hwnd,
                WM_APP_SPOTLIGHT_BACKDROP,
                vec![self.hwnd],
                runtime.capture_visible.get(),
            ));
        }
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let capture_rect = if self.morph_progress.is_some() {
            self.target_window
        } else {
            self.window
        };
        let Some(region) = CaptureRegion::padded_to_screen(
            capture_rect,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        let interval = capture_interval_for(is_running_on_battery(), self.visible);
        if let Some(feed) = self.backdrop.as_mut() {
            feed.request(region, interval, force);
        }
    }

    pub(super) fn ensure_renderer(&mut self) -> windows::core::Result<()> {
        if self.renderer.is_none() {
            self.renderer = Some(SpotlightRenderer::new(self.screen.dpi)?);
        }
        Ok(())
    }

    pub(super) fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible || self.hwnd == 0 {
            return;
        }
        if let Err(error) = self.ensure_renderer() {
            crate::host::log_present_error("spotlight renderer", Err(error));
            self.hide();
            return;
        }
        let layout = compute_spotlight_layout(
            (self.window.x, self.window.y),
            self.window.width,
            self.window.height,
            self.screen.dpi,
            self.view,
            self.filtered.len(),
            self.first_visible,
        );
        self.layout = Some(layout.clone());
        let visible_icon_keys: HashSet<&str> = layout
            .results
            .iter()
            .filter_map(|result| self.filtered.get(result.index))
            .filter_map(|app_index| self.apps.get(*app_index))
            .map(|entry| entry.key.as_str())
            .collect();
        self.icons_attempted
            .retain(|key| visible_icon_keys.contains(key.as_str()));
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.retain_visible_icons(&visible_icon_keys);
        }
        let mut icons = Vec::<(String, IconBitmap)>::new();
        for result in &layout.results {
            let Some(app_index) = self.filtered.get(result.index).copied() else {
                continue;
            };
            let Some(entry) = self.apps.get(app_index) else {
                continue;
            };
            if self.icons_attempted.insert(entry.key.clone())
                && let Some(icon) = extract_dock_icon(entry)
            {
                icons.push((entry.key.clone(), icon));
            }
        }
        let paint_entries: Vec<SpotlightPaintEntry<'_>> = self
            .filtered
            .iter()
            .filter_map(|index| self.apps.get(*index))
            .map(|entry| SpotlightPaintEntry {
                key: &entry.key,
                title: &entry.title,
            })
            .collect();
        let state = SpotlightPaintState {
            layout: &layout,
            entries: &paint_entries,
            mode: self.mode,
            view: self.view,
            query: &self.query,
            cursor_byte: self.query_state.cursor,
            selected_all: self.query_state.selected_all,
            math_result: self.math_result.as_deref(),
            selected: self.selected,
            hovered: self.hovered,
            content_opacity: self.morph_progress.unwrap_or(1.0),
            appearance: crate::theme_state::appearance(runtime),
        };
        let render_result = self.renderer.as_mut().map(|renderer| {
            for (key, icon) in icons {
                let _ = renderer.set_icon(key, icon.width, icon.height, &icon.pixels);
            }
            renderer.present(self.hwnd, &state)
        });
        if let Some(Err(error)) = render_result {
            crate::host::log_present_error("spotlight", Err(error));
            self.hide();
        }
    }
}
