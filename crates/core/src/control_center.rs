use crate::bar_focus::BarKey;
use crate::dock::{DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL, DockPreferences};
use crate::layout::{Rect, scale};
use crate::media::{MediaCommand, NowPlaying};
use crate::menu_bar::{BarHideMode, BarItem, MenuBarPreferences};
use crate::ports::ControlCenterAction;
use crate::status::{PowerMode, SystemStatus};
use crate::theme::ThemeMode;

pub const UPDATE_CHECKING_LABEL: &str = "Checking...";
pub const SLIDER_KEY_STEP_PERCENT: i8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlCenterPage {
    #[default]
    Main,
    Wifi,
    Bluetooth,
    Sound,
    Display,
    OttoSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BluetoothDeviceKind {
    Headphones,
    Speaker,
    Mouse,
    Keyboard,
    Phone,
    #[default]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal_percent: u8,
    pub secured: bool,
    pub connected: bool,
    pub saved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WifiState {
    pub supported: bool,
    pub enabled: bool,
    pub busy: bool,
    pub connected_ssid: Option<String>,
    pub networks: Vec<WifiNetwork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BluetoothDevice {
    pub id: String,
    pub name: String,
    pub kind: BluetoothDeviceKind,
    pub connected: bool,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BluetoothState {
    pub supported: bool,
    pub enabled: bool,
    pub busy: bool,
    pub devices: Vec<BluetoothDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub const AREA_COUNT: usize = 6;
pub const AREA_CARD_COLUMNS: usize = 2;
const CHIP_COLUMNS: usize = 3;
const HERO_HEIGHT_PERCENT: i32 = 30;
const SCENE_HEIGHT_PERCENT: i32 = 42;
const AREA_CARD_TEXT_HEIGHT: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OttoSettingsArea {
    Dock,
    MenuBar,
    Notch,
    NotchTools,
    Appearance,
    System,
}

impl OttoSettingsArea {
    pub const ALL: [Self; AREA_COUNT] = [
        Self::Dock,
        Self::MenuBar,
        Self::Notch,
        Self::NotchTools,
        Self::Appearance,
        Self::System,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dock => "Dock",
            Self::MenuBar => "Menu Bar",
            Self::Notch => "Notch",
            Self::NotchTools => "Notch Tools",
            Self::Appearance => "Appearance",
            Self::System => "System",
        }
    }

    #[must_use]
    pub const fn blurb(self) -> &'static str {
        match self {
            Self::Dock => "Apps and shortcuts",
            Self::MenuBar => "What shows, and when",
            Self::Notch => "Island, HUD, privacy",
            Self::NotchTools => "Music, focus, clips",
            Self::Appearance => "Windows and logo",
            Self::System => "Startup and keys",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Dock => 0,
            Self::MenuBar => 1,
            Self::Notch => 2,
            Self::NotchTools => 3,
            Self::Appearance => 4,
            Self::System => 5,
        }
    }
}

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

    #[must_use]
    pub const fn is_chips(self) -> bool {
        matches!(self, Self::ShowInBar)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OttoSettingKind {
    TopBar,
    Notch,
    Dock,
    WindowControls,
    Genie,
    LiquidGlass,
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
    pub const ALL: [Self; 33] = [
        Self::TopBar,
        Self::Notch,
        Self::Dock,
        Self::WindowControls,
        Self::Genie,
        Self::LiquidGlass,
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
    pub const fn area(self) -> OttoSettingsArea {
        match self {
            Self::Dock
            | Self::DockHideTaskbar
            | Self::DockShowDownloads
            | Self::DockShowTrash
            | Self::DockAutohide
            | Self::DockReserveSpace
            | Self::DockMagnification => OttoSettingsArea::Dock,
            Self::TopBar
            | Self::MenuBarBackground
            | Self::MenuBarKeepNotch
            | Self::MenuBarTray
            | Self::MenuBarStatus
            | Self::MenuBarSearch
            | Self::MenuBarNotifications
            | Self::MenuBarClock => OttoSettingsArea::MenuBar,
            Self::Notch | Self::DynamicIsland | Self::NotchHud | Self::PrivacyIndicators => {
                OttoSettingsArea::Notch
            }
            Self::MediaModule
            | Self::OnlineLyrics
            | Self::FocusModule
            | Self::ClipboardHistory
            | Self::QuickCapture
            | Self::DropShelf => OttoSettingsArea::NotchTools,
            Self::WindowControls | Self::Genie | Self::ClassicLogo | Self::LiquidGlass => {
                OttoSettingsArea::Appearance
            }
            Self::Hotkeys
            | Self::LaunchAtLogin
            | Self::CaptureVisible
            | Self::BackgroundCleanup => OttoSettingsArea::System,
        }
    }

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
    pub const fn chip_label(self) -> &'static str {
        match self {
            Self::MenuBarTray => "Tray",
            Self::MenuBarStatus => "Status",
            Self::MenuBarSearch => "Search",
            Self::MenuBarNotifications => "Alerts",
            Self::MenuBarClock => "Clock",
            _ => self.label(),
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
            Self::LiquidGlass => "Liquid Glass",
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
            Self::LiquidGlass => "Live wallpaper blur & refraction",
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

    #[must_use]
    pub const fn is_core_module(self) -> bool {
        matches!(
            self,
            Self::TopBar
                | Self::Notch
                | Self::Dock
                | Self::WindowControls
                | Self::Genie
                | Self::PrivacyIndicators
                | Self::ClipboardHistory
                | Self::DropShelf
                | Self::NotchHud
                | Self::MediaModule
                | Self::FocusModule
        )
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
    pub const fn area(self) -> OttoSettingsArea {
        match self {
            Self::Toggle(kind) => kind.area(),
            Self::DockIconSize => OttoSettingsArea::Dock,
            Self::MenuBarHide => OttoSettingsArea::MenuBar,
        }
    }

    #[must_use]
    pub const fn section(self) -> OttoSettingSection {
        match self {
            Self::Toggle(kind) => kind.section(),
            Self::DockIconSize => OttoSettingSection::Behavior,
            Self::MenuBarHide => OttoSettingSection::Visibility,
        }
    }

    #[must_use]
    pub const fn is_chip(self) -> bool {
        self.section().is_chips()
    }

    #[must_use]
    pub const fn chip_label(self) -> &'static str {
        match self {
            Self::Toggle(kind) => kind.chip_label(),
            _ => self.label(),
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
    pub liquid_glass: bool,
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
            OttoSettingKind::LiquidGlass => self.system.liquid_glass,
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
    pub fn visible_items(&self, area: OttoSettingsArea) -> Vec<OttoSettingItem> {
        OttoSettingItem::ALL
            .into_iter()
            .filter(|item| item.area() == area && self.is_item_available(*item))
            .collect()
    }

    #[must_use]
    pub fn area_status(&self, area: OttoSettingsArea) -> String {
        let master_off = match area {
            OttoSettingsArea::Dock => !self.plugins.dock,
            OttoSettingsArea::MenuBar => !self.plugins.top_bar,
            OttoSettingsArea::Notch | OttoSettingsArea::NotchTools => !self.plugins.notch,
            _ => false,
        };
        if master_off {
            return String::from("Off");
        }
        let (on, total) = OttoSettingItem::ALL
            .into_iter()
            .filter(|item| item.area() == area)
            .filter_map(|item| match item {
                OttoSettingItem::Toggle(kind) => Some(self.is_enabled(kind)),
                _ => None,
            })
            .fold((0usize, 0usize), |(on, total), enabled| {
                (on + usize::from(enabled), total + 1)
            });
        format!("{on} of {total} on")
    }

    #[must_use]
    pub fn active_module_count(&self) -> (usize, usize) {
        let mut active = 0usize;
        let mut total = 0usize;
        for kind in OttoSettingKind::ALL {
            if kind.is_core_module() {
                total += 1;
                if self.is_enabled(kind) {
                    active += 1;
                }
            }
        }
        (active, total)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubPanelGroupSpec {
    pub title: &'static str,
    pub count: usize,
    pub chips: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubPanelGroupLayout {
    pub title: &'static str,
    pub header: Rect,
    pub container: Rect,
    pub start_index: usize,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ControlCenterSnapshot {
    pub page: ControlCenterPage,
    pub otto_area: Option<OttoSettingsArea>,
    pub status: SystemStatus,
    pub focus_active: bool,
    pub media: Option<NowPlaying>,
    pub wifi: WifiState,
    pub bluetooth: BluetoothState,
    pub audio_outputs: Vec<AudioOutputDevice>,
    pub audio_switch_failed: bool,
    pub settings: OttoSettingsSnapshot,
    pub update_status: Option<String>,
    pub last_error: Option<String>,
}

impl ControlCenterSnapshot {
    #[must_use]
    pub fn subpanel_item_count(&self, page: ControlCenterPage) -> usize {
        match page {
            ControlCenterPage::Main | ControlCenterPage::Display => 0,
            ControlCenterPage::Wifi => self.wifi.networks.len(),
            ControlCenterPage::Bluetooth => self.bluetooth.devices.len(),
            ControlCenterPage::Sound => self.audio_outputs.len(),
            ControlCenterPage::OttoSettings => self
                .otto_area
                .map_or(0, |area| self.settings.visible_items(area).len()),
        }
    }

    #[must_use]
    pub fn subpanel_groups(&self) -> Vec<SubPanelGroupSpec> {
        match self.page {
            ControlCenterPage::Main | ControlCenterPage::Display => Vec::new(),
            ControlCenterPage::Wifi => non_empty_group("Wi-Fi Networks", self.wifi.networks.len()),
            ControlCenterPage::Bluetooth => {
                non_empty_group("Devices", self.bluetooth.devices.len())
            }
            ControlCenterPage::Sound => non_empty_group("Output Devices", self.audio_outputs.len()),
            ControlCenterPage::OttoSettings => {
                let items = self
                    .otto_area
                    .map_or_else(Vec::new, |area| self.settings.visible_items(area));
                let mut groups: Vec<SubPanelGroupSpec> = Vec::new();
                for item in items {
                    let section = item.section();
                    if let Some(last) = groups.last_mut()
                        && last.title == section.title()
                    {
                        last.count += 1;
                    } else {
                        groups.push(SubPanelGroupSpec {
                            title: section.title(),
                            count: 1,
                            chips: section.is_chips(),
                        });
                    }
                }
                groups
            }
        }
    }
}

fn non_empty_group(title: &'static str, count: usize) -> Vec<SubPanelGroupSpec> {
    if count == 0 {
        Vec::new()
    } else {
        vec![SubPanelGroupSpec {
            title,
            count,
            chips: false,
        }]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCenterMetrics {
    pub width: i32,
    pub padding: i32,
    pub gap: i32,
    pub shell_radius: i32,
    pub card_radius: i32,
    pub connectivity_height: i32,
    pub display_card_height: i32,
    pub sound_card_height: i32,
    pub bottom_row_height: i32,
    pub header_height: i32,
    pub slider_card_height: i32,
    pub row_height: i32,
    pub row_gap: i32,
    pub footer_height: i32,
    pub max_visible_rows: i32,
    pub group_header_height: i32,
    pub group_gap: i32,
    pub error_banner_height: i32,
}

impl ControlCenterMetrics {
    pub const DEFAULT: Self = Self {
        width: 376,
        padding: 12,
        gap: 8,
        shell_radius: 26,
        card_radius: 14,
        connectivity_height: 144,
        display_card_height: 64,
        sound_card_height: 64,
        bottom_row_height: 128,
        header_height: 40,
        slider_card_height: 64,
        row_height: 44,
        row_gap: 0,
        footer_height: 36,
        max_visible_rows: 6,
        group_header_height: 24,
        group_gap: 12,
        error_banner_height: 28,
    };
}

impl Default for ControlCenterMetrics {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCenterPlacement {
    pub bounds: Rect,
    pub dpi: u32,
    pub anchor_right: i32,
    pub anchor_y: i32,
    pub page: ControlCenterPage,
    pub item_count: usize,
    pub scroll_offset: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterHitTarget {
    WifiToggle,
    WifiOpenPage,
    BluetoothToggle,
    BluetoothOpenPage,
    FocusToggle,
    FocusOpenSettings,
    BatteryTile,
    DisplayOpenPage,
    DisplaySlider,
    SoundOpenPage,
    SoundMuteToggle,
    SoundSlider,
    MediaBody,
    MediaPrev,
    MediaPlayPause,
    MediaNext,
    OttoSettingsTile,
    BackButton,
    HeaderToggle,
    OttoAreaCard(OttoSettingsArea),
    SubPanelItem(usize),
    FooterPrimary,
    FooterSecondary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControlCenterLayout {
    pub window: Rect,
    pub page: ControlCenterPage,
    pub shell_radius: i32,
    pub card_radius: i32,
    pub connectivity_card: Rect,
    pub wifi_toggle: Rect,
    pub wifi_row: Rect,
    pub bluetooth_toggle: Rect,
    pub bluetooth_row: Rect,
    pub focus_toggle: Rect,
    pub focus_settings: Rect,
    pub battery_tile: Rect,
    pub display_card: Rect,
    pub display_header: Rect,
    pub display_icon: Rect,
    pub display_slider: Rect,
    pub sound_card: Rect,
    pub sound_header: Rect,
    pub sound_mute: Rect,
    pub sound_slider: Rect,
    pub media_card: Rect,
    pub media_body: Rect,
    pub media_prev: Rect,
    pub media_play_pause: Rect,
    pub media_next: Rect,
    pub otto_settings_tile: Rect,
    pub back_button: Rect,
    pub header_rect: Rect,
    pub header_toggle: Rect,
    pub area: Option<OttoSettingsArea>,
    pub area_cards: [Rect; AREA_COUNT],
    pub hero: Rect,
    pub error_banner: Rect,
    pub list_viewport: Rect,
    pub groups: Vec<SubPanelGroupLayout>,
    pub items: Vec<Rect>,
    pub scroll_offset: i32,
    pub max_scroll: i32,
    pub row_height: i32,
    pub footer_primary: Rect,
    pub footer_secondary: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlCenterCommand {
    OpenPage(ControlCenterPage),
    Back,
    OpenOttoArea(OttoSettingsArea),
    ToggleWifi,
    ConnectWifi(String),
    ToggleBluetooth,
    ToggleBluetoothDevice(String, bool),
    ToggleFocus,
    CycleTheme,
    SetThemeMode(ThemeMode),
    SetBrightness(u8),
    SetVolume(u8),
    ToggleMute,
    SelectAudioOutput(String),
    Media(MediaCommand),
    OpenNotchMedia,
    ToggleSetting(OttoSettingKind),
    CycleDockIconSize,
    CycleMenuBarHide,
    OpenSettings(ControlCenterAction),
    OpenLogsFolder,
    CheckForUpdates,
    OpenReleasesPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterFocusOutcome {
    Moved(ControlCenterHitTarget),
    Activate(ControlCenterHitTarget),
    AdjustSlider {
        target: ControlCenterHitTarget,
        delta: i8,
    },
    Exited,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControlCenterFocus {
    current: Option<ControlCenterHitTarget>,
}

impl ControlCenterFocus {
    #[must_use]
    pub const fn current(self) -> Option<ControlCenterHitTarget> {
        self.current
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        self.current.is_some()
    }

    pub fn enter(&mut self, targets: &[ControlCenterHitTarget]) -> Option<ControlCenterHitTarget> {
        self.current = targets.first().copied();
        self.current
    }

    pub fn enter_target(
        &mut self,
        targets: &[ControlCenterHitTarget],
        preferred: ControlCenterHitTarget,
    ) -> Option<ControlCenterHitTarget> {
        self.current = targets
            .iter()
            .copied()
            .find(|target| *target == preferred)
            .or_else(|| targets.first().copied());
        self.current
    }

    pub fn exit(&mut self) -> bool {
        self.current.take().is_some()
    }

    pub fn revalidate(&mut self, targets: &[ControlCenterHitTarget]) -> bool {
        let Some(current) = self.current else {
            return false;
        };
        if targets.contains(&current) {
            return false;
        }
        self.current = targets.first().copied();
        true
    }

    pub fn handle(
        &mut self,
        key: BarKey,
        targets: &[ControlCenterHitTarget],
    ) -> ControlCenterFocusOutcome {
        let Some(current) = self.current else {
            return ControlCenterFocusOutcome::Unchanged;
        };
        if key == BarKey::Exit || targets.is_empty() {
            self.current = None;
            return ControlCenterFocusOutcome::Exited;
        }
        let Some(index) = targets.iter().position(|target| *target == current) else {
            let first = targets[0];
            self.current = Some(first);
            return ControlCenterFocusOutcome::Moved(first);
        };
        if matches!(
            current,
            ControlCenterHitTarget::DisplaySlider | ControlCenterHitTarget::SoundSlider
        ) && matches!(key, BarKey::Previous | BarKey::Next)
        {
            let delta = if key == BarKey::Next {
                SLIDER_KEY_STEP_PERCENT
            } else {
                -SLIDER_KEY_STEP_PERCENT
            };
            return ControlCenterFocusOutcome::AdjustSlider {
                target: current,
                delta,
            };
        }
        if key == BarKey::Activate {
            return ControlCenterFocusOutcome::Activate(current);
        }
        let last = targets.len() - 1;
        let next = match key {
            BarKey::First => 0,
            BarKey::Last => last,
            BarKey::Next | BarKey::NextTab | BarKey::Down => (index + 1) % targets.len(),
            BarKey::Previous | BarKey::PreviousTab | BarKey::Up => {
                index.checked_sub(1).unwrap_or(last)
            }
            BarKey::Activate | BarKey::Exit => return ControlCenterFocusOutcome::Unchanged,
        };
        let target = targets[next];
        self.current = Some(target);
        if target == current {
            ControlCenterFocusOutcome::Unchanged
        } else {
            ControlCenterFocusOutcome::Moved(target)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCenterClickOutcome {
    pub command: ControlCenterCommand,
    pub dismiss: bool,
}

#[must_use]
pub const fn next_dock_icon_size(current: i32) -> i32 {
    match current {
        DOCK_ICON_SMALL => DOCK_ICON_MEDIUM,
        DOCK_ICON_MEDIUM => DOCK_ICON_LARGE,
        _ => DOCK_ICON_SMALL,
    }
}

#[must_use]
pub const fn power_mode_label(mode: PowerMode) -> &'static str {
    match mode {
        PowerMode::BatterySaver => "Battery Saver",
        PowerMode::Standard => "Standard",
    }
}

#[must_use]
pub fn compute_control_center_layout(
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
) -> ControlCenterLayout {
    compute_control_center_layout_with(metrics, placement, None, &[], false)
}

#[must_use]
pub fn compute_control_center_layout_for_snapshot(
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    snapshot: &ControlCenterSnapshot,
) -> ControlCenterLayout {
    let groups = snapshot.subpanel_groups();
    let has_error =
        placement.page == ControlCenterPage::OttoSettings && snapshot.last_error.is_some();
    compute_control_center_layout_with(metrics, placement, snapshot.otto_area, &groups, has_error)
}

fn compute_control_center_layout_with(
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    area: Option<OttoSettingsArea>,
    groups: &[SubPanelGroupSpec],
    has_error: bool,
) -> ControlCenterLayout {
    let dpi = placement.dpi.max(96);
    let width = scale(metrics.width, dpi);
    let pad = scale(metrics.padding, dpi);
    let gap = scale(metrics.gap, dpi);
    let inner_w = (width - pad * 2).max(scale(120, dpi));

    let mut layout = ControlCenterLayout {
        page: placement.page,
        area: area.filter(|_| placement.page == ControlCenterPage::OttoSettings),
        shell_radius: scale(metrics.shell_radius, dpi),
        card_radius: scale(metrics.card_radius, dpi),
        row_height: scale(metrics.row_height, dpi),
        ..ControlCenterLayout::default()
    };

    let height = if placement.page == ControlCenterPage::Main {
        layout_main_page(&mut layout, metrics, dpi, pad, gap, inner_w)
    } else {
        layout_subpanel_page(
            &mut layout,
            metrics,
            placement,
            groups,
            has_error,
            dpi,
            pad,
            gap,
            inner_w,
        )
    };

    let margin = scale(8, dpi);
    let max_x = (placement.bounds.right() - width - margin).max(placement.bounds.x);
    let x = (placement.anchor_right - width).clamp(
        placement.bounds.x + margin.min(max_x - placement.bounds.x),
        max_x,
    );
    let max_y = (placement.bounds.bottom() - height - margin).max(placement.bounds.y);
    let y = placement.anchor_y.clamp(placement.bounds.y, max_y);

    layout.window = Rect {
        x,
        y,
        width,
        height,
    };
    layout
}

fn layout_main_page(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let y2 = layout_main_top(layout, metrics, dpi, pad, gap, inner_w) + gap;
    let y4 = layout_main_sliders(layout, metrics, dpi, pad, gap, inner_w, y2) + gap;
    let h4 = layout_main_bottom(layout, metrics, dpi, pad, gap, inner_w, y4);
    y4 + h4 + pad
}

fn layout_main_top(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let y1 = pad;
    let h1 = scale(metrics.connectivity_height, dpi);
    let left_w = (inner_w - gap) / 2;
    let right_w = inner_w - gap - left_w;
    let right_x = pad + left_w + gap;

    layout.connectivity_card = Rect {
        x: pad,
        y: y1,
        width: left_w,
        height: h1,
    };
    layout_connectivity_rows(layout, dpi);

    let split_h = (h1 - gap) / 2;
    layout.focus_toggle = Rect {
        x: right_x,
        y: y1,
        width: right_w,
        height: split_h,
    };
    let gear_d = scale(20, dpi);
    layout.focus_settings = Rect {
        x: layout.focus_toggle.right() - gear_d - scale(6, dpi),
        y: y1 + scale(6, dpi),
        width: gear_d,
        height: gear_d,
    };
    layout.battery_tile = Rect {
        x: right_x,
        y: layout.focus_toggle.bottom() + gap,
        width: right_w,
        height: h1 - gap - split_h,
    };
    y1 + h1
}

fn layout_main_sliders(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y2: i32,
) -> i32 {
    let card_pad = scale(10, dpi);
    let card_hdr_h = scale(24, dpi);
    let slider_h = scale(26, dpi);
    let slider_x = pad + card_pad;
    let slider_w = (inner_w - card_pad * 2).max(1);

    let h2 = scale(metrics.display_card_height, dpi);
    let display_row_y = y2 + card_hdr_h + scale(4, dpi);
    layout.display_card = Rect {
        x: pad,
        y: y2,
        width: inner_w,
        height: h2,
    };
    layout.display_header = Rect {
        x: pad,
        y: y2,
        width: inner_w,
        height: card_hdr_h,
    };
    layout.display_slider = Rect {
        x: slider_x,
        y: display_row_y,
        width: slider_w,
        height: slider_h,
    };
    layout.display_icon = Rect {
        x: slider_x + scale(2, dpi),
        y: display_row_y,
        width: slider_h,
        height: slider_h,
    };

    let y3 = y2 + h2 + gap;
    let h3 = scale(metrics.sound_card_height, dpi);
    let sound_row_y = y3 + card_hdr_h + scale(4, dpi);
    layout.sound_card = Rect {
        x: pad,
        y: y3,
        width: inner_w,
        height: h3,
    };
    layout.sound_header = Rect {
        x: pad,
        y: y3,
        width: inner_w,
        height: card_hdr_h,
    };
    let mute_gap = scale(8, dpi);
    layout.sound_mute = Rect {
        x: slider_x,
        y: sound_row_y,
        width: slider_h,
        height: slider_h,
    };
    layout.sound_slider = Rect {
        x: slider_x + slider_h + mute_gap,
        y: sound_row_y,
        width: (slider_w - slider_h - mute_gap).max(1),
        height: slider_h,
    };
    y3 + h3
}

fn layout_main_bottom(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y4: i32,
) -> i32 {
    let h4 = scale(metrics.bottom_row_height, dpi);
    let bar_h = scale(46, dpi);
    let media_h = (h4 - gap - bar_h).max(scale(64, dpi));
    layout.media_card = Rect {
        x: pad,
        y: y4,
        width: inner_w,
        height: media_h,
    };

    let btn_d = scale(28, dpi);
    let btn_gap = scale(6, dpi);
    let right_pad = scale(10, dpi);
    let controls_w = btn_d * 3 + btn_gap * 2;
    let btn_x0 = layout.media_card.right() - right_pad - controls_w;
    let btn_y = y4 + (media_h - btn_d) / 2;

    layout.media_body = Rect {
        x: pad,
        y: y4,
        width: (btn_x0 - pad - scale(4, dpi)).max(scale(80, dpi)),
        height: media_h,
    };
    layout.media_prev = Rect {
        x: btn_x0,
        y: btn_y,
        width: btn_d,
        height: btn_d,
    };
    layout.media_play_pause = Rect {
        x: layout.media_prev.right() + btn_gap,
        y: btn_y,
        width: btn_d,
        height: btn_d,
    };
    layout.media_next = Rect {
        x: layout.media_play_pause.right() + btn_gap,
        y: btn_y,
        width: btn_d,
        height: btn_d,
    };

    layout.otto_settings_tile = Rect {
        x: pad,
        y: layout.media_card.bottom() + gap,
        width: inner_w,
        height: bar_h,
    };

    h4
}

fn layout_connectivity_rows(layout: &mut ControlCenterLayout, dpi: u32) {
    let card = layout.connectivity_card;
    let inset = scale(8, dpi);
    let row_gap = scale(6, dpi);
    let slot_h = ((card.height - inset * 2 - row_gap) / 2).max(scale(40, dpi));
    let circle_d = scale(32, dpi).min(slot_h - scale(6, dpi));
    let row_x = card.x + scale(4, dpi);
    let row_w = (card.width - scale(8, dpi)).max(1);
    let circle_x = row_x + scale(6, dpi);

    let y0 = card.y + inset;
    layout.wifi_row = Rect {
        x: row_x,
        y: y0,
        width: row_w,
        height: slot_h,
    };
    layout.wifi_toggle = Rect {
        x: circle_x,
        y: y0 + (slot_h - circle_d) / 2,
        width: circle_d,
        height: circle_d,
    };

    let y1 = y0 + slot_h + row_gap;
    layout.bluetooth_row = Rect {
        x: row_x,
        y: y1,
        width: row_w,
        height: slot_h,
    };
    layout.bluetooth_toggle = Rect {
        x: circle_x,
        y: y1 + (slot_h - circle_d) / 2,
        width: circle_d,
        height: circle_d,
    };
}

#[allow(clippy::too_many_arguments)]
fn layout_subpanel_page(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
    has_error: bool,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let mut y = layout_subpanel_header(layout, metrics, placement.page, dpi, pad, gap, inner_w);
    if placement.page == ControlCenterPage::OttoSettings {
        return layout_otto_settings_page(
            layout, metrics, placement, groups, has_error, dpi, pad, gap, inner_w, y,
        );
    }
    match placement.page {
        ControlCenterPage::Sound => {
            y = layout_subpanel_sound(layout, metrics, dpi, pad, gap, inner_w, y);
        }
        ControlCenterPage::Display => {
            y = layout_subpanel_display(layout, metrics, dpi, pad, gap, inner_w, y);
        }
        ControlCenterPage::Main
        | ControlCenterPage::Wifi
        | ControlCenterPage::Bluetooth
        | ControlCenterPage::OttoSettings => {}
    }

    let footer_h = scale(metrics.footer_height, dpi);
    if placement.page != ControlCenterPage::Display {
        y = layout_subpanel_list(
            layout, metrics, placement, groups, dpi, pad, gap, inner_w, y, footer_h,
        );
    }
    layout_subpanel_footer(layout, placement.page, pad, gap, inner_w, y, footer_h);
    y + footer_h + pad
}

fn layout_subpanel_header(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    page: ControlCenterPage,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let hdr_h = scale(metrics.header_height, dpi);
    layout.header_rect = Rect {
        x: pad,
        y: pad,
        width: inner_w,
        height: hdr_h,
    };
    let back_d = scale(28, dpi);
    layout.back_button = Rect {
        x: pad,
        y: pad + (hdr_h - back_d) / 2,
        width: back_d,
        height: back_d,
    };
    if matches!(page, ControlCenterPage::Wifi | ControlCenterPage::Bluetooth) {
        let toggle_w = scale(40, dpi);
        let toggle_h = scale(22, dpi);
        layout.header_toggle = Rect {
            x: pad + inner_w - toggle_w,
            y: pad + (hdr_h - toggle_h) / 2,
            width: toggle_w,
            height: toggle_h,
        };
    }
    pad + hdr_h + gap
}

fn layout_subpanel_sound(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
) -> i32 {
    let slider_card_h = scale(metrics.slider_card_height, dpi);
    let card_pad = scale(10, dpi);
    let card_hdr_h = scale(24, dpi);
    let slider_h = scale(26, dpi);
    layout.sound_card = Rect {
        x: pad,
        y,
        width: inner_w,
        height: slider_card_h,
    };
    layout.sound_header = Rect {
        x: pad,
        y,
        width: inner_w,
        height: card_hdr_h,
    };
    let slider_x = pad + card_pad;
    let slider_w = (inner_w - card_pad * 2).max(1);
    let mute_gap = scale(8, dpi);
    let slider_y = y + card_hdr_h + scale(4, dpi);
    layout.sound_mute = Rect {
        x: slider_x,
        y: slider_y,
        width: slider_h,
        height: slider_h,
    };
    layout.sound_slider = Rect {
        x: slider_x + slider_h + mute_gap,
        y: slider_y,
        width: (slider_w - slider_h - mute_gap).max(1),
        height: slider_h,
    };
    y + slider_card_h + gap
}

fn layout_subpanel_display(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
) -> i32 {
    let slider_card_h = scale(metrics.slider_card_height, dpi);
    let card_pad = scale(10, dpi);
    let card_hdr_h = scale(24, dpi);
    let slider_h = scale(26, dpi);
    layout.display_card = Rect {
        x: pad,
        y,
        width: inner_w,
        height: slider_card_h,
    };
    let dx = pad + card_pad;
    let dy = y + card_hdr_h + scale(4, dpi);
    layout.display_slider = Rect {
        x: dx,
        y: dy,
        width: (inner_w - card_pad * 2).max(1),
        height: slider_h,
    };
    layout.display_icon = Rect {
        x: dx + scale(2, dpi),
        y: dy,
        width: slider_h,
        height: slider_h,
    };
    y + slider_card_h + gap
}

#[allow(clippy::too_many_arguments)]
fn layout_otto_settings_page(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
    has_error: bool,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
) -> i32 {
    let footer_h = scale(metrics.footer_height, dpi);
    if layout.area.is_none() {
        let y = layout_area_cards(layout, dpi, pad, gap, inner_w, y);
        layout_subpanel_footer(
            layout,
            ControlCenterPage::OttoSettings,
            pad,
            gap,
            inner_w,
            y,
            footer_h,
        );
        return y + footer_h + pad;
    }
    let hero_h = inner_w * HERO_HEIGHT_PERCENT / 100;
    layout.hero = Rect {
        x: pad,
        y,
        width: inner_w,
        height: hero_h,
    };
    let mut y = y + hero_h + gap;
    if has_error {
        let banner_h = scale(metrics.error_banner_height, dpi);
        layout.error_banner = Rect {
            x: pad,
            y,
            width: inner_w,
            height: banner_h,
        };
        y += banner_h + scale(6, dpi);
    }
    let end = layout_subpanel_list(
        layout, metrics, placement, groups, dpi, pad, gap, inner_w, y, 0,
    );
    end - gap + pad
}

fn layout_area_cards(
    layout: &mut ControlCenterLayout,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
) -> i32 {
    let card_w = (inner_w - gap) / AREA_CARD_COLUMNS as i32;
    let card_h = area_card_scene_height(card_w) + scale(AREA_CARD_TEXT_HEIGHT, dpi);
    for (index, rect) in layout.area_cards.iter_mut().enumerate() {
        let column = (index % AREA_CARD_COLUMNS) as i32;
        let row = (index / AREA_CARD_COLUMNS) as i32;
        let x = pad + column * (card_w + gap);
        let width = if column == AREA_CARD_COLUMNS as i32 - 1 {
            pad + inner_w - x
        } else {
            card_w
        };
        *rect = Rect {
            x,
            y: y + row * (card_h + gap),
            width,
            height: card_h,
        };
    }
    let rows = AREA_COUNT.div_ceil(AREA_CARD_COLUMNS) as i32;
    y + rows * card_h + (rows - 1) * gap + gap
}

const fn area_card_scene_height(card_width: i32) -> i32 {
    card_width * SCENE_HEIGHT_PERCENT / 100
}

#[must_use]
pub const fn area_card_scene(card: Rect) -> Rect {
    Rect {
        x: card.x,
        y: card.y,
        width: card.width,
        height: area_card_scene_height(card.width),
    }
}

const fn chips_rows(count: usize) -> i32 {
    count.div_ceil(CHIP_COLUMNS) as i32
}

const fn chip_metrics(row_h: i32) -> (i32, i32, i32) {
    (row_h * 3 / 4, row_h / 8, row_h / 4)
}

fn group_content_height(group: &SubPanelGroupSpec, row_h: i32) -> i32 {
    if !group.chips {
        return i32::try_from(group.count).unwrap_or(64) * row_h;
    }
    let (chip_h, chip_gap, inset) = chip_metrics(row_h);
    let rows = chips_rows(group.count);
    rows * chip_h + (rows - 1).max(0) * chip_gap + inset * 2
}

fn chip_rect(container: Rect, row_h: i32, offset: usize) -> Rect {
    let (chip_h, chip_gap, inset) = chip_metrics(row_h);
    let columns = CHIP_COLUMNS as i32;
    let inner_w = container.width - inset * 2;
    let chip_w = (inner_w - (columns - 1) * chip_gap) / columns;
    let column = (offset % CHIP_COLUMNS) as i32;
    let row = (offset / CHIP_COLUMNS) as i32;
    Rect {
        x: container.x + inset + column * (chip_w + chip_gap),
        y: container.y + inset + row * (chip_h + chip_gap),
        width: chip_w,
        height: chip_h,
    }
}

fn compute_subpanel_list_total_height(
    non_empty: &[SubPanelGroupSpec],
    row_h: i32,
    group_hdr_h: i32,
    group_gap: i32,
    pad_v: i32,
) -> i32 {
    if non_empty.is_empty() {
        return row_h;
    }
    let mut total_h = pad_v;
    for (idx, group) in non_empty.iter().enumerate() {
        if idx > 0 {
            total_h += group_gap;
        }
        if !group.title.is_empty() {
            total_h += group_hdr_h;
        }
        total_h += group_content_height(group, row_h);
    }
    total_h
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn layout_subpanel_list(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
    footer_h: i32,
) -> i32 {
    let row_h = scale(metrics.row_height, dpi);
    let group_hdr_h = scale(metrics.group_header_height, dpi);
    let group_gap = scale(metrics.group_gap, dpi);
    let bottom_pad = scale(4, dpi);

    let fallback = [SubPanelGroupSpec {
        title: "",
        count: placement.item_count,
        chips: false,
    }];
    let specs: &[SubPanelGroupSpec] = if groups.is_empty() && placement.item_count > 0 {
        &fallback
    } else {
        groups
    };

    let non_empty: Vec<SubPanelGroupSpec> = specs.iter().copied().filter(|g| g.count > 0).collect();
    let total_h =
        compute_subpanel_list_total_height(&non_empty, row_h, group_hdr_h, group_gap, bottom_pad);

    let cap_rows = metrics.max_visible_rows.max(1);
    let cap_h = cap_rows * row_h + group_hdr_h * 2 + group_gap + scale(18, dpi);
    let max_win_h = (placement.bounds.height - scale(16, dpi)).max(scale(260, dpi));
    let footer_reserve = gap + footer_h + pad;
    let avail_h = (max_win_h - y - footer_reserve).max(row_h);
    let viewport_h = total_h.min(cap_h).min(avail_h);

    layout.list_viewport = Rect {
        x: pad,
        y,
        width: inner_w,
        height: viewport_h,
    };
    layout.max_scroll = (total_h - viewport_h).max(0);
    layout.scroll_offset = placement.scroll_offset.clamp(0, layout.max_scroll);

    let content_w = if layout.max_scroll > 0 {
        (inner_w - scale(8, dpi)).max(1)
    } else {
        inner_w
    };

    layout.items.reserve(placement.item_count);
    let mut cursor_y = y - layout.scroll_offset;
    for (g_idx, group) in non_empty.iter().enumerate() {
        if g_idx > 0 {
            cursor_y += group_gap;
        }
        let header = if group.title.is_empty() {
            Rect::default()
        } else {
            let h = Rect {
                x: pad,
                y: cursor_y,
                width: content_w,
                height: group_hdr_h,
            };
            cursor_y += group_hdr_h;
            h
        };
        let start_index = layout.items.len();
        let container_y = cursor_y;
        let group_h = group_content_height(group, row_h);
        let container = Rect {
            x: pad,
            y: container_y,
            width: content_w,
            height: group_h,
        };
        for offset in 0..group.count {
            if layout.items.len() >= placement.item_count {
                break;
            }
            if group.chips {
                layout.items.push(chip_rect(container, row_h, offset));
                continue;
            }
            layout.items.push(Rect {
                x: pad,
                y: cursor_y,
                width: content_w,
                height: row_h,
            });
            cursor_y += row_h;
        }
        if group.chips {
            cursor_y += group_h;
        }
        let actual_count = layout.items.len() - start_index;
        if actual_count > 0 {
            layout.groups.push(SubPanelGroupLayout {
                title: group.title,
                header,
                container,
                start_index,
                count: actual_count,
            });
        }
    }

    while layout.items.len() < placement.item_count {
        layout.items.push(Rect {
            x: pad,
            y: cursor_y,
            width: content_w,
            height: row_h,
        });
        cursor_y += row_h;
    }

    y + viewport_h + gap
}

fn layout_subpanel_footer(
    layout: &mut ControlCenterLayout,
    page: ControlCenterPage,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
    footer_h: i32,
) {
    if matches!(
        page,
        ControlCenterPage::Display | ControlCenterPage::OttoSettings
    ) {
        let half = (inner_w - gap) / 2;
        layout.footer_primary = Rect {
            x: pad,
            y,
            width: half,
            height: footer_h,
        };
        layout.footer_secondary = Rect {
            x: pad + half + gap,
            y,
            width: inner_w - gap - half,
            height: footer_h,
        };
    } else {
        layout.footer_primary = Rect {
            x: pad,
            y,
            width: inner_w,
            height: footer_h,
        };
    }
}

impl ControlCenterLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<ControlCenterHitTarget> {
        if self.page == ControlCenterPage::Main {
            return self.hit_test_main(x, y);
        }
        if self.back_button.contains(x, y) {
            return Some(ControlCenterHitTarget::BackButton);
        }
        if self.header_toggle.contains(x, y) {
            return Some(ControlCenterHitTarget::HeaderToggle);
        }
        if self.page == ControlCenterPage::Display && self.display_slider.contains(x, y) {
            return Some(ControlCenterHitTarget::DisplaySlider);
        }
        if self.page == ControlCenterPage::Sound {
            if self.sound_mute.contains(x, y) {
                return Some(ControlCenterHitTarget::SoundMuteToggle);
            }
            if self.sound_slider.contains(x, y) {
                return Some(ControlCenterHitTarget::SoundSlider);
            }
        }
        if self.page == ControlCenterPage::OttoSettings {
            for (index, rect) in self.area_cards.iter().enumerate() {
                if rect.contains(x, y) {
                    return Some(ControlCenterHitTarget::OttoAreaCard(
                        OttoSettingsArea::ALL[index],
                    ));
                }
            }
        }
        if self.list_viewport.contains(x, y) {
            for (index, rect) in self.items.iter().enumerate() {
                if rect.contains(x, y) {
                    return Some(ControlCenterHitTarget::SubPanelItem(index));
                }
            }
        }
        if self.footer_primary.contains(x, y) {
            return Some(ControlCenterHitTarget::FooterPrimary);
        }
        if self.footer_secondary.contains(x, y) {
            return Some(ControlCenterHitTarget::FooterSecondary);
        }
        None
    }

    fn hit_test_main(&self, x: i32, y: i32) -> Option<ControlCenterHitTarget> {
        let direct = [
            (self.wifi_toggle, ControlCenterHitTarget::WifiToggle),
            (self.wifi_row, ControlCenterHitTarget::WifiOpenPage),
            (
                self.bluetooth_toggle,
                ControlCenterHitTarget::BluetoothToggle,
            ),
            (
                self.bluetooth_row,
                ControlCenterHitTarget::BluetoothOpenPage,
            ),
            (
                self.focus_settings,
                ControlCenterHitTarget::FocusOpenSettings,
            ),
            (self.focus_toggle, ControlCenterHitTarget::FocusToggle),
            (self.battery_tile, ControlCenterHitTarget::BatteryTile),
            (self.display_slider, ControlCenterHitTarget::DisplaySlider),
            (self.display_header, ControlCenterHitTarget::DisplayOpenPage),
            (self.sound_mute, ControlCenterHitTarget::SoundMuteToggle),
            (self.sound_slider, ControlCenterHitTarget::SoundSlider),
            (self.sound_header, ControlCenterHitTarget::SoundOpenPage),
            (self.media_prev, ControlCenterHitTarget::MediaPrev),
            (
                self.media_play_pause,
                ControlCenterHitTarget::MediaPlayPause,
            ),
            (self.media_next, ControlCenterHitTarget::MediaNext),
            (self.media_body, ControlCenterHitTarget::MediaBody),
            (
                self.otto_settings_tile,
                ControlCenterHitTarget::OttoSettingsTile,
            ),
        ];
        for (rect, target) in direct {
            if rect.contains(x, y) {
                return Some(target);
            }
        }
        None
    }

    #[must_use]
    pub fn slider_rect(&self, target: ControlCenterHitTarget) -> Option<Rect> {
        match target {
            ControlCenterHitTarget::DisplaySlider if !self.display_slider.is_empty() => {
                Some(self.display_slider)
            }
            ControlCenterHitTarget::SoundSlider if !self.sound_slider.is_empty() => {
                Some(self.sound_slider)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn slider_value_at(&self, target: ControlCenterHitTarget, x: i32) -> Option<u8> {
        let rect = self.slider_rect(target)?;
        if x <= rect.x {
            return Some(0);
        }
        if x >= rect.right() {
            return Some(100);
        }
        let rel = i64::from(x - rect.x);
        let width = i64::from(rect.width.max(1));
        let percent = ((rel * 100 + width / 2) / width).clamp(0, 100);
        u8::try_from(percent).ok()
    }

    #[must_use]
    pub fn target_rect(&self, target: ControlCenterHitTarget) -> Option<Rect> {
        let rect = match target {
            ControlCenterHitTarget::WifiToggle => self.wifi_toggle,
            ControlCenterHitTarget::WifiOpenPage => self.wifi_row,
            ControlCenterHitTarget::BluetoothToggle => self.bluetooth_toggle,
            ControlCenterHitTarget::BluetoothOpenPage => self.bluetooth_row,
            ControlCenterHitTarget::FocusToggle => self.focus_toggle,
            ControlCenterHitTarget::FocusOpenSettings => self.focus_settings,
            ControlCenterHitTarget::BatteryTile => self.battery_tile,
            ControlCenterHitTarget::DisplayOpenPage => self.display_header,
            ControlCenterHitTarget::DisplaySlider => self.display_slider,
            ControlCenterHitTarget::SoundOpenPage => self.sound_header,
            ControlCenterHitTarget::SoundMuteToggle => self.sound_mute,
            ControlCenterHitTarget::SoundSlider => self.sound_slider,
            ControlCenterHitTarget::MediaBody => self.media_body,
            ControlCenterHitTarget::MediaPrev => self.media_prev,
            ControlCenterHitTarget::MediaPlayPause => self.media_play_pause,
            ControlCenterHitTarget::MediaNext => self.media_next,
            ControlCenterHitTarget::OttoSettingsTile => self.otto_settings_tile,
            ControlCenterHitTarget::BackButton => self.back_button,
            ControlCenterHitTarget::HeaderToggle => self.header_toggle,
            ControlCenterHitTarget::OttoAreaCard(area) => self.area_cards[area.index()],
            ControlCenterHitTarget::SubPanelItem(index) => {
                let item = *self.items.get(index)?;
                return self.list_viewport.intersection(&item);
            }
            ControlCenterHitTarget::FooterPrimary => self.footer_primary,
            ControlCenterHitTarget::FooterSecondary => self.footer_secondary,
        };
        (!rect.is_empty()).then_some(rect)
    }
}

#[must_use]
pub fn control_center_action(
    snapshot: &ControlCenterSnapshot,
    target: ControlCenterHitTarget,
) -> Option<ControlCenterClickOutcome> {
    let outcome =
        match target {
            ControlCenterHitTarget::WifiToggle => {
                if !snapshot.wifi.supported {
                    return None;
                }
                stay(ControlCenterCommand::ToggleWifi)
            }
            ControlCenterHitTarget::WifiOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Wifi))
            }
            ControlCenterHitTarget::BluetoothToggle => {
                if !snapshot.bluetooth.supported {
                    return None;
                }
                stay(ControlCenterCommand::ToggleBluetooth)
            }
            ControlCenterHitTarget::BluetoothOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Bluetooth))
            }
            ControlCenterHitTarget::FocusToggle => {
                if snapshot.settings.plugins.notch && snapshot.settings.system.focus_module {
                    stay(ControlCenterCommand::ToggleFocus)
                } else {
                    stay(ControlCenterCommand::OpenPage(
                        ControlCenterPage::OttoSettings,
                    ))
                }
            }
            ControlCenterHitTarget::FocusOpenSettings => dismiss(
                ControlCenterCommand::OpenSettings(ControlCenterAction::OpenFocus),
            ),
            ControlCenterHitTarget::BatteryTile => dismiss(ControlCenterCommand::OpenSettings(
                ControlCenterAction::OpenPowerSettings,
            )),
            ControlCenterHitTarget::DisplayOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Display))
            }
            ControlCenterHitTarget::DisplaySlider | ControlCenterHitTarget::SoundSlider => {
                return None;
            }
            ControlCenterHitTarget::SoundOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Sound))
            }
            ControlCenterHitTarget::SoundMuteToggle => stay(ControlCenterCommand::ToggleMute),
            ControlCenterHitTarget::MediaBody => {
                if snapshot.settings.plugins.notch && snapshot.settings.system.media_module {
                    dismiss(ControlCenterCommand::OpenNotchMedia)
                } else {
                    stay(ControlCenterCommand::OpenPage(
                        ControlCenterPage::OttoSettings,
                    ))
                }
            }
            ControlCenterHitTarget::MediaPrev => snapshot
                .settings
                .system
                .media_module
                .then_some(stay(ControlCenterCommand::Media(MediaCommand::Previous)))?,
            ControlCenterHitTarget::MediaPlayPause => {
                snapshot.settings.system.media_module.then_some(stay(
                    ControlCenterCommand::Media(MediaCommand::TogglePlayPause),
                ))?
            }
            ControlCenterHitTarget::MediaNext => snapshot
                .settings
                .system
                .media_module
                .then_some(stay(ControlCenterCommand::Media(MediaCommand::Next)))?,
            ControlCenterHitTarget::OttoSettingsTile => stay(ControlCenterCommand::OpenPage(
                ControlCenterPage::OttoSettings,
            )),
            ControlCenterHitTarget::BackButton => stay(ControlCenterCommand::Back),
            ControlCenterHitTarget::HeaderToggle => match snapshot.page {
                ControlCenterPage::Wifi if snapshot.wifi.supported => {
                    stay(ControlCenterCommand::ToggleWifi)
                }
                ControlCenterPage::Bluetooth if snapshot.bluetooth.supported => {
                    stay(ControlCenterCommand::ToggleBluetooth)
                }
                _ => return None,
            },
            ControlCenterHitTarget::OttoAreaCard(area) => {
                stay(ControlCenterCommand::OpenOttoArea(area))
            }
            ControlCenterHitTarget::SubPanelItem(index) => {
                return subpanel_item_action(snapshot, index);
            }
            ControlCenterHitTarget::FooterPrimary => return footer_primary_action(snapshot.page),
            ControlCenterHitTarget::FooterSecondary => return footer_secondary_action(snapshot),
        };
    Some(outcome)
}

#[must_use]
pub fn is_update_available_status(status: Option<&str>) -> bool {
    status.is_some_and(|text| text.starts_with("Update v"))
}

#[must_use]
pub const fn focus_row_subtitle(focus_module: bool, focus_active: bool) -> &'static str {
    if !focus_module {
        "Disabled"
    } else if focus_active {
        "Focus On"
    } else {
        "Off"
    }
}

#[must_use]
pub fn step_slider_percent(current: u8, delta: i8) -> u8 {
    let stepped = (i16::from(current.min(100)) + i16::from(delta)).clamp(0, 100);
    u8::try_from(stepped).unwrap_or(0)
}

#[must_use]
pub const fn subpanel_opener_target(page: ControlCenterPage) -> Option<ControlCenterHitTarget> {
    match page {
        ControlCenterPage::Main => None,
        ControlCenterPage::Wifi => Some(ControlCenterHitTarget::WifiOpenPage),
        ControlCenterPage::Bluetooth => Some(ControlCenterHitTarget::BluetoothOpenPage),
        ControlCenterPage::Sound => Some(ControlCenterHitTarget::SoundOpenPage),
        ControlCenterPage::Display => Some(ControlCenterHitTarget::DisplayOpenPage),
        ControlCenterPage::OttoSettings => Some(ControlCenterHitTarget::OttoSettingsTile),
    }
}

#[must_use]
pub fn scroll_offset_for_item(
    layout: &ControlCenterLayout,
    target: ControlCenterHitTarget,
) -> Option<i32> {
    let ControlCenterHitTarget::SubPanelItem(index) = target else {
        return None;
    };
    let item = layout.items.get(index)?;
    if layout.list_viewport.is_empty() || layout.max_scroll <= 0 {
        return None;
    }
    let header_h = layout
        .groups
        .iter()
        .find(|group| group.start_index == index && !group.header.is_empty())
        .map_or(0, |group| group.header.height);
    let item_top = item.y - header_h + layout.scroll_offset - layout.list_viewport.y;
    let item_bottom = item.y + item.height + layout.scroll_offset - layout.list_viewport.y;
    let next = if item.y - header_h < layout.list_viewport.y {
        item_top
    } else if item.bottom() > layout.list_viewport.bottom() {
        item_bottom - layout.list_viewport.height
    } else {
        return None;
    }
    .clamp(0, layout.max_scroll);
    (next != layout.scroll_offset).then_some(next)
}

#[must_use]
pub fn control_center_focus_targets(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> Vec<ControlCenterHitTarget> {
    if layout.page == ControlCenterPage::Main {
        main_focus_targets(layout, snapshot)
    } else {
        subpanel_focus_targets(layout, snapshot)
    }
}

fn main_focus_targets(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> Vec<ControlCenterHitTarget> {
    let mut candidates = vec![
        (
            layout.wifi_toggle,
            snapshot.wifi.supported,
            ControlCenterHitTarget::WifiToggle,
        ),
        (layout.wifi_row, true, ControlCenterHitTarget::WifiOpenPage),
        (
            layout.bluetooth_toggle,
            snapshot.bluetooth.supported,
            ControlCenterHitTarget::BluetoothToggle,
        ),
        (
            layout.bluetooth_row,
            true,
            ControlCenterHitTarget::BluetoothOpenPage,
        ),
    ];
    candidates.extend([
        (
            layout.focus_toggle,
            true,
            ControlCenterHitTarget::FocusToggle,
        ),
        (
            layout.focus_settings,
            true,
            ControlCenterHitTarget::FocusOpenSettings,
        ),
        (
            layout.battery_tile,
            true,
            ControlCenterHitTarget::BatteryTile,
        ),
        (
            layout.display_header,
            true,
            ControlCenterHitTarget::DisplayOpenPage,
        ),
        (
            layout.display_slider,
            snapshot.status.brightness.is_some(),
            ControlCenterHitTarget::DisplaySlider,
        ),
        (
            layout.sound_header,
            true,
            ControlCenterHitTarget::SoundOpenPage,
        ),
        (
            layout.sound_mute,
            true,
            ControlCenterHitTarget::SoundMuteToggle,
        ),
        (
            layout.sound_slider,
            true,
            ControlCenterHitTarget::SoundSlider,
        ),
        (layout.media_body, true, ControlCenterHitTarget::MediaBody),
        (
            layout.media_prev,
            snapshot.settings.system.media_module,
            ControlCenterHitTarget::MediaPrev,
        ),
        (
            layout.media_play_pause,
            snapshot.settings.system.media_module,
            ControlCenterHitTarget::MediaPlayPause,
        ),
        (
            layout.media_next,
            snapshot.settings.system.media_module,
            ControlCenterHitTarget::MediaNext,
        ),
        (
            layout.otto_settings_tile,
            true,
            ControlCenterHitTarget::OttoSettingsTile,
        ),
    ]);
    candidates
        .into_iter()
        .filter_map(|(rect, enabled, target)| (enabled && !rect.is_empty()).then_some(target))
        .collect()
}

fn subpanel_focus_targets(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> Vec<ControlCenterHitTarget> {
    let mut out = Vec::new();
    if !layout.back_button.is_empty() {
        out.push(ControlCenterHitTarget::BackButton);
    }
    match layout.page {
        ControlCenterPage::Wifi if snapshot.wifi.supported && !layout.header_toggle.is_empty() => {
            out.push(ControlCenterHitTarget::HeaderToggle);
        }
        ControlCenterPage::Bluetooth
            if snapshot.bluetooth.supported && !layout.header_toggle.is_empty() =>
        {
            out.push(ControlCenterHitTarget::HeaderToggle);
        }
        ControlCenterPage::Sound => {
            if !layout.sound_mute.is_empty() {
                out.push(ControlCenterHitTarget::SoundMuteToggle);
            }
            if !layout.sound_slider.is_empty() {
                out.push(ControlCenterHitTarget::SoundSlider);
            }
        }
        ControlCenterPage::Display => {
            if snapshot.status.brightness.is_some() && !layout.display_slider.is_empty() {
                out.push(ControlCenterHitTarget::DisplaySlider);
            }
        }
        ControlCenterPage::OttoSettings => {
            for (index, area) in OttoSettingsArea::ALL.into_iter().enumerate() {
                if !layout.area_cards[index].is_empty() {
                    out.push(ControlCenterHitTarget::OttoAreaCard(area));
                }
            }
        }
        _ => {}
    }
    for index in 0..layout.items.len() {
        out.push(ControlCenterHitTarget::SubPanelItem(index));
    }
    if !layout.footer_primary.is_empty() {
        out.push(ControlCenterHitTarget::FooterPrimary);
    }
    if !layout.footer_secondary.is_empty() {
        out.push(ControlCenterHitTarget::FooterSecondary);
    }
    out
}

fn subpanel_item_action(
    snapshot: &ControlCenterSnapshot,
    index: usize,
) -> Option<ControlCenterClickOutcome> {
    match snapshot.page {
        ControlCenterPage::Wifi => {
            let network = snapshot.wifi.networks.get(index)?;
            Some(stay(ControlCenterCommand::ConnectWifi(
                network.ssid.clone(),
            )))
        }
        ControlCenterPage::Bluetooth => {
            let device = snapshot.bluetooth.devices.get(index)?;
            Some(stay(ControlCenterCommand::ToggleBluetoothDevice(
                device.id.clone(),
                !device.connected,
            )))
        }
        ControlCenterPage::Sound => {
            let device = snapshot.audio_outputs.get(index)?;
            Some(stay(ControlCenterCommand::SelectAudioOutput(
                device.id.clone(),
            )))
        }
        ControlCenterPage::OttoSettings => {
            let items = snapshot.settings.visible_items(snapshot.otto_area?);
            let item = *items.get(index)?;
            match item {
                OttoSettingItem::Toggle(kind) => {
                    Some(stay(ControlCenterCommand::ToggleSetting(kind)))
                }
                OttoSettingItem::DockIconSize => {
                    Some(stay(ControlCenterCommand::CycleDockIconSize))
                }
                OttoSettingItem::MenuBarHide => Some(stay(ControlCenterCommand::CycleMenuBarHide)),
            }
        }
        ControlCenterPage::Main | ControlCenterPage::Display => None,
    }
}

fn footer_primary_action(page: ControlCenterPage) -> Option<ControlCenterClickOutcome> {
    match page {
        ControlCenterPage::Wifi => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenWifi,
        ))),
        ControlCenterPage::Bluetooth => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenBluetooth,
        ))),
        ControlCenterPage::Sound => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenSound,
        ))),
        ControlCenterPage::Display => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenDisplay,
        ))),
        ControlCenterPage::OttoSettings => Some(dismiss(ControlCenterCommand::OpenLogsFolder)),
        ControlCenterPage::Main => None,
    }
}

fn footer_secondary_action(snapshot: &ControlCenterSnapshot) -> Option<ControlCenterClickOutcome> {
    match snapshot.page {
        ControlCenterPage::Display => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenNightLight,
        ))),
        ControlCenterPage::OttoSettings => {
            let status = snapshot.update_status.as_deref();
            if status == Some(UPDATE_CHECKING_LABEL) {
                None
            } else if is_update_available_status(status) {
                Some(dismiss(ControlCenterCommand::OpenReleasesPage))
            } else {
                Some(stay(ControlCenterCommand::CheckForUpdates))
            }
        }
        ControlCenterPage::Main
        | ControlCenterPage::Wifi
        | ControlCenterPage::Bluetooth
        | ControlCenterPage::Sound => None,
    }
}

const fn stay(command: ControlCenterCommand) -> ControlCenterClickOutcome {
    ControlCenterClickOutcome {
        command,
        dismiss: false,
    }
}

const fn dismiss(command: ControlCenterCommand) -> ControlCenterClickOutcome {
    ControlCenterClickOutcome {
        command,
        dismiss: true,
    }
}

#[must_use]
pub fn parse_latest_release_tag(json: &str) -> Option<String> {
    let bytes = json.as_bytes();
    let mut index = 0usize;
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b'"' => {
                let (key, next_index) = parse_json_string(json, index)?;
                index = skip_json_whitespace(bytes, next_index);
                if bytes.get(index) == Some(&b':') {
                    index = skip_json_whitespace(bytes, index + 1);
                    if depth == 1 && key == "tag_name" && bytes.get(index) == Some(&b'"') {
                        let (value, _) = parse_json_string(json, index)?;
                        return (!value.trim().is_empty()).then_some(value);
                    }
                }
            }
            _ => index += 1,
        }
    }
    None
}

fn skip_json_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn parse_json_string(input: &str, start_quote: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.get(start_quote) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut index = start_quote + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some((out, index + 1)),
            b'\\' => {
                index += 1;
                let escaped = *bytes.get(index)?;
                match escaped {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000C}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = input.get(index + 1..index + 5)?;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                        }
                        index += 4;
                    }
                    other => out.push(char::from(other)),
                }
                index += 1;
            }
            byte if byte < 0x80 => {
                out.push(char::from(byte));
                index += 1;
            }
            _ => {
                let tail = &input[index..];
                let ch = tail.chars().next()?;
                out.push(ch);
                index += ch.len_utf8();
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSemVer<'a> {
    core: [u64; 3],
    prerelease: Option<&'a str>,
}

fn parse_semver(raw: &str) -> Option<ParsedSemVer<'_>> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let without_build = trimmed.split('+').next()?;
    let (core_str, prerelease) = match without_build.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre)),
        Some((_, _)) => return None,
        None => (without_build, None),
    };
    let mut parts = core_str.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = match parts.next() {
        Some(value) => value.parse::<u64>().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(ParsedSemVer {
        core: [major, minor, patch],
        prerelease,
    })
}

fn compare_prerelease(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut left_parts = left.split('.');
    let mut right_parts = right.split('.');
    loop {
        match (left_parts.next(), right_parts.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let ordering = match (a.parse::<u64>(), b.parse::<u64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => a.cmp(b),
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

fn compare_semver(candidate: &str, current: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;

    let left = parse_semver(candidate)?;
    let right = parse_semver(current)?;
    let core_order = left.core.cmp(&right.core);
    if core_order != Ordering::Equal {
        return Some(core_order);
    }
    Some(match (left.prerelease, right.prerelease) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => compare_prerelease(a, b),
    })
}

#[must_use]
pub fn format_update_status(current: &str, http_status: u16, body: Option<&str>) -> String {
    if http_status == 404 {
        return String::from("No releases yet");
    }
    if !(200..300).contains(&http_status) {
        return String::from("Check failed");
    }
    let Some(tag) = body.and_then(parse_latest_release_tag) else {
        return String::from("Check failed");
    };
    let Some(ordering) = compare_semver(&tag, current) else {
        return String::from("Check failed");
    };
    if ordering.is_gt() {
        let clean = tag.trim().trim_start_matches(['v', 'V']);
        format!("Update v{clean}")
    } else {
        String::from("Up to date")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        }
    }

    fn dock_area_snapshot() -> ControlCenterSnapshot {
        let mut snap = ControlCenterSnapshot {
            page: ControlCenterPage::OttoSettings,
            otto_area: Some(OttoSettingsArea::Dock),
            ..ControlCenterSnapshot::default()
        };
        snap.settings.plugins.dock = true;
        snap.settings.dock.hide_taskbar = true;
        snap
    }

    fn main_layout(dpi: u32) -> ControlCenterLayout {
        compute_control_center_layout(
            &ControlCenterMetrics::DEFAULT,
            ControlCenterPlacement {
                bounds: bounds(),
                dpi,
                anchor_right: 1750,
                anchor_y: 36,
                page: ControlCenterPage::Main,
                item_count: 0,
                scroll_offset: 0,
            },
        )
    }

    #[test]
    fn main_layout_places_window_within_work_area_across_dpis() {
        for dpi in [96, 144, 192] {
            let layout = main_layout(dpi);
            assert!(layout.window.width > 0 && layout.window.height > 0);
            assert!(layout.window.right() <= bounds().right());
            assert!(layout.window.bottom() <= bounds().bottom());
            assert!(layout.connectivity_card.width > 0);
            assert_eq!(layout.focus_toggle.x, layout.battery_tile.x);
            assert_eq!(layout.focus_toggle.width, layout.battery_tile.width);
            assert!(layout.focus_toggle.bottom() < layout.battery_tile.y);
            assert!(layout.display_slider.width > 200);
            assert!(layout.sound_slider.width > 180);
            assert_eq!(layout.sound_mute.x, layout.display_slider.x);
            assert!(layout.sound_mute.right() < layout.sound_slider.x);
            assert_eq!(layout.display_slider.right(), layout.sound_slider.right());
            assert!(layout.sound_mute.height >= scale(24, dpi));
            assert!(layout.media_body.right() <= layout.media_prev.x);
            assert!(layout.media_card.bottom() < layout.otto_settings_tile.y);
        }
    }

    #[test]
    fn hit_test_prioritizes_nested_connectivity_media_and_sound_controls() {
        let layout = main_layout(96);
        let center = |r: Rect| (r.x + r.width / 2, r.y + r.height / 2);

        let (x, y) = center(layout.wifi_toggle);
        assert_eq!(
            layout.hit_test(x, y),
            Some(ControlCenterHitTarget::WifiToggle)
        );
        assert_eq!(
            layout.hit_test(layout.wifi_row.right() - 10, layout.wifi_row.y + 8),
            Some(ControlCenterHitTarget::WifiOpenPage)
        );
        let (bx, by) = center(layout.bluetooth_toggle);
        assert_eq!(
            layout.hit_test(bx, by),
            Some(ControlCenterHitTarget::BluetoothToggle)
        );
        let (battery_x, battery_y) = center(layout.battery_tile);
        assert_eq!(
            layout.hit_test(battery_x, battery_y),
            Some(ControlCenterHitTarget::BatteryTile)
        );
        let (fsx, fsy) = center(layout.focus_settings);
        assert_eq!(
            layout.hit_test(fsx, fsy),
            Some(ControlCenterHitTarget::FocusOpenSettings)
        );
        assert_eq!(
            layout.hit_test(
                layout.focus_toggle.x + 12,
                layout.focus_toggle.bottom() - 12
            ),
            Some(ControlCenterHitTarget::FocusToggle)
        );
        let (mx, my) = center(layout.sound_mute);
        assert_eq!(
            layout.hit_test(mx, my),
            Some(ControlCenterHitTarget::SoundMuteToggle)
        );
        let (px, py) = center(layout.media_play_pause);
        assert_eq!(
            layout.hit_test(px, py),
            Some(ControlCenterHitTarget::MediaPlayPause)
        );
        let (mbx, mby) = center(layout.media_body);
        assert_eq!(
            layout.hit_test(mbx, mby),
            Some(ControlCenterHitTarget::MediaBody)
        );
    }

    #[test]
    fn slider_value_clamps_bounds_and_interpolates_midpoint() {
        let layout = main_layout(96);
        assert_eq!(
            layout.slider_value_at(ControlCenterHitTarget::DisplaySlider, 0),
            Some(0)
        );
        assert_eq!(
            layout.slider_value_at(
                ControlCenterHitTarget::DisplaySlider,
                layout.display_slider.right() + 50
            ),
            Some(100)
        );
        let mid = layout.display_slider.x + layout.display_slider.width / 2;
        let pct = layout
            .slider_value_at(ControlCenterHitTarget::DisplaySlider, mid)
            .unwrap();
        assert!((49..=51).contains(&pct));
        assert_eq!(
            layout.slider_value_at(ControlCenterHitTarget::BatteryTile, mid),
            None
        );
    }

    #[test]
    fn scrollable_subpanel_clips_items_to_viewport_and_clamps_scroll() {
        let snap = dock_area_snapshot();
        let count = snap.subpanel_item_count(ControlCenterPage::OttoSettings);
        let layout = compute_control_center_layout_for_snapshot(
            &ControlCenterMetrics::DEFAULT,
            ControlCenterPlacement {
                bounds: bounds(),
                dpi: 96,
                anchor_right: 1750,
                anchor_y: 36,
                page: ControlCenterPage::OttoSettings,
                item_count: count,
                scroll_offset: 9999,
            },
            &snap,
        );
        assert_eq!(count, 8);
        assert!(layout.max_scroll > 0);
        assert_eq!(layout.scroll_offset, layout.max_scroll);
        assert!(layout.footer_primary.is_empty() && layout.footer_secondary.is_empty());
        let last_idx = count - 1;
        let last = layout.items[last_idx];
        assert!(last.bottom() <= layout.list_viewport.bottom());
        assert_eq!(
            layout.hit_test(last.x + 12, last.y + 12),
            Some(ControlCenterHitTarget::SubPanelItem(last_idx))
        );
    }

    #[test]
    fn actions_keep_controls_open_except_external_handoffs() {
        let mut snap = ControlCenterSnapshot {
            page: ControlCenterPage::OttoSettings,
            ..ControlCenterSnapshot::default()
        };
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::MediaPlayPause),
            None
        );
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::MediaBody),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::OpenPage(ControlCenterPage::OttoSettings),
                dismiss: false,
            })
        );
        snap.settings.system.media_module = true;
        snap.settings.plugins.notch = true;
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::MediaPlayPause).map(|o| o.dismiss),
            Some(false)
        );
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::MediaBody).map(|o| o.dismiss),
            Some(true)
        );
        snap.settings.plugins.notch = false;
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::MediaBody),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::OpenPage(ControlCenterPage::OttoSettings),
                dismiss: false,
            })
        );
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FooterPrimary).map(|o| o.dismiss),
            Some(true)
        );
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FooterSecondary)
                .map(|o| o.dismiss),
            Some(false)
        );
        snap.page = ControlCenterPage::Main;
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::WifiToggle),
            None
        );
        snap.wifi.supported = true;
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::WifiToggle),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::ToggleWifi,
                dismiss: false,
            })
        );
    }

    #[test]
    fn release_tag_parser_ignores_nested_objects_and_braces_in_strings() {
        let json = r#"{
            "author": { "tag_name": "v9.9.9" },
            "body": "notes with {braces} and \"quotes\"",
            "tag_name": "v0.2.1"
        }"#;
        assert_eq!(parse_latest_release_tag(json).as_deref(), Some("v0.2.1"));
        assert_eq!(parse_latest_release_tag(r#"{"tag_name":"   "}"#), None);
        assert_eq!(parse_latest_release_tag("{invalid"), None);
    }

    #[test]
    fn update_status_reports_newer_matching_unreleased_and_errors() {
        assert_eq!(
            format_update_status("0.1.0", 200, Some(r#"{"tag_name":"v0.2.0"}"#)),
            "Update v0.2.0"
        );
        assert_eq!(
            format_update_status("0.1.0", 200, Some(r#"{"tag_name":"v0.1.0"}"#)),
            "Up to date"
        );
        assert_eq!(
            format_update_status("0.2.0", 200, Some(r#"{"tag_name":"v0.1.5"}"#)),
            "Up to date"
        );
        assert_eq!(
            format_update_status("0.1.0-beta.1", 200, Some(r#"{"tag_name":"v0.1.0"}"#)),
            "Update v0.1.0"
        );
        assert_eq!(
            format_update_status("0.1.0", 200, Some(r#"{"tag_name":"v0.1.0-rc.1"}"#)),
            "Up to date"
        );
        assert_eq!(format_update_status("0.1.0", 404, None), "No releases yet");
        assert_eq!(format_update_status("0.1.0", 502, None), "Check failed");
        assert_eq!(
            format_update_status("0.1.0", 200, Some(r#"{"name":"no tag"}"#)),
            "Check failed"
        );
    }

    #[test]
    fn footer_secondary_opens_releases_when_update_available_and_ignores_while_checking() {
        let mut snap = ControlCenterSnapshot {
            page: ControlCenterPage::OttoSettings,
            ..ControlCenterSnapshot::default()
        };
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FooterSecondary),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::CheckForUpdates,
                dismiss: false,
            })
        );
        snap.update_status = Some(String::from(super::UPDATE_CHECKING_LABEL));
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FooterSecondary),
            None
        );
        snap.update_status = Some(String::from("Update v0.3.0"));
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FooterSecondary),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::OpenReleasesPage,
                dismiss: true,
            })
        );
        snap.update_status = Some(String::from("Up to date"));
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FooterSecondary),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::CheckForUpdates,
                dismiss: false,
            })
        );
    }

    #[test]
    fn focus_targets_respect_page_and_capabilities() {
        let mut snap = ControlCenterSnapshot::default();
        let layout = compute_control_center_layout(
            &ControlCenterMetrics::DEFAULT,
            ControlCenterPlacement {
                bounds: bounds(),
                dpi: 96,
                anchor_right: 1700,
                anchor_y: 34,
                page: ControlCenterPage::Main,
                item_count: 0,
                scroll_offset: 0,
            },
        );
        let limited = super::control_center_focus_targets(&layout, &snap);
        assert!(!limited.contains(&ControlCenterHitTarget::WifiToggle));
        assert!(!limited.contains(&ControlCenterHitTarget::BluetoothToggle));
        assert!(!limited.contains(&ControlCenterHitTarget::DisplaySlider));
        assert!(limited.contains(&ControlCenterHitTarget::SoundSlider));
        assert_eq!(
            limited.last(),
            Some(&ControlCenterHitTarget::OttoSettingsTile)
        );

        snap.wifi.supported = true;
        snap.bluetooth.supported = true;
        snap.status.brightness = Some(55);
        let full = super::control_center_focus_targets(&layout, &snap);
        assert_eq!(full.first(), Some(&ControlCenterHitTarget::WifiToggle));
        assert!(full.contains(&ControlCenterHitTarget::BluetoothToggle));
        assert!(full.contains(&ControlCenterHitTarget::DisplaySlider));

        snap.page = ControlCenterPage::Display;
        let display_layout = compute_control_center_layout(
            &ControlCenterMetrics::DEFAULT,
            ControlCenterPlacement {
                bounds: bounds(),
                dpi: 96,
                anchor_right: 1700,
                anchor_y: 34,
                page: ControlCenterPage::Display,
                item_count: 0,
                scroll_offset: 0,
            },
        );
        let display_targets = super::control_center_focus_targets(&display_layout, &snap);
        assert_eq!(
            display_targets,
            vec![
                ControlCenterHitTarget::BackButton,
                ControlCenterHitTarget::DisplaySlider,
                ControlCenterHitTarget::FooterPrimary,
                ControlCenterHitTarget::FooterSecondary,
            ]
        );
    }

    #[test]
    fn control_center_focus_navigates_adjusts_sliders_and_preserves_activation() {
        use super::{ControlCenterFocus, ControlCenterFocusOutcome, SLIDER_KEY_STEP_PERCENT};
        use crate::bar_focus::BarKey;

        let targets = [
            ControlCenterHitTarget::BackButton,
            ControlCenterHitTarget::SoundMuteToggle,
            ControlCenterHitTarget::SoundSlider,
            ControlCenterHitTarget::SubPanelItem(0),
            ControlCenterHitTarget::FooterPrimary,
        ];
        let mut focus = ControlCenterFocus::default();
        assert_eq!(
            focus.enter(&targets),
            Some(ControlCenterHitTarget::BackButton)
        );
        assert_eq!(
            focus.handle(BarKey::Previous, &targets),
            ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::FooterPrimary)
        );
        assert_eq!(
            focus.handle(BarKey::Down, &targets),
            ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::BackButton)
        );
        assert_eq!(
            focus.handle(BarKey::NextTab, &targets),
            ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SoundMuteToggle)
        );
        assert_eq!(
            focus.handle(BarKey::Activate, &targets),
            ControlCenterFocusOutcome::Activate(ControlCenterHitTarget::SoundMuteToggle)
        );
        assert_eq!(
            focus.current(),
            Some(ControlCenterHitTarget::SoundMuteToggle)
        );
        assert_eq!(
            focus.handle(BarKey::Next, &targets),
            ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SoundSlider)
        );
        assert_eq!(
            focus.handle(BarKey::Next, &targets),
            ControlCenterFocusOutcome::AdjustSlider {
                target: ControlCenterHitTarget::SoundSlider,
                delta: SLIDER_KEY_STEP_PERCENT,
            }
        );
        assert_eq!(
            focus.handle(BarKey::Previous, &targets),
            ControlCenterFocusOutcome::AdjustSlider {
                target: ControlCenterHitTarget::SoundSlider,
                delta: -SLIDER_KEY_STEP_PERCENT,
            }
        );
        assert_eq!(
            focus.handle(BarKey::Down, &targets),
            ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SubPanelItem(0))
        );
        assert_eq!(
            focus.handle(BarKey::Up, &targets),
            ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SoundSlider)
        );
        assert_eq!(
            focus.handle(BarKey::Exit, &targets),
            ControlCenterFocusOutcome::Exited
        );
        assert!(!focus.is_active());
    }

    #[test]
    fn focus_scroll_reveals_offscreen_subpanel_items_and_steps_slider_safely() {
        let snap = dock_area_snapshot();
        let place = |scroll_offset| ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1700,
            anchor_y: 34,
            page: ControlCenterPage::OttoSettings,
            item_count: 8,
            scroll_offset,
        };
        let mut layout = compute_control_center_layout_for_snapshot(
            &ControlCenterMetrics::DEFAULT,
            place(0),
            &snap,
        );
        assert_eq!(
            super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(0)),
            None
        );
        let scroll_down =
            super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(7))
                .expect("bottom item should scroll into view");
        assert!(scroll_down > 0 && scroll_down <= layout.max_scroll);

        layout = compute_control_center_layout_for_snapshot(
            &ControlCenterMetrics::DEFAULT,
            place(scroll_down),
            &snap,
        );
        assert_eq!(
            super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(7)),
            None
        );
        assert_eq!(
            super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(0)),
            Some(0)
        );
        assert_eq!(
            super::scroll_offset_for_item(&layout, ControlCenterHitTarget::BackButton),
            None
        );
        assert_eq!(super::step_slider_percent(98, 5), 100);
        assert_eq!(super::step_slider_percent(2, -5), 0);
        assert_eq!(
            super::subpanel_opener_target(ControlCenterPage::Wifi),
            Some(ControlCenterHitTarget::WifiOpenPage)
        );
        assert_eq!(super::subpanel_opener_target(ControlCenterPage::Main), None);
    }

    #[test]
    fn reserve_space_needs_the_dock_and_a_hidden_windows_taskbar() {
        let reserve = OttoSettingItem::Toggle(OttoSettingKind::DockReserveSpace);
        let mut settings = OttoSettingsSnapshot::default();
        assert!(!settings.is_item_available(reserve));
        settings.plugins.dock = true;
        settings.dock.hide_taskbar = false;
        assert!(!settings.is_item_available(reserve));
        assert!(
            !settings
                .visible_items(OttoSettingsArea::Dock)
                .contains(&reserve)
        );
        settings.dock.hide_taskbar = true;
        assert!(settings.is_item_available(reserve));
        settings.dock.reserve_space = true;
        assert!(settings.is_enabled(OttoSettingKind::DockReserveSpace));
    }

    #[test]
    fn every_setting_belongs_to_exactly_one_area_and_dependents_stay_hidden() {
        let total: usize = OttoSettingsArea::ALL
            .into_iter()
            .map(|area| {
                OttoSettingItem::ALL
                    .into_iter()
                    .filter(|item| item.area() == area)
                    .count()
            })
            .sum();
        assert_eq!(total, OttoSettingItem::ALL.len());

        let mut snap = ControlCenterSnapshot {
            page: ControlCenterPage::OttoSettings,
            otto_area: Some(OttoSettingsArea::Dock),
            ..ControlCenterSnapshot::default()
        };
        assert_eq!(
            snap.settings.visible_items(OttoSettingsArea::Dock),
            vec![OttoSettingItem::Toggle(OttoSettingKind::Dock)]
        );
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::SubPanelItem(1)),
            None
        );

        snap.settings.plugins.dock = true;
        snap.settings.dock.hide_taskbar = false;
        let dock_items = snap.settings.visible_items(OttoSettingsArea::Dock);
        assert_eq!(dock_items.len(), 7);
        assert_eq!(dock_items[1], OttoSettingItem::DockIconSize);
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::SubPanelItem(1)),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::CycleDockIconSize,
                dismiss: false,
            })
        );

        snap.otto_area = Some(OttoSettingsArea::NotchTools);
        let tools = snap.settings.visible_items(OttoSettingsArea::NotchTools);
        let lyrics = OttoSettingItem::Toggle(OttoSettingKind::OnlineLyrics);
        assert!(!tools.contains(&lyrics));
        snap.settings.system.media_module = true;
        let tools = snap.settings.visible_items(OttoSettingsArea::NotchTools);
        let lyrics_idx = tools
            .iter()
            .position(|item| *item == lyrics)
            .expect("lyrics item");
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::SubPanelItem(lyrics_idx)),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::ToggleSetting(OttoSettingKind::OnlineLyrics),
                dismiss: false,
            })
        );
        assert_eq!(snap.settings.active_module_count(), (2, 11));
        assert_eq!(
            snap.settings.visible_items(OttoSettingsArea::Notch),
            vec![OttoSettingItem::Toggle(OttoSettingKind::Notch)]
        );
        snap.settings.plugins.notch = true;
        assert_eq!(
            snap.settings.visible_items(OttoSettingsArea::Notch).len(),
            4
        );
        assert_eq!(snap.settings.active_module_count(), (3, 11));
    }

    #[test]
    fn area_status_reads_off_for_a_disabled_master_and_counts_toggles_otherwise() {
        let mut settings = OttoSettingsSnapshot::default();
        assert_eq!(settings.area_status(OttoSettingsArea::Dock), "Off");
        assert_eq!(settings.area_status(OttoSettingsArea::MenuBar), "Off");
        assert_eq!(settings.area_status(OttoSettingsArea::Notch), "Off");
        assert_eq!(settings.area_status(OttoSettingsArea::NotchTools), "Off");
        settings.plugins.notch = true;
        assert_eq!(settings.area_status(OttoSettingsArea::Notch), "1 of 4 on");
        settings.plugins.dock = true;
        settings.dock.hide_taskbar = false;
        settings.dock.show_downloads = false;
        settings.dock.show_trash = false;
        settings.dock.autohide = false;
        settings.dock.reserve_space = false;
        settings.dock.magnification = true;
        assert_eq!(settings.area_status(OttoSettingsArea::Dock), "2 of 7 on");
        settings.plugins.top_bar = true;
        settings.menu_bar.show_background = true;
        settings.menu_bar.keep_notch = false;
        assert_eq!(settings.area_status(OttoSettingsArea::MenuBar), "7 of 8 on");
    }

    #[test]
    fn the_overview_lists_six_cards_and_an_area_page_lists_its_own_groups() {
        let placement = |item_count| ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1700,
            anchor_y: 34,
            page: ControlCenterPage::OttoSettings,
            item_count,
            scroll_offset: 0,
        };
        let mut snap = ControlCenterSnapshot {
            page: ControlCenterPage::OttoSettings,
            ..ControlCenterSnapshot::default()
        };
        let overview = compute_control_center_layout_for_snapshot(
            &ControlCenterMetrics::DEFAULT,
            placement(0),
            &snap,
        );
        assert!(overview.items.is_empty());
        assert!(overview.area_cards.iter().all(|card| !card.is_empty()));
        assert!(!overview.footer_primary.is_empty());
        let card = overview.area_cards[OttoSettingsArea::Notch.index()];
        assert_eq!(
            overview.hit_test(card.x + 8, card.y + 8),
            Some(ControlCenterHitTarget::OttoAreaCard(
                OttoSettingsArea::Notch
            ))
        );
        assert_eq!(
            control_center_action(
                &snap,
                ControlCenterHitTarget::OttoAreaCard(OttoSettingsArea::Notch)
            ),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::OpenOttoArea(OttoSettingsArea::Notch),
                dismiss: false,
            })
        );
        let targets = control_center_focus_targets(&overview, &snap);
        assert_eq!(targets[0], ControlCenterHitTarget::BackButton);
        assert!(targets.contains(&ControlCenterHitTarget::OttoAreaCard(
            OttoSettingsArea::System
        )));

        snap.otto_area = Some(OttoSettingsArea::MenuBar);
        snap.settings.plugins.top_bar = true;
        snap.settings.plugins.notch = true;
        snap.last_error = Some(String::from("Disable dependent plugin first"));
        let count = snap.subpanel_item_count(ControlCenterPage::OttoSettings);
        let detail = compute_control_center_layout_for_snapshot(
            &ControlCenterMetrics::DEFAULT,
            placement(count),
            &snap,
        );
        assert_eq!(count, 9);
        assert!(detail.area_cards.iter().all(Rect::is_empty));
        assert!(detail.footer_primary.is_empty());
        assert!(!detail.hero.is_empty());
        assert!(detail.hero.bottom() <= detail.error_banner.y);
        assert!(detail.error_banner.bottom() <= detail.list_viewport.y);
        let titles: Vec<_> = detail.groups.iter().map(|group| group.title).collect();
        assert_eq!(titles, ["", "Visibility", "Show in the Bar"]);
        let chips = detail.groups[2];
        assert_eq!(chips.count, 5);
        let first = detail.items[chips.start_index];
        let last = detail.items[chips.start_index + 4];
        assert!(chips.container.contains(first.x, first.y));
        assert!(first.width < chips.container.width / 2);
        assert!(last.y > first.y);
        assert!(detail.window.bottom() >= chips.container.bottom());
    }

    #[test]
    fn menu_bar_settings_cycle_the_hide_mode_and_toggle_items_by_visibility() {
        let mut snap = ControlCenterSnapshot {
            page: ControlCenterPage::OttoSettings,
            otto_area: Some(OttoSettingsArea::MenuBar),
            ..ControlCenterSnapshot::default()
        };
        snap.settings.plugins.top_bar = true;
        let items = snap.settings.visible_items(OttoSettingsArea::MenuBar);
        let hide = items
            .iter()
            .position(|item| *item == OttoSettingItem::MenuBarHide)
            .expect("hide item");
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::SubPanelItem(hide)),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::CycleMenuBarHide,
                dismiss: false,
            })
        );
        assert!(snap.settings.is_enabled(OttoSettingKind::MenuBarClock));
        assert!(snap.settings.is_enabled(OttoSettingKind::MenuBarBackground));
        snap.settings.menu_bar.hidden_items =
            snap.settings.menu_bar.hidden_items.toggled(BarItem::Clock);
        assert!(!snap.settings.is_enabled(OttoSettingKind::MenuBarClock));
        assert!(snap.settings.is_enabled(OttoSettingKind::MenuBarTray));
    }

    #[test]
    fn focus_row_separates_pomodoro_toggle_and_windows_dnd_settings_trigger() {
        assert_eq!(focus_row_subtitle(false, false), "Disabled");
        assert_eq!(focus_row_subtitle(false, true), "Disabled");
        assert_eq!(focus_row_subtitle(true, false), "Off");
        assert_eq!(focus_row_subtitle(true, true), "Focus On");

        let mut snap = ControlCenterSnapshot::default();
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FocusToggle),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::OpenPage(ControlCenterPage::OttoSettings),
                dismiss: false,
            })
        );
        snap.settings.system.focus_module = true;
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FocusToggle),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::OpenPage(ControlCenterPage::OttoSettings),
                dismiss: false,
            })
        );
        snap.settings.plugins.notch = true;
        assert_eq!(
            control_center_action(&snap, ControlCenterHitTarget::FocusToggle),
            Some(ControlCenterClickOutcome {
                command: ControlCenterCommand::ToggleFocus,
                dismiss: false,
            })
        );
        let layout = main_layout(96);
        assert!(layout.focus_toggle.contains(
            layout.focus_settings.x + layout.focus_settings.width / 2,
            layout.focus_settings.y + layout.focus_settings.height / 2
        ));
        assert!(layout.focus_settings.width >= 20);
    }

    #[test]
    fn control_center_shell_and_cards_are_concentric() {
        let metrics = ControlCenterMetrics::DEFAULT;
        assert_eq!(metrics.shell_radius, metrics.card_radius + metrics.padding);
        let layout = main_layout(96);
        assert_eq!(layout.shell_radius, 26);
        assert_eq!(layout.card_radius, 14);
        assert_eq!(layout.connectivity_card.x, 12);
        assert_eq!(layout.connectivity_card.y, 12);
    }
}
