#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BarHideMode {
    Always,
    DesktopOnly,
    #[default]
    FullScreenOnly,
    Never,
}

impl BarHideMode {
    #[must_use]
    pub const fn hides(self, full_screen_app: bool) -> bool {
        match self {
            Self::Always => true,
            Self::DesktopOnly => !full_screen_app,
            Self::FullScreenOnly => full_screen_app,
            Self::Never => false,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::FullScreenOnly => Self::DesktopOnly,
            Self::DesktopOnly => Self::Always,
            Self::Always => Self::Never,
            Self::Never => Self::FullScreenOnly,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Always => "Always",
            Self::DesktopOnly => "Desktop",
            Self::FullScreenOnly => "Full Screen",
            Self::Never => "Never",
        }
    }

    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::DesktopOnly => "desktop",
            Self::FullScreenOnly => "fullscreen",
            Self::Never => "never",
        }
    }

    #[must_use]
    pub fn parse_config(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "always" => Some(Self::Always),
            "desktop" => Some(Self::DesktopOnly),
            "fullscreen" => Some(Self::FullScreenOnly),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BarItem {
    Tray,
    Status,
    Search,
    Notifications,
    Clock,
}

impl BarItem {
    pub const ALL: [Self; 5] = [
        Self::Tray,
        Self::Status,
        Self::Search,
        Self::Notifications,
        Self::Clock,
    ];

    const fn bit(self) -> u8 {
        1 << self as u8
    }

    #[must_use]
    pub const fn config_name(self) -> &'static str {
        match self {
            Self::Tray => "tray",
            Self::Status => "status",
            Self::Search => "search",
            Self::Notifications => "notifications",
            Self::Clock => "clock",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tray => "Menu Bar Tray",
            Self::Status => "Menu Bar Status",
            Self::Search => "Menu Bar Search",
            Self::Notifications => "Menu Bar Notifications",
            Self::Clock => "Menu Bar Clock",
        }
    }

    #[must_use]
    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::Tray => "Overflow of tray icons",
            Self::Status => "Wi-Fi, volume and battery",
            Self::Search => "Spotlight search button",
            Self::Notifications => "Notification Center bell",
            Self::Clock => "Date, time and calendar",
        }
    }

    fn parse_config(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|item| item.config_name() == name)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BarItemSet(u8);

impl BarItemSet {
    #[must_use]
    pub const fn contains(self, item: BarItem) -> bool {
        self.0 & item.bit() != 0
    }

    #[must_use]
    pub const fn toggled(self, item: BarItem) -> Self {
        Self(self.0 ^ item.bit())
    }

    #[must_use]
    pub fn parse_config(value: &str) -> Self {
        Self(
            value
                .split(',')
                .filter_map(BarItem::parse_config)
                .fold(0, |bits, item| bits | item.bit()),
        )
    }

    #[must_use]
    pub fn config_value(self) -> String {
        BarItem::ALL
            .into_iter()
            .filter(|item| self.contains(*item))
            .map(BarItem::config_name)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuBarPreferences {
    pub hide_mode: BarHideMode,
    pub hidden_items: BarItemSet,
    pub show_background: bool,
    pub keep_notch: bool,
}

impl Default for MenuBarPreferences {
    fn default() -> Self {
        Self {
            hide_mode: BarHideMode::default(),
            hidden_items: BarItemSet::default(),
            show_background: true,
            keep_notch: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BarHideMode, BarItem, BarItemSet, MenuBarPreferences};

    #[test]
    fn each_mode_hides_in_its_own_states() {
        assert!(BarHideMode::Always.hides(false) && BarHideMode::Always.hides(true));
        assert!(BarHideMode::DesktopOnly.hides(false) && !BarHideMode::DesktopOnly.hides(true));
        assert!(
            !BarHideMode::FullScreenOnly.hides(false) && BarHideMode::FullScreenOnly.hides(true)
        );
        assert!(!BarHideMode::Never.hides(false) && !BarHideMode::Never.hides(true));
    }

    #[test]
    fn default_mode_keeps_the_full_screen_only_behavior() {
        assert_eq!(BarHideMode::default(), BarHideMode::FullScreenOnly);
        assert_eq!(
            MenuBarPreferences::default().hide_mode,
            BarHideMode::FullScreenOnly
        );
        assert!(MenuBarPreferences::default().show_background);
        assert!(!MenuBarPreferences::default().keep_notch);
    }

    #[test]
    fn cycling_visits_every_mode_and_returns() {
        let mut mode = BarHideMode::default();
        let mut seen = vec![mode];
        for _ in 0..4 {
            mode = mode.next();
            seen.push(mode);
        }
        assert_eq!(seen.first(), seen.last());
        for candidate in [
            BarHideMode::Always,
            BarHideMode::DesktopOnly,
            BarHideMode::FullScreenOnly,
            BarHideMode::Never,
        ] {
            assert!(seen.contains(&candidate));
        }
    }

    #[test]
    fn mode_config_round_trips_and_rejects_unknown_values() {
        for mode in [
            BarHideMode::Always,
            BarHideMode::DesktopOnly,
            BarHideMode::FullScreenOnly,
            BarHideMode::Never,
        ] {
            assert_eq!(BarHideMode::parse_config(mode.config_value()), Some(mode));
        }
        assert_eq!(BarHideMode::parse_config("sometimes"), None);
    }

    #[test]
    fn item_set_toggles_and_round_trips_in_a_stable_order() {
        let set = BarItemSet::default()
            .toggled(BarItem::Clock)
            .toggled(BarItem::Tray);
        assert!(set.contains(BarItem::Clock) && set.contains(BarItem::Tray));
        assert!(!set.contains(BarItem::Search));
        assert_eq!(set.config_value(), "tray,clock");
        assert_eq!(BarItemSet::parse_config("tray,clock"), set);
        assert_eq!(set.toggled(BarItem::Clock).config_value(), "tray");
    }

    #[test]
    fn item_set_parse_ignores_unknown_names_and_blanks() {
        let set = BarItemSet::parse_config(" search ,bogus,,notifications");
        assert_eq!(set.config_value(), "search,notifications");
        assert_eq!(BarItemSet::parse_config(""), BarItemSet::default());
    }
}
