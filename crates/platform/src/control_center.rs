use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{
    AudioOutputDevice, BluetoothState, CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR,
    CMD_CC_DISPLAY, CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE,
    CMD_CC_SOUND, CMD_CC_WIFI, ControlCenterAction, ControlCenterPort, MediaCommand, NowPlaying,
    SystemStatus, UPDATE_CHECKING_LABEL, WifiState, format_update_status,
};
use windows::Foundation::Uri;
use windows::Web::Http::{HttpClient, HttpCompletionOption};
use windows::core::HSTRING;

use crate::actions::{
    execute_control_center_command, open_night_light_settings, open_power_settings,
};
use crate::audio::{read_audio_outputs, set_audio_output, set_system_mute, set_system_volume};
use crate::com::ComGuard;
use crate::connectivity::{
    connect_wifi, read_bluetooth_state, read_wifi_state, set_bluetooth_enabled, set_wifi_enabled,
    toggle_bluetooth_device,
};
use crate::media::read_now_playing;
use crate::status::{read_system_status, set_display_brightness};
use crate::winrt_io::{await_until, read_stream_bounded};
use crate::worker::join_within;

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/howlil/otto-os/releases/latest";
const RELEASES_PAGE_URL: &str = "https://github.com/howlil/otto-os/releases";
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_millis(2_500);
const UPDATE_JOIN_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_RELEASE_JSON_BYTES: usize = 128 * 1024;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct UpdateCheckCache {
    in_flight: bool,
    status: Option<String>,
}

impl UpdateCheckCache {
    fn begin(&mut self) -> bool {
        if self.in_flight {
            return false;
        }
        self.in_flight = true;
        self.status = Some(String::from(UPDATE_CHECKING_LABEL));
        true
    }

    fn finish(&mut self, status: String) {
        self.in_flight = false;
        self.status = Some(status);
    }

    fn current(&self) -> Option<String> {
        self.status.clone()
    }
}

static UPDATE_CHECK_CACHE: OnceLock<Mutex<UpdateCheckCache>> = OnceLock::new();
static UPDATE_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static UPDATE_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);

fn is_shutting_down() -> bool {
    UPDATE_SHUTTING_DOWN.load(Ordering::Acquire)
}

pub fn shutdown_update_check_task() {
    UPDATE_SHUTTING_DOWN.store(true, Ordering::Release);
    if let Some(handle) = UPDATE_TASK.lock().ok().and_then(|mut task| task.take()) {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
    if let Ok(mut guard) = update_cache().lock() {
        guard.in_flight = false;
    }
}

pub(crate) fn resume_update_check_task() {
    UPDATE_SHUTTING_DOWN.store(false, Ordering::Release);
}

fn update_cache() -> &'static Mutex<UpdateCheckCache> {
    UPDATE_CHECK_CACHE.get_or_init(|| Mutex::new(UpdateCheckCache::default()))
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsControlCenter;

impl ControlCenterPort for WindowsControlCenter {
    fn read_system_status(&self) -> SystemStatus {
        read_system_status()
    }

    fn read_now_playing(&self) -> Option<NowPlaying> {
        read_now_playing()
    }

    fn set_volume(&self, percent: u8) -> Option<(u8, bool)> {
        set_system_volume(percent)
    }

    fn set_muted(&self, muted: bool) -> Option<(u8, bool)> {
        set_system_mute(muted)
    }

    fn set_brightness(&self, percent: u8) -> Option<u8> {
        set_display_brightness(percent)
    }

    fn read_launch_at_login(&self) -> bool {
        crate::startup::read_launch_at_login()
    }

    fn set_launch_at_login(&self, enabled: bool) -> bool {
        crate::startup::set_launch_at_login(enabled)
    }

    fn open_logs_folder(&self) {
        let _ = crate::diagnostics::open_logs_folder();
    }

    fn read_audio_outputs(&self) -> Vec<AudioOutputDevice> {
        read_audio_outputs()
    }

    fn set_audio_output(&self, id: &str) -> bool {
        set_audio_output(id)
    }

    fn check_for_updates(&self) -> Option<String> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let Ok(handle) = thread::Builder::new()
            .name(String::from("otto-update-check"))
            .spawn(move || {
                let _ = sender.send(check_for_updates_blocking());
            })
        else {
            return Some(String::from("Check failed"));
        };
        let status = receiver
            .recv_timeout(UPDATE_JOIN_TIMEOUT)
            .unwrap_or_else(|_| String::from("Check failed"));
        let _ = join_within(handle, Duration::from_millis(100));
        crate::diagnostics::log_info("updates", &status);
        Some(status)
    }

    fn start_update_check(&self) -> bool {
        if is_shutting_down() {
            return false;
        }
        let Ok(mut task_slot) = UPDATE_TASK.lock() else {
            return false;
        };
        if is_shutting_down() {
            return false;
        }
        if let Some(prev) = task_slot.take() {
            if prev.is_finished() {
                let _ = prev.join();
            } else {
                *task_slot = Some(prev);
                return false;
            }
        }
        let Ok(mut guard) = update_cache().lock() else {
            return false;
        };
        if !guard.begin() {
            return false;
        }
        drop(guard);
        let spawned = thread::Builder::new()
            .name(String::from("otto-update-check"))
            .spawn(move || {
                let status = check_for_updates_blocking();
                if is_shutting_down() {
                    if let Ok(mut guard) = update_cache().lock() {
                        guard.in_flight = false;
                    }
                    return;
                }
                crate::diagnostics::log_info("updates", &status);
                if let Ok(mut guard) = update_cache().lock() {
                    guard.finish(status);
                }
                if !is_shutting_down() {
                    crate::connectivity::post_control_center_refresh();
                }
            });
        if let Ok(handle) = spawned {
            *task_slot = Some(handle);
            true
        } else {
            if let Ok(mut guard) = update_cache().lock() {
                guard.finish(String::from("Check failed"));
            }
            false
        }
    }

    fn poll_update_status(&self) -> Option<String> {
        update_cache().lock().ok().and_then(|guard| guard.current())
    }

    fn open_releases_page(&self) {
        let _ = crate::actions::open_external_url(RELEASES_PAGE_URL);
    }

    fn execute(&self, action: ControlCenterAction) {
        match action {
            ControlCenterAction::Media(cmd @ MediaCommand::Seek(_)) => {
                if !super::send_media_command(cmd) {
                    super::signal_command_failure();
                }
                return;
            }
            ControlCenterAction::OpenNightLight => {
                open_night_light_settings();
                return;
            }
            ControlCenterAction::OpenPowerSettings => {
                open_power_settings();
                return;
            }
            _ => {}
        }
        let command = match action {
            ControlCenterAction::Media(MediaCommand::TogglePlayPause) => CMD_CC_MEDIA_TOGGLE,
            ControlCenterAction::Media(MediaCommand::Next) => CMD_CC_MEDIA_NEXT,
            ControlCenterAction::Media(MediaCommand::Previous) => CMD_CC_MEDIA_PREVIOUS,
            ControlCenterAction::Media(MediaCommand::Seek(_))
            | ControlCenterAction::OpenNightLight
            | ControlCenterAction::OpenPowerSettings => unreachable!(),
            ControlCenterAction::OpenCalendar => CMD_CC_CALENDAR,
            ControlCenterAction::OpenCalendarSettings => CMD_CAL_SETTINGS,
            ControlCenterAction::OpenWifi => CMD_CC_WIFI,
            ControlCenterAction::OpenBluetooth => CMD_CC_BLUETOOTH,
            ControlCenterAction::OpenFocus => CMD_CC_FOCUS,
            ControlCenterAction::OpenDisplay => CMD_CC_DISPLAY,
            ControlCenterAction::OpenSound => CMD_CC_SOUND,
        };
        execute_control_center_command(command);
    }

    fn read_wifi_state(&self) -> WifiState {
        read_wifi_state()
    }

    fn set_wifi_enabled(&self, enabled: bool) -> bool {
        set_wifi_enabled(enabled)
    }

    fn connect_wifi(&self, ssid: &str) -> bool {
        connect_wifi(ssid)
    }

    fn read_bluetooth_state(&self) -> BluetoothState {
        read_bluetooth_state()
    }

    fn set_bluetooth_enabled(&self, enabled: bool) -> bool {
        set_bluetooth_enabled(enabled)
    }

    fn toggle_bluetooth_device(&self, id: &str, connect: bool) -> bool {
        toggle_bluetooth_device(id, connect)
    }

    fn read_notifications(&self) -> otto_core::NotificationCenterSnapshot {
        crate::notifications::read_notifications_snapshot()
    }

    fn dismiss_notification(&self, id: u32) -> bool {
        crate::notifications::dismiss_notification(id)
    }

    fn clear_notifications(&self) -> bool {
        crate::notifications::clear_notifications()
    }

    fn request_notification_access(&self) -> otto_core::NotificationAccess {
        crate::notifications::request_notification_access()
    }
}

fn check_for_updates_blocking() -> String {
    if is_shutting_down() {
        return String::from("Check failed");
    }
    let _com = ComGuard::multithreaded();
    let Ok(client) = HttpClient::new() else {
        return String::from("Check failed");
    };
    if let Ok(headers) = client.DefaultRequestHeaders() {
        let _ = headers.TryAppendWithoutValidation(
            &HSTRING::from("User-Agent"),
            &HSTRING::from(concat!(
                "OttoOS/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/howlil/otto-os)"
            )),
        );
        let _ = headers.TryAppendWithoutValidation(
            &HSTRING::from("Accept"),
            &HSTRING::from("application/vnd.github+json"),
        );
    }
    if is_shutting_down() {
        return String::from("Check failed");
    }
    let Ok(uri) = Uri::CreateUri(&HSTRING::from(RELEASES_LATEST_URL)) else {
        return String::from("Check failed");
    };
    let deadline = Instant::now() + UPDATE_CHECK_TIMEOUT;
    let Some(response) = await_until!(
        client.GetWithOptionAsync(&uri, HttpCompletionOption::ResponseHeadersRead),
        deadline
    ) else {
        return String::from("Check failed");
    };
    let status_code = response
        .StatusCode()
        .ok()
        .and_then(|code| u16::try_from(code.0).ok())
        .unwrap_or(0);
    if !(200..300).contains(&status_code) {
        return format_update_status(env!("CARGO_PKG_VERSION"), status_code, None);
    }
    let body = read_update_body(&response, deadline);
    format_update_status(env!("CARGO_PKG_VERSION"), status_code, body.as_deref())
}

fn read_update_body(
    response: &windows::Web::Http::HttpResponseMessage,
    deadline: Instant,
) -> Option<String> {
    let content = response.Content().ok()?;
    let mut declared_length = 0u64;
    if matches!(content.TryComputeLength(&mut declared_length), Ok(true))
        && declared_length > MAX_RELEASE_JSON_BYTES as u64
    {
        return None;
    }
    let stream = await_until!(content.ReadAsInputStreamAsync(), deadline)?;
    let bytes = read_stream_bounded(&stream, MAX_RELEASE_JSON_BYTES, deadline, &is_shutting_down)?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        UPDATE_TASK, UpdateCheckCache, WindowsControlCenter, resume_update_check_task,
        shutdown_update_check_task,
    };
    use otto_core::{ControlCenterPort, UPDATE_CHECKING_LABEL};

    #[test]
    fn update_cache_guards_concurrent_checks_and_publishes_completion() {
        let mut cache = UpdateCheckCache::default();
        assert_eq!(cache.current(), None);
        assert!(cache.begin());
        assert_eq!(cache.current().as_deref(), Some(UPDATE_CHECKING_LABEL));
        assert!(!cache.begin());

        cache.finish(String::from("Update v0.2.0"));
        assert_eq!(cache.current().as_deref(), Some("Update v0.2.0"));
        assert!(cache.begin());
        assert_eq!(cache.current().as_deref(), Some(UPDATE_CHECKING_LABEL));
    }

    #[test]
    fn shutdown_blocks_async_update_check_until_resumed() {
        struct ResumeGuard;
        impl Drop for ResumeGuard {
            fn drop(&mut self) {
                shutdown_update_check_task();
                resume_update_check_task();
            }
        }

        let _resume = ResumeGuard;
        shutdown_update_check_task();
        assert!(!WindowsControlCenter.start_update_check());
        assert!(UPDATE_TASK.lock().unwrap().is_none());
    }
}
