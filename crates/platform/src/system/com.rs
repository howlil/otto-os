use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

pub struct ComGuard;

impl ComGuard {
    #[must_use]
    pub fn multithreaded() -> Option<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .is_ok()
            .then_some(Self)
    }

    pub(crate) fn single_threaded() -> Option<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .is_ok()
            .then_some(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub struct OleGuard;

impl OleGuard {
    #[must_use]
    pub fn acquire() -> Option<Self> {
        unsafe { OleInitialize(None) }.is_ok().then_some(Self)
    }
}

impl Drop for OleGuard {
    fn drop(&mut self) {
        unsafe {
            OleUninitialize();
        }
    }
}
