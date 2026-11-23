#![windows_subsystem = "windows"]

mod appearance;
mod bar;
mod control_center;
mod dock;
mod focus_shield;
mod genie;
mod host;
mod menu;
mod notch;
mod settings;
mod spotlight;
mod window_controls;

pub(crate) use appearance::{backdrop_feed, theme_state};
pub(crate) use bar::controls as bar_controls;
pub(crate) use bar::notch_io;
pub(crate) use host::win_msg;
pub(crate) use settings as settings_window;

use windows::core::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupCommand {
    Run,
    Quit,
    RestoreShell,
}

fn startup_command<I, S>(args: I) -> StartupCommand
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut restore_shell = false;
    for arg in args {
        match arg.as_ref() {
            "--quit" => return StartupCommand::Quit,
            "--restore-shell" => restore_shell = true,
            _ => {}
        }
    }
    if restore_shell {
        StartupCommand::RestoreShell
    } else {
        StartupCommand::Run
    }
}

struct SessionShutdownGuard<F: FnOnce()> {
    on_drop: Option<F>,
}

impl<F: FnOnce()> SessionShutdownGuard<F> {
    fn new(on_drop: F) -> Self {
        Self {
            on_drop: Some(on_drop),
        }
    }

    fn run(mut self) {
        if let Some(cleanup) = self.on_drop.take() {
            cleanup();
        }
    }
}

impl<F: FnOnce()> Drop for SessionShutdownGuard<F> {
    fn drop(&mut self) {
        if let Some(cleanup) = self.on_drop.take() {
            cleanup();
        }
    }
}

const fn should_emergency_restore_shell(
    abort_profile: bool,
    is_main_thread: bool,
    in_caught_wnd_proc: bool,
) -> bool {
    abort_profile || (is_main_thread && !in_caught_wnd_proc)
}

struct AppleFontsGuard;

impl AppleFontsGuard {
    fn load() -> Self {
        otto_platform::load_apple_fonts();
        Self
    }
}

impl Drop for AppleFontsGuard {
    fn drop(&mut self) {
        otto_platform::remove_apple_fonts();
    }
}

fn main() -> Result<()> {
    let command = startup_command(std::env::args().skip(1));
    if command == StartupCommand::Quit {
        return otto_platform::quit_running_instance();
    }
    let _instance = match otto_platform::InstanceGuard::acquire() {
        Ok(Some(instance)) => instance,
        Ok(None) => return Ok(()),
        Err(error) => {
            otto_platform::log_error("app", &format!("instance lock failed: {error}"));
            otto_platform::show_error_dialog(
                "Otto OS Failed to Start",
                &format!("Could not create the single-instance lock.\n\nDetails: {error}"),
            );
            return Err(error);
        }
    };
    let _ = otto_platform::restore_hidden_windows();
    otto_platform::cleanup_stale_updates();
    if command == StartupCommand::RestoreShell {
        let _ = otto_platform::restore_shell_from_recovery();
        otto_platform::log_info("app", "restored shell via --restore-shell");
        return Ok(());
    }
    if !otto_platform::is_supported_windows_version() {
        let build = otto_platform::current_windows_build_label();
        otto_platform::log_error("app", &format!("unsupported Windows build: {build}"));
        otto_platform::show_error_dialog(
            "Unsupported Windows Version",
            &format!(
                "Otto OS requires Windows 11 22H2 (build 22621) or later.\n\nDetected build: {build}"
            ),
        );
        return Ok(());
    }
    otto_platform::log_info(
        "app",
        &format!(
            "starting version={} os={}",
            env!("CARGO_PKG_VERSION"),
            otto_platform::current_windows_build_label()
        ),
    );
    let _ole = otto_platform::OleGuard::acquire();
    install_shell_recovery_panic_hook();
    otto_platform::enable_per_monitor_dpi();
    let fonts = AppleFontsGuard::load();

    let _ = otto_platform::install_window_event_hooks();
    let blur = !std::env::args().any(|arg| arg == "--no-blur");
    if let Err(error) = host::start(blur) {
        otto_platform::uninstall_window_event_hooks();
        otto_platform::log_error("app", &format!("host start failed: {error}"));
        otto_platform::show_error_dialog(
            "Otto OS Failed to Start",
            &format!("Otto OS could not initialize desktop surfaces.\n\nDetails: {error}"),
        );
        return Err(error);
    }
    let session_guard = SessionShutdownGuard::new(host::shutdown_session);
    if let Err(error) = host::apply_cli_overrides() {
        session_guard.run();
        otto_platform::log_error("app", &format!("cli overrides failed: {error}"));
        otto_platform::show_error_dialog(
            "Otto OS Failed to Start",
            &format!("Otto OS could not apply startup configuration.\n\nDetails: {error}"),
        );
        return Err(error);
    }
    let _ = otto_platform::install_keyboard_hook();
    host::run_message_loop();
    session_guard.run();
    drop(fonts);
    otto_platform::log_info("app", "shutdown complete");
    Ok(())
}

fn install_shell_recovery_panic_hook() {
    let main_thread = std::thread::current().id();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = panic_message(info);
        let location = info.location().map_or_else(
            || String::from("unknown"),
            |loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()),
        );
        otto_platform::log_error("panic", &format!("at {location}: {message}"));
        let is_main = std::thread::current().id() == main_thread;
        if should_emergency_restore_shell(
            cfg!(panic = "abort"),
            is_main,
            host::in_safe_window_proc(),
        ) {
            let _ = otto_platform::restore_hidden_windows();
            let _ = otto_platform::restore_shell_from_recovery();
        }
        default_hook(info);
    }));
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(msg) = info.payload().downcast_ref::<&str>() {
        return String::from(*msg);
    }
    if let Some(msg) = info.payload().downcast_ref::<String>() {
        return msg.clone();
    }
    String::from("unknown panic")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{
        SessionShutdownGuard, StartupCommand, should_emergency_restore_shell, startup_command,
    };

    #[test]
    fn startup_command_prioritizes_quit_then_restore_then_run() {
        assert_eq!(startup_command(["--no-blur"]), StartupCommand::Run);
        assert_eq!(
            startup_command(["--restore-shell"]),
            StartupCommand::RestoreShell
        );
        assert_eq!(
            startup_command(["--restore-shell", "--quit"]),
            StartupCommand::Quit
        );
    }

    #[test]
    fn session_shutdown_guard_runs_once_on_explicit_run_or_unwind() {
        let count = Cell::new(0usize);
        let guard = SessionShutdownGuard::new(|| count.set(count.get() + 1));
        guard.run();
        assert_eq!(count.get(), 1);

        let unwind_count = Cell::new(0usize);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = SessionShutdownGuard::new(|| unwind_count.set(unwind_count.get() + 1));
            panic!("simulated fatal unwind");
        }));
        assert!(result.is_err());
        assert_eq!(unwind_count.get(), 1);
    }

    #[test]
    fn emergency_shell_restore_skips_caught_wnd_proc_and_worker_panics() {
        assert!(!should_emergency_restore_shell(false, false, false));
        assert!(!should_emergency_restore_shell(false, false, true));
        assert!(!should_emergency_restore_shell(false, true, true));
        assert!(should_emergency_restore_shell(false, true, false));
        assert!(should_emergency_restore_shell(true, false, true));
        assert!(should_emergency_restore_shell(true, true, true));
    }
}
