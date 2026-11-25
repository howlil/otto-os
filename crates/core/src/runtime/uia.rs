use crate::control_center::{
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot,
    control_center_focus_targets,
};
use crate::dock::layout::DockLayout;
use crate::layout::{HitTarget, Layout, Rect};
use crate::notch::focus_stats_view::FocusStatsRange;

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

pub use super::uia_codec::{decode_bar_hit_target, encode_bar_hit_target};

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
        ControlCenterHitTarget::BackButton => 19,
        ControlCenterHitTarget::HeaderToggle => 20,
        ControlCenterHitTarget::FooterPrimary => 21,
        ControlCenterHitTarget::FooterSecondary => 22,
        ControlCenterHitTarget::CanvasBackground => 23,
        ControlCenterHitTarget::ToolbarAdd => 24,
        ControlCenterHitTarget::ToolbarEdit => 25,
        ControlCenterHitTarget::ToolbarReset => 26,
        ControlCenterHitTarget::AddControlButton => 27,
        ControlCenterHitTarget::SubPanelItem(idx) => 1000 + idx as i32,
        ControlCenterHitTarget::GalleryItem(idx) => 2000 + idx as i32,
        ControlCenterHitTarget::TileRemove(id) => 10_000 + id as i32,
        ControlCenterHitTarget::TileResize(id) => 20_000 + id as i32,
        ControlCenterHitTarget::TileMove(id) => 30_000 + id as i32,
        ControlCenterHitTarget::TileAction(id) => 40_000 + id as i32,
        ControlCenterHitTarget::TileSlider(id) => 50_000 + id as i32,
        ControlCenterHitTarget::TileDetail(id) => 60_000 + id as i32,
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
        19 => Some(ControlCenterHitTarget::BackButton),
        20 => Some(ControlCenterHitTarget::HeaderToggle),
        21 => Some(ControlCenterHitTarget::FooterPrimary),
        22 => Some(ControlCenterHitTarget::FooterSecondary),
        23 => Some(ControlCenterHitTarget::CanvasBackground),
        24 => Some(ControlCenterHitTarget::ToolbarAdd),
        25 => Some(ControlCenterHitTarget::ToolbarEdit),
        26 => Some(ControlCenterHitTarget::ToolbarReset),
        27 => Some(ControlCenterHitTarget::AddControlButton),
        1000..=1999 => Some(ControlCenterHitTarget::SubPanelItem((tag - 1000) as usize)),
        2000..=2999 => Some(ControlCenterHitTarget::GalleryItem((tag - 2000) as usize)),
        10_000..=19_999 => Some(ControlCenterHitTarget::TileRemove((tag - 10_000) as u32)),
        20_000..=29_999 => Some(ControlCenterHitTarget::TileResize((tag - 20_000) as u32)),
        30_000..=39_999 => Some(ControlCenterHitTarget::TileMove((tag - 30_000) as u32)),
        40_000..=49_999 => Some(ControlCenterHitTarget::TileAction((tag - 40_000) as u32)),
        50_000..=59_999 => Some(ControlCenterHitTarget::TileSlider((tag - 50_000) as u32)),
        60_000..=69_999 => Some(ControlCenterHitTarget::TileDetail((tag - 60_000) as u32)),
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
        HitTarget::PrivacyIndicator => layout.privacy.dots,
        _ => return None,
    };
    (!rect.is_empty()).then_some(rect)
}

fn bar_target_role(target: HitTarget) -> AccessibleRole {
    match target {
        HitTarget::NotchTabPomodoro
        | HitTarget::NotchTabStats
        | HitTarget::NotchTabShelf
        | HitTarget::NotchTabClipboard
        | HitTarget::NotchTabTasks
        | HitTarget::NotchTabMedia
        | HitTarget::FocusStatsRange(_) => AccessibleRole::Tab,
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
        HitTarget::TaskEdit(i) => (
            format!("otto.notch.tasks.edit.{i}"),
            format!("Edit Task {}", i + 1),
        ),
        HitTarget::TaskRemove(i) => (
            format!("otto.notch.tasks.remove.{i}"),
            format!("Remove Task {}", i + 1),
        ),
        _ => return None,
    };
    Some(pair)
}

fn bar_focus_target_metadata(target: HitTarget) -> Option<(String, String)> {
    let pair = match target {
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
        HitTarget::PomodoroCategoryPill => (
            String::from("otto.notch.focus.category"),
            String::from("Focus Category"),
        ),
        HitTarget::PomodoroOpenSettings => (
            String::from("otto.notch.focus.settings"),
            String::from("Focus Mode Settings"),
        ),
        HitTarget::PomodoroCategoryOption(i) => (
            format!("otto.notch.focus.category.{i}"),
            format!("Focus Category Option {}", i + 1),
        ),
        HitTarget::PomodoroDurationDec => (
            String::from("otto.notch.focus.duration.dec"),
            String::from("Decrease Focus Minutes"),
        ),
        HitTarget::PomodoroDurationInc => (
            String::from("otto.notch.focus.duration.inc"),
            String::from("Increase Focus Minutes"),
        ),
        HitTarget::PomodoroTimeEdit => (
            String::from("otto.notch.focus.time.edit"),
            String::from("Edit Custom Focus Duration"),
        ),
        HitTarget::FocusStatsRange(range) => (
            format!("otto.notch.focus.stats.range.{}", range.index()),
            String::from(range.label()),
        ),
        HitTarget::FocusStatsOpenSettings => (
            String::from("otto.notch.focus.stats.details"),
            String::from("Open Focus Activity Details"),
        ),
        HitTarget::FocusStatsDay(i) => (
            format!("otto.notch.focus.stats.day.{i}"),
            format!("Screen Time Day {}", i + 1),
        ),
        HitTarget::FocusHeatmapFilter(i) => (
            format!("otto.notch.focus.heatmap.filter.{i}"),
            format!("Heatmap Filter {}", i + 1),
        ),
        HitTarget::FocusHeatmapCell(i) => (
            format!("otto.notch.focus.heatmap.cell.{i}"),
            format!("Heatmap Day {}", i + 1),
        ),
        _ => return None,
    };
    Some(pair)
}

fn bar_notch_target_metadata(target: HitTarget) -> Option<(String, String)> {
    if let Some(pair) = bar_shelf_clipboard_metadata(target) {
        return Some(pair);
    }
    if let Some(pair) = bar_focus_target_metadata(target) {
        return Some(pair);
    }
    let pair = match target {
        HitTarget::Notch => (String::from("otto.notch"), String::from("Notch")),
        HitTarget::NotchSecondary => (
            String::from("otto.notch.secondary"),
            String::from("Secondary Notch"),
        ),
        HitTarget::NotchTabPomodoro => {
            (String::from("otto.notch.tab.focus"), String::from("Focus"))
        }
        HitTarget::NotchTabStats => (
            String::from("otto.notch.tab.stats"),
            String::from("Screen Time"),
        ),
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
                app_name.to_owned()
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
                .to_owned();
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

fn cc_meta(id: &str, name: &str, role: AccessibleRole) -> (String, String, AccessibleRole) {
    (String::from(id), String::from(name), role)
}

fn control_center_subpanel_meta(
    target: ControlCenterHitTarget,
) -> Option<(String, String, AccessibleRole)> {
    let meta = match target {
        ControlCenterHitTarget::BackButton => {
            cc_meta("otto.cc.back", "Back", AccessibleRole::Button)
        }
        ControlCenterHitTarget::HeaderToggle => {
            cc_meta("otto.cc.header_toggle", "Toggle", AccessibleRole::Button)
        }
        ControlCenterHitTarget::SubPanelItem(idx) => (
            format!("otto.cc.item.{idx}"),
            format!("Option {}", idx + 1),
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FooterPrimary => cc_meta(
            "otto.cc.footer.primary",
            "Open Settings",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FooterSecondary => cc_meta(
            "otto.cc.footer.secondary",
            "Check for Updates",
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
        ControlCenterHitTarget::WifiToggle => {
            cc_meta("otto.cc.wifi.toggle", "Wi-Fi", AccessibleRole::Button)
        }
        ControlCenterHitTarget::WifiOpenPage => cc_meta(
            "otto.cc.wifi.page",
            "Wi-Fi Settings",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::BluetoothToggle => cc_meta(
            "otto.cc.bluetooth.toggle",
            "Bluetooth",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::BluetoothOpenPage => cc_meta(
            "otto.cc.bluetooth.page",
            "Bluetooth Settings",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::FocusToggle => {
            cc_meta("otto.cc.focus.toggle", "Focus Mode", AccessibleRole::Button)
        }
        ControlCenterHitTarget::FocusOpenSettings => cc_meta(
            "otto.cc.focus.settings",
            "Focus Settings",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::BatteryTile => {
            cc_meta("otto.cc.battery", "Battery", AccessibleRole::Button)
        }
        ControlCenterHitTarget::DisplayOpenPage => {
            cc_meta("otto.cc.display.page", "Display", AccessibleRole::Button)
        }
        ControlCenterHitTarget::DisplaySlider => cc_meta(
            "otto.cc.display.slider",
            "Brightness",
            AccessibleRole::Slider,
        ),
        ControlCenterHitTarget::SoundOpenPage => {
            cc_meta("otto.cc.sound.page", "Sound", AccessibleRole::Button)
        }
        ControlCenterHitTarget::SoundMuteToggle => {
            cc_meta("otto.cc.sound.mute", "Mute Sound", AccessibleRole::Button)
        }
        ControlCenterHitTarget::SoundSlider => {
            cc_meta("otto.cc.sound.slider", "Volume", AccessibleRole::Slider)
        }
        ControlCenterHitTarget::MediaBody => {
            cc_meta("otto.cc.media", "Now Playing", AccessibleRole::Button)
        }
        ControlCenterHitTarget::MediaPrev => cc_meta(
            "otto.cc.media.prev",
            "Previous Track",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::MediaPlayPause => cc_meta(
            "otto.cc.media.play_pause",
            "Play or Pause",
            AccessibleRole::Button,
        ),
        ControlCenterHitTarget::MediaNext => {
            cc_meta("otto.cc.media.next", "Next Track", AccessibleRole::Button)
        }
        _ => cc_meta("otto.cc.item", "Control", AccessibleRole::Button),
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
        ControlCenterSnapshot, compute_control_center_layout_for_snapshot,
    };
    use crate::dock::layout::{DockInput, DockMetrics, compute_dock_layout};
    use crate::layout::{HitTarget, Metrics, Rect, compute_layout_for_app};
    use crate::notch::focus_stats_view::FocusStatsRange;

    #[test]
    fn bar_and_control_center_invoke_tags_round_trip() {
        for target in [
            HitTarget::Start,
            HitTarget::Notch,
            HitTarget::PomodoroPreset(2),
            HitTarget::PomodoroCategoryPill,
            HitTarget::PomodoroOpenSettings,
            HitTarget::NotchTabStats,
            HitTarget::FocusStatsRange(FocusStatsRange::Today),
            HitTarget::FocusStatsRange(FocusStatsRange::Week),
            HitTarget::FocusStatsOpenSettings,
            HitTarget::FocusStatsDay(4),
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
        let cc_layout = compute_control_center_layout_for_snapshot(
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
            &snapshot,
        );
        let cc_tree = build_control_center_accessible_tree(&cc_layout, &snapshot);
        assert!(!cc_tree.nodes.is_empty());
        assert!(cc_tree.nodes.iter().all(|node| node.invoke_tag.is_some()));
    }
}
