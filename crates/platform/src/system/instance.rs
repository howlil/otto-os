use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, E_FAIL, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE, HWND,
    LPARAM, WPARAM,
};
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcessId, OpenMutexW, SYNCHRONIZATION_ACCESS_RIGHTS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
};
use windows::core::{Error, PCWSTR, Result, w};

const INSTANCE_MUTEX_NAME: PCWSTR = w!("Local\\OttoNotchBar");
const BAR_WINDOW_CLASS: PCWSTR = w!("OttoNotchBar");
const QUIT_TIMEOUT: Duration = Duration::from_secs(5);
const QUIT_POLL_INTERVAL: Duration = Duration::from_millis(25);
const SYNCHRONIZE_ACCESS: SYNCHRONIZATION_ACCESS_RIGHTS =
    SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000);

#[derive(Debug)]
pub struct InstanceGuard(HANDLE);

impl InstanceGuard {
    pub fn acquire() -> Result<Option<Self>> {
        Self::acquire_named(INSTANCE_MUTEX_NAME)
    }

    fn acquire_named(name: PCWSTR) -> Result<Option<Self>> {
        unsafe {
            let created = CreateMutexW(None, true, name);
            let already_exists = GetLastError() == ERROR_ALREADY_EXISTS;
            match created {
                Ok(handle) if already_exists => {
                    let _ = CloseHandle(handle);
                    Ok(None)
                }
                Ok(handle) => Ok(Some(Self(handle))),
                Err(error) => Err(error),
            }
        }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub fn quit_running_instance() -> Result<()> {
    let exited = quit_named_instance(INSTANCE_MUTEX_NAME, Some(BAR_WINDOW_CLASS), QUIT_TIMEOUT)?;
    let _ = crate::appbar::restore_shell_from_recovery();
    if exited {
        crate::diagnostics::log_info("app", "closed running instance via --quit");
        Ok(())
    } else {
        crate::diagnostics::log_error("app", "timed out waiting for --quit");
        Err(Error::new(
            E_FAIL,
            "Timed out waiting for the running Otto OS instance to exit",
        ))
    }
}

fn quit_named_instance(
    mutex_name: PCWSTR,
    window_class: Option<PCWSTR>,
    timeout: Duration,
) -> Result<bool> {
    let deadline = Instant::now() + timeout;
    let mut posted_hwnd: Option<isize> = None;
    loop {
        let running = mutex_exists(mutex_name)?;
        let window = window_class.and_then(find_foreign_window);
        if !running && window.is_none() {
            return Ok(true);
        }
        if let Some(hwnd) = window {
            let raw = hwnd.0 as isize;
            if posted_hwnd != Some(raw) {
                let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
                posted_hwnd = Some(raw);
            }
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(QUIT_POLL_INTERVAL);
    }
}

fn mutex_exists(name: PCWSTR) -> Result<bool> {
    match unsafe { OpenMutexW(SYNCHRONIZE_ACCESS, false, name) } {
        Ok(handle) => {
            let _ = unsafe { CloseHandle(handle) };
            Ok(true)
        }
        Err(error) if error.code() == ERROR_FILE_NOT_FOUND.to_hresult() => Ok(false),
        Err(error) => Err(error),
    }
}

fn find_foreign_window(window_class: PCWSTR) -> Option<HWND> {
    let hwnd = unsafe { FindWindowW(window_class, PCWSTR::null()) }.ok()?;
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&raw mut process_id));
    }
    (process_id != 0 && process_id != unsafe { GetCurrentProcessId() }).then_some(hwnd)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{InstanceGuard, quit_named_instance};
    use windows::core::w;

    #[test]
    fn the_first_owner_gets_the_lock_and_a_second_attempt_sees_it_taken() {
        let name = w!("Local\\OttoE2ETestInstanceGuardOwnership");
        let first = InstanceGuard::acquire_named(name).expect("create the mutex");
        assert!(first.is_some());
        let second = InstanceGuard::acquire_named(name).expect("open the existing mutex");
        assert!(second.is_none());
    }

    #[test]
    fn dropping_the_owner_releases_the_lock() {
        let name = w!("Local\\OttoE2ETestInstanceGuardRelease");
        let first = InstanceGuard::acquire_named(name).expect("create the mutex");
        assert!(first.is_some());
        drop(first);
        let again = InstanceGuard::acquire_named(name).expect("create the mutex again");
        assert!(again.is_some());
    }

    #[test]
    fn a_name_the_system_rejects_is_an_error_not_a_taken_lock() {
        let name = w!("Local\\OttoE2ETest\\nested\\name");
        assert!(InstanceGuard::acquire_named(name).is_err());
    }

    #[test]
    fn quit_succeeds_immediately_when_no_instance_exists() {
        let name = w!("Local\\OttoE2ETestQuitAbsent");
        assert_eq!(
            quit_named_instance(name, None, Duration::from_millis(50)),
            Ok(true)
        );
    }

    #[test]
    fn quit_times_out_when_lock_stays_held_without_window() {
        let name = w!("Local\\OttoE2ETestQuitHeld");
        let guard = InstanceGuard::acquire_named(name).expect("create the mutex");
        assert!(guard.is_some());
        assert_eq!(
            quit_named_instance(name, None, Duration::from_millis(50)),
            Ok(false)
        );
    }
}
