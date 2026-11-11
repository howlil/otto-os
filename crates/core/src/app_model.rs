#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppIdentity {
    UserModelId(String),
    Executable(String),
}

impl AppIdentity {
    #[must_use]
    pub fn user_model_id(id: &str) -> Self {
        Self::UserModelId(String::from(id.trim()))
    }

    #[must_use]
    pub fn executable(path: &str) -> Self {
        Self::Executable(path.trim().to_lowercase())
    }

    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::UserModelId(id) => format!("aumid:{id}"),
            Self::Executable(path) => path.clone(),
        }
    }

    #[must_use]
    pub const fn is_executable_fallback(&self) -> bool {
        matches!(self, Self::Executable(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowIdentity {
    pub hwnd: isize,
    pub process_id: u32,
    pub thread_id: u32,
    pub generation: u64,
}

impl WindowIdentity {
    #[must_use]
    pub const fn is_same_window(&self, live: &Self) -> bool {
        self.hwnd == live.hwnd
            && self.process_id == live.process_id
            && self.thread_id == live.thread_id
            && self.generation == live.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Available,
    Unsupported,
    TemporarilyUnavailable,
    Denied,
}

impl Capability {
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    #[must_use]
    pub const fn first_blocker(self, other: Self) -> Self {
        if self.is_available() { other } else { self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    Completed,
    Unsupported,
    TemporarilyUnavailable,
    Denied,
    TargetGone,
    TimedOut,
    Cancelled,
}

impl CommandOutcome {
    #[must_use]
    pub const fn from_capability(capability: Capability) -> Option<Self> {
        match capability {
            Capability::Available => None,
            Capability::Unsupported => Some(Self::Unsupported),
            Capability::TemporarilyUnavailable => Some(Self::TemporarilyUnavailable),
            Capability::Denied => Some(Self::Denied),
        }
    }

    #[must_use]
    pub const fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPresence {
    Visible,
    Minimized,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRole {
    Primary,
    OwnedDialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct WindowTraits {
    pub closable: bool,
    pub minimizable: bool,
    pub maximizable: bool,
    pub resizable: bool,
    pub blocked_by_modal: bool,
    pub elevated: bool,
    pub responding: bool,
    pub full_screen_shortcut: bool,
    pub title_bar: bool,
}

impl WindowTraits {
    pub const STANDARD: Self = Self {
        closable: true,
        minimizable: true,
        maximizable: true,
        resizable: true,
        blocked_by_modal: false,
        elevated: false,
        responding: true,
        full_screen_shortcut: false,
        title_bar: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    Focus,
    Close,
    Minimize,
    Zoom,
    ToggleFullScreen,
    TileLeft,
    TileRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub identity: WindowIdentity,
    pub app: AppIdentity,
    pub app_name: String,
    pub executable: String,
    pub title: String,
    pub role: WindowRole,
    pub presence: WindowPresence,
    pub zoomed: bool,
    pub full_screen: bool,
    pub active: bool,
    pub traits: WindowTraits,
}

impl WindowInfo {
    #[must_use]
    pub const fn capability(&self, command: WindowCommand) -> Capability {
        if matches!(command, WindowCommand::Focus) {
            return Capability::Available;
        }
        if self.traits.elevated {
            return Capability::Denied;
        }
        if !self.traits.responding || self.traits.blocked_by_modal {
            return Capability::TemporarilyUnavailable;
        }
        match command {
            WindowCommand::Focus => Capability::Available,
            WindowCommand::Close => supported(self.traits.closable),
            WindowCommand::Minimize => {
                if !self.traits.minimizable {
                    Capability::Unsupported
                } else if matches!(self.presence, WindowPresence::Visible) {
                    Capability::Available
                } else {
                    Capability::TemporarilyUnavailable
                }
            }
            WindowCommand::Zoom => {
                if !self.traits.maximizable {
                    Capability::Unsupported
                } else if self.full_screen {
                    Capability::TemporarilyUnavailable
                } else {
                    Capability::Available
                }
            }
            WindowCommand::ToggleFullScreen => supported(self.traits.full_screen_shortcut),
            WindowCommand::TileLeft | WindowCommand::TileRight => {
                if !self.traits.resizable {
                    Capability::Unsupported
                } else if self.full_screen {
                    Capability::TemporarilyUnavailable
                } else {
                    Capability::Available
                }
            }
        }
    }

    #[must_use]
    pub const fn is_primary(&self) -> bool {
        matches!(self.role, WindowRole::Primary)
    }
}

const fn supported(flag: bool) -> Capability {
    if flag {
        Capability::Available
    } else {
        Capability::Unsupported
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    Activate,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub identity: AppIdentity,
    pub name: String,
    pub executable: String,
    pub windows: Vec<WindowInfo>,
}

impl AppInfo {
    pub fn primary_windows(&self) -> impl Iterator<Item = &WindowInfo> {
        self.windows.iter().filter(|window| window.is_primary())
    }

    #[must_use]
    pub fn process_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .windows
            .iter()
            .map(|window| window.identity.process_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.windows.iter().any(|window| window.active)
    }

    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.has_primary_window()
            && self
                .primary_windows()
                .all(|window| window.presence == WindowPresence::Hidden)
    }

    #[must_use]
    pub fn is_minimized(&self) -> bool {
        self.has_primary_window()
            && !self.is_hidden()
            && self
                .primary_windows()
                .all(|window| window.presence != WindowPresence::Visible)
    }

    #[must_use]
    pub fn has_primary_window(&self) -> bool {
        self.primary_windows().next().is_some()
    }

    #[must_use]
    pub fn capability(&self, command: AppCommand, other_apps_visible: bool) -> Capability {
        let elevated = self.windows.iter().any(|window| window.traits.elevated);
        let any_presence = |presence| {
            self.primary_windows()
                .any(|window| window.presence == presence)
        };
        match command {
            AppCommand::Activate => Capability::Available,
            AppCommand::Hide => {
                if !self.has_primary_window() {
                    Capability::Unsupported
                } else if elevated {
                    Capability::Denied
                } else if any_presence(WindowPresence::Visible)
                    || any_presence(WindowPresence::Minimized)
                {
                    Capability::Available
                } else {
                    Capability::TemporarilyUnavailable
                }
            }
            AppCommand::HideOthers => {
                if other_apps_visible {
                    Capability::Available
                } else {
                    Capability::TemporarilyUnavailable
                }
            }
            AppCommand::ShowAll => {
                if any_presence(WindowPresence::Hidden) {
                    Capability::Available
                } else {
                    Capability::TemporarilyUnavailable
                }
            }
            AppCommand::Quit => {
                if !self.has_primary_window() {
                    Capability::Unsupported
                } else if elevated {
                    Capability::Denied
                } else if self.primary_windows().any(|window| window.traits.closable) {
                    Capability::Available
                } else {
                    Capability::Unsupported
                }
            }
        }
    }
}

#[must_use]
pub fn group_windows(windows: Vec<WindowInfo>) -> Vec<AppInfo> {
    let mut apps: Vec<AppInfo> = Vec::new();
    for window in windows {
        if let Some(app) = apps.iter_mut().find(|app| app.identity == window.app) {
            app.windows.push(window);
        } else {
            apps.push(AppInfo {
                identity: window.app.clone(),
                name: window.app_name.clone(),
                executable: window.executable.clone(),
                windows: vec![window],
            });
        }
    }
    apps
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StableOrder {
    keys: Vec<String>,
}

impl StableOrder {
    pub fn arrange(&mut self, present: &[String]) -> Vec<usize> {
        self.keys.retain(|key| present.contains(key));
        for key in present {
            if !self.keys.contains(key) {
                self.keys.push(key.clone());
            }
        }
        self.keys
            .iter()
            .filter_map(|key| present.iter().position(|candidate| candidate == key))
            .collect()
    }

    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Launch,
    Unhide,
    Restore(WindowIdentity),
    Focus(WindowIdentity),
}

#[must_use]
pub fn decide_activation(app: &AppInfo) -> Activation {
    let primary: Vec<&WindowInfo> = app.primary_windows().collect();
    let Some(first) = primary.first() else {
        return Activation::Launch;
    };
    if primary
        .iter()
        .any(|window| window.presence == WindowPresence::Hidden)
    {
        return Activation::Unhide;
    }
    let visible: Vec<&WindowInfo> = primary
        .iter()
        .copied()
        .filter(|window| window.presence == WindowPresence::Visible)
        .collect();
    if let Some(active_index) = primary.iter().position(|window| window.active) {
        if visible.len() > 1 {
            let active = primary[active_index].identity;
            let position = visible
                .iter()
                .position(|window| window.identity == active)
                .unwrap_or(0);
            return Activation::Focus(visible[(position + 1) % visible.len()].identity);
        }
        return Activation::Focus(primary[active_index].identity);
    }
    match visible.first() {
        Some(window) => Activation::Focus(window.identity),
        None => Activation::Restore(first.identity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(hwnd: isize, process_id: u32) -> WindowIdentity {
        WindowIdentity {
            hwnd,
            process_id,
            thread_id: process_id + 1,
            generation: u64::from(process_id) * 10,
        }
    }

    fn window(hwnd: isize, process_id: u32, exe: &str) -> WindowInfo {
        WindowInfo {
            identity: identity(hwnd, process_id),
            app: AppIdentity::executable(exe),
            app_name: String::from(exe),
            executable: String::from(exe),
            title: format!("window {hwnd}"),
            role: WindowRole::Primary,
            presence: WindowPresence::Visible,
            zoomed: false,
            full_screen: false,
            active: false,
            traits: WindowTraits::STANDARD,
        }
    }

    fn app(windows: Vec<WindowInfo>) -> AppInfo {
        group_windows(windows).remove(0)
    }

    #[test]
    fn identity_distinguishes_a_reused_handle_from_the_original_window() {
        let original = identity(100, 7);
        let reused_by_other_process = WindowIdentity {
            process_id: 9,
            ..original
        };
        let reused_after_restart = WindowIdentity {
            generation: original.generation + 1,
            ..original
        };
        assert!(original.is_same_window(&original));
        assert!(!original.is_same_window(&reused_by_other_process));
        assert!(!original.is_same_window(&reused_after_restart));
    }

    #[test]
    fn user_model_identity_is_preferred_and_executable_is_an_explicit_fallback() {
        let packaged = AppIdentity::user_model_id("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App");
        let classic = AppIdentity::executable("C:\\Tools\\App.EXE");
        assert!(!packaged.is_executable_fallback());
        assert!(classic.is_executable_fallback());
        assert_eq!(classic.key(), "c:\\tools\\app.exe");
        assert!(packaged.key().starts_with("aumid:"));
    }

    #[test]
    fn windows_of_several_processes_group_under_one_application() {
        let apps = group_windows(vec![
            window(1, 10, "browser.exe"),
            window(2, 20, "editor.exe"),
            window(3, 11, "browser.exe"),
        ]);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].windows.len(), 2);
        assert_eq!(apps[0].process_ids(), vec![10, 11]);
        assert_eq!(apps[1].windows.len(), 1);
    }

    #[test]
    fn close_and_quit_are_separate_capabilities() {
        let mut fixed = window(1, 10, "tool.exe");
        fixed.traits.closable = false;
        let mut closable = window(2, 10, "tool.exe");
        closable.role = WindowRole::Primary;
        let application = app(vec![fixed.clone(), closable]);
        assert_eq!(
            fixed.capability(WindowCommand::Close),
            Capability::Unsupported
        );
        assert_eq!(
            application.capability(AppCommand::Quit, false),
            Capability::Available
        );
    }

    #[test]
    fn hide_and_minimize_are_separate_capabilities() {
        let mut no_minimize_box = window(1, 10, "dialogish.exe");
        no_minimize_box.traits.minimizable = false;
        let application = app(vec![no_minimize_box.clone()]);
        assert_eq!(
            no_minimize_box.capability(WindowCommand::Minimize),
            Capability::Unsupported
        );
        assert_eq!(
            application.capability(AppCommand::Hide, false),
            Capability::Available
        );
    }

    #[test]
    fn full_screen_is_not_offered_through_zoom() {
        let plain = window(1, 10, "notepad.exe");
        assert_eq!(plain.capability(WindowCommand::Zoom), Capability::Available);
        assert_eq!(
            plain.capability(WindowCommand::ToggleFullScreen),
            Capability::Unsupported
        );
        let mut browser = window(2, 20, "browser.exe");
        browser.traits.full_screen_shortcut = true;
        browser.full_screen = true;
        assert_eq!(
            browser.capability(WindowCommand::ToggleFullScreen),
            Capability::Available
        );
        assert_eq!(
            browser.capability(WindowCommand::Zoom),
            Capability::TemporarilyUnavailable
        );
    }

    #[test]
    fn elevated_hung_and_modal_blocked_windows_report_why_they_refuse() {
        let mut elevated = window(1, 10, "admin.exe");
        elevated.traits.elevated = true;
        assert_eq!(
            elevated.capability(WindowCommand::Close),
            Capability::Denied
        );
        assert_eq!(
            elevated.capability(WindowCommand::Focus),
            Capability::Available
        );
        let mut hung = window(2, 20, "hung.exe");
        hung.traits.responding = false;
        assert_eq!(
            hung.capability(WindowCommand::Minimize),
            Capability::TemporarilyUnavailable
        );
        let mut blocked = window(3, 30, "editor.exe");
        blocked.traits.blocked_by_modal = true;
        assert_eq!(
            blocked.capability(WindowCommand::Close),
            Capability::TemporarilyUnavailable
        );
        assert_eq!(
            app(vec![elevated]).capability(AppCommand::Quit, false),
            Capability::Denied
        );
    }

    #[test]
    fn application_without_windows_cannot_be_hidden_or_quit() {
        let windowless = AppInfo {
            identity: AppIdentity::executable("agent.exe"),
            name: String::from("Agent"),
            executable: String::from("agent.exe"),
            windows: Vec::new(),
        };
        assert_eq!(
            windowless.capability(AppCommand::Hide, true),
            Capability::Unsupported
        );
        assert_eq!(
            windowless.capability(AppCommand::Quit, true),
            Capability::Unsupported
        );
        assert_eq!(decide_activation(&windowless), Activation::Launch);
    }

    #[test]
    fn owned_dialogs_do_not_count_as_application_windows() {
        let mut dialog = window(5, 10, "editor.exe");
        dialog.role = WindowRole::OwnedDialog;
        let application = app(vec![dialog]);
        assert!(!application.has_primary_window());
        assert_eq!(decide_activation(&application), Activation::Launch);
    }

    #[test]
    fn hidden_and_minimized_states_are_reported_separately() {
        let mut hidden = window(1, 10, "mail.exe");
        hidden.presence = WindowPresence::Hidden;
        let hidden_app = app(vec![hidden]);
        assert!(hidden_app.is_hidden());
        assert!(!hidden_app.is_minimized());
        assert_eq!(
            hidden_app.capability(AppCommand::ShowAll, false),
            Capability::Available
        );
        assert_eq!(decide_activation(&hidden_app), Activation::Unhide);

        let mut minimized = window(2, 20, "notes.exe");
        minimized.presence = WindowPresence::Minimized;
        let minimized_app = app(vec![minimized.clone()]);
        assert!(minimized_app.is_minimized());
        assert!(!minimized_app.is_hidden());
        assert_eq!(
            decide_activation(&minimized_app),
            Activation::Restore(minimized.identity)
        );
    }

    #[test]
    fn activation_cycles_visible_windows_of_the_active_application() {
        let mut first = window(1, 10, "editor.exe");
        first.active = true;
        let second = window(2, 10, "editor.exe");
        let mut parked = window(3, 10, "editor.exe");
        parked.presence = WindowPresence::Minimized;
        let application = app(vec![first.clone(), second.clone(), parked]);
        assert_eq!(
            decide_activation(&application),
            Activation::Focus(second.identity)
        );

        let inactive = app(vec![
            window(7, 30, "viewer.exe"),
            window(8, 30, "viewer.exe"),
        ]);
        assert_eq!(
            decide_activation(&inactive),
            Activation::Focus(identity(7, 30))
        );
        let single_active = app(vec![first.clone()]);
        assert_eq!(
            decide_activation(&single_active),
            Activation::Focus(first.identity)
        );
    }

    #[test]
    fn hide_others_depends_on_other_visible_applications() {
        let application = app(vec![window(1, 10, "editor.exe")]);
        assert_eq!(
            application.capability(AppCommand::HideOthers, true),
            Capability::Available
        );
        assert_eq!(
            application.capability(AppCommand::HideOthers, false),
            Capability::TemporarilyUnavailable
        );
    }

    #[test]
    fn stable_order_keeps_positions_across_focus_changes() {
        let mut order = StableOrder::default();
        let first = [String::from("a"), String::from("b"), String::from("c")];
        assert_eq!(order.arrange(&first), vec![0, 1, 2]);
        let refocused = [String::from("c"), String::from("a"), String::from("b")];
        assert_eq!(order.arrange(&refocused), vec![1, 2, 0]);
        let changed = [String::from("d"), String::from("c"), String::from("a")];
        assert_eq!(order.arrange(&changed), vec![2, 1, 0]);
        assert_eq!(order.keys(), ["a", "c", "d"]);
    }

    #[test]
    fn outcome_reports_the_blocking_capability() {
        assert_eq!(CommandOutcome::from_capability(Capability::Available), None);
        assert_eq!(
            CommandOutcome::from_capability(Capability::Denied),
            Some(CommandOutcome::Denied)
        );
        assert_eq!(
            Capability::Available.first_blocker(Capability::Unsupported),
            Capability::Unsupported
        );
        assert_eq!(
            Capability::Denied.first_blocker(Capability::Unsupported),
            Capability::Denied
        );
    }
}
