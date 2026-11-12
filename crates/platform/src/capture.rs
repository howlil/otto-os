use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture};

#[must_use]
const fn owns_capture(owner: isize, hwnd: isize) -> bool {
    hwnd != 0 && owner == hwnd
}

#[must_use]
pub fn release_capture_if_owned(hwnd: isize) -> bool {
    let owner = unsafe { GetCapture() }.0 as isize;
    if !owns_capture(owner, hwnd) {
        return false;
    }
    unsafe { ReleaseCapture() }.is_ok()
}

#[cfg(test)]
mod tests {
    use super::owns_capture;

    #[test]
    fn only_the_window_holding_capture_releases_it() {
        assert!(owns_capture(42, 42));
        assert!(!owns_capture(42, 7));
        assert!(!owns_capture(0, 42));
        assert!(!owns_capture(0, 0));
        assert!(!owns_capture(42, 0));
    }
}
