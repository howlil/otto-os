use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible, SW_SHOWNA, ShowWindow};

use crate::window::hidden::{self, Concealment};

mod commands;
mod context;
mod inspect;
mod process;

pub(crate) use self::commands::current_identity;
pub use self::commands::execute_window_command;
pub use self::commands::hide_application;
pub use self::commands::hide_other_applications;
pub(crate) use self::commands::identity_is_live;
pub use self::commands::quit_application;
pub use self::commands::show_application;
pub use self::commands::window_command_capability;
pub use self::commands::window_identity;
pub use self::context::ApplicationContext;
pub use self::context::application_of;
pub use self::inspect::ForegroundApplication;
pub(crate) use self::inspect::covers_monitor;
pub use self::inspect::enumerate_windows;
pub use self::inspect::foreground_application;
pub use self::inspect::inspect_window;
pub(crate) use self::inspect::owner_ids;
pub(crate) use self::process::is_window_uipi_blocked;

#[must_use]
pub fn restore_hidden_windows() -> usize {
    let mut restored = 0;
    for entry in hidden::take_all() {
        let identity = entry.identity;
        if !identity_is_live(identity) {
            continue;
        }
        let hwnd = HWND(identity.hwnd as *mut c_void);
        match entry.kind {
            Concealment::Hidden => {
                if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOWNA);
                    }
                    restored += 1;
                }
            }
            Concealment::TransitionsDisabled => {
                if crate::window::ops::set_dwm_transition_policy(hwnd, true) {
                    restored += 1;
                }
            }
            Concealment::RoundedCorners => {
                if crate::window::corners::reset(identity.hwnd) {
                    restored += 1;
                }
            }
        }
    }
    restored
}

#[cfg(test)]
mod fixture_tests {
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use otto_core::{
        AppCommand, AppInfo, Capability, CommandOutcome, WindowCommand, WindowInfo, WindowPresence,
        group_windows,
    };

    use super::{
        enumerate_windows, execute_window_command, hide_application, quit_application,
        restore_hidden_windows, show_application,
    };
    use crate::window::hidden::{self, Concealment};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const STEP_TIMEOUT: Duration = Duration::from_secs(15);

    struct Fixture {
        child: Child,
        marker: String,
        data_dir: std::path::PathBuf,
    }

    impl Fixture {
        fn launch() -> Self {
            let marker = format!("OttoFixture{}", std::process::id());
            let data_dir = std::env::temp_dir().join(format!("{marker}-data"));
            std::fs::create_dir_all(&data_dir).expect("fixture data directory");
            unsafe {
                std::env::set_var(crate::file::DATA_DIR_OVERRIDE, &data_dir);
            }
            let script = format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 $script:open = 2; \
                 $closed = {{ $script:open--; if ($script:open -le 0) {{ [Windows.Forms.Application]::Exit() }} }}; \
                 foreach ($name in 'A','B') {{ $form = New-Object Windows.Forms.Form; \
                 $form.Text = '{marker}-' + $name; $form.Width = 420; $form.Height = 260; \
                 $form.add_FormClosed($closed); $form.Show() }}; \
                 [Windows.Forms.Application]::Run()"
            );
            let child = Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .expect("fixture process");
            Self {
                child,
                marker,
                data_dir,
            }
        }

        fn windows(&self) -> Vec<WindowInfo> {
            enumerate_windows(std::process::id())
                .into_iter()
                .filter(|window| window.title.starts_with(&self.marker))
                .collect()
        }

        fn app(&self) -> Option<AppInfo> {
            group_windows(self.windows()).into_iter().next()
        }

        fn window(&self, name: &str) -> Option<WindowInfo> {
            let title = format!("{}-{name}", self.marker);
            self.windows()
                .into_iter()
                .find(|window| window.title == title)
        }

        fn wait_for(&self, description: &str, mut condition: impl FnMut(&Self) -> bool) {
            let deadline = Instant::now() + STEP_TIMEOUT;
            while Instant::now() < deadline {
                if condition(self) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            panic!("fixture did not reach: {description}");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = restore_hidden_windows();
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn window_and_application_commands_stay_distinct_on_a_real_application() {
        let own = std::process::id();
        let mut fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let first = fixture.window("A").expect("window A");
        let second = fixture.window("B").expect("window B");
        assert_eq!(first.app, second.app);
        assert_eq!(first.capability(WindowCommand::Zoom), Capability::Available);

        assert_eq!(
            execute_window_command(first.identity, WindowCommand::ToggleFullScreen, own),
            CommandOutcome::Unsupported
        );
        assert_eq!(
            execute_window_command(first.identity, WindowCommand::Zoom, own),
            CommandOutcome::Completed
        );
        fixture.wait_for("window A zoomed", |f| {
            f.window("A").is_some_and(|window| window.zoomed)
        });
        assert!(!fixture.window("A").expect("window A").full_screen);

        assert_eq!(
            execute_window_command(second.identity, WindowCommand::Minimize, own),
            CommandOutcome::Completed
        );
        fixture.wait_for("window B minimized", |f| {
            f.window("B")
                .is_some_and(|window| window.presence == WindowPresence::Minimized)
        });
        let app = fixture.app().expect("fixture application");
        assert!(!app.is_hidden());
        assert_eq!(
            app.capability(AppCommand::Hide, true),
            Capability::Available
        );

        assert_eq!(hide_application(&app, own), CommandOutcome::Completed);
        fixture.wait_for("application hidden", |f| {
            f.app().is_some_and(|app| app.is_hidden())
        });
        assert_eq!(hidden::recorded(Concealment::Hidden).len(), 2);
        assert!(fixture.data_dir.join("hidden_windows.txt").exists());

        let hidden = fixture.app().expect("hidden application");
        assert_eq!(show_application(&hidden, own), CommandOutcome::Completed);
        fixture.wait_for("application shown again", |f| {
            f.app().is_some_and(|app| !app.is_hidden())
        });
        assert!(hidden::recorded(Concealment::Hidden).is_empty());

        assert_eq!(
            execute_window_command(second.identity, WindowCommand::Close, own),
            CommandOutcome::Completed
        );
        fixture.wait_for("only window A left", |f| f.windows().len() == 1);
        assert!(fixture.child.try_wait().expect("fixture status").is_none());
        assert_eq!(
            execute_window_command(second.identity, WindowCommand::Close, own),
            CommandOutcome::TargetGone
        );

        let remaining = fixture.app().expect("remaining application");
        assert_eq!(quit_application(&remaining, own), CommandOutcome::Completed);
        let deadline = Instant::now() + STEP_TIMEOUT;
        while fixture.child.try_wait().expect("fixture status").is_none() {
            assert!(
                Instant::now() < deadline,
                "quit did not end the application"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(fixture.windows().is_empty());
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn suppressed_transitions_are_recorded_first_and_recovered_after_a_crash() {
        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let window = fixture.window("A").expect("window A");
        let hwnd = window.identity.hwnd;
        assert!(crate::window::ops::set_window_transitions_enabled(
            hwnd, false
        ));
        assert_eq!(
            hidden::recorded(Concealment::TransitionsDisabled),
            vec![window.identity]
        );
        assert!(fixture.data_dir.join("hidden_windows.txt").exists());
        assert_eq!(restore_hidden_windows(), 1);
        assert!(hidden::recorded(Concealment::TransitionsDisabled).is_empty());
        assert!(crate::window::ops::set_window_transitions_enabled(
            hwnd, false
        ));
        assert!(crate::window::ops::set_window_transitions_enabled(
            hwnd, true
        ));
        assert!(hidden::recorded(Concealment::TransitionsDisabled).is_empty());
        assert!(!fixture.data_dir.join("hidden_windows.txt").exists());
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn rounding_forces_default_windows_only_and_every_restore_path_resets_them() {
        use crate::window::corners::{
            preference, restore_rounded_corners, round_window_corners, set_preference,
        };
        use windows::Win32::Graphics::Dwm::{DWMWCP_DEFAULT, DWMWCP_DONOTROUND, DWMWCP_ROUND};

        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let first = fixture.window("A").expect("window A").identity;
        let second = fixture.window("B").expect("window B").identity;
        assert_eq!(preference(first.hwnd), Some(DWMWCP_DEFAULT));

        assert!(round_window_corners(first.hwnd));
        assert_eq!(preference(first.hwnd), Some(DWMWCP_ROUND));
        assert_eq!(hidden::recorded(Concealment::RoundedCorners), vec![first]);
        assert!(fixture.data_dir.join("hidden_windows.txt").exists());

        assert!(set_preference(second.hwnd, DWMWCP_DONOTROUND));
        assert!(!round_window_corners(second.hwnd));
        assert_eq!(preference(second.hwnd), Some(DWMWCP_DONOTROUND));
        assert_eq!(hidden::recorded(Concealment::RoundedCorners).len(), 1);

        assert_eq!(restore_rounded_corners(), 1);
        assert_eq!(preference(first.hwnd), Some(DWMWCP_DEFAULT));
        assert_eq!(preference(second.hwnd), Some(DWMWCP_DONOTROUND));
        assert!(hidden::recorded(Concealment::RoundedCorners).is_empty());
        assert!(!fixture.data_dir.join("hidden_windows.txt").exists());

        assert!(round_window_corners(first.hwnd));
        assert_eq!(preference(first.hwnd), Some(DWMWCP_ROUND));
        assert_eq!(restore_hidden_windows(), 1);
        assert_eq!(preference(first.hwnd), Some(DWMWCP_DEFAULT));
    }

    #[test]
    #[ignore = "native fixture: minimizes and restores a real window"]
    fn quiet_minimize_and_restore_reach_the_native_state_and_release_overrides() {
        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let hwnd = fixture.window("B").expect("window B").identity.hwnd;
        assert!(crate::window::ops::minimize_window_native_quiet(hwnd));
        fixture.wait_for("window B minimized", |f| {
            f.window("B")
                .is_some_and(|window| window.presence == WindowPresence::Minimized)
        });
        assert!(crate::window::ops::set_window_transitions_enabled(
            hwnd, true
        ));
        assert!(crate::window::ops::restore_window_native_quiet(hwnd));
        fixture.wait_for("window B restored", |f| {
            f.window("B")
                .is_some_and(|window| window.presence == WindowPresence::Visible)
        });
        assert!(crate::window::ops::set_window_transitions_enabled(
            hwnd, true
        ));
        assert!(hidden::recorded(Concealment::TransitionsDisabled).is_empty());
    }

    #[test]
    #[ignore = "native fixture: opens two real windows on the desktop"]
    fn windows_hidden_before_a_crash_are_shown_again_from_the_ledger() {
        let own = std::process::id();
        let fixture = Fixture::launch();
        fixture.wait_for("two fixture windows", |f| f.windows().len() == 2);
        let app = fixture.app().expect("fixture application");
        assert_eq!(hide_application(&app, own), CommandOutcome::Completed);
        fixture.wait_for("application hidden", |f| {
            f.app().is_some_and(|app| app.is_hidden())
        });
        assert_eq!(restore_hidden_windows(), 2);
        fixture.wait_for("windows visible after recovery", |f| {
            f.windows()
                .iter()
                .all(|window| window.presence == WindowPresence::Visible)
        });
        assert!(!fixture.data_dir.join("hidden_windows.txt").exists());
        assert_eq!(restore_hidden_windows(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::commands::summarize;
    use super::context::tile_rect;
    use super::inspect::{
        is_full_screen, is_recorded_hidden, uses_full_screen_shortcut, window_role,
    };
    use super::process::filetime_value;
    use otto_core::{CommandOutcome, Rect, WindowIdentity};
    use windows::Win32::Foundation::{FILETIME, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{WS_CAPTION, WS_POPUP};

    #[test]
    fn windows_owned_by_a_hidden_framework_window_are_application_windows() {
        use otto_core::WindowRole;
        assert_eq!(window_role(false, false), WindowRole::Primary);
        assert_eq!(window_role(true, false), WindowRole::OwnedDialog);
        assert_eq!(window_role(true, true), WindowRole::Primary);
    }

    #[test]
    fn maximized_window_filling_the_monitor_is_not_full_screen() {
        assert!(!is_full_screen(true, true, WS_CAPTION.0));
        assert!(is_full_screen(true, false, WS_POPUP.0));
        assert!(is_full_screen(true, true, WS_POPUP.0));
        assert!(!is_full_screen(false, false, WS_POPUP.0));
    }

    #[test]
    fn generation_combines_both_halves_of_the_creation_time() {
        let time = FILETIME {
            dwLowDateTime: 0x89AB_CDEF,
            dwHighDateTime: 0x0123_4567,
        };
        assert_eq!(filetime_value(time), 0x0123_4567_89AB_CDEF);
    }

    #[test]
    fn tiling_splits_the_work_area_without_overlap_or_gap() {
        let work = RECT {
            left: -1921,
            top: 32,
            right: 0,
            bottom: 1080,
        };
        let left = tile_rect(work, true);
        let right = tile_rect(work, false);
        assert_eq!(
            left,
            Rect {
                x: -1921,
                y: 32,
                width: 960,
                height: 1048
            }
        );
        assert_eq!(left.right(), right.x);
        assert_eq!(right.right(), 0);
    }

    #[test]
    fn full_screen_shortcut_is_limited_to_known_applications() {
        assert!(uses_full_screen_shortcut("C:\\Apps\\Chrome.exe"));
        assert!(uses_full_screen_shortcut("C:\\Windows\\explorer.exe"));
        assert!(!uses_full_screen_shortcut("C:\\Windows\\notepad.exe"));
        assert!(!uses_full_screen_shortcut(""));
    }

    #[test]
    fn batch_outcome_prefers_success_then_denial_then_blockage_then_absence() {
        assert_eq!(
            summarize([CommandOutcome::Denied, CommandOutcome::Completed]),
            CommandOutcome::Completed
        );
        assert_eq!(
            summarize([
                CommandOutcome::TemporarilyUnavailable,
                CommandOutcome::Denied
            ]),
            CommandOutcome::Denied
        );
        assert_eq!(
            summarize([
                CommandOutcome::Unsupported,
                CommandOutcome::TemporarilyUnavailable
            ]),
            CommandOutcome::TemporarilyUnavailable
        );
        assert_eq!(
            summarize([CommandOutcome::Unsupported]),
            CommandOutcome::Unsupported
        );
        assert_eq!(
            summarize(Vec::<CommandOutcome>::new()),
            CommandOutcome::TargetGone
        );
    }

    #[test]
    fn hidden_ledger_entry_matches_only_the_same_window_of_the_same_process() {
        let recorded = WindowIdentity {
            hwnd: 100,
            process_id: 7,
            thread_id: 8,
            generation: 900,
        };
        let ledger = [recorded];
        assert!(is_recorded_hidden(&ledger, &recorded));
        let reused_handle_other_process = WindowIdentity {
            process_id: 9,
            ..recorded
        };
        let reused_handle_restarted_process = WindowIdentity {
            generation: 901,
            ..recorded
        };
        assert!(!is_recorded_hidden(&ledger, &reused_handle_other_process));
        assert!(!is_recorded_hidden(
            &ledger,
            &reused_handle_restarted_process
        ));
        assert!(!is_recorded_hidden(&[], &recorded));
    }
}
