use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{UPDATE_CHECKING_LABEL, format_update_status};
use windows::Foundation::Uri;
use windows::Web::Http::{HttpClient, HttpCompletionOption};
use windows::core::HSTRING;

use crate::com::ComGuard;
use crate::winrt_io::{await_until, read_stream_bounded};
use crate::worker::join_within;

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/howlil/otto-os/releases/latest";
const RELEASES_PAGE_URL: &str = "https://github.com/howlil/otto-os/releases";
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_millis(2_500);
const UPDATE_JOIN_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_RELEASE_JSON_BYTES: usize = 128 * 1024;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);

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

pub(super) fn check_for_updates_sync() -> String {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-update-check"))
        .spawn(move || {
            let _ = sender.send(check_for_updates_blocking());
        })
    else {
        return String::from("Check failed");
    };
    let status = receiver
        .recv_timeout(UPDATE_JOIN_TIMEOUT)
        .unwrap_or_else(|_| String::from("Check failed"));
    let _ = join_within(handle, Duration::from_millis(100));
    crate::diagnostics::log_info("updates", &status);
    status
}

pub(super) fn start_async_update_check() -> bool {
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
                crate::bar::connectivity::post_control_center_refresh();
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

pub(super) fn poll_async_update_status() -> Option<String> {
    update_cache().lock().ok().and_then(|guard| guard.current())
}

pub(super) fn open_releases_page() {
    let _ = crate::actions::open_external_url(RELEASES_PAGE_URL);
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
        UPDATE_TASK, UpdateCheckCache, resume_update_check_task, shutdown_update_check_task,
    };
    use crate::control_center::WindowsControlCenter;
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
