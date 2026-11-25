use crate::host::RuntimeContext;
use otto_core::{
    Capability, CommandOutcome, CommandScope, WindowCommand, WindowControlAction, WindowIdentity,
    WindowInfo, application_command_targets, window_control_scope,
};
use otto_platform::{
    application_of, execute_window_command, inspect_window, signal_command_failure,
};

use super::{Overlay, is_alt_pressed};

pub(crate) fn open_tiling_menu(
    runtime: &RuntimeContext,
    window_hwnd: isize,
    own_pid: u32,
    anchor_x: i32,
    anchor_y: i32,
) {
    let Some(window) = inspect_window(window_hwnd, own_pid) else {
        return;
    };
    crate::menu::open_window_tiling_menu(runtime, &window, anchor_x, anchor_y);
}

fn execute_window_action(
    runtime: &RuntimeContext,
    window: &WindowInfo,
    own_pid: u32,
    action: WindowControlAction,
) {
    let alt_down = is_alt_pressed();
    let command = match action {
        WindowControlAction::Close => WindowCommand::Close,
        WindowControlAction::Minimize => WindowCommand::Minimize,
        WindowControlAction::ToggleMaximize if alt_down => WindowCommand::ToggleFullScreen,
        WindowControlAction::ToggleMaximize => WindowCommand::Zoom,
    };
    let identity = window.identity;
    let live = inspect_window(identity.hwnd, own_pid)
        .filter(|live| live.identity.is_same_window(&identity));
    let Some(live) = live else {
        signal_command_failure();
        return;
    };
    if live.capability(command) != Capability::Available {
        signal_command_failure();
        return;
    }
    if window_control_scope(action, alt_down) == CommandScope::Application {
        execute_on_application(runtime, identity, own_pid, command);
        return;
    }
    if command == WindowCommand::Minimize {
        let _ = crate::genie::minimize_window(runtime, identity.hwnd);
        return;
    }
    if execute_window_command(identity, command, own_pid) != CommandOutcome::Completed {
        signal_command_failure();
    }
}

fn execute_on_application(
    runtime: &RuntimeContext,
    clicked: WindowIdentity,
    own_pid: u32,
    command: WindowCommand,
) {
    let targets = application_of(clicked, own_pid)
        .map(|context| application_command_targets(&context.app, clicked, command))
        .unwrap_or_default();
    if targets.is_empty() {
        signal_command_failure();
        return;
    }
    let mut failed = false;
    for target in targets {
        let completed = if command == WindowCommand::Minimize && target.is_same_window(&clicked) {
            crate::genie::minimize_window(runtime, target.hwnd)
        } else {
            execute_window_command(target, command, own_pid) == CommandOutcome::Completed
        };
        failed |= !completed;
    }
    if failed {
        signal_command_failure();
    }
}

impl Overlay {
    pub(super) fn open_window_menu(&mut self, runtime: &RuntimeContext) {
        let Some(target) = self.target else {
            return;
        };
        let Some(window) = inspect_window(target.hwnd, self.own_pid) else {
            return;
        };
        let (ox, oy) = self.render_origin();
        let Some(zoom_center) = self.layout.center_of(WindowControlAction::ToggleMaximize) else {
            return;
        };
        let anchor_x = ox + zoom_center.round() as i32 - self.layout.height / 2;
        crate::menu::open_window_tiling_menu(
            runtime,
            &window,
            anchor_x,
            oy + self.layout.height + 2,
        );
        self.window = Some(window);
    }

    pub(super) fn execute_button_action(
        &self,
        runtime: &RuntimeContext,
        action: WindowControlAction,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        execute_window_action(runtime, window, self.own_pid, action);
    }
}
