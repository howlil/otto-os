use crate::notch::pomodoro::FocusSubMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    Start,
    AppMenu,
    FileMenu,
    EditMenu,
    ViewMenu,
    WindowMenu,
    HelpMenu,
    MenuOverflow,
    Notch,
    NotchSecondary,
    PomodoroPlayPause,
    PomodoroReset,
    PomodoroSkip,
    PomodoroStartNext,
    PomodoroExtendFive,
    PomodoroPreset(u8),
    FocusMode(FocusSubMode),
    NotchTabPomodoro,
    NotchTabShelf,
    NotchTabClipboard,
    NotchTabTasks,
    NotchTabMedia,
    ShelfAdd,
    ShelfClear,
    ShelfPager,
    ShelfItem(u8),
    ShelfCopyItem(u8),
    ShelfRemoveItem(u8),
    ClipboardPauseToggle,
    ClipboardClear,
    ClipboardPager,
    ClipboardItem(u8),
    ClipboardRemoveItem(u8),
    TasksAdd,
    TasksClearDone,
    TasksPager,
    TaskItem(u8),
    TaskCheck(u8),
    TaskEdit(u8),
    TaskRemove(u8),
    PrivacyIndicator,
    PrivacyMicMute,
    MediaPlayPause,
    MediaPrevious,
    MediaNext,
    MediaSeekbar,
    MediaLyricsToggle,
    Tray,
    StatusWifi,
    StatusVolume,
    StatusBattery,
    QuickSettings,
    Search,
    ControlCenter,
    Notifications,
    Clock,
    NotificationCard,
    NotificationClose,
    NotificationAccept,
    NotificationDecline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetScope {
    BarMenu,
    BarStatus,
    BarTrailing,
    Notch,
    Focus,
    Shelf,
    Clipboard,
    Tasks,
    Privacy,
    Media,
    Notification,
}

impl TargetScope {
    #[must_use]
    pub const fn is_bar(self) -> bool {
        matches!(self, Self::BarMenu | Self::BarStatus | Self::BarTrailing)
    }

    #[must_use]
    pub const fn is_notch_surface(self) -> bool {
        !self.is_bar()
    }
}

impl HitTarget {
    #[must_use]
    pub const fn scope(self) -> TargetScope {
        match self {
            Self::Start
            | Self::AppMenu
            | Self::FileMenu
            | Self::EditMenu
            | Self::ViewMenu
            | Self::WindowMenu
            | Self::HelpMenu
            | Self::MenuOverflow => TargetScope::BarMenu,
            Self::Tray
            | Self::StatusWifi
            | Self::StatusVolume
            | Self::StatusBattery
            | Self::QuickSettings => TargetScope::BarStatus,
            Self::Search | Self::ControlCenter | Self::Notifications | Self::Clock => {
                TargetScope::BarTrailing
            }
            Self::Notch
            | Self::NotchSecondary
            | Self::NotchTabPomodoro
            | Self::NotchTabShelf
            | Self::NotchTabClipboard
            | Self::NotchTabTasks
            | Self::NotchTabMedia => TargetScope::Notch,
            Self::PomodoroPlayPause
            | Self::PomodoroReset
            | Self::PomodoroSkip
            | Self::PomodoroStartNext
            | Self::PomodoroExtendFive
            | Self::PomodoroPreset(_)
            | Self::FocusMode(_) => TargetScope::Focus,
            Self::ShelfAdd
            | Self::ShelfClear
            | Self::ShelfPager
            | Self::ShelfItem(_)
            | Self::ShelfCopyItem(_)
            | Self::ShelfRemoveItem(_) => TargetScope::Shelf,
            Self::ClipboardPauseToggle
            | Self::ClipboardClear
            | Self::ClipboardPager
            | Self::ClipboardItem(_)
            | Self::ClipboardRemoveItem(_) => TargetScope::Clipboard,
            Self::TasksAdd
            | Self::TasksClearDone
            | Self::TasksPager
            | Self::TaskItem(_)
            | Self::TaskCheck(_)
            | Self::TaskEdit(_)
            | Self::TaskRemove(_) => TargetScope::Tasks,
            Self::PrivacyIndicator | Self::PrivacyMicMute => TargetScope::Privacy,
            Self::MediaPlayPause
            | Self::MediaPrevious
            | Self::MediaNext
            | Self::MediaSeekbar
            | Self::MediaLyricsToggle => TargetScope::Media,
            Self::NotificationCard
            | Self::NotificationClose
            | Self::NotificationAccept
            | Self::NotificationDecline => TargetScope::Notification,
        }
    }
}
