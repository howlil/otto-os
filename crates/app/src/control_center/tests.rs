use std::sync::{Arc, Mutex};

use otto_core::{
    BarKey, ControlCenterAction, ControlCenterCommand, ControlCenterHitTarget, ControlCenterPage,
    ControlCenterPort, NowPlaying, OttoSettingKind, SystemStatus, control_center_focus_targets,
};
use otto_platform::MenuNavEvent;

use crate::host::RuntimeContext;
use crate::settings::toggle_setting;

use super::{ControlCenterSurface, PostClickAction, poll_dismiss_action};

#[derive(Default)]
struct MockControlCenterState {
    brightness: Option<u8>,
    volume: u8,
    muted: bool,
}

struct MockControlCenterPort(Arc<Mutex<MockControlCenterState>>);

impl ControlCenterPort for MockControlCenterPort {
    fn read_system_status(&self) -> SystemStatus {
        let state = self.0.lock().unwrap();
        SystemStatus {
            brightness: state.brightness,
            volume: state.volume,
            muted: state.muted,
            ..SystemStatus::default()
        }
    }

    fn read_now_playing(&self) -> Option<NowPlaying> {
        None
    }

    fn set_volume(&self, percent: u8) -> Option<(u8, bool)> {
        let mut state = self.0.lock().unwrap();
        state.volume = percent;
        state.muted = percent == 0;
        Some((state.volume, state.muted))
    }

    fn set_muted(&self, muted: bool) -> Option<(u8, bool)> {
        let mut state = self.0.lock().unwrap();
        state.muted = muted;
        Some((state.volume, state.muted))
    }

    fn set_brightness(&self, percent: u8) -> Option<u8> {
        let mut state = self.0.lock().unwrap();
        state.brightness = Some(percent);
        state.brightness
    }

    fn execute(&self, _action: ControlCenterAction) {}
}

#[test]
fn inside_click_and_held_drag_stay_open_while_escape_and_outside_click_dismiss() {
    let mut was_down = true;
    assert!(!poll_dismiss_action(None, true, &mut was_down, true, false));
    assert!(was_down);

    assert!(!poll_dismiss_action(None, false, &mut was_down, true, true));
    assert!(!was_down);

    assert!(!poll_dismiss_action(None, true, &mut was_down, true, true));
    assert!(was_down);

    assert!(!poll_dismiss_action(
        None,
        false,
        &mut was_down,
        true,
        false
    ));
    assert!(!poll_dismiss_action(
        None,
        true,
        &mut was_down,
        false,
        false
    ));
    assert!(!poll_dismiss_action(
        None,
        false,
        &mut was_down,
        true,
        false
    ));
    assert!(poll_dismiss_action(None, true, &mut was_down, true, false));

    assert!(poll_dismiss_action(
        Some(MenuNavEvent::Escape),
        false,
        &mut was_down,
        false,
        true,
    ));
}

#[test]
fn keyboard_navigation_steps_sliders_scrolls_list_and_clears_on_pointer_hover() {
    let state = Arc::new(Mutex::new(MockControlCenterState {
        brightness: Some(60),
        volume: 50,
        ..MockControlCenterState::default()
    }));
    let runtime = RuntimeContext::new(false);
    let mut surface = ControlCenterSurface::new(0);
    surface.port = Box::new(MockControlCenterPort(Arc::clone(&state)));
    surface.snapshot.page = ControlCenterPage::Main;
    surface.visible = true;
    surface.sync_snapshot(&runtime);
    surface.relayout();

    let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
    assert_eq!(
        surface
            .focus
            .enter_target(&targets, ControlCenterHitTarget::DisplaySlider),
        Some(ControlCenterHitTarget::DisplaySlider)
    );
    assert!(matches!(
        surface.on_key(&runtime, BarKey::Next),
        PostClickAction::None
    ));
    assert_eq!(surface.snapshot.status.brightness, Some(65));
    assert_eq!(
        surface.focus.current(),
        Some(ControlCenterHitTarget::DisplaySlider)
    );
    assert!(matches!(
        surface.on_key(&runtime, BarKey::Previous),
        PostClickAction::None
    ));
    assert_eq!(surface.snapshot.status.brightness, Some(60));

    let rect = surface.layout.wifi_row;
    surface.on_mouse_move(&runtime, rect.x + 8, rect.y + 8);
    assert!(!surface.focus.is_active());
}

#[test]
fn module_toggles_queue_expected_runtime_updates() {
    let runtime = RuntimeContext::new(false);
    runtime.glass_enabled.set(false);
    let mut surface = ControlCenterSurface::new(0);
    surface.port = Box::new(MockControlCenterPort(Arc::new(Mutex::new(
        MockControlCenterState::default(),
    ))));
    surface.snapshot.page = ControlCenterPage::Main;
    surface.visible = true;
    surface.sync_snapshot(&runtime);
    surface.relayout();
    *runtime.control_center.borrow_mut() = Some(surface);

    toggle_setting(&runtime, OttoSettingKind::Notch);
    toggle_setting(&runtime, OttoSettingKind::Dock);
    toggle_setting(&runtime, OttoSettingKind::Genie);
    toggle_setting(&runtime, OttoSettingKind::PrivacyIndicators);
    toggle_setting(&runtime, OttoSettingKind::ClipboardHistory);
    toggle_setting(&runtime, OttoSettingKind::DropShelf);
    toggle_setting(&runtime, OttoSettingKind::NotchHud);
    toggle_setting(&runtime, OttoSettingKind::MediaModule);
    toggle_setting(&runtime, OttoSettingKind::FocusModule);
    toggle_setting(&runtime, OttoSettingKind::QuickCapture);
    toggle_setting(&runtime, OttoSettingKind::DockHideTaskbar);

    assert_eq!(runtime.pending_notch_enabled.take(), Some(false));
    assert_eq!(
        runtime.take_plugin_toggles(),
        vec![
            (crate::host::PluginId::Dock, false),
            (crate::host::PluginId::Genie, false)
        ]
    );
    assert_eq!(runtime.pending_privacy_indicators.take(), Some(false));
    assert_eq!(runtime.pending_clipboard_history.take(), Some(false));
    assert_eq!(runtime.pending_drop_shelf.take(), Some(false));
    assert_eq!(runtime.pending_notch_hud.take(), Some(false));
    assert_eq!(runtime.pending_media_module.take(), Some(false));
    assert_eq!(runtime.pending_focus_module.take(), Some(false));
    assert_eq!(runtime.pending_quick_capture.take(), Some(true));
    let dock_prefs = runtime.pending_dock_preferences.take().unwrap();
    assert!(!dock_prefs.hide_taskbar);

    runtime.media_module_enabled.set(false);
    runtime.focus_module_enabled.set(false);
    runtime.focus_active.set(true);
    {
        let mut guard = runtime.control_center.borrow_mut();
        let cc = guard.as_mut().unwrap();
        cc.sync_snapshot(&runtime);
        assert!(cc.snapshot.media.is_none());
        assert!(!cc.snapshot.focus_active);
        assert!(!cc.snapshot.settings.system.media_module);
        assert!(!cc.snapshot.settings.system.focus_module);
    }
}

#[test]
fn press_capture_cancels_on_slide_out_and_triggers_on_release_inside() {
    let runtime = RuntimeContext::new(false);
    let mut surface = ControlCenterSurface::new(0);
    surface.port = Box::new(MockControlCenterPort(Arc::new(Mutex::new(
        MockControlCenterState::default(),
    ))));
    surface.snapshot.page = ControlCenterPage::Main;
    surface.visible = true;
    surface.sync_snapshot(&runtime);
    surface.relayout();

    let battery_rect = surface.layout.battery_tile;
    let gx = battery_rect.x + battery_rect.width / 2;
    let gy = battery_rect.y + battery_rect.height / 2;

    surface.begin_press(&runtime, gx, gy);
    assert_eq!(surface.pressed, Some(ControlCenterHitTarget::BatteryTile));
    assert!(matches!(
        surface.release_press(&runtime, 0, 0),
        PostClickAction::None
    ));
    assert_eq!(surface.pressed, None);

    surface.begin_press(&runtime, gx, gy);
    match surface.release_press(&runtime, gx, gy) {
        PostClickAction::Command(
            ControlCenterCommand::OpenSettings(ControlCenterAction::OpenPowerSettings),
            true,
        ) => {}
        _ => panic!("expected power settings when released inside"),
    }
}

#[test]
fn hide_clears_cached_snapshot_and_notch_off_suppresses_media() {
    let runtime = RuntimeContext::new(false);
    let mut surface = ControlCenterSurface::new(0);
    surface.port = Box::new(MockControlCenterPort(Arc::new(Mutex::new(
        MockControlCenterState::default(),
    ))));
    surface.visible = true;
    surface.snapshot.media = Some(NowPlaying {
        title: String::from("Track"),
        artist: String::from("Artist"),
        playing: true,
        ..NowPlaying::default()
    });
    surface.hide();
    assert!(surface.snapshot.media.is_none());
    assert!(surface.snapshot.audio_outputs.is_empty());

    runtime.notch_enabled.set(false);
    runtime.media_module_enabled.set(true);
    surface.sync_snapshot(&runtime);
    assert!(surface.snapshot.media.is_none());
}

#[test]
fn edit_mode_resize_move_and_background_unwind_work_as_expected() {
    let runtime = RuntimeContext::new(false);
    let mut surface = ControlCenterSurface::new(0);
    surface.port = Box::new(MockControlCenterPort(Arc::new(Mutex::new(
        MockControlCenterState::default(),
    ))));
    surface.snapshot.page = ControlCenterPage::Main;
    surface.visible = true;
    surface.sync_snapshot(&runtime);
    surface.relayout();

    let bg_action = surface.on_click(&runtime, 8, 8);
    assert!(matches!(
        bg_action,
        PostClickAction::Command(ControlCenterCommand::Dismiss, true)
    ));

    surface.snapshot.editing = true;
    surface.relayout();
    let edit_bg = surface.on_click(&runtime, 8, 8);
    assert!(matches!(
        edit_bg,
        PostClickAction::Command(ControlCenterCommand::ExitEditMode, false)
    ));

    let first_tile = surface.layout.tiles[0];
    let rx = first_tile.resize_handle.right() - 4;
    let ry = first_tile.resize_handle.bottom() - 4;
    surface.begin_press(&runtime, rx, ry);
    assert!(matches!(
        surface.drag,
        Some(super::DragMode::ResizeTile { .. })
    ));
    surface.on_mouse_move(&runtime, rx - 80, ry - 80);
    let _ = surface.release_press(&runtime, rx - 80, ry - 80);
    assert!(surface.drag.is_none());
}
