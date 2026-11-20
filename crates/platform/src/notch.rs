use otto_core::{
    ClipboardPayload, NotchAction, NotchPort, NowPlaying, PrivacyState, ShelfPayload,
    ShelfThumbnail,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsNotchPort;

impl NotchPort for WindowsNotchPort {
    fn read_now_playing(&self) -> Option<NowPlaying> {
        super::read_now_playing()
    }

    fn read_privacy_state(&self) -> PrivacyState {
        super::privacy::read_privacy_state()
    }

    fn pick_shelf_files(&self) -> Vec<String> {
        super::shelf::pick_shelf_files()
    }

    fn inspect_shelf_file(&self, path: &str, max_px: u32) -> (bool, Option<ShelfThumbnail>) {
        super::shelf::inspect_shelf_file(path, max_px)
    }

    fn start_shelf_drag(&self, hwnd: isize, payload: &ShelfPayload) -> bool {
        super::shelf::start_shelf_drag(hwnd, payload)
    }

    fn copy_clipboard(&self, payload: &ClipboardPayload) -> Option<u32> {
        super::clipboard::copy_clipboard_payload(payload)
    }

    fn execute(&self, action: NotchAction) {
        match action {
            NotchAction::Media(command) => {
                if !super::send_media_command(command) {
                    super::signal_command_failure();
                }
            }
            NotchAction::PlayPomodoroChime => super::play_pomodoro_chime(),
            NotchAction::ToggleMicMute => {
                if super::privacy::toggle_mic_mute().is_none() {
                    super::signal_command_failure();
                }
            }
            NotchAction::ShelfOpen(payload) => {
                if !super::shelf::open_shelf_payload(&payload) {
                    super::signal_command_failure();
                }
            }
            NotchAction::ShelfCopy(payload) => {
                if !super::shelf::copy_shelf_payload(&payload) {
                    super::signal_command_failure();
                }
            }
            NotchAction::ShelfReveal(payload) => {
                if !super::shelf::reveal_shelf_payload(&payload) {
                    super::signal_command_failure();
                }
            }
            NotchAction::NotificationOpen(id) => {
                if !super::notifications::dismiss_notification(id) {
                    super::signal_command_failure();
                }
            }
            NotchAction::NotificationDismiss(id) => {
                if !super::notifications::remove_notification(id) {
                    super::signal_command_failure();
                }
            }
        }
    }
}
