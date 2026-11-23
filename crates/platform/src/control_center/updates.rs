use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{
    ReleaseUpdateInfo, Sha256, UpdateState, is_allowed_release_page_url, parse_release_update,
    parse_sha256_for_asset,
};
use windows::Foundation::Uri;
use windows::Web::Http::{HttpClient, HttpCompletionOption, HttpResponseMessage};
use windows::core::HSTRING;

use crate::com::ComGuard;
use crate::file::{data_dir, move_file_atomically};
use crate::winrt_io::{await_until, copy_stream_bounded, read_stream_bounded};
use crate::worker::join_within;

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/howlil/otto-os/releases/latest";
const RELEASES_PAGE_URL: &str = "https://github.com/howlil/otto-os/releases";
const UPDATES_DIR_NAME: &str = "updates";
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(6);
const UPDATE_JOIN_TIMEOUT: Duration = Duration::from_secs(7);
const UPDATE_CHECKSUM_TIMEOUT: Duration = Duration::from_secs(15);
const UPDATE_DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(3);
const MAX_RELEASE_JSON_BYTES: usize = 256 * 1024;
const MAX_CHECKSUMS_BYTES: usize = 64 * 1024;
const MAX_INSTALLER_BYTES: u64 = 64 * 1024 * 1024;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const INSTALLER_ARGS: [&str; 6] = [
    "/SP-",
    "/SILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/CLOSEAPPLICATIONS",
    "/RELAUNCH",
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct UpdateCache {
    in_flight: bool,
    state: UpdateState,
    ready_installer: Option<PathBuf>,
}

impl UpdateCache {
    fn begin_check(&mut self) -> bool {
        if self.in_flight {
            return false;
        }
        self.in_flight = true;
        self.ready_installer = None;
        self.state = UpdateState::Checking;
        true
    }

    fn begin_install(&mut self) -> Option<ReleaseUpdateInfo> {
        if self.in_flight {
            return None;
        }
        let info = self.state.installable_release()?.clone();
        self.in_flight = true;
        self.ready_installer = None;
        self.state = UpdateState::Downloading {
            version: info.version.clone(),
            percent: Some(0),
        };
        Some(info)
    }

    fn set_download_progress(&mut self, version: &str, percent: Option<u8>) {
        if self.in_flight {
            self.state = UpdateState::Downloading {
                version: version.to_owned(),
                percent,
            };
        }
    }

    fn set_verifying(&mut self, version: &str) {
        if self.in_flight {
            self.state = UpdateState::Verifying {
                version: version.to_owned(),
            };
        }
    }

    fn finish_check(&mut self, state: UpdateState) {
        self.in_flight = false;
        self.ready_installer = None;
        self.state = state;
    }

    fn finish_install_ready(&mut self, version: String, installer: PathBuf) {
        self.in_flight = false;
        self.ready_installer = Some(installer);
        self.state = UpdateState::Installing { version };
    }

    fn finish_install_failed(&mut self, message: impl Into<String>, info: ReleaseUpdateInfo) {
        self.in_flight = false;
        self.ready_installer = None;
        self.state = UpdateState::Failed {
            message: message.into(),
            retry_available: Some(info),
        };
    }

    fn mark_launch_failed(&mut self, message: &str) {
        let retry_available = match &self.state {
            UpdateState::Available(info)
            | UpdateState::Failed {
                retry_available: Some(info),
                ..
            } => Some(info.clone()),
            _ => None,
        };
        self.in_flight = false;
        self.ready_installer = None;
        self.state = UpdateState::Failed {
            message: message.to_owned(),
            retry_available,
        };
    }

    fn current_state(&self) -> UpdateState {
        self.state.clone()
    }

    fn current_label(&self) -> Option<String> {
        self.state.status_label()
    }

    fn take_ready_installer(&mut self) -> Option<PathBuf> {
        self.ready_installer.take()
    }
}

struct TempPartGuard {
    path: PathBuf,
    armed: bool,
}

impl TempPartGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn defuse(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempPartGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

static UPDATE_CACHE: OnceLock<Mutex<UpdateCache>> = OnceLock::new();
static UPDATE_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static UPDATE_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static LAST_INSTALL_INFO: OnceLock<Mutex<Option<ReleaseUpdateInfo>>> = OnceLock::new();

fn is_shutting_down() -> bool {
    UPDATE_SHUTTING_DOWN.load(Ordering::Acquire)
}

fn update_cache() -> &'static Mutex<UpdateCache> {
    UPDATE_CACHE.get_or_init(|| Mutex::new(UpdateCache::default()))
}

fn last_install_info() -> &'static Mutex<Option<ReleaseUpdateInfo>> {
    LAST_INSTALL_INFO.get_or_init(|| Mutex::new(None))
}

fn updates_dir() -> Option<PathBuf> {
    data_dir().map(|root| root.join(UPDATES_DIR_NAME))
}

pub fn cleanup_stale_updates() {
    let Some(dir) = updates_dir() else {
        return;
    };
    cleanup_updates_in(&dir);
}

fn cleanup_updates_in(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension().and_then(|e| e.to_str())
            && (ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("part"))
        {
            let _ = fs::remove_file(path);
        }
    }
}

pub fn shutdown_update_check_task() {
    UPDATE_SHUTTING_DOWN.store(true, Ordering::Release);
    if let Some(handle) = UPDATE_TASK.lock().ok().and_then(|mut task| task.take()) {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
    if let Ok(mut guard) = update_cache().lock() {
        guard.in_flight = false;
        guard.ready_installer = None;
    }
}

pub(crate) fn resume_update_check_task() {
    UPDATE_SHUTTING_DOWN.store(false, Ordering::Release);
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
    let state = receiver
        .recv_timeout(UPDATE_JOIN_TIMEOUT)
        .unwrap_or_else(|_| UpdateState::Failed {
            message: String::from("Check failed"),
            retry_available: None,
        });
    let _ = join_within(handle, Duration::from_millis(100));
    let label = state
        .status_label()
        .unwrap_or_else(|| String::from("Check failed"));
    crate::diagnostics::log_info("updates", &label);
    label
}

pub(super) fn start_async_update_check() -> bool {
    if is_shutting_down() {
        return false;
    }
    let Ok(mut task_slot) = UPDATE_TASK.lock() else {
        return false;
    };
    if is_shutting_down() || !reap_finished_task(&mut task_slot) {
        return false;
    }
    let Ok(mut guard) = update_cache().lock() else {
        return false;
    };
    if !guard.begin_check() {
        return false;
    }
    drop(guard);
    let spawned = thread::Builder::new()
        .name(String::from("otto-update-check"))
        .spawn(move || {
            let state = check_for_updates_blocking();
            if is_shutting_down() {
                if let Ok(mut guard) = update_cache().lock() {
                    guard.in_flight = false;
                }
                return;
            }
            if let Some(label) = state.status_label() {
                crate::diagnostics::log_info("updates", &label);
            }
            if let Ok(mut guard) = update_cache().lock() {
                guard.finish_check(state);
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
            guard.finish_check(UpdateState::Failed {
                message: String::from("Check failed"),
                retry_available: None,
            });
        }
        false
    }
}

pub(super) fn start_async_update_install() -> bool {
    if is_shutting_down() {
        return false;
    }
    let Ok(mut task_slot) = UPDATE_TASK.lock() else {
        return false;
    };
    if is_shutting_down() || !reap_finished_task(&mut task_slot) {
        return false;
    }
    let info = {
        let Ok(mut guard) = update_cache().lock() else {
            return false;
        };
        let Some(info) = guard.begin_install() else {
            return false;
        };
        info
    };
    if let Ok(mut slot) = last_install_info().lock() {
        *slot = Some(info.clone());
    }
    let spawned = thread::Builder::new()
        .name(String::from("otto-update-install"))
        .spawn(move || run_install_worker(info));
    if let Ok(handle) = spawned {
        *task_slot = Some(handle);
        true
    } else {
        if let Ok(mut guard) = update_cache().lock() {
            guard.finish_install_failed("Could not start download", last_saved_info_or_empty());
        }
        false
    }
}

fn last_saved_info_or_empty() -> ReleaseUpdateInfo {
    last_install_info()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or(ReleaseUpdateInfo {
            version: String::new(),
            tag: String::new(),
            installer_name: String::new(),
            installer_url: String::new(),
            checksums_url: None,
            release_url: String::from(RELEASES_PAGE_URL),
        })
}

fn reap_finished_task(slot: &mut Option<JoinHandle<()>>) -> bool {
    if let Some(prev) = slot.take() {
        if prev.is_finished() {
            let _ = prev.join();
        } else {
            *slot = Some(prev);
            return false;
        }
    }
    true
}

pub(super) fn poll_async_update_status() -> Option<String> {
    update_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.current_label())
}

pub(super) fn poll_async_update_state() -> UpdateState {
    update_cache()
        .lock()
        .ok()
        .map_or(UpdateState::Idle, |guard| guard.current_state())
}

pub(super) fn take_ready_installer_path() -> Option<String> {
    update_cache()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take_ready_installer())
        .map(|path| path.to_string_lossy().into_owned())
}

pub(super) fn mark_update_install_failed(message: &str) {
    let saved = last_install_info()
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Ok(mut guard) = update_cache().lock() {
        if let Some(info) = saved {
            guard.finish_install_failed(message, info);
        } else {
            guard.mark_launch_failed(message);
        }
    }
}

pub(super) fn launch_staged_installer(path_str: &str) -> bool {
    let Some(dir) = updates_dir() else {
        return false;
    };
    let path = Path::new(path_str);
    if !is_staged_installer_path(path, &dir) || !path.is_file() {
        crate::diagnostics::log_error("updates", "refused to launch unverified installer path");
        return false;
    }
    match Command::new(path).args(INSTALLER_ARGS).spawn() {
        Ok(_) => {
            crate::diagnostics::log_info("updates", "launched verified update installer");
            true
        }
        Err(error) => {
            crate::diagnostics::log_error(
                "updates",
                &format!("failed to spawn installer: {error}"),
            );
            false
        }
    }
}

fn is_staged_installer_path(path: &Path, expected_dir: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if parent != expected_dir {
        return false;
    }
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name.starts_with("otto-os-setup-")
        && name.ends_with("-x64.exe")
        && !name.contains("..")
        && !name.contains(['/', '\\'])
}

pub(super) fn open_releases_page() {
    let url = update_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.state.release_page_url().map(str::to_owned))
        .filter(|candidate| is_allowed_release_page_url(candidate))
        .unwrap_or_else(|| String::from(RELEASES_PAGE_URL));
    let _ = crate::actions::open_external_url(&url);
}

fn create_http_client(accept: Option<&str>) -> Option<HttpClient> {
    let client = HttpClient::new().ok()?;
    if let Ok(headers) = client.DefaultRequestHeaders() {
        let _ = headers.TryAppendWithoutValidation(
            &HSTRING::from("User-Agent"),
            &HSTRING::from(concat!(
                "OttoOS/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/howlil/otto-os)"
            )),
        );
        if let Some(accept_header) = accept {
            let _ = headers.TryAppendWithoutValidation(
                &HSTRING::from("Accept"),
                &HSTRING::from(accept_header),
            );
        }
    }
    Some(client)
}

fn response_uses_https(response: &HttpResponseMessage) -> bool {
    response
        .RequestMessage()
        .ok()
        .and_then(|req| req.RequestUri().ok())
        .and_then(|uri| uri.SchemeName().ok())
        .is_some_and(|scheme| scheme.to_string().eq_ignore_ascii_case("https"))
}

fn response_status_code(response: &HttpResponseMessage) -> u16 {
    response
        .StatusCode()
        .ok()
        .and_then(|code| u16::try_from(code.0).ok())
        .unwrap_or(0)
}

fn check_for_updates_blocking() -> UpdateState {
    if is_shutting_down() {
        return UpdateState::Failed {
            message: String::from("Check failed"),
            retry_available: None,
        };
    }
    let _com = ComGuard::multithreaded();
    let Some(client) = create_http_client(Some("application/vnd.github+json")) else {
        return UpdateState::Failed {
            message: String::from("Check failed"),
            retry_available: None,
        };
    };
    if is_shutting_down() {
        return UpdateState::Failed {
            message: String::from("Check failed"),
            retry_available: None,
        };
    }
    let Ok(uri) = Uri::CreateUri(&HSTRING::from(RELEASES_LATEST_URL)) else {
        return UpdateState::Failed {
            message: String::from("Check failed"),
            retry_available: None,
        };
    };
    let deadline = Instant::now() + UPDATE_CHECK_TIMEOUT;
    let Some(response) = await_until!(
        client.GetWithOptionAsync(&uri, HttpCompletionOption::ResponseHeadersRead),
        deadline
    ) else {
        return UpdateState::Failed {
            message: String::from("Could not reach GitHub Releases"),
            retry_available: None,
        };
    };
    if !response_uses_https(&response) {
        return UpdateState::Failed {
            message: String::from("Insecure redirect rejected"),
            retry_available: None,
        };
    }
    let status_code = response_status_code(&response);
    if !(200..300).contains(&status_code) {
        return parse_release_update(env!("CARGO_PKG_VERSION"), status_code, None);
    }
    let body = read_text_body(&response, MAX_RELEASE_JSON_BYTES, deadline);
    parse_release_update(env!("CARGO_PKG_VERSION"), status_code, body.as_deref())
}

fn run_install_worker(info: ReleaseUpdateInfo) {
    let _com = ComGuard::multithreaded();
    let Some(dir) = updates_dir() else {
        finish_worker_error("Update folder unavailable", info);
        return;
    };
    match download_and_verify_installer(&info, &dir) {
        Ok(installer_path) => {
            if is_shutting_down() {
                let _ = fs::remove_file(&installer_path);
                if let Ok(mut guard) = update_cache().lock() {
                    guard.in_flight = false;
                }
                return;
            }
            crate::diagnostics::log_info(
                "updates",
                &format!("verified installer for v{}", info.version),
            );
            if let Ok(mut guard) = update_cache().lock() {
                guard.finish_install_ready(info.version, installer_path);
            }
        }
        Err(message) => {
            if is_shutting_down() {
                if let Ok(mut guard) = update_cache().lock() {
                    guard.in_flight = false;
                }
                return;
            }
            crate::diagnostics::log_error("updates", &format!("update failed: {message}"));
            finish_worker_error(message, info);
        }
    }
}

fn finish_worker_error(message: &str, info: ReleaseUpdateInfo) {
    if let Ok(mut guard) = update_cache().lock() {
        guard.finish_install_failed(message, info);
    }
}

fn download_and_verify_installer(
    info: &ReleaseUpdateInfo,
    dest_dir: &Path,
) -> Result<PathBuf, &'static str> {
    if !info.can_auto_install() {
        return Err("Unverified release asset");
    }
    let checksums_url = info
        .checksums_url
        .as_deref()
        .ok_or("Missing release checksums")?;
    fs::create_dir_all(dest_dir).map_err(|_| "Could not prepare updates folder")?;
    cleanup_updates_in(dest_dir);

    let client = create_http_client(None).ok_or("Could not initialize HTTP client")?;
    let expected_hash = fetch_expected_checksum(&client, checksums_url, &info.installer_name)?;

    let uri = Uri::CreateUri(&HSTRING::from(info.installer_url.as_str()))
        .map_err(|_| "Invalid installer URL")?;
    let deadline = Instant::now() + UPDATE_DOWNLOAD_TIMEOUT;
    let response = await_until!(
        client.GetWithOptionAsync(&uri, HttpCompletionOption::ResponseHeadersRead),
        deadline
    )
    .ok_or("Installer download timed out")?;
    if !response_uses_https(&response) {
        return Err("Insecure download redirect rejected");
    }
    if !(200..300).contains(&response_status_code(&response)) {
        return Err("Installer download failed");
    }
    let content = response
        .Content()
        .map_err(|_| "Missing installer response")?;
    let mut declared_length = 0u64;
    let has_length = matches!(content.TryComputeLength(&mut declared_length), Ok(true));
    if has_length && declared_length > MAX_INSTALLER_BYTES {
        return Err("Installer exceeds size limit");
    }
    let stream = await_until!(content.ReadAsInputStreamAsync(), deadline)
        .ok_or("Could not read installer stream")?;

    let part_path = dest_dir.join(format!("{}.part", info.installer_name));
    let final_path = dest_dir.join(&info.installer_name);
    let mut guard = TempPartGuard::new(part_path.clone());
    let declared = has_length.then_some(declared_length);
    let (header_ok, actual_hash) =
        stream_installer_file(&stream, &part_path, &info.version, declared, deadline)?;

    if let Ok(mut cache) = update_cache().lock() {
        cache.set_verifying(&info.version);
    }
    commit_verified_installer(
        &part_path,
        &final_path,
        header_ok,
        actual_hash,
        expected_hash,
    )?;
    guard.defuse();
    Ok(final_path)
}

fn stream_installer_file(
    stream: &windows::Storage::Streams::IInputStream,
    part_path: &Path,
    version: &str,
    declared_length: Option<u64>,
    deadline: Instant,
) -> Result<(bool, [u8; 32]), &'static str> {
    let mut file = File::create(part_path).map_err(|_| "Could not write installer file")?;
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let mut last_percent: Option<u8> = Some(0);
    let mut magic = [0u8; 2];
    let mut magic_len = 0usize;

    let copied = copy_stream_bounded(
        stream,
        MAX_INSTALLER_BYTES,
        deadline,
        &is_shutting_down,
        |chunk| {
            if file.write_all(chunk).is_err() {
                return false;
            }
            if magic_len < 2 {
                let take = (2 - magic_len).min(chunk.len());
                magic[magic_len..magic_len + take].copy_from_slice(&chunk[..take]);
                magic_len += take;
            }
            hasher.update(chunk);
            written = written.saturating_add(chunk.len() as u64);
            let next_percent = declared_length.filter(|&total| total > 0).map(|total| {
                u8::try_from((written.saturating_mul(100) / total).min(100)).unwrap_or(100)
            });
            if next_percent != last_percent {
                last_percent = next_percent;
                if let Ok(mut cache) = update_cache().lock() {
                    cache.set_download_progress(version, next_percent);
                }
            }
            true
        },
    );
    if copied.is_none() {
        return Err("Installer download interrupted");
    }
    file.flush().map_err(|_| "Could not flush installer file")?;
    file.sync_all()
        .map_err(|_| "Could not sync installer file")?;
    Ok((magic_len == 2 && magic == *b"MZ", hasher.finalize()))
}

fn commit_verified_installer(
    part_path: &Path,
    final_path: &Path,
    header_ok: bool,
    actual_hash: [u8; 32],
    expected_hash: [u8; 32],
) -> Result<(), &'static str> {
    if !header_ok {
        return Err("Downloaded file is not a valid Windows installer");
    }
    if actual_hash != expected_hash {
        return Err("SHA-256 checksum mismatch");
    }
    move_file_atomically(part_path, final_path).map_err(|_| "Could not stage installer")
}

fn fetch_expected_checksum(
    client: &HttpClient,
    checksums_url: &str,
    installer_name: &str,
) -> Result<[u8; 32], &'static str> {
    let uri = Uri::CreateUri(&HSTRING::from(checksums_url)).map_err(|_| "Invalid checksum URL")?;
    let deadline = Instant::now() + UPDATE_CHECKSUM_TIMEOUT;
    let response = await_until!(
        client.GetWithOptionAsync(&uri, HttpCompletionOption::ResponseHeadersRead),
        deadline
    )
    .ok_or("Could not fetch release checksums")?;
    if !response_uses_https(&response) {
        return Err("Insecure checksum redirect rejected");
    }
    if !(200..300).contains(&response_status_code(&response)) {
        return Err("Could not download release checksums");
    }
    let body = read_text_body(&response, MAX_CHECKSUMS_BYTES, deadline)
        .ok_or("Could not read release checksums")?;
    parse_sha256_for_asset(&body, installer_name).ok_or("Installer missing from SHA256SUMS.txt")
}

fn read_text_body(
    response: &HttpResponseMessage,
    limit: usize,
    deadline: Instant,
) -> Option<String> {
    let content = response.Content().ok()?;
    let mut declared_length = 0u64;
    if matches!(content.TryComputeLength(&mut declared_length), Ok(true))
        && declared_length > limit as u64
    {
        return None;
    }
    let stream = await_until!(content.ReadAsInputStreamAsync(), deadline)?;
    let bytes = read_stream_bounded(&stream, limit, deadline, &is_shutting_down)?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{
        TempPartGuard, UPDATE_TASK, UpdateCache, cleanup_updates_in, commit_verified_installer,
        is_staged_installer_path, resume_update_check_task, shutdown_update_check_task,
    };
    use crate::control_center::WindowsControlCenter;
    use otto_core::{
        ControlCenterPort, ReleaseUpdateInfo, UPDATE_CHECKING_LABEL, UpdateState, sha256_digest,
    };

    fn sample_release() -> ReleaseUpdateInfo {
        ReleaseUpdateInfo {
            version: String::from("1.2.1"),
            tag: String::from("v1.2.1"),
            installer_name: String::from("otto-os-setup-1.2.1-x64.exe"),
            installer_url: String::from(
                "https://github.com/howlil/otto-os/releases/download/v1.2.1/otto-os-setup-1.2.1-x64.exe",
            ),
            checksums_url: Some(String::from(
                "https://github.com/howlil/otto-os/releases/download/v1.2.1/SHA256SUMS.txt",
            )),
            release_url: String::from("https://github.com/howlil/otto-os/releases/tag/v1.2.1"),
        }
    }

    #[test]
    fn update_cache_guards_concurrent_checks_installs_and_retries() {
        let mut cache = UpdateCache::default();
        assert_eq!(cache.current_label(), None);
        assert!(cache.begin_check());
        assert_eq!(
            cache.current_label().as_deref(),
            Some(UPDATE_CHECKING_LABEL)
        );
        assert!(!cache.begin_check());
        assert!(cache.begin_install().is_none());

        let release = sample_release();
        cache.finish_check(UpdateState::Available(release.clone()));
        assert_eq!(cache.current_label().as_deref(), Some("Update v1.2.1"));

        let started = cache.begin_install().expect("install starts");
        assert_eq!(started, release);
        assert!(!cache.begin_check());
        assert!(cache.begin_install().is_none());

        cache.set_download_progress("1.2.1", Some(42));
        assert_eq!(
            cache.current_state(),
            UpdateState::Downloading {
                version: String::from("1.2.1"),
                percent: Some(42),
            }
        );
        cache.set_verifying("1.2.1");
        assert_eq!(
            cache.current_state(),
            UpdateState::Verifying {
                version: String::from("1.2.1"),
            }
        );

        cache.finish_install_failed("Checksum mismatch", release.clone());
        assert!(cache.take_ready_installer().is_none());
        assert!(cache.begin_install().is_some());

        let staged = PathBuf::from(r"C:\Temp\otto-os-setup-1.2.1-x64.exe");
        cache.finish_install_ready(String::from("1.2.1"), staged.clone());
        assert_eq!(cache.take_ready_installer(), Some(staged));
        assert_eq!(cache.take_ready_installer(), None);
    }

    #[test]
    fn staged_installer_verification_commits_only_valid_matching_pe_files() {
        let dir = std::env::temp_dir().join(format!("otto-os-update-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");

        let part = dir.join("otto-os-setup-1.2.1-x64.exe.part");
        let final_path = dir.join("otto-os-setup-1.2.1-x64.exe");
        let payload = b"MZvalid-installer-bytes";
        let expected = sha256_digest(payload);

        fs::write(&part, b"NOPE").expect("write invalid magic");
        {
            let _guard = TempPartGuard::new(part.clone());
            assert!(
                commit_verified_installer(&part, &final_path, false, expected, expected).is_err()
            );
        }
        assert!(!part.exists());
        assert!(!final_path.exists());

        fs::write(&part, payload).expect("write mismatched payload");
        {
            let _guard = TempPartGuard::new(part.clone());
            let mut wrong = expected;
            wrong[0] ^= 0xFF;
            assert!(commit_verified_installer(&part, &final_path, true, expected, wrong).is_err());
        }
        assert!(!part.exists());
        assert!(!final_path.exists());

        fs::write(&part, payload).expect("write valid payload");
        {
            let mut guard = TempPartGuard::new(part.clone());
            commit_verified_installer(&part, &final_path, true, expected, expected)
                .expect("commit verified installer");
            guard.defuse();
        }
        assert!(!part.exists());
        assert_eq!(fs::read(&final_path).expect("read committed"), payload);
        assert!(is_staged_installer_path(&final_path, &dir));
        assert!(!is_staged_installer_path(&dir.join("evil.exe"), &dir));
        assert!(!is_staged_installer_path(
            &dir.join("nested").join("otto-os-setup-1.2.1-x64.exe"),
            &dir
        ));

        cleanup_updates_in(&dir);
        assert!(!final_path.exists());
        let _ = fs::remove_dir_all(&dir);
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
        assert!(!WindowsControlCenter.start_update_install());
        assert!(UPDATE_TASK.lock().unwrap().is_none());
    }
}
