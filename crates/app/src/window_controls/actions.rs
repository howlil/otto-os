use crate::host::RuntimeContext;
use otto_core::{
    Capability, CommandOutcome, CommandScope, WindowCommand, WindowControlAction, WindowIdentity,
    application_command_targets, window_control_scope,
};
use otto_platform::{
    application_of, execute_window_command, inspect_window, signal_command_failure,
};

use super::Overlay;
use super::is_alt_pressed;

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
        let alt_down = is_alt_pressed();
        let command = match action {
            WindowControlAction::Close => WindowCommand::Close,
            WindowControlAction::Minimize => WindowCommand::Minimize,
            WindowControlAction::ToggleMaximize if alt_down => WindowCommand::ToggleFullScreen,
            WindowControlAction::ToggleMaximize => WindowCommand::Zoom,
        };
        let identity = window.identity;
        let live = inspect_window(identity.hwnd, self.own_pid)
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
            self.execute_on_application(runtime, identity, command);
            return;
        }
        if command == WindowCommand::Minimize {
            let _ = crate::genie::minimize_window(runtime, identity.hwnd);
            return;
        }
        if execute_window_command(identity, command, self.own_pid) != CommandOutcome::Completed {
            signal_command_failure();
        }
    }

    pub(super) fn execute_on_application(
        &self,
        runtime: &RuntimeContext,
        clicked: WindowIdentity,
        command: WindowCommand,
    ) {
        let targets = application_of(clicked, self.own_pid)
            .map(|context| application_command_targets(&context.app, clicked, command))
            .unwrap_or_default();
        if targets.is_empty() {
            signal_command_failure();
            return;
        }
        let mut failed = false;
        for target in targets {
            let completed = if command == WindowCommand::Minimize && target.is_same_window(&clicked)
            {
                crate::genie::minimize_window(runtime, target.hwnd)
            } else {
                execute_window_command(target, command, self.own_pid) == CommandOutcome::Completed
            };
            failed |= !completed;
        }
        if failed {
            signal_command_failure();
        }
    }
}
