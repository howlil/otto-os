use crate::bar::menu_bar::{BarHideMode, BarItem, MenuBarPreferences};
use crate::dock::layout::{DOCK_ICON_LARGE, DOCK_ICON_SMALL, DockPreferences};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OttoSettingSection {
    Main,
    Behavior,
    ShowOnDock,
    WindowsTaskbar,
    Visibility,
    ShowInBar,
    Live,
    Pockets,
}

impl OttoSettingSection {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Main => "",
            Self::Behavior => "Behavior",
            Self::ShowOnDock => "Show on Dock",
            Self::WindowsTaskbar => "Windows Taskbar",
            Self::Visibility => "Visibility",
            Self::ShowInBar => "Show in the Bar",
            Self::Live => "Live",
            Self::Pockets => "Pockets",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OttoSettingKind {
    TopBar,
    Notch,
    Dock,
    WindowControls,
    Genie,
    PrivacyIndicators,
    ClipboardHistory,
    DropShelf,
    NotchHud,
    DynamicIsland,
    MediaModule,
    FocusModule,
    QuickCapture,
    OnlineLyrics,
    ClassicLogo,
    DockHideTaskbar,
    DockShowDownloads,
    DockShowTrash,
    DockAutohide,
    DockReserveSpace,
    DockMagnification,
    Hotkeys,
    LaunchAtLogin,
    CaptureVisible,
    BackgroundCleanup,
    MenuBarBackground,
    MenuBarKeepNotch,
    MenuBarTray,
    MenuBarStatus,
    MenuBarSearch,
    MenuBarNotifications,
    MenuBarClock,
}

impl OttoSettingKind {
    pub const ALL: [Self; 32] = [
        Self::TopBar,
        Self::Notch,
        Self::Dock,
        Self::WindowControls,
        Self::Genie,
        Self::PrivacyIndicators,
        Self::ClipboardHistory,
        Self::DropShelf,
        Self::NotchHud,
        Self::DynamicIsland,
        Self::MediaModule,
        Self::FocusModule,
        Self::QuickCapture,
        Self::OnlineLyrics,
        Self::ClassicLogo,
        Self::DockHideTaskbar,
        Self::DockShowDownloads,
        Self::DockShowTrash,
        Self::DockAutohide,
        Self::DockReserveSpace,
        Self::DockMagnification,
        Self::Hotkeys,
        Self::LaunchAtLogin,
        Self::CaptureVisible,
        Self::BackgroundCleanup,
        Self::MenuBarBackground,
        Self::MenuBarKeepNotch,
        Self::MenuBarTray,
        Self::MenuBarStatus,
        Self::MenuBarSearch,
        Self::MenuBarNotifications,
        Self::MenuBarClock,
    ];

    #[must_use]
    pub const fn section(self) -> OttoSettingSection {
        match self {
            Self::DockAutohide | Self::DockMagnification => OttoSettingSection::Behavior,
            Self::DockShowDownloads | Self::DockShowTrash => OttoSettingSection::ShowOnDock,
            Self::DockHideTaskbar | Self::DockReserveSpace => OttoSettingSection::WindowsTaskbar,
            Self::MenuBarBackground | Self::MenuBarKeepNotch => OttoSettingSection::Visibility,
            Self::MenuBarTray
            | Self::MenuBarStatus
            | Self::MenuBarSearch
            | Self::MenuBarNotifications
            | Self::MenuBarClock => OttoSettingSection::ShowInBar,
            Self::MediaModule | Self::OnlineLyrics | Self::FocusModule => OttoSettingSection::Live,
            Self::ClipboardHistory | Self::QuickCapture | Self::DropShelf => {
                OttoSettingSection::Pockets
            }
            _ => OttoSettingSection::Main,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopBar => "Top Bar",
            Self::Notch => "Notch",
            Self::Dock => "Otto Dock",
            Self::WindowControls => "Traffic Lights",
            Self::Genie => "Genie Minimize",
            Self::PrivacyIndicators => "Privacy Dots",
            Self::ClipboardHistory => "Clipboard History",
            Self::DropShelf => "Drop Shelf (Pocket)",
            Self::NotchHud => "Notch System HUD",
            Self::DynamicIsland => "Dynamic Island",
            Self::MediaModule => "Now Playing Media",
            Self::FocusModule => "Focus & Pomodoro",
            Self::QuickCapture => "Quick Capture",
            Self::OnlineLyrics => "Online Lyrics",
            Self::ClassicLogo => "Classic Brand Logo",
            Self::DockHideTaskbar => "Hide Windows Taskbar",
            Self::DockShowDownloads => "Downloads Stack",
            Self::DockShowTrash => "Trash Bin",
            Self::DockAutohide => "Auto-Hide Dock",
            Self::DockReserveSpace => "Reserve Screen Space",
            Self::DockMagnification => "Dock Magnification",
            Self::Hotkeys => "Global Hotkeys",
            Self::LaunchAtLogin => "Launch at Login",
            Self::CaptureVisible => "Visible in Captures",
            Self::BackgroundCleanup => "Background Cleanup",
            Self::MenuBarBackground => "Menu Bar Background",
            Self::MenuBarKeepNotch => "Notch Stays Visible",
            Self::MenuBarTray => BarItem::Tray.label(),
            Self::MenuBarStatus => BarItem::Status.label(),
            Self::MenuBarSearch => BarItem::Search.label(),
            Self::MenuBarNotifications => BarItem::Notifications.label(),
            Self::MenuBarClock => BarItem::Clock.label(),
        }
    }

    #[must_use]
    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::TopBar => "Menu bar strip & status icons (Ctrl+Alt+B)",
            Self::Notch => "Top notch, Dynamic Island & live activities",
            Self::Dock => "App switcher, running dots & Dock",
            Self::WindowControls => "macOS-style window titlebar buttons",
            Self::Genie => "Smooth warp minimize animation",
            Self::PrivacyIndicators => "Mic & camera activity indicator",
            Self::ClipboardHistory => "Track copies in Notch Clippings",
            Self::DropShelf => "Drag & drop pocket inside Notch",
            Self::NotchHud => "Notch volume & brightness OSD",
            Self::DynamicIsland => "Floating pill instead of attached notch",
            Self::MediaModule => "System media watcher & playback bar",
            Self::FocusModule => "Pomodoro, timer & stopwatch tab",
            Self::QuickCapture => "Tasks tab & Ctrl+Alt+Q capture",
            Self::OnlineLyrics => "Fetch synced & plain lyrics online",
            Self::ClassicLogo => "Classic emblem in left menu bar",
            Self::DockHideTaskbar => "Auto-hide taskbar while Dock runs",
            Self::DockShowDownloads => "Show Downloads folder stack",
            Self::DockShowTrash => "Show Recycle Bin item on Dock",
            Self::DockAutohide => "Hide Dock when a window covers it",
            Self::DockReserveSpace => "Windows stop above the Dock like a taskbar",
            Self::DockMagnification => "Zoom Dock icons under pointer",
            Self::Hotkeys => "Shortcuts for Bar, Notch & Focus",
            Self::LaunchAtLogin => "Start Otto OS automatically at sign-in",
            Self::CaptureVisible => "Show overlays in screenshots",
            Self::BackgroundCleanup => "Throttle idle apps hogging CPU",
            Self::MenuBarBackground => "Glass behind the menu bar",
            Self::MenuBarKeepNotch => "Keep the notch when the bar hides",
            Self::MenuBarTray => BarItem::Tray.subtitle(),
            Self::MenuBarStatus => BarItem::Status.subtitle(),
            Self::MenuBarSearch => BarItem::Search.subtitle(),
            Self::MenuBarNotifications => BarItem::Notifications.subtitle(),
            Self::MenuBarClock => BarItem::Clock.subtitle(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OttoSettingItem {
    Toggle(OttoSettingKind),
    DockIconSize,
    MenuBarHide,
}

impl OttoSettingItem {
    pub const ALL: [Self; 34] = [
        Self::Toggle(OttoSettingKind::Dock),
        Self::DockIconSize,
        Self::Toggle(OttoSettingKind::DockMagnification),
        Self::Toggle(OttoSettingKind::DockAutohide),
        Self::Toggle(OttoSettingKind::DockShowDownloads),
        Self::Toggle(OttoSettingKind::DockShowTrash),
        Self::Toggle(OttoSettingKind::DockHideTaskbar),
        Self::Toggle(OttoSettingKind::DockReserveSpace),
        Self::Toggle(OttoSettingKind::TopBar),
        Self::MenuBarHide,
        Self::Toggle(OttoSettingKind::MenuBarKeepNotch),
        Self::Toggle(OttoSettingKind::MenuBarBackground),
        Self::Toggle(OttoSettingKind::MenuBarTray),
        Self::Toggle(OttoSettingKind::MenuBarStatus),
        Self::Toggle(OttoSettingKind::MenuBarSearch),
        Self::Toggle(OttoSettingKind::MenuBarNotifications),
        Self::Toggle(OttoSettingKind::MenuBarClock),
        Self::Toggle(OttoSettingKind::Notch),
        Self::Toggle(OttoSettingKind::DynamicIsland),
        Self::Toggle(OttoSettingKind::NotchHud),
        Self::Toggle(OttoSettingKind::PrivacyIndicators),
        Self::Toggle(OttoSettingKind::MediaModule),
        Self::Toggle(OttoSettingKind::OnlineLyrics),
        Self::Toggle(OttoSettingKind::FocusModule),
        Self::Toggle(OttoSettingKind::QuickCapture),
        Self::Toggle(OttoSettingKind::ClipboardHistory),
        Self::Toggle(OttoSettingKind::DropShelf),
        Self::Toggle(OttoSettingKind::WindowControls),
        Self::Toggle(OttoSettingKind::Genie),
        Self::Toggle(OttoSettingKind::ClassicLogo),
        Self::Toggle(OttoSettingKind::LaunchAtLogin),
        Self::Toggle(OttoSettingKind::Hotkeys),
        Self::Toggle(OttoSettingKind::CaptureVisible),
        Self::Toggle(OttoSettingKind::BackgroundCleanup),
    ];

    #[must_use]
    pub const fn section(self) -> OttoSettingSection {
        match self {
            Self::Toggle(kind) => kind.section(),
            Self::DockIconSize => OttoSettingSection::Behavior,
            Self::MenuBarHide => OttoSettingSection::Visibility,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Toggle(kind) => kind.label(),
            Self::DockIconSize => "Dock Icon Size",
            Self::MenuBarHide => "Auto-Hide Menu Bar",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct PluginSettingsState {
    pub top_bar: bool,
    pub notch: bool,
    pub dock: bool,
    pub window_controls: bool,
    pub genie: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct SystemSettingsState {
    pub launch_at_login: bool,
    pub hotkeys_enabled: bool,
    pub capture_visible: bool,
    pub classic_logo: bool,
    pub dynamic_island: bool,
    pub privacy_indicators: bool,
    pub clipboard_history: bool,
    pub drop_shelf: bool,
    pub notch_hud: bool,
    pub media_module: bool,
    pub focus_module: bool,
    pub background_cleanup: bool,
    pub quick_capture: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OttoSettingsSnapshot {
    pub plugins: PluginSettingsState,
    pub dock: DockPreferences,
    pub online_lyrics: bool,
    pub system: SystemSettingsState,
    pub menu_bar: MenuBarPreferences,
    pub focus_activity: crate::FocusActivityState,
}

impl OttoSettingsSnapshot {
    #[must_use]
    pub const fn is_enabled(&self, kind: OttoSettingKind) -> bool {
        match kind {
            OttoSettingKind::TopBar => self.plugins.top_bar,
            OttoSettingKind::Notch => self.plugins.notch,
            OttoSettingKind::Dock => self.plugins.dock,
            OttoSettingKind::WindowControls => self.plugins.window_controls,
            OttoSettingKind::Genie => self.plugins.genie,
            OttoSettingKind::PrivacyIndicators => self.system.privacy_indicators,
            OttoSettingKind::ClipboardHistory => self.system.clipboard_history,
            OttoSettingKind::DropShelf => self.system.drop_shelf,
            OttoSettingKind::NotchHud => self.system.notch_hud,
            OttoSettingKind::DynamicIsland => self.system.dynamic_island,
            OttoSettingKind::MediaModule => self.system.media_module,
            OttoSettingKind::FocusModule => self.system.focus_module,
            OttoSettingKind::QuickCapture => self.system.quick_capture,
            OttoSettingKind::OnlineLyrics => self.online_lyrics,
            OttoSettingKind::ClassicLogo => self.system.classic_logo,
            OttoSettingKind::DockHideTaskbar => self.dock.hide_taskbar,
            OttoSettingKind::DockShowDownloads => self.dock.show_downloads,
            OttoSettingKind::DockShowTrash => self.dock.show_trash,
            OttoSettingKind::DockAutohide => self.dock.autohide,
            OttoSettingKind::DockReserveSpace => self.dock.reserve_space,
            OttoSettingKind::DockMagnification => self.dock.magnification,
            OttoSettingKind::Hotkeys => self.system.hotkeys_enabled,
            OttoSettingKind::LaunchAtLogin => self.system.launch_at_login,
            OttoSettingKind::CaptureVisible => self.system.capture_visible,
            OttoSettingKind::BackgroundCleanup => self.system.background_cleanup,
            OttoSettingKind::MenuBarBackground => self.menu_bar.show_background,
            OttoSettingKind::MenuBarKeepNotch => self.menu_bar.keep_notch,
            OttoSettingKind::MenuBarTray => !self.menu_bar.hidden_items.contains(BarItem::Tray),
            OttoSettingKind::MenuBarStatus => !self.menu_bar.hidden_items.contains(BarItem::Status),
            OttoSettingKind::MenuBarSearch => !self.menu_bar.hidden_items.contains(BarItem::Search),
            OttoSettingKind::MenuBarNotifications => {
                !self.menu_bar.hidden_items.contains(BarItem::Notifications)
            }
            OttoSettingKind::MenuBarClock => !self.menu_bar.hidden_items.contains(BarItem::Clock),
        }
    }

    #[must_use]
    pub const fn is_item_available(&self, item: OttoSettingItem) -> bool {
        match item {
            OttoSettingItem::Toggle(
                OttoSettingKind::DockHideTaskbar
                | OttoSettingKind::DockShowDownloads
                | OttoSettingKind::DockShowTrash
                | OttoSettingKind::DockAutohide
                | OttoSettingKind::DockMagnification,
            )
            | OttoSettingItem::DockIconSize => self.plugins.dock,
            OttoSettingItem::Toggle(OttoSettingKind::DockReserveSpace) => {
                self.plugins.dock && self.dock.hide_taskbar
            }
            OttoSettingItem::Toggle(OttoSettingKind::OnlineLyrics) => self.system.media_module,
            OttoSettingItem::Toggle(OttoSettingKind::MenuBarKeepNotch) => {
                self.plugins.top_bar
                    && self.plugins.notch
                    && !matches!(self.menu_bar.hide_mode, BarHideMode::Never)
            }
            OttoSettingItem::Toggle(
                OttoSettingKind::MenuBarBackground
                | OttoSettingKind::MenuBarTray
                | OttoSettingKind::MenuBarStatus
                | OttoSettingKind::MenuBarSearch
                | OttoSettingKind::MenuBarNotifications
                | OttoSettingKind::MenuBarClock,
            )
            | OttoSettingItem::MenuBarHide => self.plugins.top_bar,
            OttoSettingItem::Toggle(
                OttoSettingKind::DynamicIsland
                | OttoSettingKind::NotchHud
                | OttoSettingKind::PrivacyIndicators,
            ) => self.plugins.notch,
            OttoSettingItem::Toggle(_) => true,
        }
    }

    #[must_use]
    pub const fn item_subtitle(&self, item: OttoSettingItem) -> &'static str {
        match item {
            OttoSettingItem::Toggle(kind) => kind.subtitle(),
            OttoSettingItem::DockIconSize => "Base size for Dock icons",
            OttoSettingItem::MenuBarHide => "Hide the bar until the pointer reaches the top",
        }
    }

    #[must_use]
    pub const fn dock_size_label(&self) -> &'static str {
        match self.dock.icon_size {
            DOCK_ICON_SMALL => "Small",
            DOCK_ICON_LARGE => "Large",
            _ => "Medium",
        }
    }

    #[must_use]
    pub const fn row_count() -> usize {
        OttoSettingItem::ALL.len()
    }
}
