use otto_core::{
    ClipboardPayload, NotchAction, NotchPort, NowPlaying, PrivacyState, ShelfPayload,
    ShelfThumbnail,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsNotchPort;

impl NotchPort for WindowsNotchPort {
    fn read_now_playing(&self) -> Option<NowPlaying> {
        crate::read_now_playing()
    }

    fn read_privacy_state(&self) -> PrivacyState {
        crate::notch::privacy::read_privacy_state()
    }

    fn pick_shelf_files(&self) -> Vec<String> {
        crate::notch::shelf::pick_shelf_files()
    }

    fn inspect_shelf_file(&self, path: &str, max_px: u32) -> (bool, Option<ShelfThumbnail>) {
        crate::notch::shelf::inspect_shelf_file(path, max_px)
    }

    fn start_shelf_drag(&self, hwnd: isize, payload: &ShelfPayload) -> bool {
        crate::notch::shelf::start_shelf_drag(hwnd, payload)
    }

    fn copy_clipboard(&self, payload: &ClipboardPayload) -> Option<u32> {
        crate::notch::clipboard::copy_clipboard_payload(payload)
    }

    fn execute(&self, action: NotchAction) {
        match action {
            NotchAction::Media(command) => {
                if !crate::send_media_command(command) {
                    crate::signal_command_failure();
                }
            }
            NotchAction::PlayPomodoroChime => crate::play_pomodoro_chime(),
            NotchAction::ToggleMicMute => {
                if crate::notch::privacy::toggle_mic_mute().is_none() {
                    crate::signal_command_failure();
                }
            }
            NotchAction::ShelfOpen(payload) => {
                if !crate::notch::shelf::open_shelf_payload(&payload) {
                    crate::signal_command_failure();
                }
            }
            NotchAction::ShelfCopy(payload) => {
                if !crate::notch::shelf::copy_shelf_payload(&payload) {
                    crate::signal_command_failure();
                }
            }
            NotchAction::ShelfReveal(payload) => {
                if !crate::notch::shelf::reveal_shelf_payload(&payload) {
                    crate::signal_command_failure();
                }
            }
            NotchAction::NotificationOpen(id) => {
                if !crate::notch::notifications::dismiss_notification(id) {
                    crate::signal_command_failure();
                }
            }
            NotchAction::NotificationDismiss(id) => {
                if !crate::notch::notifications::remove_notification(id) {
                    crate::signal_command_failure();
                }
            }
        }
    }
}
