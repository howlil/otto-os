use otto_core::{NotchAction, NotchPort, NowPlaying};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsNotchPort;

impl NotchPort for WindowsNotchPort {
    fn read_now_playing(&self) -> Option<NowPlaying> {
        super::read_now_playing()
    }

    fn execute(&self, action: NotchAction) {
        match action {
            NotchAction::Media(command) => {
                if !super::send_media_command(command) {
                    super::signal_command_failure();
                }
            }
            NotchAction::OpenTaskView => super::open_task_view(),
            NotchAction::PlayPomodoroChime => super::play_pomodoro_chime(),
        }
    }
}
