use super::messages::{dispatch_runtime_message, handle_host_hotkey};
use super::plugins::ConfigSource;
use super::{
    Error, HCURSOR, HINSTANCE, HOST, HWND, Host, MODAL_ACTION_ACTIVE, ModalActionGuard,
    PLUGIN_DESCRIPTORS, PluginConfig, PluginId, PluginState, Result, RuntimeContext, SurfaceSlot,
    Surfaces, WINDOW_EX_STYLE, bar, dependencies_running, descriptor, reverse_lifecycle_order,
    set_dock_preferences, set_online_lyrics_enabled, set_plugin_enabled, should_post_quit,
    update_preferences,
};
use windows::Win32::Foundation::LRESULT;

struct TestHostGuard;

impl Drop for TestHostGuard {
    fn drop(&mut self) {
        HOST.with(|cell| {
            if let Ok(mut guard) = cell.try_borrow_mut() {
                let _ = guard.take();
            }
        });
    }
}

fn host_with_required_plugins() -> Host {
    let mut config = PluginConfig::default();
    config.set_enabled(PluginId::ControlCenter, false);
    Host {
        instance: HINSTANCE::default(),
        cursor: HCURSOR::default(),
        ex_style: WINDOW_EX_STYLE::default(),
        config,
        recovery_hotkeys: true,
        tray_icon: None,
        taskbar_created_message: None,
        surfaces: Some(Surfaces {
            bar: hwnd(1),
            panel: hwnd(2),
            menu: hwnd(3),
            dock: None,
            window_controls: None,
            genie: None,
            control_center: None,
            spotlight: None,
        }),
        started: vec![PluginId::PopupMenu, PluginId::TopBar, PluginId::Spotlight],
        states: PLUGIN_DESCRIPTORS
            .iter()
            .map(|descriptor| (descriptor.id, PluginState::Disabled))
            .collect(),
        shutting_down: false,
        runtime: RuntimeContext::new(false),
    }
}

fn failing_save(_config: &PluginConfig) -> Result<()> {
    Err(Error::new(
        windows::Win32::Foundation::E_FAIL,
        "simulated save failure",
    ))
}

#[allow(clippy::unnecessary_wraps)]
fn working_save(_config: &PluginConfig) -> Result<()> {
    Ok(())
}

fn is_running(host: &Host, plugin: PluginId) -> bool {
    host.started.contains(&plugin)
}

#[test]
fn failed_save_while_enabling_leaves_the_plugin_stopped_and_disabled() {
    let mut host = host_with_required_plugins();
    let result = host.set_plugin_enabled(PluginId::ControlCenter, true, failing_save);
    assert!(result.is_err());
    assert!(!is_running(&host, PluginId::ControlCenter));
    assert!(!host.config.is_enabled(PluginId::ControlCenter));
}

#[test]
fn failed_save_while_disabling_leaves_the_plugin_running_and_enabled() {
    let mut host = host_with_required_plugins();
    host.set_plugin_enabled(PluginId::ControlCenter, true, working_save)
        .unwrap();
    assert!(is_running(&host, PluginId::ControlCenter));
    let result = host.set_plugin_enabled(PluginId::ControlCenter, false, failing_save);
    assert!(result.is_err());
    assert!(is_running(&host, PluginId::ControlCenter));
    assert!(host.config.is_enabled(PluginId::ControlCenter));
}

#[test]
fn successful_toggle_keeps_runtime_and_configuration_in_step() {
    let mut host = host_with_required_plugins();
    host.set_plugin_enabled(PluginId::ControlCenter, true, working_save)
        .unwrap();
    host.set_plugin_enabled(PluginId::ControlCenter, false, working_save)
        .unwrap();
    assert!(!is_running(&host, PluginId::ControlCenter));
    assert!(!host.config.is_enabled(PluginId::ControlCenter));
    host.stop_plugin(PluginId::ControlCenter);
    host.stop_plugin(PluginId::ControlCenter);
    assert_eq!(
        host.started,
        vec![PluginId::PopupMenu, PluginId::TopBar, PluginId::Spotlight]
    );
}

#[test]
fn enabling_a_plugin_whose_dependency_is_not_running_is_rejected() {
    let mut host = host_with_required_plugins();
    host.started.clear();
    let result = host.set_plugin_enabled(PluginId::ControlCenter, true, working_save);
    assert!(result.is_err());
    assert!(!host.config.is_enabled(PluginId::ControlCenter));
}

#[test]
fn newer_configuration_version_is_preserved_and_never_overwritten() {
    let config = PluginConfig::parse("version=2\nenabled=popup_menu,top_bar\nfuture=1\n");
    assert_eq!(config.source, ConfigSource::Unsupported);
    assert!(config.is_enabled(PluginId::TopBar));
    assert!(config.is_enabled(PluginId::Dock));
    assert!(config.save().is_err());
    assert_eq!(
        PluginConfig::parse("version=banana\n").source,
        ConfigSource::Unsupported
    );
}

#[test]
fn current_and_legacy_configuration_versions_are_accepted() {
    assert_eq!(
        PluginConfig::parse("version=1\nenabled=dock\n").source,
        ConfigSource::Supported
    );
    assert_eq!(
        PluginConfig::parse("enabled=dock\n").source,
        ConfigSource::Supported
    );
    assert_eq!(PluginConfig::default().source, ConfigSource::Defaults);
}

fn hwnd(value: usize) -> HWND {
    HWND(value as *mut _)
}

#[test]
fn config_keeps_required_plugins_and_ignores_unknown_plugins() {
    let config = PluginConfig::parse("version=1\nenabled=dock,unknown\n");
    assert!(config.is_enabled(PluginId::PopupMenu));
    assert!(config.is_enabled(PluginId::Spotlight));
    assert!(config.is_enabled(PluginId::Dock));
    assert!(!config.is_enabled(PluginId::WindowControls));
}

#[test]
fn config_can_leave_the_top_bar_disabled() {
    let config = PluginConfig::parse("version=1\nenabled=popup_menu,spotlight,dock\n");
    assert!(!config.is_enabled(PluginId::TopBar));
    assert!(config.is_enabled(PluginId::Dock));
    let mut toggled = PluginConfig::default();
    toggled.set_enabled(PluginId::TopBar, false);
    assert!(!toggled.is_enabled(PluginId::TopBar));
    assert!(toggled.is_enabled(PluginId::PopupMenu));
    assert!(toggled.is_enabled(PluginId::Spotlight));
}

#[test]
fn dock_preferences_load_from_config_and_default_when_absent() {
    let legacy = PluginConfig::parse(
        "version=1
enabled=dock
online_lyrics=false
",
    );
    assert_eq!(legacy.dock, otto_core::DockPreferences::default());
    let saved = PluginConfig::parse(
        "version=1
enabled=dock
dock_autohide=true
dock_magnification=false
dock_icon_size=64
",
    );
    assert!(saved.dock.autohide);
    assert!(!saved.dock.magnification);
    assert_eq!(saved.dock.icon_size, 64);
}

#[test]
fn config_defaults_all_plugins_when_enabled_field_is_missing() {
    let config = PluginConfig::parse("version=1\n");
    assert!(config.is_enabled(PluginId::TopBar));
    assert!(config.is_enabled(PluginId::ControlCenter));
    assert!(!config.online_lyrics);
}

#[test]
fn config_parses_online_lyrics_as_an_explicit_opt_in() {
    let config = PluginConfig::parse("version=1\nenabled=popup_menu,top_bar\nonline_lyrics=true\n");
    assert!(config.online_lyrics);
    let disabled = PluginConfig::parse("version=1\nenabled=popup_menu,top_bar\n");
    assert!(!disabled.online_lyrics);
}

#[test]
fn descriptors_are_declared_in_dependency_order() {
    let mut started = Vec::new();
    for descriptor in PLUGIN_DESCRIPTORS {
        assert!(
            dependencies_running(&started, descriptor.dependencies),
            "{} dependencies must start first",
            descriptor.id.name()
        );
        started.push(descriptor.id);
    }
}

#[test]
fn descriptors_register_surfaces_and_virtual_features() {
    assert!(descriptor(PluginId::Dock).surface.is_some());
    assert!(descriptor(PluginId::WindowControls).surface.is_some());
    assert!(descriptor(PluginId::Genie).surface.is_some());
    assert!(descriptor(PluginId::ControlCenter).surface.is_some());
    assert!(descriptor(PluginId::Dock).start.is_some());
    assert!(descriptor(PluginId::ControlCenter).start.is_some());
}

#[test]
fn surface_registry_stores_and_takes_dynamic_slots() {
    let mut surfaces = Surfaces {
        bar: hwnd(1),
        panel: hwnd(2),
        menu: hwnd(3),
        dock: None,
        window_controls: None,
        genie: None,
        control_center: None,
        spotlight: None,
    };
    let dock = hwnd(4);
    let control_center = hwnd(5);
    let spotlight = hwnd(6);
    surfaces.store(SurfaceSlot::Dock, dock).unwrap();
    surfaces
        .store(SurfaceSlot::ControlCenter, control_center)
        .unwrap();
    surfaces.store(SurfaceSlot::Spotlight, spotlight).unwrap();
    assert_eq!(surfaces.get(SurfaceSlot::Dock), Some(dock));
    assert_eq!(
        surfaces.get(SurfaceSlot::ControlCenter),
        Some(control_center)
    );
    assert_eq!(surfaces.get(SurfaceSlot::Spotlight), Some(spotlight));
    assert_eq!(surfaces.take(SurfaceSlot::Dock), Some(dock));
    assert_eq!(
        surfaces.take(SurfaceSlot::ControlCenter),
        Some(control_center)
    );
    assert_eq!(surfaces.take(SurfaceSlot::Spotlight), Some(spotlight));
    assert_eq!(surfaces.get(SurfaceSlot::Dock), None);
    assert_eq!(surfaces.get(SurfaceSlot::ControlCenter), None);
    assert_eq!(surfaces.get(SurfaceSlot::Spotlight), None);
    assert!(surfaces.store(SurfaceSlot::Menu, hwnd(7)).is_err());
}

#[test]
fn dependents_are_listed_before_the_plugin_they_rely_on() {
    let mut host = host_with_required_plugins();
    host.started = vec![
        PluginId::PopupMenu,
        PluginId::TopBar,
        PluginId::Spotlight,
        PluginId::Dock,
        PluginId::ControlCenter,
    ];
    assert_eq!(
        host.dependents_first(PluginId::TopBar),
        vec![PluginId::TopBar]
    );
    assert_eq!(
        host.dependents_first(PluginId::PopupMenu),
        vec![
            PluginId::ControlCenter,
            PluginId::Dock,
            PluginId::Spotlight,
            PluginId::TopBar,
            PluginId::PopupMenu,
        ]
    );
    assert_eq!(
        host.dependents_first(PluginId::Genie),
        vec![PluginId::Genie]
    );
}

#[test]
fn turning_the_top_bar_off_keeps_control_center_and_spotlight_running() {
    let mut host = host_with_required_plugins();
    host.config.set_enabled(PluginId::ControlCenter, true);
    host.started = vec![
        PluginId::PopupMenu,
        PluginId::TopBar,
        PluginId::Spotlight,
        PluginId::ControlCenter,
    ];
    let saved = std::cell::RefCell::new(Vec::new());
    host.set_plugin_enabled(PluginId::TopBar, false, |config| {
        *saved.borrow_mut() = config.enabled.clone();
        Ok(())
    })
    .unwrap();
    assert_eq!(
        host.started,
        vec![
            PluginId::PopupMenu,
            PluginId::Spotlight,
            PluginId::ControlCenter,
        ]
    );
    assert!(!host.runtime.top_bar_enabled.get());
    assert!(host.runtime.notch_enabled.get());
    assert!(host.config.is_enabled(PluginId::ControlCenter));
    assert!(host.config.is_enabled(PluginId::Spotlight));
    let saved = saved.into_inner();
    assert!(!saved.contains(&PluginId::TopBar));
    assert!(saved.contains(&PluginId::ControlCenter));
}

#[test]
fn failed_save_keeps_the_top_bar_and_its_dependents_running() {
    let mut host = host_with_required_plugins();
    let result = host.set_plugin_enabled(PluginId::TopBar, false, failing_save);
    assert!(result.is_err());
    assert!(is_running(&host, PluginId::TopBar));
    assert!(is_running(&host, PluginId::Spotlight));
    assert!(host.config.is_enabled(PluginId::TopBar));
}

#[test]
fn top_bar_stays_on_when_the_recovery_hotkeys_are_unavailable() {
    let mut host = host_with_required_plugins();
    host.recovery_hotkeys = false;
    let result = host.set_plugin_enabled(PluginId::TopBar, false, working_save);
    assert!(result.is_err());
    assert!(is_running(&host, PluginId::TopBar));
    assert!(host.config.is_enabled(PluginId::TopBar));
    host.set_plugin_enabled(PluginId::Dock, false, working_save)
        .unwrap();
}

#[test]
fn only_the_top_bar_can_be_turned_off_among_required_plugins() {
    let mut host = host_with_required_plugins();
    assert!(super::user_can_toggle(PluginId::TopBar));
    assert!(super::user_can_toggle(PluginId::Dock));
    assert!(!super::user_can_toggle(PluginId::PopupMenu));
    assert!(!super::user_can_toggle(PluginId::Spotlight));
    let result = host.set_plugin_enabled(PluginId::Spotlight, false, working_save);
    assert!(result.is_err());
    assert!(is_running(&host, PluginId::Spotlight));
}

#[test]
fn enabling_popup_menu_restarts_the_dependents_that_stay_enabled() {
    let mut host = host_with_required_plugins();
    host.config.set_enabled(PluginId::Dock, false);
    host.config.set_enabled(PluginId::WindowControls, false);
    host.config.set_enabled(PluginId::Genie, false);
    host.config.set_enabled(PluginId::ControlCenter, true);
    host.started = vec![PluginId::PopupMenu];
    host.start_dependents(PluginId::PopupMenu);
    assert!(is_running(&host, PluginId::TopBar));
    assert!(is_running(&host, PluginId::Spotlight));
    assert!(is_running(&host, PluginId::ControlCenter));
    assert!(!is_running(&host, PluginId::Dock));
}

#[test]
fn shell_and_host_hotkeys_queue_toggles_and_route_independent_surfaces() {
    let runtime = RuntimeContext::new(false);
    assert!(handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_TOGGLE_TOP_BAR
    ));
    assert!(handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_TOGGLE_DOCK
    ));
    assert_eq!(
        runtime.take_plugin_toggles(),
        vec![(PluginId::Dock, false), (PluginId::TopBar, false)]
    );
    runtime.top_bar_enabled.set(false);
    assert!(handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_TOGGLE_TOP_BAR
    ));
    assert_eq!(
        runtime.take_plugin_toggles(),
        vec![(PluginId::TopBar, true)]
    );
    assert!(handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_OTTO_SETTINGS
    ));
    assert!(handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_CONTROL_CENTER
    ));
    assert!(handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_SPOTLIGHT
    ));
    assert!(!handle_host_hotkey(
        &runtime,
        otto_platform::HOTKEY_TOGGLE_NOTCH
    ));
    assert!(runtime.take_plugin_toggles().is_empty());
}

#[test]
fn startup_starts_control_center_and_spotlight_even_when_top_bar_is_disabled() {
    let mut config = PluginConfig::default();
    config.set_enabled(PluginId::TopBar, false);
    let startable = PLUGIN_DESCRIPTORS
        .iter()
        .filter(|descriptor| {
            config.is_enabled(descriptor.id)
                && super::dependencies_enabled(&config, descriptor.dependencies)
        })
        .map(|descriptor| descriptor.id)
        .collect::<Vec<_>>();
    assert!(!startable.contains(&PluginId::TopBar));
    assert!(startable.contains(&PluginId::Spotlight));
    assert!(startable.contains(&PluginId::ControlCenter));
    assert!(startable.contains(&PluginId::Dock));
    assert!(startable.contains(&PluginId::PopupMenu));
}

#[test]
fn shutdown_order_reverses_start_order() {
    let started = vec![
        PluginId::PopupMenu,
        PluginId::TopBar,
        PluginId::Spotlight,
        PluginId::Genie,
        PluginId::Dock,
        PluginId::WindowControls,
        PluginId::ControlCenter,
    ];
    let stopped = reverse_lifecycle_order(&started).collect::<Vec<_>>();
    assert_eq!(
        stopped,
        vec![
            PluginId::ControlCenter,
            PluginId::WindowControls,
            PluginId::Dock,
            PluginId::Genie,
            PluginId::Spotlight,
            PluginId::TopBar,
            PluginId::PopupMenu,
        ]
    );
}

#[test]
fn preferences_round_trip_through_plugin_config() {
    let mut config = PluginConfig::default();
    config.preferences.capture_visible = true;
    config.preferences.hotkeys_enabled = false;
    config.preferences.classic_logo = true;
    config.preferences.notch_enabled = false;
    config.preferences.onboarding_seen = true;
    let serialized = format!(
        "version=1\nenabled=popup_menu,top_bar\nonline_lyrics=false\n{}\n{}\n",
        config.dock.config_lines().trim_end_matches('\n'),
        config.preferences.config_lines().trim_end_matches('\n'),
    );
    let parsed = PluginConfig::parse(&serialized);
    assert_eq!(parsed.source, ConfigSource::Supported);
    assert_eq!(parsed.preferences, config.preferences);
}

#[test]
fn reentrant_host_borrow_mut_returns_error_instead_of_panicking() {
    let _cleanup = TestHostGuard;
    HOST.with(|cell| *cell.borrow_mut() = Some(host_with_required_plugins()));
    HOST.with(|cell| {
        let _borrow = cell.borrow_mut();
        assert!(set_plugin_enabled(PluginId::ControlCenter, true).is_err());
        assert!(set_online_lyrics_enabled(true).is_err());
        assert!(set_dock_preferences(otto_core::DockPreferences::default()).is_err());
        assert!(update_preferences(|_| {}).is_err());
    });
}

#[test]
fn host_shutdown_marks_shutting_down_while_host_remains_registered() {
    struct BackgroundResumeGuard;
    impl Drop for BackgroundResumeGuard {
        fn drop(&mut self) {
            otto_platform::resume_background_tasks();
        }
    }

    let _cleanup = TestHostGuard;
    let _resume = BackgroundResumeGuard;
    let mut host = host_with_required_plugins();
    host.set_plugin_enabled(PluginId::ControlCenter, true, working_save)
        .unwrap();
    assert!(
        host.surfaces
            .as_ref()
            .and_then(|surfaces| surfaces.get(SurfaceSlot::ControlCenter))
            .is_some()
    );
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    assert!(should_post_quit(1));
    assert!(!should_post_quit(2));
    let hwnds = HOST.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(Host::shutdown)
            .unwrap_or_default()
    });
    assert!(hwnds.is_empty());
    assert!(!should_post_quit(1));
    HOST.with(|cell| {
        let borrow = cell.borrow();
        let host = borrow
            .as_ref()
            .expect("host remains present during shutdown");
        assert!(host.shutting_down);
        assert!(host.started.is_empty());
    });
}

#[test]
fn nested_modal_flag_blocks_preparing_second_modal_action() {
    let _cleanup = TestHostGuard;
    let host = host_with_required_plugins();
    *host.runtime.pending_modal_action.borrow_mut() =
        Some(bar::DeferredModalAction::PickShelfFiles);
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    MODAL_ACTION_ACTIVE.with(|flag| flag.set(true));
    let _modal_guard = ModalActionGuard;
    let result = dispatch_runtime_message(|_| Some(LRESULT(7)));
    assert_eq!(result, Some(LRESULT(7)));
    HOST.with(|cell| {
        let borrow = cell.borrow();
        let host = borrow.as_ref().expect("host remains registered");
        assert!(host.runtime.pending_modal_action.borrow().is_none());
    });
}

#[test]
fn pending_destroy_hwnds_are_queued_and_drained_once() {
    let runtime = RuntimeContext::new(false);
    runtime.queue_destroy_hwnd(0);
    runtime.queue_destroy_hwnd(42);
    runtime.queue_destroy_hwnd(99);
    assert_eq!(
        runtime.take_pending_destroy_hwnds(),
        vec![hwnd(42), hwnd(99)]
    );
    assert!(runtime.take_pending_destroy_hwnds().is_empty());
}

#[test]
fn end_session_dispatch_stops_host_and_is_idempotent() {
    let _cleanup = TestHostGuard;
    let host = host_with_required_plugins();
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    let result = dispatch_runtime_message(|runtime| {
        runtime.pending_end_session.set(true);
        Some(LRESULT(0))
    });
    assert_eq!(result, Some(LRESULT(0)));
    assert!(HOST.with(|cell| cell.borrow().is_none()));
    super::shutdown_session();
    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
    let posted_quit = unsafe {
        windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
            &mut msg,
            None,
            windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
            windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
            windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
        )
    };
    assert!(posted_quit.as_bool());
}

#[test]
fn queued_plugin_toggles_preserve_multiple_plugins_without_clobbering() {
    let runtime = RuntimeContext::new(false);
    runtime.queue_plugin_toggle(PluginId::Dock, false);
    runtime.queue_plugin_toggle(PluginId::Genie, false);
    runtime.queue_plugin_toggle(PluginId::WindowControls, true);
    assert_eq!(
        runtime.take_plugin_toggles(),
        vec![
            (PluginId::Dock, false),
            (PluginId::WindowControls, true),
            (PluginId::Genie, false),
        ]
    );
    assert!(runtime.take_plugin_toggles().is_empty());
}

#[test]
fn safe_window_proc_tracks_catch_depth_across_normal_and_panicking_handlers() {
    assert!(!super::in_safe_window_proc());
    let ok = super::messages::safe_window_proc(
        HWND::default(),
        0,
        windows::Win32::Foundation::WPARAM(0),
        windows::Win32::Foundation::LPARAM(0),
        || {
            assert!(super::in_safe_window_proc());
            Some(LRESULT(99))
        },
    );
    assert_eq!(ok, LRESULT(99));
    assert!(!super::in_safe_window_proc());

    let _ = super::messages::safe_window_proc(
        HWND::default(),
        0,
        windows::Win32::Foundation::WPARAM(0),
        windows::Win32::Foundation::LPARAM(0),
        || {
            assert!(super::in_safe_window_proc());
            panic!("simulated wnd_proc panic");
        },
    );
    assert!(!super::in_safe_window_proc());
}
