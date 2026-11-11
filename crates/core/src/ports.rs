use crate::media::{MediaCommand, NowPlaying};
use crate::status::SystemStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchAction {
    Media(MediaCommand),
    OpenTaskView,
    PlayPomodoroChime,
}

pub trait NotchPort {
    fn read_now_playing(&self) -> Option<NowPlaying>;

    fn execute(&self, action: NotchAction);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterAction {
    Media(MediaCommand),
    OpenCalendar,
    OpenCalendarSettings,
    OpenWifi,
    OpenBluetooth,
    OpenFocus,
    OpenDisplay,
    OpenSound,
}

pub trait ControlCenterPort {
    fn read_system_status(&self) -> SystemStatus;

    fn read_now_playing(&self) -> Option<NowPlaying>;

    fn set_volume(&self, percent: u8) -> Option<(u8, bool)>;

    fn set_brightness(&self, percent: u8) -> u8;

    fn execute(&self, action: ControlCenterAction);
}
