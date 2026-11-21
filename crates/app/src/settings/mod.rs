use otto_core::{
    ContentLayout, ModuleRow, OttoSettingsSnapshot, SettingsContent, SettingsHit, SettingsLayout,
    SettingsPage, build_content, compute_content_layout, compute_settings_layout, resolve_modules,
};
use otto_platform::{settings_client_size, settings_window_dpi};
use otto_render::{SettingsPaintState, SettingsRenderer};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::ValidateRect;

use crate::host::{RuntimeContext, module_status};

mod actions;
mod input;
mod lifecycle;
mod state;

pub(crate) use self::input::handle_message;
pub(crate) use self::lifecycle::{open, repaint, repaint_if_focus_page, take_hwnd, toggle};
pub(crate) use self::state::settings_snapshot;
#[cfg(test)]
pub(crate) use self::state::toggle_setting;

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
    pub(super) update_status: Option<String>,
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
            update_status: None,
            tracking_leave: false,
            renderer: None,
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
    let (modules, snapshot) = modules_for(runtime, launch);
    let update_status = with_window(runtime, |window| window.update_status.clone()).flatten();
    let content = build_content(
        &snapshot,
        page,
        &modules,
        env!("CARGO_PKG_VERSION"),
        update_status.as_deref(),
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
        }
        let drawn = window.renderer.as_mut().map(|renderer| {
            renderer.draw(&SettingsPaintState {
                layout: &window.layout,
                content: &window.content,
                blocks: &window.blocks,
                page: window.page,
                scroll: window.scroll,
                hovered: window.hovered,
                dark: otto_platform::is_system_in_dark_mode(),
                glass: window.glass,
            })
        });
        if !matches!(drawn, Some(Ok(()))) {
            window.renderer = None;
        }
    });
    unsafe {
        let _ = ValidateRect(Some(HWND(hwnd as *mut _)), None);
    }
}
