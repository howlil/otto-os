use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::{PCWSTR, Result, w};

#[derive(Debug)]
pub struct InstanceGuard(HANDLE);

impl InstanceGuard {
    pub fn acquire() -> Result<Option<Self>> {
        Self::acquire_named(w!("Local\\OttoNotchBar"))
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

#[cfg(test)]
mod tests {
    use super::InstanceGuard;
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
}
