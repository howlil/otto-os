use std::collections::HashSet;

use otto_core::{
    BarWindowControlsView, ContentLayout, FocusAppFilter, FocusPageState, FocusTextField,
    GeneralPageState, ModuleRow, OttoSettingsSnapshot, SettingsContent, SettingsHit,
    SettingsLayout, SettingsPage, UpdateState, WindowControlsState, build_content,
    compute_content_layout, compute_settings_layout, resolve_modules,
};
use otto_platform::{
    collect_focus_catalog_apps, collect_process_audit, current_process_id, extract_app_icon_bitmap,
    is_settings_window_maximized, settings_client_size, settings_window_dpi, validate_window_rect,
};
use otto_render::{SettingsPaintState, SettingsRenderer};

use crate::host::{RuntimeContext, module_status};

mod actions;
mod input;
mod lifecycle;
mod state;

pub(crate) use self::input::handle_message;
pub(crate) use self::lifecycle::{
    open, open_focus_activity, open_focus_modes, refresh_appearance,
    refresh_process_audit_if_general, repaint, repaint_if_focus_page, take_hwnd, toggle,
};
pub(crate) use self::state::settings_snapshot;
#[cfg(test)]
pub(crate) use self::state::toggle_setting;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CatalogLoadState {
    Unloaded,
    Loaded,
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct SettingsWindow {
    pub(super) hwnd: isize,
    pub(super) page: SettingsPage,
    pub(super) layout: SettingsLayout,
    pub(super) content: SettingsContent,
    pub(super) blocks: ContentLayout,
    pub(super) hovered: Option<SettingsHit>,
    pub(super) pressed: Option<SettingsHit>,
    pub(super) scroll: i32,
    pub(super) dpi: u32,
    pub(super) launch_at_login: bool,
    pub(super) glass: bool,
    pub(super) active: bool,
    pub(super) maximized: bool,
    pub(super) alt_down: bool,
    pub(super) green_hover_armed: bool,
    pub(super) update_state: UpdateState,
    pub(super) focus_state: FocusPageState,
    pub(super) focus_catalog: CatalogLoadState,
    pub(super) general_state: GeneralPageState,
    pub(super) audit_loaded: CatalogLoadState,
    pub(super) icons_attempted: HashSet<String>,
    pub(super) pending_high_surrogate: Option<u16>,
    pub(super) tracking_leave: bool,
    pub(super) renderer: Option<SettingsRenderer>,
}

impl SettingsWindow {
    pub(super) fn new(hwnd: isize) -> Self {
        let dpi = settings_window_dpi(hwnd);
        let (width, height) = settings_client_size(hwnd);
        Self {
            hwnd,
            page: SettingsPage::Overview,
            layout: compute_settings_layout(width, height, dpi),
            content: SettingsContent::default(),
            blocks: ContentLayout::default(),
            hovered: None,
            pressed: None,
            scroll: 0,
            dpi,
            launch_at_login: false,
            glass: false,
            active: true,
            maximized: is_settings_window_maximized(hwnd),
            alt_down: false,
            green_hover_armed: false,
            update_state: UpdateState::Idle,
            focus_state: FocusPageState::default(),
            focus_catalog: CatalogLoadState::Unloaded,
            general_state: GeneralPageState::default(),
            audit_loaded: CatalogLoadState::Unloaded,
            icons_attempted: HashSet::new(),
            pending_high_surrogate: None,
            tracking_leave: false,
            renderer: None,
        }
    }

    pub(super) fn ensure_focus_catalog(&mut self, has_recent_activity: bool) {
        if self.focus_catalog == CatalogLoadState::Loaded {
            return;
        }
        self.focus_state.catalog = collect_focus_catalog_apps();
        self.focus_catalog = CatalogLoadState::Loaded;
        if !has_recent_activity && self.focus_state.app_filter == FocusAppFilter::Recent {
            self.focus_state.app_filter = FocusAppFilter::AllApps;
        }
    }

    pub(super) fn refresh_process_audit(&mut self) {
        self.general_state.audit = collect_process_audit(current_process_id());
        self.audit_loaded = CatalogLoadState::Loaded;
    }

    pub(super) fn ensure_process_audit(&mut self) {
        if self.audit_loaded != CatalogLoadState::Loaded {
            self.refresh_process_audit();
        }
    }

    pub(super) fn sync_visible_app_icons(&mut self) {
        if !matches!(self.page, SettingsPage::Focus | SettingsPage::General) {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let viewport = self.layout.viewport;
        let mut visible_keys: HashSet<String> = HashSet::new();
        let mut pending: Vec<(String, Option<String>)> = Vec::new();
        for (section, card) in self.content.sections.iter().zip(&self.blocks.cards) {
            for (row, rect) in section.rows.iter().zip(&card.rows) {
                if rect.bottom() < viewport.y || rect.y > viewport.bottom() {
                    continue;
                }
                let Some(icon_key) = row.app_icon.as_deref() else {
                    continue;
                };
                visible_keys.insert(icon_key.to_owned());
                if !renderer.has_app_icon(icon_key)
                    && self.icons_attempted.insert(icon_key.to_owned())
                {
                    pending.push((icon_key.to_owned(), row.detail_hint.clone()));
                }
            }
        }
        let visible_refs: HashSet<&str> = visible_keys.iter().map(String::as_str).collect();
        renderer.retain_app_icons(&visible_refs);
        self.icons_attempted
            .retain(|key| visible_refs.contains(key.as_str()));
        for (icon_key, detail) in pending {
            if let Some(icon) = extract_app_icon_bitmap(&icon_key, detail.as_deref()) {
                let _ = renderer.set_app_icon(icon_key, icon.width, icon.height, &icon.pixels);
            }
        }
    }

    pub(super) fn recompute(&mut self) {
        let mut blocks = compute_content_layout(self.layout.viewport, &self.content, self.dpi, 0);
        self.scroll = self.scroll.clamp(0, self.layout.max_scroll(&blocks));
        if self.scroll > 0 {
            blocks =
                compute_content_layout(self.layout.viewport, &self.content, self.dpi, self.scroll);
        }
        self.blocks = blocks;
    }

    pub(super) fn relayout(&mut self) {
        let (width, height) = settings_client_size(self.hwnd);
        self.dpi = settings_window_dpi(self.hwnd);
        self.layout = compute_settings_layout(width, height, self.dpi);
        self.recompute();
    }

    pub(super) fn select_page(&mut self, page: SettingsPage) {
        self.page = page;
        self.scroll = 0;
        self.hovered = None;
        self.pressed = None;
        self.focus_state.focused_field = FocusTextField::None;
        self.focus_state.website_error = None;
        self.general_state.status_note = None;
        if page == SettingsPage::General {
            self.audit_loaded = CatalogLoadState::Unloaded;
        }
        self.pending_high_surrogate = None;
    }

    pub(super) fn hit(&self, x: i32, y: i32) -> Option<SettingsHit> {
        self.layout.hit_test(x, y, &self.blocks)
    }
}

pub(super) fn with_window<T>(
    runtime: &RuntimeContext,
    action: impl FnOnce(&mut SettingsWindow) -> T,
) -> Option<T> {
    let mut guard = runtime.settings_window.try_borrow_mut().ok()?;
    guard.as_mut().map(action)
}

fn modules_for(
    runtime: &RuntimeContext,
    launch_at_login: bool,
) -> (Vec<ModuleRow>, OttoSettingsSnapshot) {
    let snapshot = settings_snapshot(runtime, launch_at_login);
    let modules = resolve_modules(&snapshot, module_status);
    (modules, snapshot)
}

pub(super) fn refresh_model(runtime: &RuntimeContext) {
    let Some((page, launch)) = with_window(runtime, |window| (window.page, window.launch_at_login))
    else {
        return;
    };
    let (modules, mut snapshot) = modules_for(runtime, launch);
    let session = if page == SettingsPage::Focus {
        crate::bar::focus_session_status(runtime)
    } else {
        otto_core::FocusSessionStatus::NotRunning
    };
    let has_recent = !snapshot.focus_activity.today.apps.is_empty()
        || snapshot
            .focus_activity
            .history
            .iter()
            .any(|day| !day.apps.is_empty());
    let Some((update_state, focus_state, general_state)) = with_window(runtime, |window| {
        if page == SettingsPage::Focus {
            window.ensure_focus_catalog(has_recent);
            window.focus_state.session = session;
        } else if page == SettingsPage::General {
            window.ensure_process_audit();
        }
        (
            window.update_state.clone(),
            window.focus_state.clone(),
            window.general_state.clone(),
        )
    }) else {
        return;
    };
    snapshot.process_audit = general_state;
    let content = build_content(
        &snapshot,
        page,
        &modules,
        env!("CARGO_PKG_VERSION"),
        &update_state,
        &focus_state,
    );
    with_window(runtime, |window| {
        window.content = content;
        window.recompute();
    });
}

pub(super) fn paint(runtime: &RuntimeContext, hwnd: isize) {
    with_window(runtime, |window| {
        if window.renderer.is_none() {
            let (width, height) = settings_client_size(hwnd);
            window.renderer = SettingsRenderer::new(hwnd, width, height, window.dpi).ok();
            window.icons_attempted.clear();
        }
        window.sync_visible_app_icons();
        let app = crate::theme_state::appearance(runtime);
        let accent = app.resolved_accent();
        let hovered_control = matches!(window.hovered, Some(SettingsHit::WindowControl(_)));
        let pressed_control = match (window.pressed, window.hovered) {
            (Some(SettingsHit::WindowControl(p)), Some(SettingsHit::WindowControl(h)))
                if p == h =>
            {
                Some(p)
            }
            _ => None,
        };
        let window_controls = BarWindowControlsView {
            state: WindowControlsState {
                active: window.active,
                close: true,
                minimize: true,
                zoom: true,
                alt_zoom_glyph: hovered_control && window.alt_down,
            },
            maximized: window.maximized,
            hovered: hovered_control,
            pressed: pressed_control,
        };
        let drawn = window.renderer.as_mut().map(|renderer| {
            renderer.draw(&SettingsPaintState {
                layout: &window.layout,
                content: &window.content,
                blocks: &window.blocks,
                page: window.page,
                scroll: window.scroll,
                hovered: window.hovered,
                window_controls,
                dark: app.content_theme().is_dark(),
                glass: window.glass,
                high_contrast: app.contrast.is_some(),
                contrast: app.contrast,
                accent: accent.fill,
                on_accent: accent.on_accent,
            })
        });
        if !matches!(drawn, Some(Ok(()))) {
            window.renderer = None;
            window.icons_attempted.clear();
        }
    });
    validate_window_rect(hwnd);
}
