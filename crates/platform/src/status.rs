use std::mem::size_of;
use std::sync::atomic::{AtomicU8, Ordering};

use otto_core::{BatteryStatus, ClockTime, SystemStatus};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
use windows::core::w;

const BATTERY_FLAG_NO_BATTERY: u8 = 128;
const BATTERY_FLAG_CHARGING: u8 = 8;

const IOCTL_VIDEO_QUERY_SUPPORTED_BRIGHTNESS: u32 = 0x0023_0494;
const IOCTL_VIDEO_QUERY_DISPLAY_BRIGHTNESS: u32 = 0x0023_0498;
const IOCTL_VIDEO_SET_DISPLAY_BRIGHTNESS: u32 = 0x0023_049C;

static FALLBACK_VOLUME: AtomicU8 = AtomicU8::new(50);
static FALLBACK_BRIGHTNESS: AtomicU8 = AtomicU8::new(80);

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct DisplayBrightness {
    policy: u8,
    ac_percent: u8,
    dc_percent: u8,
}

#[must_use]
pub fn read_system_status() -> SystemStatus {
    unsafe {
        let mut power = SYSTEM_POWER_STATUS::default();
        let battery = if GetSystemPowerStatus(&mut power).is_ok() {
            let no_battery =
                power.BatteryFlag & BATTERY_FLAG_NO_BATTERY != 0 || power.BatteryLifePercent == 255;
            let charging =
                power.ACLineStatus == 1 || (power.BatteryFlag & BATTERY_FLAG_CHARGING != 0);
            BatteryStatus {
                percent: (!no_battery).then_some(power.BatteryLifePercent.min(100)),
                charging,
            }
        } else {
            BatteryStatus::default()
        };

        let now = GetLocalTime();
        let clock = ClockTime {
            year: now.wYear,
            weekday: now.wDayOfWeek as u8,
            day: now.wDay as u8,
            month: now.wMonth as u8,
            hour: now.wHour as u8,
            minute: now.wMinute as u8,
        };

        let (volume, muted) = read_volume_and_mute();
        let brightness = read_brightness();
        let caps_lock = (GetKeyState(i32::from(VK_CAPITAL.0)) & 1) != 0;

        SystemStatus {
            battery,
            clock,
            volume,
            muted,
            brightness,
            caps_lock,
        }
    }
}

#[must_use]
pub fn step_system_volume(wheel_delta: i16) -> Option<(u8, bool)> {
    let step: i32 = match wheel_delta.cmp(&0) {
        std::cmp::Ordering::Greater => 5,
        std::cmp::Ordering::Less => -5,
        std::cmp::Ordering::Equal => return None,
    };
    let current = with_audio_endpoint(|endpoint| {
        let current = unsafe { endpoint.GetMasterVolumeLevelScalar() }
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        Some(current)
    })?;
    let current_pct = (current * 100.0).round() as i32;
    set_system_volume((current_pct + step).clamp(0, 100) as u8)
}

#[must_use]
pub fn set_system_volume(percent: u8) -> Option<(u8, bool)> {
    let next_pct = percent.min(100);
    with_audio_endpoint(|endpoint| unsafe {
        let _ = endpoint.SetMasterVolumeLevelScalar(f32::from(next_pct) / 100.0, std::ptr::null());
        let muted = next_pct == 0;
        let _ = endpoint.SetMute(muted, std::ptr::null());
        FALLBACK_VOLUME.store(next_pct, Ordering::Relaxed);
        Some((next_pct, muted))
    })
}

#[must_use]
pub fn toggle_system_mute() -> Option<(u8, bool)> {
    with_audio_endpoint(|endpoint| unsafe {
        let current_mute = endpoint
            .GetMute()
            .unwrap_or(windows::core::BOOL(0))
            .as_bool();
        let next_mute = !current_mute;
        let _ = endpoint.SetMute(next_mute, std::ptr::null());
        let current = endpoint.GetMasterVolumeLevelScalar().unwrap_or(0.5);
        let pct = (current * 100.0).round() as u8;
        Some((pct, next_mute))
    })
}

#[must_use]
pub fn step_display_brightness(wheel_delta: i16) -> u8 {
    let step: i32 = match wheel_delta.cmp(&0) {
        std::cmp::Ordering::Greater => 5,
        std::cmp::Ordering::Less => -5,
        std::cmp::Ordering::Equal => return read_brightness(),
    };
    let current = i32::from(read_brightness());
    set_display_brightness((current + step).clamp(0, 100) as u8)
}

#[must_use]
pub fn set_display_brightness(percent: u8) -> u8 {
    let target = percent.min(100);
    FALLBACK_BRIGHTNESS.store(target, Ordering::Relaxed);

    if let Some(handle) = open_lcd_device() {
        unsafe {
            let snapped = snap_supported_brightness(handle, target);
            let payload = DisplayBrightness {
                policy: 3,
                ac_percent: snapped,
                dc_percent: snapped,
            };
            let mut returned = 0u32;
            let _ = DeviceIoControl(
                handle,
                IOCTL_VIDEO_SET_DISPLAY_BRIGHTNESS,
                Some((&raw const payload).cast()),
                size_of::<DisplayBrightness>() as u32,
                None,
                0,
                Some(&raw mut returned),
                None,
            );
            let _ = CloseHandle(handle);
            FALLBACK_BRIGHTNESS.store(snapped, Ordering::Relaxed);
            return snapped;
        }
    }
    target
}

fn read_volume_and_mute() -> (u8, bool) {
    let Some((pct, muted)) = with_audio_endpoint(|endpoint| unsafe {
        let scalar = endpoint
            .GetMasterVolumeLevelScalar()
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        let muted = endpoint.GetMute().is_ok_and(windows::core::BOOL::as_bool);
        let pct = (scalar * 100.0).round() as u8;
        Some((pct, muted))
    }) else {
        return (FALLBACK_VOLUME.load(Ordering::Relaxed), false);
    };
    FALLBACK_VOLUME.store(pct, Ordering::Relaxed);
    (pct, muted)
}

fn read_brightness() -> u8 {
    let Some(handle) = open_lcd_device() else {
        return FALLBACK_BRIGHTNESS.load(Ordering::Relaxed);
    };
    unsafe {
        let mut info = DisplayBrightness::default();
        let mut returned = 0u32;
        let ok = DeviceIoControl(
            handle,
            IOCTL_VIDEO_QUERY_DISPLAY_BRIGHTNESS,
            None,
            0,
            Some((&raw mut info).cast()),
            size_of::<DisplayBrightness>() as u32,
            Some(&raw mut returned),
            None,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if ok && returned >= size_of::<DisplayBrightness>() as u32 {
            let level = if info.policy == 1 {
                info.ac_percent
            } else {
                info.dc_percent.max(info.ac_percent)
            }
            .min(100);
            FALLBACK_BRIGHTNESS.store(level, Ordering::Relaxed);
            level
        } else {
            FALLBACK_BRIGHTNESS.load(Ordering::Relaxed)
        }
    }
}

fn open_lcd_device() -> Option<HANDLE> {
    unsafe {
        CreateFileW(
            w!("\\\\.\\LCD"),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .ok()
    }
}

unsafe fn snap_supported_brightness(handle: HANDLE, target: u8) -> u8 {
    let mut levels = [0u8; 256];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_VIDEO_QUERY_SUPPORTED_BRIGHTNESS,
            None,
            0,
            Some(levels.as_mut_ptr().cast()),
            levels.len() as u32,
            Some(&raw mut returned),
            None,
        )
        .is_ok()
    };
    if !ok || returned == 0 {
        return target;
    }
    levels[..returned as usize]
        .iter()
        .copied()
        .min_by_key(|&lvl| (i32::from(lvl) - i32::from(target)).abs())
        .unwrap_or(target)
}

struct ComApartmentGuard;

impl ComApartmentGuard {
    fn new() -> Option<Self> {
        if unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_err() {
            return None;
        }
        Some(Self)
    }
}

impl Drop for ComApartmentGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn with_audio_endpoint<T>(operation: impl FnOnce(&IAudioEndpointVolume) -> Option<T>) -> Option<T> {
    let _com = ComApartmentGuard::new()?;
    let endpoint = unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        device.Activate(CLSCTX_INPROC_SERVER, None).ok()?
    };
    operation(&endpoint)
}
