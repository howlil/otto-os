use crate::host::RuntimeContext;
use otto_core::{
    ControlCenterAction, ControlCenterCommand, ControlCenterPage, ThemeMode,
    control_center_focus_targets, subpanel_opener_target,
};
use otto_platform::{post_window_message, save_control_center_grid};

use super::with_cc;

pub(super) fn execute_command(runtime: &RuntimeContext, command: ControlCenterCommand) {
    match command {
        ControlCenterCommand::OpenPage(page) => navigate_open_page(runtime, page),
        ControlCenterCommand::Back => navigate_back(runtime),
        ControlCenterCommand::OpenSettingsWindow(page) => {
            crate::settings_window::open(runtime, Some(page));
        }
        ControlCenterCommand::ToggleWifi
        | ControlCenterCommand::ConnectWifi(_)
        | ControlCenterCommand::ToggleBluetooth
        | ControlCenterCommand::ToggleBluetoothDevice(_, _) => {
            execute_connectivity_command(runtime, command);
        }
        ControlCenterCommand::ToggleFocus => {
            if runtime.notch_enabled.get() && runtime.focus_module_enabled.get() {
                crate::bar::toggle_focus_mode(runtime);
            }
        }
        ControlCenterCommand::SetBrightness(pct) => {
            with_cc(runtime, |surface| {
                if let Some(next) = surface.port.set_brightness(pct) {
                    surface.snapshot.status.brightness = Some(next);
                }
            });
        }
        ControlCenterCommand::SetVolume(pct) => {
            with_cc(runtime, |surface| {
                if let Some((vol, muted)) = surface.port.set_volume(pct) {
                    surface.snapshot.status.volume = vol;
                    surface.snapshot.status.muted = muted;
                }
            });
        }
        ControlCenterCommand::ToggleMute => {
            with_cc(runtime, |surface| {
                let next = !surface.snapshot.status.muted;
                if let Some((vol, muted)) = surface.port.set_muted(next) {
                    surface.snapshot.status.volume = vol;
                    surface.snapshot.status.muted = muted;
                }
            });
        }
        ControlCenterCommand::SelectAudioOutput(id) => {
            with_cc(runtime, |surface| {
                surface.snapshot.audio_switch_failed = !surface.port.set_audio_output(&id);
            });
        }
        ControlCenterCommand::Media(cmd) => {
            if runtime.notch_enabled.get() && runtime.media_module_enabled.get() {
                with_cc(runtime, |surface| {
                    surface.port.execute(ControlCenterAction::Media(cmd));
                });
            }
        }
        ControlCenterCommand::OpenNotchMedia => {
            if runtime.notch_enabled.get() && runtime.media_module_enabled.get() {
                crate::bar::open_notch_media(runtime);
            }
        }
        ControlCenterCommand::OpenSettings(action) => {
            with_cc(runtime, |surface| surface.port.execute(action));
        }
        _ => execute_grid_command(runtime, &command),
    }
}

fn execute_grid_command(runtime: &RuntimeContext, command: &ControlCenterCommand) {
    match command {
        ControlCenterCommand::ToggleEditMode => {
            with_cc(runtime, |surface| {
                surface.snapshot.editing = !surface.snapshot.editing;
                surface.drag = None;
                surface.hovered = None;
                surface.pressed = None;
            });
        }
        ControlCenterCommand::ExitEditMode => {
            with_cc(runtime, |surface| {
                surface.snapshot.editing = false;
                surface.drag = None;
                surface.hovered = None;
                surface.pressed = None;
            });
        }
        ControlCenterCommand::OpenGallery => {
            navigate_open_page(runtime, ControlCenterPage::Gallery);
        }
        ControlCenterCommand::AddControl(kind) => {
            let kind = *kind;
            with_cc(runtime, |surface| {
                if surface.snapshot.grid.add_control(kind).is_some() {
                    let _ = save_control_center_grid(&surface.snapshot.grid);
                }
                surface.snapshot.page = ControlCenterPage::Main;
                surface.snapshot.editing = true;
                surface.scroll_offset = 0;
                surface.hovered = None;
                surface.pressed = None;
            });
        }
        ControlCenterCommand::RemoveTile(id) => {
            let id = *id;
            with_cc(runtime, |surface| {
                if surface.snapshot.grid.remove_tile(id) {
                    let _ = save_control_center_grid(&surface.snapshot.grid);
                }
                surface.hovered = None;
                surface.pressed = None;
            });
        }
        ControlCenterCommand::CycleTileSize(id) => {
            let id = *id;
            with_cc(runtime, |surface| {
                if surface.snapshot.grid.cycle_tile_size(id) {
                    let _ = save_control_center_grid(&surface.snapshot.grid);
                }
            });
        }
        ControlCenterCommand::ResetGrid => {
            with_cc(runtime, |surface| {
                surface.snapshot.grid.reset();
                let _ = save_control_center_grid(&surface.snapshot.grid);
                surface.hovered = None;
                surface.pressed = None;
            });
        }
        ControlCenterCommand::ToggleDarkMode => {
            let mut prefs = runtime.appearance_preferences.get();
            prefs.theme = if crate::theme_state::appearance(runtime).theme
                == otto_core::ResolvedTheme::Dark
            {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            runtime.appearance_preferences.set(prefs);
            runtime.pending_appearance.set(Some(prefs));
            crate::theme_state::refresh_theme(runtime);
        }
        ControlCenterCommand::OpenFinder => {
            let bar_hwnd = runtime.bar_hwnd.get();
            if bar_hwnd != 0 {
                let _ = post_window_message(bar_hwnd, crate::win_msg::WM_APP_OPEN_FINDER, 0, 0);
            }
        }
        ControlCenterCommand::OpenSpotlight => {
            crate::spotlight::toggle_search(runtime, None);
        }
        _ => {}
    }
}

pub(super) fn navigate_open_page(runtime: &RuntimeContext, page: ControlCenterPage) {
    with_cc(runtime, |surface| {
        if surface.snapshot.page != page {
            surface.start_morph();
        }
        surface.snapshot.page = page;
        surface.snapshot.audio_switch_failed = false;
        surface.scroll_offset = 0;
        surface.hovered = None;
        surface.pressed = None;
        surface.sync_snapshot(runtime);
        surface.relayout();
        if surface.focus.is_active() {
            let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
            let _ = surface.focus.enter(&targets);
            surface.ensure_focused_visible();
        }
        surface.place();
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub(super) fn navigate_back(runtime: &RuntimeContext) {
    with_cc(runtime, |surface| {
        let prev_page = surface.snapshot.page;
        if prev_page != ControlCenterPage::Main {
            surface.start_morph();
        }
        surface.snapshot.page = ControlCenterPage::Main;
        surface.scroll_offset = 0;
        surface.hovered = None;
        surface.pressed = None;
        surface.sync_snapshot(runtime);
        surface.relayout();
        if surface.focus.is_active() {
            let targets = control_center_focus_targets(&surface.layout, &surface.snapshot);
            if let Some(preferred) = subpanel_opener_target(prev_page) {
                let _ = surface.focus.enter_target(&targets, preferred);
            } else {
                let _ = surface.focus.enter(&targets);
            }
            surface.ensure_focused_visible();
        }
        surface.place();
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub(super) fn execute_connectivity_command(
    runtime: &RuntimeContext,
    command: ControlCenterCommand,
) {
    match command {
        ControlCenterCommand::ToggleWifi => {
            with_cc(runtime, |surface| {
                let next = !surface.snapshot.wifi.enabled;
                if surface.port.set_wifi_enabled(next) {
                    surface.snapshot.wifi.enabled = next;
                }
            });
        }
        ControlCenterCommand::ConnectWifi(ssid) => {
            with_cc(runtime, |surface| {
                if !surface.port.connect_wifi(&ssid) {
                    surface.port.execute(ControlCenterAction::OpenWifi);
                }
            });
        }
        ControlCenterCommand::ToggleBluetooth => {
            with_cc(runtime, |surface| {
                let next = !surface.snapshot.bluetooth.enabled;
                if surface.port.set_bluetooth_enabled(next) {
                    surface.snapshot.bluetooth.enabled = next;
                }
            });
        }
        ControlCenterCommand::ToggleBluetoothDevice(id, connect) => {
            with_cc(runtime, |surface| {
                let _ = surface.port.toggle_bluetooth_device(&id, connect);
            });
        }
        _ => {}
    }
}

pub(super) fn refresh_and_present(runtime: &RuntimeContext) {
    with_cc(runtime, |surface| {
        if surface.visible {
            surface.sync_snapshot(runtime);
            surface.relayout();
            surface.place();
            surface.sync_backdrop(runtime, true);
            surface.present(runtime);
        }
    });
}
