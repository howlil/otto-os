use crate::app_model::{AppCommand, AppInfo, Capability, WindowCommand, WindowInfo};
use crate::layout::HitTarget;

use super::commands::CMD_OVERFLOW_MENU_BASE;
use super::rows::MenuRow;

#[must_use]
pub fn is_top_bar_menu_target(target: HitTarget) -> bool {
    matches!(
        target,
        HitTarget::Start
            | HitTarget::AppMenu
            | HitTarget::FileMenu
            | HitTarget::EditMenu
            | HitTarget::ViewMenu
            | HitTarget::WindowMenu
            | HitTarget::HelpMenu
            | HitTarget::MenuOverflow
    )
}

#[must_use]
pub const fn top_bar_menu_title(target: HitTarget) -> Option<&'static str> {
    match target {
        HitTarget::FileMenu => Some("File"),
        HitTarget::EditMenu => Some("Edit"),
        HitTarget::ViewMenu => Some("View"),
        HitTarget::WindowMenu => Some("Window"),
        HitTarget::HelpMenu => Some("Help"),
        _ => None,
    }
}

pub const APPLICATION_MENUS: [HitTarget; 5] = [
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
];

pub const OVERFLOW_TARGETS: [HitTarget; 13] = [
    HitTarget::Start,
    HitTarget::AppMenu,
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
    HitTarget::Tray,
    HitTarget::QuickSettings,
    HitTarget::Search,
    HitTarget::ControlCenter,
    HitTarget::Notifications,
    HitTarget::Clock,
];

pub(super) fn overflow_title(target: HitTarget, app_name: &str) -> Option<String> {
    let title = match target {
        HitTarget::Start => "System",
        HitTarget::AppMenu => app_name,
        HitTarget::Tray => "Tray Icons",
        HitTarget::QuickSettings => "Quick Settings",
        HitTarget::Search => "Search",
        HitTarget::ControlCenter => "Control Center",
        HitTarget::Notifications => "Notifications",
        HitTarget::Clock => "Calendar",
        other => top_bar_menu_title(other)?,
    };
    Some(String::from(title))
}

#[must_use]
pub fn build_menu_overflow_rows(hidden: &[HitTarget], app_name: &str) -> Vec<MenuRow> {
    hidden
        .iter()
        .filter_map(|target| {
            let position = OVERFLOW_TARGETS
                .iter()
                .position(|candidate| candidate == target)?;
            Some(MenuRow::item(
                CMD_OVERFLOW_MENU_BASE + position,
                overflow_title(*target, app_name)?,
                "",
            ))
        })
        .collect()
}

#[must_use]
pub fn overflow_menu_target(command: usize) -> Option<HitTarget> {
    OVERFLOW_TARGETS
        .get(command.checked_sub(CMD_OVERFLOW_MENU_BASE)?)
        .copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMenuState {
    pub close: Capability,
    pub minimize: Capability,
    pub zoom: Capability,
    pub full_screen: Capability,
    pub tile: Capability,
    pub shortcuts: Capability,
    pub zoomed: bool,
    pub full_screen_active: bool,
}

impl WindowMenuState {
    #[must_use]
    pub const fn of(window: &WindowInfo) -> Self {
        let shortcuts = if window.traits.elevated {
            Capability::Denied
        } else if !window.traits.responding || window.traits.blocked_by_modal {
            Capability::TemporarilyUnavailable
        } else {
            Capability::Available
        };
        Self {
            close: window.capability(WindowCommand::Close),
            minimize: window.capability(WindowCommand::Minimize),
            zoom: window.capability(WindowCommand::Zoom),
            full_screen: window.capability(WindowCommand::ToggleFullScreen),
            tile: window.capability(WindowCommand::TileLeft),
            shortcuts,
            zoomed: window.zoomed,
            full_screen_active: window.full_screen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMenuState {
    pub hide: Capability,
    pub hide_others: Capability,
    pub show_all: Capability,
    pub quit: Capability,
}

impl AppMenuState {
    pub const NONE: Self = Self {
        hide: Capability::Unsupported,
        hide_others: Capability::Unsupported,
        show_all: Capability::Unsupported,
        quit: Capability::Unsupported,
    };

    #[must_use]
    pub fn of(app: &AppInfo, other_apps_visible: bool) -> Self {
        Self {
            hide: app.capability(AppCommand::Hide, other_apps_visible),
            hide_others: app.capability(AppCommand::HideOthers, other_apps_visible),
            show_all: match app.capability(AppCommand::ShowAll, other_apps_visible) {
                Capability::Available => Capability::Available,
                _ => Capability::Unsupported,
            },
            quit: app.capability(AppCommand::Quit, other_apps_visible),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMenuEntry {
    pub title: String,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuContext<'a> {
    pub app_name: &'a str,
    pub file_manager: bool,
    pub tabbed: bool,
    pub window: Option<WindowMenuState>,
    pub app: AppMenuState,
    pub app_windows: &'a [WindowMenuEntry],
}

impl<'a> MenuContext<'a> {
    #[must_use]
    pub const fn without_window(app_name: &'a str) -> Self {
        Self {
            app_name,
            file_manager: false,
            tabbed: false,
            window: None,
            app: AppMenuState::NONE,
            app_windows: &[],
        }
    }

    pub(super) fn shortcuts(&self) -> Capability {
        self.window
            .map_or(Capability::Unsupported, |window| window.shortcuts)
    }

    pub(super) fn window_capability(
        &self,
        select: impl Fn(&WindowMenuState) -> Capability,
    ) -> Capability {
        self.window.as_ref().map_or(Capability::Unsupported, select)
    }
}
