#![windows_subsystem = "windows"]

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
    let Some(_instance) = otto_platform::InstanceGuard::acquire() else {
        return Ok(());
    };
    otto_platform::enable_per_monitor_dpi();
    otto_platform::load_apple_fonts();

    let blur = !std::env::args().any(|arg| arg == "--no-blur");
    host::start(blur)?;
    if let Err(error) = host::apply_cli_overrides() {
        host::stop();
        return Err(error);
    }
    let _ = otto_platform::install_os_event_hooks();
    host::run_message_loop();
    otto_platform::uninstall_os_event_hooks();
    host::stop();
    Ok(())
}
