use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

#[derive(Debug)]
pub struct InstanceGuard(HANDLE);

impl InstanceGuard {
    #[must_use]
    pub fn acquire() -> Option<Self> {
        unsafe {
            let handle = CreateMutexW(None, true, w!("Local\\OttoNotchBar")).ok()?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Self(handle))
        }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}
