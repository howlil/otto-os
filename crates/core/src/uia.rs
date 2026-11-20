use crate::control_center::{
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot, OttoSettingsArea,
    control_center_focus_targets,
};
use crate::dock::DockLayout;
use crate::layout::{HitTarget, Layout, Rect};
use crate::pomodoro::FocusSubMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibleRole {
    Toolbar,
    Button,
    Tab,
    Slider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleNode {
    pub runtime_id: i32,
    pub automation_id: String,
    pub name: String,
    pub role: AccessibleRole,
    pub rect: Rect,
    pub enabled: bool,
    pub invoke_tag: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleTree {
    pub root_name: String,
    pub root_automation_id: String,
    pub root_rect: Rect,
    pub nodes: Vec<AccessibleNode>,
}

#[must_use]
pub const fn encode_bar_hit_target(target: HitTarget) -> i32 {
    match target {
        HitTarget::Start => 1,
        HitTarget::AppMenu => 2,
        HitTarget::FileMenu => 3,
        HitTarget::EditMenu => 4,
        HitTarget::ViewMenu => 5,
        HitTarget::WindowMenu => 6,
        HitTarget::HelpMenu => 7,
        HitTarget::MenuOverflow => 8,
        HitTarget::Notch => 9,
        HitTarget::NotchSecondary => 10,
        HitTarget::PomodoroPlayPause => 11,
        HitTarget::PomodoroReset => 12,
        HitTarget::PomodoroSkip => 13,
        HitTarget::PomodoroStartNext => 14,
        HitTarget::PomodoroExtendFive => 15,
        HitTarget::NotchTabPomodoro => 16,
        HitTarget::NotchTabShelf => 17,
        HitTarget::NotchTabClipboard => 18,
        HitTarget::NotchTabMedia => 19,
        HitTarget::ShelfAdd => 20,
        HitTarget::ShelfClear => 21,
        HitTarget::ShelfPager => 22,
        HitTarget::ClipboardPauseToggle => 23,
        HitTarget::ClipboardClear => 24,
        HitTarget::ClipboardPager => 25,
        HitTarget::PrivacyIndicator => 26,
        HitTarget::PrivacyMicMute => 27,
        HitTarget::MediaPlayPause => 28,
        HitTarget::MediaPrevious => 29,
        HitTarget::MediaNext => 30,
        HitTarget::MediaSeekbar => 31,
        HitTarget::MediaLyricsToggle => 32,
        HitTarget::Tray => 33,
        HitTarget::StatusWifi => 34,
        HitTarget::StatusVolume => 35,
        HitTarget::StatusBattery => 36,
        HitTarget::QuickSettings => 37,
        HitTarget::Search => 38,
        HitTarget::ControlCenter => 39,
        HitTarget::Clock => 40,
        HitTarget::Notifications => 41,
        HitTarget::NotificationCard => 42,
        HitTarget::NotificationClose => 43,
        HitTarget::NotificationAccept => 44,
        HitTarget::NotificationDecline => 45,
        HitTarget::NotchTabTasks => 46,
        HitTarget::TasksAdd => 47,
        HitTarget::TasksClearDone => 48,
        HitTarget::TasksPager => 49,
        HitTarget::PomodoroPreset(i) => 100 + i as i32,
        HitTarget::FocusMode(mode) => 200 + mode.index() as i32,
        HitTarget::ShelfItem(i) => 300 + i as i32,
        HitTarget::ShelfCopyItem(i) => 400 + i as i32,
        HitTarget::ShelfRemoveItem(i) => 500 + i as i32,
        HitTarget::ClipboardItem(i) => 600 + i as i32,
        HitTarget::ClipboardRemoveItem(i) => 700 + i as i32,
        HitTarget::TaskItem(i) => 800 + i as i32,
        HitTarget::TaskCheck(i) => 900 + i as i32,
        HitTarget::TaskRemove(i) => 1000 + i as i32,
    }
}

#[must_use]
pub const fn decode_bar_hit_target(tag: i32) -> Option<HitTarget> {
    match tag {
        1 => Some(HitTarget::Start),
        2 => Some(HitTarget::AppMenu),
        3 => Some(HitTarget::FileMenu),
        4 => Some(HitTarget::EditMenu),
        5 => Some(HitTarget::ViewMenu),
        6 => Some(HitTarget::WindowMenu),
        7 => Some(HitTarget::HelpMenu),
        8 => Some(HitTarget::MenuOverflow),
        9 => Some(HitTarget::Notch),
        10 => Some(HitTarget::NotchSecondary),
        11 => Some(HitTarget::PomodoroPlayPause),
        12 => Some(HitTarget::PomodoroReset),
        13 => Some(HitTarget::PomodoroSkip),
        14 => Some(HitTarget::PomodoroStartNext),
        15 => Some(HitTarget::PomodoroExtendFive),
        16 => Some(HitTarget::NotchTabPomodoro),
        17 => Some(HitTarget::NotchTabShelf),
        18 => Some(HitTarget::NotchTabClipboard),
        19 => Some(HitTarget::NotchTabMedia),
        20 => Some(HitTarget::ShelfAdd),
        21 => Some(HitTarget::ShelfClear),
        22 => Some(HitTarget::ShelfPager),
        23 => Some(HitTarget::ClipboardPauseToggle),
        24 => Some(HitTarget::ClipboardClear),
        25 => Some(HitTarget::ClipboardPager),
        26 => Some(HitTarget::PrivacyIndicator),
        27 => Some(HitTarget::PrivacyMicMute),
        28 => Some(HitTarget::MediaPlayPause),
        29 => Some(HitTarget::MediaPrevious),
        30 => Some(HitTarget::MediaNext),
        31 => Some(HitTarget::MediaSeekbar),
        32 => Some(HitTarget::MediaLyricsToggle),
        33 => Some(HitTarget::Tray),
        34 => Some(HitTarget::StatusWifi),
        35 => Some(HitTarget::StatusVolume),
        36 => Some(HitTarget::StatusBattery),
        37 => Some(HitTarget::QuickSettings),
        38 => Some(HitTarget::Search),
        39 => Some(HitTarget::ControlCenter),
        40 => Some(HitTarget::Clock),
        41 => Some(HitTarget::Notifications),
        42 => Some(HitTarget::NotificationCard),
        43 => Some(HitTarget::NotificationClose),
        44 => Some(HitTarget::NotificationAccept),
        45 => Some(HitTarget::NotificationDecline),
        46 => Some(HitTarget::NotchTabTasks),
        47 => Some(HitTarget::TasksAdd),
        48 => Some(HitTarget::TasksClearDone),
        49 => Some(HitTarget::TasksPager),
        100..=199 => Some(HitTarget::PomodoroPreset((tag - 100) as u8)),
        200 => Some(HitTarget::FocusMode(FocusSubMode::Pomodoro)),
        201 => Some(HitTarget::FocusMode(FocusSubMode::Timer)),
        202 => Some(HitTarget::FocusMode(FocusSubMode::Stopwatch)),
        300..=399 => Some(HitTarget::ShelfItem((tag - 300) as u8)),
        400..=499 => Some(HitTarget::ShelfCopyItem((tag - 400) as u8)),
        500..=599 => Some(HitTarget::ShelfRemoveItem((tag - 500) as u8)),
        600..=699 => Some(HitTarget::ClipboardItem((tag - 600) as u8)),
        700..=799 => Some(HitTarget::ClipboardRemoveItem((tag - 700) as u8)),
        800..=899 => Some(HitTarget::TaskItem((tag - 800) as u8)),
        900..=999 => Some(HitTarget::TaskCheck((tag - 900) as u8)),
        1000..=1099 => Some(HitTarget::TaskRemove((tag - 1000) as u8)),
        _ => None,
    }
}

#[must_use]
pub const fn encode_control_center_target(target: ControlCenterHitTarget) -> i32 {
    match target {
        ControlCenterHitTarget::WifiToggle => 1,
        ControlCenterHitTarget::WifiOpenPage => 2,
        ControlCenterHitTarget::BluetoothToggle => 3,
        ControlCenterHitTarget::BluetoothOpenPage => 4,
        ControlCenterHitTarget::FocusToggle => 5,
        ControlCenterHitTarget::FocusOpenSettings => 6,
        ControlCenterHitTarget::BatteryTile => 8,
        ControlCenterHitTarget::DisplayOpenPage => 9,
        ControlCenterHitTarget::DisplaySlider => 10,
        ControlCenterHitTarget::SoundOpenPage => 11,
        ControlCenterHitTarget::SoundMuteToggle => 12,
        ControlCenterHitTarget::SoundSlider => 13,
        ControlCenterHitTarget::MediaBody => 14,
        ControlCenterHitTarget::MediaPrev => 15,
        ControlCenterHitTarget::MediaPlayPause => 16,
        ControlCenterHitTarget::MediaNext => 17,
        ControlCenterHitTarget::OttoSettingsTile => 18,
        ControlCenterHitTarget::BackButton => 19,
        ControlCenterHitTarget::HeaderToggle => 20,
        ControlCenterHitTarget::FooterPrimary => 21,
        ControlCenterHitTarget::FooterSecondary => 22,
        ControlCenterHitTarget::OttoAreaCard(area) => 40 + area.index() as i32,
        ControlCenterHitTarget::SubPanelItem(idx) => 1000 + idx as i32,
    }
}

#[must_use]
pub const fn decode_control_center_target(tag: i32) -> Option<ControlCenterHitTarget> {
    match tag {
        1 => Some(ControlCenterHitTarget::WifiToggle),
        2 => Some(ControlCenterHitTarget::WifiOpenPage),
        3 => Some(ControlCenterHitTarget::BluetoothToggle),
        4 => Some(ControlCenterHitTarget::BluetoothOpenPage),
        5 => Some(ControlCenterHitTarget::FocusToggle),
        6 => Some(ControlCenterHitTarget::FocusOpenSettings),
        8 => Some(ControlCenterHitTarget::BatteryTile),
        9 => Some(ControlCenterHitTarget::DisplayOpenPage),
        10 => Some(ControlCenterHitTarget::DisplaySlider),
        11 => Some(ControlCenterHitTarget::SoundOpenPage),
        12 => Some(ControlCenterHitTarget::SoundMuteToggle),
        13 => Some(ControlCenterHitTarget::SoundSlider),
        14 => Some(ControlCenterHitTarget::MediaBody),
        15 => Some(ControlCenterHitTarget::MediaPrev),
        16 => Some(ControlCenterHitTarget::MediaPlayPause),
        17 => Some(ControlCenterHitTarget::MediaNext),
        18 => Some(ControlCenterHitTarget::OttoSettingsTile),
        19 => Some(ControlCenterHitTarget::BackButton),
        20 => Some(ControlCenterHitTarget::HeaderToggle),
        21 => Some(ControlCenterHitTarget::FooterPrimary),
        22 => Some(ControlCenterHitTarget::FooterSecondary),
        40..=45 => Some(ControlCenterHitTarget::OttoAreaCard(
            OttoSettingsArea::ALL[(tag - 40) as usize],
        )),
        1000..=99999 => Some(ControlCenterHitTarget::SubPanelItem((tag - 1000) as usize)),
        _ => None,
    }
}

fn bar_target_rect(layout: &Layout, target: HitTarget) -> Option<Rect> {
    if let Some(rect) = layout.slot_rect(target) {
        return (!rect.is_empty()).then_some(rect);
    }
    if let Some(rect) = layout.notch_control_rect(target) {
        return Some(rect);
    }
    let rect = match target {
        HitTarget::Notch => layout.notch,
        HitTarget::NotchSecondary => layout.notch_secondary,
        HitTarget::PrivacyIndicator => layout.privacy_dots,
        _ => return None,
    };
    (!rect.is_empty()).then_some(rect)
}

fn bar_target_role(target: HitTarget) -> AccessibleRole {
    match target {
        HitTarget::NotchTabPomodoro
        | HitTarget::NotchTabShelf
        | HitTarget::NotchTabClipboard
        | HitTarget::NotchTabTasks
        | HitTarget::NotchTabMedia => AccessibleRole::Tab,
        HitTarget::MediaSeekbar => AccessibleRole::Slider,
        _ => AccessibleRole::Button,
    }
}

fn bar_shelf_clipboard_metadata(target: HitTarget) -> Option<(String, String)> {
    let pair = match target {
        HitTarget::ShelfAdd => (
            String::from("otto.notch.shelf.add"),
            String::from("Add Files"),
        ),
        HitTarget::ShelfClear => (
            String::from("otto.notch.shelf.clear"),
            String::from("Clear Shelf"),
        ),
        HitTarget::ShelfPager => (
            String::from("otto.notch.shelf.pager"),
            String::from("Shelf Page"),
        ),
        HitTarget::ShelfItem(i) => (
            format!("otto.notch.shelf.item.{i}"),
            format!("Shelf Item {}", i + 1),
        ),
        HitTarget::ShelfCopyItem(i) => (
            format!("otto.notch.shelf.copy.{i}"),
            format!("Copy Shelf Item {}", i + 1),
        ),
        HitTarget::ShelfRemoveItem(i) => (
            format!("otto.notch.shelf.remove.{i}"),
            format!("Remove Shelf Item {}", i + 1),
        ),
        HitTarget::ClipboardPauseToggle => (
            String::from("otto.notch.clipboard.pause"),
            String::from("Pause Clipboard"),
        ),
        HitTarget::ClipboardClear => (
            String::from("otto.notch.clipboard.clear"),
            String::from("Clear Clipboard"),
        ),
        HitTarget::ClipboardPager => (
            String::from("otto.notch.clipboard.pager"),
            String::from("Clipboard Page"),
        ),
        HitTarget::ClipboardItem(i) => (
            format!("otto.notch.clipboard.item.{i}"),
            format!("Clipboard Item {}", i + 1),
        ),
        HitTarget::ClipboardRemoveItem(i) => (
            format!("otto.notch.clipboard.remove.{i}"),
            format!("Remove Clipboard Item {}", i + 1),
        ),
        HitTarget::TasksAdd => (
            String::from("otto.notch.tasks.add"),
            String::from("Add Task"),
        ),
        HitTarget::TasksClearDone => (
            String::from("otto.notch.tasks.clear_done"),
            String::from("Clear Done Tasks"),
        ),
        HitTarget::TasksPager => (
            String::from("otto.notch.tasks.pager"),
            String::from("Tasks Page"),
        ),
        HitTarget::TaskItem(i) => (
            format!("otto.notch.tasks.item.{i}"),
            format!("Focus on Task {}", i + 1),
        ),
        HitTarget::TaskCheck(i) => (
            format!("otto.notch.tasks.check.{i}"),
            format!("Toggle Task {} Done", i + 1),
        ),
        HitTarget::TaskRemove(i) => (
            format!("otto.notch.tasks.remove.{i}"),
            format!("Remove Task {}", i + 1),
        ),
        _ => return None,
    };
    Some(pair)
}

fn bar_notch_target_metadata(target: HitTarget) -> Option<(String, String)> {
    if let Some(pair) = bar_shelf_clipboard_metadata(target) {
        return Some(pair);
    }
    let pair = match target {
        HitTarget::Notch => (String::from("otto.notch"), String::from("Notch")),
        HitTarget::NotchSecondary => (
            String::from("otto.notch.secondary"),
            String::from("Secondary Notch"),
        ),
        HitTarget::PomodoroPlayPause => (
            String::from("otto.notch.focus.play_pause"),
            String::from("Focus Play or Pause"),
        ),
        HitTarget::PomodoroReset => (
            String::from("otto.notch.focus.reset"),
            String::from("Focus Reset"),
        ),
        HitTarget::PomodoroSkip => (
            String::from("otto.notch.focus.skip"),
            String::from("Focus Skip"),
        ),
        HitTarget::PomodoroStartNext => (
            String::from("otto.notch.focus.start_next"),
            String::from("Start Next Session"),
        ),
        HitTarget::PomodoroExtendFive => (
            String::from("otto.notch.focus.extend"),
            String::from("Extend Five Minutes"),
        ),
        HitTarget::PomodoroPreset(i) => (
            format!("otto.notch.focus.preset.{i}"),
            format!("Focus Preset {}", i + 1),
        ),
        HitTarget::FocusMode(mode) => (
            format!("otto.notch.focus.mode.{}", mode.index()),
            format!("{mode:?}"),
        ),
        HitTarget::NotchTabPomodoro => {
            (String::from("otto.notch.tab.focus"), String::from("Focus"))
        }
        HitTarget::NotchTabShelf => (String::from("otto.notch.tab.shelf"), String::from("Shelf")),
        HitTarget::NotchTabClipboard => (
            String::from("otto.notch.tab.clipboard"),
            String::from("Clipboard"),
        ),
        HitTarget::NotchTabTasks => (String::from("otto.notch.tab.tasks"), String::from("Tasks")),
        HitTarget::NotchTabMedia => (
            String::from("otto.notch.tab.media"),
            String::from("Now Playing"),
        ),
        HitTarget::PrivacyIndicator => (
            String::from("otto.notch.privacy"),
            String::from("Privacy Indicator"),
        ),
        HitTarget::PrivacyMicMute => (
            String::from("otto.notch.privacy.mic_mute"),
            String::from("Toggle Microphone Mute"),
        ),
        HitTarget::MediaPlayPause => (
            String::from("otto.notch.media.play_pause"),
            String::from("Play or Pause Media"),
        ),
        HitTarget::MediaPrevious => (
            String::from("otto.notch.media.previous"),
            String::from("Previous Track"),
        ),
        HitTarget::MediaNext => (
            String::from("otto.notch.media.next"),
            String::from("Next Track"),
        ),
        HitTarget::MediaSeekbar => (
            String::from("otto.notch.media.seekbar"),
            String::from("Media Seekbar"),
        ),
        HitTarget::MediaLyricsToggle => (
            String::from("otto.notch.media.lyrics"),
            String::from("Toggle Lyrics"),
        ),
        _ => return None,
    };
    Some(pair)
}

fn bar_target_metadata(target: HitTarget, app_name: &str) -> (String, String) {
    if let Some(pair) = bar_notch_target_metadata(target) {
        return pair;
    }
    match target {
        HitTarget::Start => (String::from("otto.bar.start"), String::from("Otto Menu")),
        HitTarget::AppMenu => (
            String::from("otto.bar.app"),
            if app_name.trim().is_empty() {
                String::from("Active App")
            } else {
                app_name.to_string()
            },
        ),
        HitTarget::FileMenu => (String::from("otto.bar.file"), String::from("File")),
        HitTarget::EditMenu => (String::from("otto.bar.edit"), String::from("Edit")),
        HitTarget::ViewMenu => (String::from("otto.bar.view"), String::from("View")),
        HitTarget::WindowMenu => (String::from("otto.bar.window"), String::from("Window")),
        HitTarget::HelpMenu => (String::from("otto.bar.help"), String::from("Help")),
        HitTarget::MenuOverflow => (
            String::from("otto.bar.overflow"),
            String::from("More Menus"),
        ),
        HitTarget::Tray => (String::from("otto.bar.tray"), String::from("System Tray")),
        HitTarget::StatusWifi => (String::from("otto.bar.wifi"), String::from("Wi-Fi")),
        HitTarget::StatusVolume => (String::from("otto.bar.volume"), String::from("Volume")),
        HitTarget::StatusBattery => (String::from("otto.bar.battery"), String::from("Battery")),
        HitTarget::QuickSettings => (
            String::from("otto.bar.quick_settings"),
            String::from("Quick Settings"),
        ),
        HitTarget::Search => (String::from("otto.bar.search"), String::from("Search")),
        HitTarget::ControlCenter => (
            String::from("otto.bar.control_center"),
            String::from("Control Center"),
        ),
        HitTarget::Notifications => (
            String::from("otto.bar.notifications"),
            String::from("Notifications"),
        ),
        HitTarget::NotificationCard => (
            String::from("otto.notch.notification"),
            String::from("Open Notification"),
        ),
        HitTarget::NotificationClose => (
            String::from("otto.notch.notification.dismiss"),
            String::from("Dismiss Notification"),
        ),
        HitTarget::NotificationAccept => (
            String::from("otto.notch.call.open"),
            String::from("Open Call Window"),
        ),
        HitTarget::NotificationDecline => (
            String::from("otto.notch.call.dismiss"),
            String::from("Dismiss Call Alert"),
        ),
        _ => (String::from("otto.bar.clock"), String::from("Clock")),
    }
}

fn translate(rect: Rect, origin: (i32, i32)) -> Rect {
    Rect {
        x: rect.x + origin.0,
        y: rect.y + origin.1,
        width: rect.width,
        height: rect.height,
    }
}

#[must_use]
pub fn build_bar_accessible_tree(
    layout: &Layout,
    origin: (i32, i32),
    app_name: &str,
    media_present: bool,
    notch_panel_only: bool,
) -> AccessibleTree {
    let nodes = layout
        .keyboard_targets_with_media(media_present)
        .into_iter()
        .filter(|target| {
            if notch_panel_only {
                layout.notch_control_rect(*target).is_some()
            } else {
                true
            }
        })
        .filter_map(|target| {
            let rect = bar_target_rect(layout, target)?;
            let (automation_id, name) = bar_target_metadata(target, app_name);
            let tag = encode_bar_hit_target(target);
            Some(AccessibleNode {
                runtime_id: tag,
                automation_id,
                name,
                role: bar_target_role(target),
                rect: translate(rect, origin),
                enabled: true,
                invoke_tag: (target != HitTarget::MediaSeekbar).then_some(tag),
            })
        })
        .collect();
    let root_rect = if notch_panel_only {
        translate(layout.notch, origin)
    } else {
        translate(layout.bar, origin)
    };
    AccessibleTree {
        root_name: if notch_panel_only {
            String::from("Otto Notch Panel")
        } else {
            String::from("Otto Top Bar")
        },
        root_automation_id: if notch_panel_only {
            String::from("otto.notch.panel")
        } else {
            String::from("otto.bar")
        },
        root_rect,
        nodes,
    }
}

#[must_use]
pub fn build_dock_accessible_tree(
    layout: &DockLayout,
    origin: (i32, i32),
    labels: &[&str],
) -> AccessibleTree {
    let nodes = layout
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.opacity > 0.05 && item.size > 0.0)
        .map(|(index, item)| {
            let name = labels
                .get(index)
                .copied()
                .filter(|label| !label.trim().is_empty())
                .unwrap_or("Dock Item")
                .to_string();
            let size = item.size.round() as i32;
            AccessibleNode {
                runtime_id: index as i32 + 1,
                automation_id: format!("otto.dock.item.{index}"),
                name,
                role: AccessibleRole::Button,
                rect: Rect {
                    x: origin.0 + item.x.round() as i32,
                    y: origin.1 + item.y.round() as i32,
                    width: size,
                    height: size,
                },
                enabled: true,
                invoke_tag: Some(index as i32),
            }
        })
        .collect();
    AccessibleTree {
        root_name: String::from("Otto Dock"),
        root_automation_id: String::from("otto.dock"),
        root_rect: Rect {
            x: origin.0,
            y: origin.1,
            width: layout.window_width,
            height: layout.window_height,
        },
        nodes,
    }
}

fn control_center_subpanel_meta(
    target: ControlCenterHitTarget,
) -> Option<(String, String, AccessibleRole)> {
    let meta = match target {
        ControlCenterHitTarget::BackButton => (
            String::from("otto.cc.back"),
            String::from("Back"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::HeaderToggle => (
            String::from("otto.cc.header_toggle"),
            String::from("Toggle"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::OttoAreaCard(area) => (
            format!("otto.cc.area.{}", area.index()),
            String::from(area.label()),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::SubPanelItem(idx) => (
            format!("otto.cc.item.{idx}"),
            format!("Option {}", idx + 1),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FooterPrimary => (
            String::from("otto.cc.footer.primary"),
            String::from("Open Settings"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FooterSecondary => (
            String::from("otto.cc.footer.secondary"),
            String::from("Check for Updates"),
            AccessibleRole::Button,
        ),
        _ => return None,
    };
    Some(meta)
}

fn control_center_target_meta(target: ControlCenterHitTarget) -> (String, String, AccessibleRole) {
    if let Some(meta) = control_center_subpanel_meta(target) {
        return meta;
    }
    match target {
        ControlCenterHitTarget::WifiToggle => (
            String::from("otto.cc.wifi.toggle"),
            String::from("Wi-Fi"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::WifiOpenPage => (
            String::from("otto.cc.wifi.page"),
            String::from("Wi-Fi Settings"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::BluetoothToggle => (
            String::from("otto.cc.bluetooth.toggle"),
            String::from("Bluetooth"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::BluetoothOpenPage => (
            String::from("otto.cc.bluetooth.page"),
            String::from("Bluetooth Settings"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FocusToggle => (
            String::from("otto.cc.focus.toggle"),
            String::from("Focus Mode"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FocusOpenSettings => (
            String::from("otto.cc.focus.settings"),
            String::from("Focus Settings"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::BatteryTile => (
            String::from("otto.cc.battery"),
            String::from("Battery"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::DisplayOpenPage => (
            String::from("otto.cc.display.page"),
            String::from("Display"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::DisplaySlider => (
            String::from("otto.cc.display.slider"),
            String::from("Brightness"),
            AccessibleRole::Slider,
        ),
        ControlCenterHitTarget::SoundOpenPage => (
            String::from("otto.cc.sound.page"),
            String::from("Sound"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::SoundMuteToggle => (
            String::from("otto.cc.sound.mute"),
            String::from("Mute Sound"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::SoundSlider => (
            String::from("otto.cc.sound.slider"),
            String::from("Volume"),
            AccessibleRole::Slider,
        ),
        ControlCenterHitTarget::MediaBody => (
            String::from("otto.cc.media"),
            String::from("Now Playing"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::MediaPrev => (
            String::from("otto.cc.media.prev"),
            String::from("Previous Track"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::MediaPlayPause => (
            String::from("otto.cc.media.play_pause"),
            String::from("Play or Pause"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::MediaNext => (
            String::from("otto.cc.media.next"),
            String::from("Next Track"),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::OttoSettingsTile => (
            String::from("otto.cc.otto_settings"),
            String::from("Otto Settings"),
            AccessibleRole::Button,
        ),
        _ => (
            String::from("otto.cc.item"),
            String::from("Control"),
            AccessibleRole::Button,
        ),
    }
}

#[must_use]
pub fn build_control_center_accessible_tree(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> AccessibleTree {
    let origin = (layout.window.x, layout.window.y);
    let nodes = control_center_focus_targets(layout, snapshot)
        .into_iter()
        .filter_map(|target| {
            let rect = layout.target_rect(target)?;
            let (automation_id, name, role) = control_center_target_meta(target);
            let tag = encode_control_center_target(target);
            Some(AccessibleNode {
                runtime_id: tag,
                automation_id,
                name,
                role,
                rect: translate(rect, origin),
                enabled: true,
                invoke_tag: Some(tag),
            })
        })
        .collect();
    AccessibleTree {
        root_name: String::from("Otto Control Center"),
        root_automation_id: String::from("otto.control_center"),
        root_rect: layout.window,
        nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccessibleRole, build_bar_accessible_tree, build_control_center_accessible_tree,
        build_dock_accessible_tree, decode_bar_hit_target, decode_control_center_target,
        encode_bar_hit_target, encode_control_center_target,
    };
    use crate::control_center::{
        ControlCenterHitTarget, ControlCenterMetrics, ControlCenterPage, ControlCenterPlacement,
        ControlCenterSnapshot, OttoSettingsArea, compute_control_center_layout,
    };
    use crate::dock::{DockInput, DockMetrics, compute_dock_layout};
    use crate::layout::{HitTarget, Metrics, Rect, compute_layout_for_app};
    use crate::pomodoro::FocusSubMode;

    #[test]
    fn bar_and_control_center_invoke_tags_round_trip() {
        for target in [
            HitTarget::Start,
            HitTarget::Notch,
            HitTarget::PomodoroPreset(2),
            HitTarget::FocusMode(FocusSubMode::Timer),
            HitTarget::ShelfCopyItem(3),
            HitTarget::ShelfRemoveItem(4),
            HitTarget::ClipboardRemoveItem(1),
            HitTarget::Notifications,
            HitTarget::Clock,
        ] {
            assert_eq!(
                decode_bar_hit_target(encode_bar_hit_target(target)),
                Some(target)
            );
        }

        for target in [
            ControlCenterHitTarget::WifiToggle,
            ControlCenterHitTarget::BatteryTile,
            ControlCenterHitTarget::OttoAreaCard(OttoSettingsArea::Notch),
            ControlCenterHitTarget::SubPanelItem(7),
            ControlCenterHitTarget::FooterSecondary,
        ] {
            assert_eq!(
                decode_control_center_target(encode_control_center_target(target)),
                Some(target)
            );
        }
    }

    #[test]
    fn accessible_trees_expose_screen_bounds_and_invoke_tags() {
        let bar_layout = compute_layout_for_app(&Metrics::MACBOOK_PRO_14, 1920, 96, "Explorer");
        let bar_tree = build_bar_accessible_tree(&bar_layout, (100, 20), "Explorer", false, false);
        assert_eq!(bar_tree.root_name, "Otto Top Bar");
        assert!(
            bar_tree
                .nodes
                .iter()
                .any(|node| node.automation_id == "otto.notch" && node.rect.y >= 20)
        );

        let dock_layout = compute_dock_layout(
            &DockMetrics::default(),
            DockInput {
                screen_width: 1920,
                dpi: 96,
                app_count: 2,
                right_count: 0,
                has_trash: false,
                cursor_x: None,
                mag_strength: 0.0,
                bounce_offsets: &[0.0, 0.0],
                item_presence: &[1.0, 1.0],
                hide_progress: 0.0,
            },
        );
        let dock_tree =
            build_dock_accessible_tree(&dock_layout, (400, 900), &["Files", "Terminal"]);
        assert_eq!(dock_tree.nodes.len(), 2);
        assert_eq!(dock_tree.nodes[0].name, "Files");
        assert_eq!(dock_tree.nodes[0].invoke_tag, Some(0));
        assert_eq!(dock_tree.nodes[1].role, AccessibleRole::Button);

        let snapshot = ControlCenterSnapshot::default();
        let cc_layout = compute_control_center_layout(
            &ControlCenterMetrics::DEFAULT,
            ControlCenterPlacement {
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                dpi: 96,
                anchor_right: 1900,
                anchor_y: 36,
                page: ControlCenterPage::Main,
                item_count: 0,
                scroll_offset: 0,
            },
        );
        let cc_tree = build_control_center_accessible_tree(&cc_layout, &snapshot);
        assert!(!cc_tree.nodes.is_empty());
        assert!(cc_tree.nodes.iter().all(|node| node.invoke_tag.is_some()));
    }
}
