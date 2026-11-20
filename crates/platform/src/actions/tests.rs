use super::hotkeys::{
    capture_hotkey_bindings, notch_hotkey_bindings, settings_hotkey_bindings, shell_hotkey_bindings,
};
use super::menu_commands::{shortcut_for, window_command_for};
use super::{
    HOTKEY_CONTROL_CENTER, HOTKEY_FOCUS_BAR, HOTKEY_OTTO_SETTINGS, HOTKEY_QUICK_CAPTURE,
    HOTKEY_SPOTLIGHT, HOTKEY_TOGGLE_DOCK, HOTKEY_TOGGLE_NOTCH, HOTKEY_TOGGLE_POMODORO,
    HOTKEY_TOGGLE_TOP_BAR, execute_top_bar_command,
};
use otto_core::{
    APPLICATION_MENUS, AppMenuState, CMD_APP_HIDE, CMD_APP_QUIT, CMD_APP_SETTINGS,
    CMD_DOCK_WINDOW_BASE, CMD_EDIT_PASTE, CMD_FILE_NEW_FOLDER, CMD_FILE_NEW_TAB, CMD_SHOW_DESKTOP,
    CMD_TASK_VIEW, CMD_VIEW_ACTUAL_SIZE, CMD_VIEW_AS_ICONS, CMD_VIEW_FULLSCREEN,
    CMD_WIN_BRING_ALL_TO_FRONT, CMD_WIN_CLOSE, Capability, CommandOutcome, MenuContext, MenuRow,
    WindowCommand, WindowIdentity, WindowMenuEntry, WindowMenuState, build_top_bar_menu_rows,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_OEM_COMMA, VK_SPACE};

const AVAILABLE_WINDOW: WindowMenuState = WindowMenuState {
    close: Capability::Available,
    minimize: Capability::Available,
    zoom: Capability::Available,
    full_screen: Capability::Available,
    tile: Capability::Available,
    shortcuts: Capability::Available,
    zoomed: false,
    full_screen_active: false,
};
const AVAILABLE_APP: AppMenuState = AppMenuState {
    hide: Capability::Available,
    hide_others: Capability::Available,
    show_all: Capability::Available,
    quit: Capability::Available,
};

#[test]
fn every_enabled_application_menu_row_has_an_executor() {
    let entries = [WindowMenuEntry {
        title: String::from("Main"),
        active: true,
    }];
    for file_manager in [false, true] {
        let context = MenuContext {
            app_name: "App",
            file_manager,
            tabbed: true,
            window: Some(AVAILABLE_WINDOW),
            app: AVAILABLE_APP,
            app_windows: &entries,
        };
        for target in APPLICATION_MENUS {
            for row in build_top_bar_menu_rows(target, &context) {
                let MenuRow::Item {
                    id, enabled: true, ..
                } = row
                else {
                    continue;
                };
                let routed = window_command_for(id).is_some()
                    || shortcut_for(id).is_some()
                    || matches!(
                        id,
                        CMD_TASK_VIEW | CMD_SHOW_DESKTOP | CMD_WIN_BRING_ALL_TO_FRONT
                    )
                    || (CMD_DOCK_WINDOW_BASE..CMD_DOCK_WINDOW_BASE + 64).contains(&id);
                assert!(routed, "{target:?} row {id} has no executor");
            }
        }
    }
    assert!(shortcut_for(CMD_APP_SETTINGS).is_some());
}

#[test]
fn distinct_labels_route_to_distinct_actions() {
    assert_ne!(
        shortcut_for(CMD_FILE_NEW_FOLDER),
        shortcut_for(CMD_FILE_NEW_TAB)
    );
    assert_ne!(
        shortcut_for(CMD_VIEW_AS_ICONS),
        shortcut_for(CMD_VIEW_ACTUAL_SIZE)
    );
    assert_eq!(
        window_command_for(CMD_VIEW_FULLSCREEN),
        Some(WindowCommand::ToggleFullScreen)
    );
    assert_eq!(
        window_command_for(CMD_WIN_CLOSE),
        Some(WindowCommand::Close)
    );
    assert!(shortcut_for(CMD_WIN_CLOSE).is_none());
    assert!(window_command_for(CMD_APP_QUIT).is_none());
}

#[test]
fn commands_for_a_window_that_no_longer_exists_reach_no_other_application() {
    let gone = WindowIdentity {
        hwnd: 0x7FFF_FFF0,
        process_id: 0xFFFF_FFF0,
        thread_id: 0xFFFF_FFF1,
        generation: 1,
    };
    let own = std::process::id();
    for cmd in [
        CMD_EDIT_PASTE,
        CMD_APP_SETTINGS,
        CMD_WIN_CLOSE,
        CMD_WIN_BRING_ALL_TO_FRONT,
        CMD_DOCK_WINDOW_BASE,
        CMD_APP_QUIT,
        CMD_APP_HIDE,
    ] {
        let result = execute_top_bar_command(cmd, Some(gone), own);
        assert_eq!(result.outcome, CommandOutcome::TargetGone, "command {cmd}");
        assert!(!result.quit_requested);
    }
    let untargeted = execute_top_bar_command(CMD_EDIT_PASTE, None, own);
    assert_eq!(untargeted.outcome, CommandOutcome::Unsupported);
}

#[test]
fn notch_hotkeys_have_unique_ids_and_include_control_center() {
    let bindings = notch_hotkey_bindings();
    let ids: Vec<i32> = bindings.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(
        ids,
        vec![
            HOTKEY_TOGGLE_NOTCH,
            HOTKEY_TOGGLE_POMODORO,
            HOTKEY_FOCUS_BAR,
            HOTKEY_CONTROL_CENTER,
            HOTKEY_SPOTLIGHT,
        ]
    );
    assert_eq!(bindings[3].2, u32::from(b'C'));
    assert_eq!(bindings[4].2, u32::from(VK_SPACE.0));
}

#[test]
fn shell_and_settings_hotkeys_do_not_collide_with_other_hotkeys() {
    let shell = shell_hotkey_bindings();
    assert_eq!(shell[0].0, HOTKEY_TOGGLE_TOP_BAR);
    assert_eq!(shell[0].2, u32::from(b'B'));
    assert_eq!(shell[1].0, HOTKEY_TOGGLE_DOCK);
    assert_eq!(shell[1].2, u32::from(b'D'));
    let settings = settings_hotkey_bindings();
    assert_eq!(settings[0].0, HOTKEY_OTTO_SETTINGS);
    assert_eq!(settings[0].2, u32::from(VK_OEM_COMMA.0));
    let capture = capture_hotkey_bindings();
    assert_eq!(capture[0].0, HOTKEY_QUICK_CAPTURE);
    let all: Vec<_> = notch_hotkey_bindings()
        .into_iter()
        .chain(shell)
        .chain(settings)
        .chain(capture)
        .collect();
    for (index, (id, modifiers, vk)) in all.iter().enumerate() {
        for (other_id, other_modifiers, other_vk) in &all[index + 1..] {
            assert_ne!(id, other_id);
            assert!(modifiers != other_modifiers || vk != other_vk);
        }
    }
}

#[test]
fn loaded_font_registry_deduplicates_paths_and_remove_drains_cleanly() {
    let mut tracked = Vec::new();
    let font = std::path::PathBuf::from("C:\\Fonts\\SF-Pro-Text-Regular.otf");
    assert!(super::fonts::record_loaded_font(&mut tracked, font.clone()));
    assert!(!super::fonts::record_loaded_font(&mut tracked, font));
    assert_eq!(tracked.len(), 1);
    super::remove_apple_fonts();
}

#[test]
fn tray_overflow_rejects_invalid_native_bounds() {
    assert_eq!(
        super::tray::tray_overflow_size(&windows::Win32::Foundation::RECT {
            left: 10,
            top: 20,
            right: 210,
            bottom: 320,
        }),
        Some((200, 300))
    );
    assert_eq!(
        super::tray::tray_overflow_size(&windows::Win32::Foundation::RECT {
            left: 10,
            top: 20,
            right: 10,
            bottom: 320,
        }),
        None
    );
}
