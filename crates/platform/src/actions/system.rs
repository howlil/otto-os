use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
use windows::core::w;

pub fn signal_command_failure() {
    unsafe {
        let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(
            windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
        );
    }
}

pub fn trim_working_set() {
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

pub fn play_pomodoro_chime() {
    unsafe {
        let _ = PlaySoundW(
            w!("SystemNotification"),
            None,
            SND_ALIAS | SND_ASYNC | SND_NODEFAULT,
        );
    }
}
