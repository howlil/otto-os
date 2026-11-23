use super::{
    Dock, LPARAM, LRESULT, RuntimeContext, WM_CLOSE, WM_MOUSEMOVE, WPARAM, handle_message,
    visibility::should_assert_topmost, wake_timer_only,
};

#[test]
fn wm_close_is_absorbed_without_destroying_surface() {
    let runtime = RuntimeContext::new(false);
    assert_eq!(
        handle_message(&runtime, 0, WM_CLOSE, WPARAM(0), LPARAM(0)),
        Some(LRESULT(0))
    );
}

#[test]
fn appbar_callback_is_handled_without_panicking_when_unmounted() {
    let runtime = RuntimeContext::new(false);
    assert_eq!(
        handle_message(
            &runtime,
            0,
            otto_platform::DOCK_APPBAR_CALLBACK_MESSAGE,
            WPARAM(1),
            LPARAM(0),
        ),
        Some(LRESULT(0))
    );
    assert_eq!(
        handle_message(
            &runtime,
            0,
            otto_platform::DOCK_APPBAR_CALLBACK_MESSAGE,
            WPARAM(2),
            LPARAM(0),
        ),
        Some(LRESULT(0))
    );
}

#[test]
fn dock_topmost_assertion_skips_hidden_dock_and_higher_otto_overlays() {
    assert!(should_assert_topmost(false, false));
    assert!(!should_assert_topmost(true, false));
    assert!(!should_assert_topmost(false, true));
    assert!(!should_assert_topmost(true, true));
    let runtime = RuntimeContext::new(false);
    assert!(!Dock::higher_overlay_open(&runtime));
}

#[test]
fn mouse_move_and_wake_timer_only_handle_unmounted_runtime() {
    let runtime = RuntimeContext::new(false);
    wake_timer_only(&runtime);
    assert_eq!(
        handle_message(&runtime, 0, WM_MOUSEMOVE, WPARAM(0), LPARAM(0)),
        Some(LRESULT(0))
    );
}

mod port_effects {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use otto_core::{
        AppIdentity, AppInfo, CommandOutcome, DockEffect, DockEntry, DockEntryOptions,
        DockItemKind, DockPort, IconBitmap, Rect, StableOrder, WindowCommand, WindowIdentity,
    };

    use super::super::actions::apply_port_effect;

    #[derive(Default)]
    struct FakePort {
        calls: RefCell<Vec<&'static str>>,
    }

    impl FakePort {
        fn note(&self, call: &'static str) {
            self.calls.borrow_mut().push(call);
        }
    }

    impl DockPort for FakePort {
        fn collect_entries(
            &self,
            _own_pid: u32,
            _order: &mut StableOrder,
            _options: DockEntryOptions,
        ) -> Vec<DockEntry> {
            Vec::new()
        }
        fn overflow_entry(&self, _hidden: usize) -> DockEntry {
            entry()
        }
        fn icon_for_entry(&self, _entry: &DockEntry) -> Option<IconBitmap> {
            None
        }
        fn icon_for_window(&self, _hwnd: isize) -> Option<IconBitmap> {
            None
        }
        fn downloads_progress(&self) -> Option<f32> {
            None
        }
        fn notification_counts(&self) -> HashMap<String, u32> {
            HashMap::new()
        }
        fn is_window_minimized(&self, _hwnd: isize) -> bool {
            false
        }
        fn window_overlaps_zone(&self, _zone: Rect, _own_pid: u32) -> bool {
            false
        }
        fn fullscreen_on_primary(&self, _width: i32, _height: i32, _own_pid: u32) -> bool {
            false
        }
        fn toggle_pin(&self, _entry: &DockEntry) {
            self.note("toggle_pin");
        }
        fn remove_pin(&self, _key: &str) -> bool {
            false
        }
        fn reorder_pins(&self, _moved_key: &str, _target_key: &str) -> bool {
            false
        }
        fn pin_at_target(&self, _entry: &DockEntry, _target_key: &str) -> bool {
            false
        }
        fn launch_new_instance(&self, _entry: &DockEntry) {
            self.note("launch");
        }
        fn open_file_with(&self, _entry: &DockEntry, _path: &str) {
            self.note("open_file_with");
        }
        fn reveal_in_explorer(&self, _entry: &DockEntry) {
            self.note("reveal");
        }
        fn force_quit(&self, _entry: &DockEntry, _own_pid: u32) -> CommandOutcome {
            self.note("force_quit");
            CommandOutcome::Completed
        }
        fn window_command(
            &self,
            _target: WindowIdentity,
            _command: WindowCommand,
            _own_pid: u32,
        ) -> CommandOutcome {
            self.note("window_command");
            CommandOutcome::Completed
        }
        fn hide_application(&self, _app: &AppInfo, _own_pid: u32) -> CommandOutcome {
            self.note("hide_application");
            CommandOutcome::Completed
        }
        fn show_application(&self, _app: &AppInfo, _own_pid: u32) -> CommandOutcome {
            self.note("show_application");
            CommandOutcome::Completed
        }
        fn quit_application(&self, _app: &AppInfo, _own_pid: u32) -> CommandOutcome {
            self.note("quit_application");
            CommandOutcome::Completed
        }
        fn hide_other_applications(&self, _keep: &AppIdentity, _own_pid: u32) -> CommandOutcome {
            self.note("hide_other_applications");
            CommandOutcome::Completed
        }
        fn empty_recycle_bin(&self) {
            self.note("empty_trash");
        }
    }

    fn entry() -> DockEntry {
        DockEntry {
            key: "app".to_owned(),
            title: "App".to_owned(),
            kind: DockItemKind::Trash { is_full: false },
            apple_icon: None,
            app: None,
            hwnds: Vec::new(),
            window_titles: Vec::new(),
            running: false,
            active: false,
            pinned: false,
            hidden: false,
            minimized: false,
        }
    }

    #[test]
    fn launch_and_trash_effects_reach_the_port_without_refreshing_the_dock() {
        let port = FakePort::default();
        let launched = apply_port_effect(&port, &DockEffect::Launch, &entry(), 1);
        let emptied = apply_port_effect(&port, &DockEffect::EmptyTrash, &entry(), 1);
        assert_eq!(*port.calls.borrow(), ["launch", "empty_trash"]);
        assert!(!launched.refresh_entries && launched.outcome.is_none());
        assert!(!emptied.refresh_entries && emptied.outcome.is_none());
    }

    #[test]
    fn pinning_and_force_quit_ask_the_dock_to_refresh() {
        let port = FakePort::default();
        let pinned = apply_port_effect(&port, &DockEffect::TogglePin, &entry(), 1);
        let quit = apply_port_effect(&port, &DockEffect::ForceQuit, &entry(), 1);
        assert!(pinned.refresh_entries && pinned.outcome.is_none());
        assert!(quit.refresh_entries);
        assert_eq!(quit.outcome, Some(CommandOutcome::Completed));
        assert_eq!(*port.calls.borrow(), ["toggle_pin", "force_quit"]);
    }

    #[test]
    fn application_effects_do_nothing_when_the_entry_has_no_application() {
        let port = FakePort::default();
        for effect in [
            DockEffect::Hide,
            DockEffect::Unhide,
            DockEffect::HideOthers,
            DockEffect::Quit,
        ] {
            let result = apply_port_effect(&port, &effect, &entry(), 1);
            assert!(result.outcome.is_none() && !result.refresh_entries);
        }
        assert!(port.calls.borrow().is_empty());
    }

    #[test]
    fn a_failed_effect_reports_its_outcome_without_touching_the_port() {
        let port = FakePort::default();
        let result = apply_port_effect(
            &port,
            &DockEffect::Fail(CommandOutcome::Denied),
            &entry(),
            1,
        );
        assert_eq!(result.outcome, Some(CommandOutcome::Denied));
        assert!(port.calls.borrow().is_empty());
    }
}
