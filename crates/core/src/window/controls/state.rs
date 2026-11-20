use crate::app_model::{AppInfo, WindowCommand, WindowIdentity, WindowInfo, WindowPresence};

use super::layout::WindowControlAction;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowControlsState {
    pub active: bool,
    pub close: bool,
    pub minimize: bool,
    pub zoom: bool,
    pub alt_zoom_glyph: bool,
}

impl WindowControlsState {
    #[must_use]
    pub const fn of(window: &WindowInfo) -> Self {
        Self {
            active: window.active,
            close: window.capability(WindowCommand::Close).is_available(),
            minimize: window.capability(WindowCommand::Minimize).is_available(),
            zoom: window.capability(WindowCommand::Zoom).is_available()
                || window
                    .capability(WindowCommand::ToggleFullScreen)
                    .is_available(),
            alt_zoom_glyph: false,
        }
    }

    #[must_use]
    pub const fn with_alt_zoom_glyph(mut self, alt: bool) -> Self {
        self.alt_zoom_glyph = alt;
        self
    }

    #[must_use]
    pub const fn allows(self, action: WindowControlAction) -> bool {
        match action {
            WindowControlAction::Close => self.close,
            WindowControlAction::Minimize => self.minimize,
            WindowControlAction::ToggleMaximize => self.zoom,
        }
    }

    #[must_use]
    pub const fn shows_color(self, action: WindowControlAction, hovered: bool) -> bool {
        self.allows(action) && (self.active || hovered)
    }

    #[must_use]
    pub const fn any(self) -> bool {
        self.close || self.minimize || self.zoom
    }
}

#[must_use]
pub const fn accepts_window_controls(window: &WindowInfo) -> bool {
    window.traits.title_bar
        && !window.traits.elevated
        && !window.full_screen
        && matches!(window.presence, WindowPresence::Visible)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandScope {
    Window,
    Application,
}

#[must_use]
pub const fn window_control_scope(action: WindowControlAction, alt: bool) -> CommandScope {
    match action {
        WindowControlAction::Close | WindowControlAction::Minimize if alt => {
            CommandScope::Application
        }
        _ => CommandScope::Window,
    }
}

#[must_use]
pub fn application_command_targets(
    app: &AppInfo,
    clicked: WindowIdentity,
    command: WindowCommand,
) -> Vec<WindowIdentity> {
    let clicked_window = app
        .windows
        .iter()
        .find(|window| window.identity.is_same_window(&clicked));
    let siblings = app
        .primary_windows()
        .filter(|window| !window.identity.is_same_window(&clicked));
    clicked_window
        .into_iter()
        .chain(siblings)
        .filter(|window| window.presence != WindowPresence::Hidden)
        .filter(|window| window.capability(command).is_available())
        .map(|window| window.identity)
        .collect()
}

#[must_use]
pub const fn window_controls_visible_under_bar(
    caption_top: i32,
    caption_height: i32,
    bar_height: Option<i32>,
) -> bool {
    match bar_height {
        Some(bar) => caption_top + caption_height > bar,
        None => true,
    }
}

#[must_use]
pub const fn window_controls_top(
    window_top: i32,
    offset_y: i32,
    bar_height: Option<i32>,
    maximized: bool,
) -> i32 {
    let natural = window_top + offset_y;
    match bar_height {
        Some(bar) if maximized && natural < bar + offset_y => bar + offset_y,
        Some(bar) if natural < bar => bar,
        _ => natural,
    }
}
