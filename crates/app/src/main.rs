#![windows_subsystem = "windows"]

mod backdrop_feed;
mod bar;
mod bar_controls;
mod dock;
mod genie;
mod host;
mod menu;
mod notch;
mod theme_state;
mod window_controls;

use windows::core::Result;

fn main() -> Result<()> {
    let _instance = match otto_platform::InstanceGuard::acquire() {
        Ok(Some(instance)) => instance,
        Ok(None) => return Ok(()),
        Err(error) => {
            eprintln!("otto-os: could not create the single-instance lock: {error}");
            return Err(error);
        }
    };
    let _ = otto_platform::restore_hidden_windows();
    if std::env::args().any(|arg| arg == "--restore-shell") {
        let _ = otto_platform::restore_shell_from_recovery();
        return Ok(());
    }
    install_shell_recovery_panic_hook();
    otto_platform::enable_per_monitor_dpi();
    otto_platform::load_apple_fonts();

    let _ = otto_platform::install_window_event_hooks();
    let blur = !std::env::args().any(|arg| arg == "--no-blur");
    if let Err(error) = host::start(blur) {
        otto_platform::uninstall_window_event_hooks();
        return Err(error);
    }
    if let Err(error) = host::apply_cli_overrides() {
        host::stop();
        otto_platform::uninstall_window_event_hooks();
        return Err(error);
    }
    let _ = otto_platform::install_keyboard_hook();
    host::run_message_loop();
    otto_platform::uninstall_keyboard_hook();
    host::stop();
    otto_platform::uninstall_window_event_hooks();
    Ok(())
}

fn install_shell_recovery_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if cfg!(panic = "abort") {
            let _ = otto_platform::restore_hidden_windows();
            let _ = otto_platform::restore_shell_from_recovery();
        }
        default_hook(info);
    }));
}
