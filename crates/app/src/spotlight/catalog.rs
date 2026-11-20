use std::time::Instant;

use crate::host::RuntimeContext;
use otto_core::{SpotlightMode, SpotlightView, rank_spotlight_results};
use otto_platform::{collect_spotlight_apps, monitor_work_area_at, primary_screen};

use super::SPOTLIGHT_CATALOG_TTL;
use super::SPOTLIGHT_MAX_QUERY_CHARS;
use super::SpotlightSurface;

impl SpotlightSurface {
    pub(super) fn load_catalog(&mut self) {
        if self
            .catalog_loaded_at
            .is_some_and(|loaded| loaded.elapsed() < SPOTLIGHT_CATALOG_TTL)
        {
            return;
        }
        self.apps = collect_spotlight_apps();
        self.catalog_loaded_at = Some(Instant::now());
        self.icons_attempted.clear();
        self.rebuild_results();
    }

    pub(super) fn rebuild_results(&mut self) {
        self.math_result = SpotlightMode::evaluate_math(&self.query);
        self.filtered = if self.mode == SpotlightMode::Search && self.query.is_empty() {
            Vec::new()
        } else {
            rank_spotlight_results(
                self.apps.iter().map(|entry| entry.title.as_str()),
                &self.query,
            )
        };
        self.first_visible = 0;
        self.hovered = None;
        self.selected = if self.filtered.is_empty() {
            None
        } else if self.mode == SpotlightMode::Search && !self.query.is_empty() {
            Some(0)
        } else {
            None
        };
    }

    pub(super) fn toggle_view(&mut self, view: SpotlightView) {
        if self.view != view {
            self.view = view;
            self.first_visible = 0;
            self.hovered = None;
            self.sync_layout();
            if self.selected.is_some() {
                self.reveal_selection();
            }
        }
    }

    pub(super) fn set_mode(&mut self, mode: SpotlightMode) {
        if self.mode != mode {
            self.mode = mode;
            self.view = match mode {
                SpotlightMode::Search => SpotlightView::List,
                SpotlightMode::Apps => SpotlightView::Grid,
            };
            self.rebuild_results();
        }
    }

    pub(super) fn set_query(&mut self, value: char) {
        if SpotlightMode::insert_char(
            &mut self.query,
            &mut self.query_state.cursor,
            &mut self.query_state.selected_all,
            value,
            SPOTLIGHT_MAX_QUERY_CHARS,
        ) {
            self.rebuild_results();
        }
    }

    pub(super) fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.screen = primary_screen();
        self.renderer = None;
        self.icons_attempted.clear();
        if self.visible {
            self.work_area = monitor_work_area_at(self.window.x, self.window.y);
            self.target_window = self.target_rect();
            self.source_window = self.window;
            self.window = self.clamp_window(self.window, self.work_area);
            self.sync_layout();
            self.sync_backdrop(runtime, true);
            self.present(runtime);
        }
    }
}
