use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{BatteryStatus, ClockTime, InputLangCode, PowerMode, SystemStatus};
use windows::UI::Shell::FocusSessionManager;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR, SetMonitorBrightness,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, POINT};
use windows::Win32::Globalization::{GetLocaleInfoW, LOCALE_SISO639LANGNAME};
use windows::Win32::Graphics::Gdi::{HMONITOR, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, VK_CAPITAL, VK_NUMLOCK,
};
use windows::Win32::UI::Shell::{
    QUERY_USER_NOTIFICATION_STATE, QUNS_ACCEPTS_NOTIFICATIONS, QUNS_QUIET_TIME,
    SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::w;

const BATTERY_FLAG_NO_BATTERY: u8 = 128;
const BATTERY_FLAG_CHARGING: u8 = 8;

const IOCTL_VIDEO_QUERY_SUPPORTED_BRIGHTNESS: u32 = 0x0023_0494;
const IOCTL_VIDEO_QUERY_DISPLAY_BRIGHTNESS: u32 = 0x0023_0498;
const IOCTL_VIDEO_SET_DISPLAY_BRIGHTNESS: u32 = 0x0023_049C;

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
        let power_ok = GetSystemPowerStatus(&mut power).is_ok();
        let battery = if power_ok {
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
        let power_mode = if power_ok {
            PowerMode::from_battery_saver(crate::theme::battery_saver_from_flag(
                power.SystemStatusFlag,
            ))
        } else {
            PowerMode::default()
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

        let (volume, muted) = crate::bar::audio::read_volume_and_mute();
        let brightness = read_brightness();
        let caps_lock = (GetKeyState(i32::from(VK_CAPITAL.0)) & 1) != 0;
        let num_lock = (GetKeyState(i32::from(VK_NUMLOCK.0)) & 1) != 0;
        let input_lang = read_foreground_input_lang();
        let dnd_active = read_system_dnd_active();

        SystemStatus {
            battery,
            clock,
            volume,
            muted,
            brightness,
            caps_lock,
            num_lock,
            power_mode,
            input_lang,
            dnd_active,
        }
    }
}

const DND_CACHE_TTL: Duration = Duration::from_secs(2);
type DndCacheEntry = Option<(Instant, Option<bool>)>;
static DND_CACHE: OnceLock<Mutex<DndCacheEntry>> = OnceLock::new();

const fn quns_to_dnd(state: QUERY_USER_NOTIFICATION_STATE) -> Option<bool> {
    if state.0 == QUNS_QUIET_TIME.0 {
        Some(true)
    } else if state.0 == QUNS_ACCEPTS_NOTIFICATIONS.0 {
        Some(false)
    } else {
        None
    }
}

const fn combine_dnd_state(focus_session: Option<bool>, quns_dnd: Option<bool>) -> Option<bool> {
    match (focus_session, quns_dnd) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), _) | (_, Some(false)) => Some(false),
        (None, None) => None,
    }
}

#[must_use]
pub fn read_system_dnd_active() -> Option<bool> {
    let cache = DND_CACHE.get_or_init(|| Mutex::new(None));
    let now = Instant::now();
    if let Ok(guard) = cache.lock()
        && let Some((cached_at, cached)) = *guard
        && now.saturating_duration_since(cached_at) < DND_CACHE_TTL
    {
        return cached;
    }

    let quns_dnd = unsafe { SHQueryUserNotificationState() }
        .ok()
        .and_then(quns_to_dnd);
    let _com = crate::com::ComGuard::multithreaded();
    let focus_session = FocusSessionManager::IsSupported()
        .ok()
        .filter(|supported| *supported)
        .and_then(|_| FocusSessionManager::GetDefault().ok())
        .and_then(|manager| manager.IsFocusActive().ok());
    let value = combine_dnd_state(focus_session, quns_dnd);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((Instant::now(), value));
    }
    value
}

fn input_lang_from_lang_id(lang_id: u32) -> InputLangCode {
    if lang_id == 0 {
        return InputLangCode::empty();
    }
    let mut buf = [0u16; 16];
    let written = unsafe { GetLocaleInfoW(lang_id, LOCALE_SISO639LANGNAME, Some(&mut buf)) };
    if written <= 1 {
        return InputLangCode::empty();
    }
    let slice = &buf[..(written as usize - 1).min(buf.len())];
    let mut ascii = [0u8; 16];
    let mut len = 0usize;
    for &unit in slice {
        if unit > 0x7F {
            let tag = String::from_utf16_lossy(slice);
            return InputLangCode::from_tag(&tag);
        }
        ascii[len] = unit as u8;
        len += 1;
    }
    let tag = std::str::from_utf8(&ascii[..len]).unwrap_or("");
    InputLangCode::from_tag(tag)
}

fn read_foreground_input_lang() -> InputLangCode {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return InputLangCode::empty();
        }
        let thread_id = GetWindowThreadProcessId(foreground, None);
        if thread_id == 0 {
            return InputLangCode::empty();
        }
        let hkl = GetKeyboardLayout(thread_id);
        if hkl.is_invalid() {
            return InputLangCode::empty();
        }
        let lang_id = (hkl.0 as usize & 0xFFFF) as u32;
        input_lang_from_lang_id(lang_id)
    }
}

#[must_use]
pub fn step_display_brightness(wheel_delta: i16) -> Option<u8> {
    let step: i32 = match wheel_delta.cmp(&0) {
        std::cmp::Ordering::Greater => 5,
        std::cmp::Ordering::Less => -5,
        std::cmp::Ordering::Equal => return read_brightness(),
    };
    let current = i32::from(read_brightness()?);
    set_display_brightness((current + step).clamp(0, 100) as u8)
}

#[must_use]
pub fn set_display_brightness(percent: u8) -> Option<u8> {
    let target = percent.min(100);
    if let Some(snapped) = set_lcd_brightness(target) {
        return Some(snapped);
    }
    set_ddc_brightness(target)
}

fn set_lcd_brightness(target: u8) -> Option<u8> {
    let handle = open_lcd_device()?;
    unsafe {
        let snapped = snap_supported_brightness(handle, target);
        let payload = DisplayBrightness {
            policy: 3,
            ac_percent: snapped,
            dc_percent: snapped,
        };
        let mut returned = 0u32;
        let ok = DeviceIoControl(
            handle,
            IOCTL_VIDEO_SET_DISPLAY_BRIGHTNESS,
            Some((&raw const payload).cast()),
            size_of::<DisplayBrightness>() as u32,
            None,
            0,
            Some(&raw mut returned),
            None,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        ok.then_some(snapped)
    }
}

fn read_brightness() -> Option<u8> {
    if let Some(level) = read_lcd_brightness() {
        return Some(level);
    }
    read_ddc_brightness_cached()
}

fn read_lcd_brightness() -> Option<u8> {
    let handle = open_lcd_device()?;
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
            Some(level)
        } else {
            None
        }
    }
}

const DDC_SUPPORTED_TTL: Duration = Duration::from_secs(15);
const DDC_UNSUPPORTED_TTL: Duration = Duration::from_mins(1);
const DDC_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const LCD_ABSENT_TTL: Duration = Duration::from_mins(1);

#[derive(Default)]
struct DdcCacheState {
    monitor: isize,
    supported: Option<bool>,
    percent: Option<u8>,
    checked_at: Option<Instant>,
    generation: u64,
}

impl DdcCacheState {
    fn store_probe(
        &mut self,
        expected_generation: u64,
        monitor: isize,
        result: Option<u8>,
        now: Instant,
    ) -> bool {
        if self.generation != expected_generation {
            return false;
        }
        self.monitor = monitor;
        self.supported = Some(result.is_some());
        self.percent = result;
        self.checked_at = Some(now);
        true
    }

    fn store_set(&mut self, monitor: isize, target: u8, ok: bool, now: Instant) {
        self.generation = self.generation.wrapping_add(1);
        self.monitor = monitor;
        self.supported = Some(ok);
        self.percent = ok.then_some(target);
        self.checked_at = Some(now);
    }

    fn mark_async_set_failed(&mut self, expected_generation: u64, monitor: isize, now: Instant) {
        if self.generation == expected_generation && self.monitor == monitor {
            self.supported = Some(false);
            self.percent = None;
            self.checked_at = Some(now);
        }
    }
}

#[derive(Default)]
struct DdcWriteMailbox {
    pending: Option<(isize, u8, u64)>,
    stopping: bool,
}

struct DdcWriteShared {
    mailbox: Mutex<DdcWriteMailbox>,
    wake: Condvar,
}

struct DdcProbeGuard;

impl Drop for DdcProbeGuard {
    fn drop(&mut self) {
        DDC_PROBING.store(false, Ordering::Release);
    }
}

static DDC_CACHE: OnceLock<Mutex<DdcCacheState>> = OnceLock::new();
static LCD_ABSENT_AT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static DDC_WRITER_SHARED: OnceLock<DdcWriteShared> = OnceLock::new();
static DDC_PROBING: AtomicBool = AtomicBool::new(false);
static DDC_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static DDC_PROBE_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static DDC_WRITER_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

fn ddc_cache() -> &'static Mutex<DdcCacheState> {
    DDC_CACHE.get_or_init(|| Mutex::new(DdcCacheState::default()))
}

fn lcd_absent_cache() -> &'static Mutex<Option<Instant>> {
    LCD_ABSENT_AT.get_or_init(|| Mutex::new(None))
}

fn ddc_writer_shared() -> &'static DdcWriteShared {
    DDC_WRITER_SHARED.get_or_init(|| DdcWriteShared {
        mailbox: Mutex::new(DdcWriteMailbox::default()),
        wake: Condvar::new(),
    })
}

pub fn shutdown_status_task() {
    DDC_SHUTTING_DOWN.store(true, Ordering::Release);
    if let Some(shared) = DDC_WRITER_SHARED.get() {
        let mut mailbox = shared
            .mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        mailbox.stopping = true;
        mailbox.pending = None;
        shared.wake.notify_all();
    }
    let probe_handle = DDC_PROBE_TASK.lock().ok().and_then(|mut task| task.take());
    if let Some(handle) = probe_handle {
        let _ = crate::worker::join_within(handle, DDC_SHUTDOWN_TIMEOUT);
    }
    let writer_handle = DDC_WRITER_TASK.lock().ok().and_then(|mut task| task.take());
    if let Some(handle) = writer_handle {
        let _ = crate::worker::join_within(handle, DDC_SHUTDOWN_TIMEOUT);
    }
}

pub(crate) fn resume_status_task() {
    DDC_SHUTTING_DOWN.store(false, Ordering::Release);
    if let Ok(mut absent) = lcd_absent_cache().lock() {
        *absent = None;
    }
    if let Some(shared) = DDC_WRITER_SHARED.get() {
        let mut mailbox = shared
            .mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        mailbox.stopping = false;
    }
}

fn primary_monitor() -> Option<HMONITOR> {
    let monitor = unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) };
    (!monitor.0.is_null()).then_some(monitor)
}

fn ddc_cache_is_fresh(state: &DdcCacheState, monitor_key: isize) -> bool {
    let Some(checked_at) = state.checked_at else {
        return false;
    };
    if state.monitor != monitor_key {
        return false;
    }
    let ttl = match state.supported {
        Some(true) => DDC_SUPPORTED_TTL,
        Some(false) => DDC_UNSUPPORTED_TTL,
        None => return false,
    };
    checked_at.elapsed() < ttl
}

fn read_ddc_brightness_cached() -> Option<u8> {
    if DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
        return None;
    }
    let monitor = primary_monitor()?;
    let monitor_key = monitor.0 as isize;
    let (cached, needs_probe, probe_generation) = {
        let state = ddc_cache().lock().ok()?;
        let same_monitor = state.monitor == monitor_key;
        let fresh = ddc_cache_is_fresh(&state, monitor_key);
        let value = if same_monitor { state.percent } else { None };
        (value, !fresh, state.generation)
    };
    if needs_probe
        && !DDC_SHUTTING_DOWN.load(Ordering::Acquire)
        && DDC_PROBING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let probe_guard = DdcProbeGuard;
        if DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
            return cached;
        }
        let Ok(mut task_slot) = DDC_PROBE_TASK.lock() else {
            return cached;
        };
        if DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
            return cached;
        }
        if let Some(prev) = task_slot.take() {
            if prev.is_finished() {
                let _ = prev.join();
            } else {
                *task_slot = Some(prev);
                return cached;
            }
        }
        let raw_monitor = monitor.0 as isize;
        let spawned = thread::Builder::new()
            .name(String::from("otto-ddc-probe"))
            .spawn(move || {
                let _probe_guard = probe_guard;
                if DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
                    return;
                }
                let hmon = HMONITOR(raw_monitor as *mut std::ffi::c_void);
                let result = read_ddc_brightness_native(hmon);
                if !DDC_SHUTTING_DOWN.load(Ordering::Acquire)
                    && let Ok(mut state) = ddc_cache().lock()
                {
                    let _ =
                        state.store_probe(probe_generation, raw_monitor, result, Instant::now());
                }
            });
        if let Ok(handle) = spawned {
            *task_slot = Some(handle);
        }
    }
    cached
}

fn ensure_ddc_writer_running() -> bool {
    let Ok(mut slot) = DDC_WRITER_TASK.lock() else {
        return false;
    };
    if DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
        return false;
    }
    if let Some(existing) = slot.take() {
        if !existing.is_finished() {
            *slot = Some(existing);
            return true;
        }
        let _ = existing.join();
    }
    let shared = ddc_writer_shared();
    let spawned = thread::Builder::new()
        .name(String::from("otto-ddc-writer"))
        .spawn(move || {
            loop {
                let job = {
                    let mut mailbox = shared
                        .mailbox
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    loop {
                        if mailbox.stopping || DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
                            return;
                        }
                        if let Some(job) = mailbox.pending.take() {
                            break job;
                        }
                        mailbox = shared
                            .wake
                            .wait(mailbox)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                };
                let (monitor_key, target, generation) = job;
                let hmon = HMONITOR(monitor_key as *mut std::ffi::c_void);
                let ok = set_ddc_brightness_native(hmon, target);
                if !ok
                    && !DDC_SHUTTING_DOWN.load(Ordering::Acquire)
                    && let Ok(mut state) = ddc_cache().lock()
                {
                    state.mark_async_set_failed(generation, monitor_key, Instant::now());
                }
            }
        });
    if let Ok(handle) = spawned {
        *slot = Some(handle);
        true
    } else {
        false
    }
}

fn enqueue_ddc_brightness(monitor_key: isize, target: u8, generation: u64) -> bool {
    let shared = ddc_writer_shared();
    {
        let mut mailbox = shared
            .mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if mailbox.stopping || DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
            return false;
        }
        mailbox.pending = Some((monitor_key, target, generation));
        shared.wake.notify_one();
    }
    ensure_ddc_writer_running()
}

fn set_ddc_brightness(target: u8) -> Option<u8> {
    if DDC_SHUTTING_DOWN.load(Ordering::Acquire) {
        return None;
    }
    let monitor = primary_monitor()?;
    let monitor_key = monitor.0 as isize;
    let async_generation = if let Ok(mut state) = ddc_cache().lock() {
        if ddc_cache_is_fresh(&state, monitor_key) && state.supported == Some(false) {
            return None;
        }
        if state.monitor == monitor_key && state.supported == Some(true) {
            state.store_set(monitor_key, target, true, Instant::now());
            Some(state.generation)
        } else {
            None
        }
    } else {
        None
    };
    if let Some(generation) = async_generation
        && enqueue_ddc_brightness(monitor_key, target, generation)
    {
        return Some(target);
    }
    let ok = set_ddc_brightness_native(monitor, target);
    if let Ok(mut state) = ddc_cache().lock() {
        state.store_set(monitor_key, target, ok, Instant::now());
    }
    ok.then_some(target)
}

struct PhysicalMonitors(Vec<PHYSICAL_MONITOR>);

impl PhysicalMonitors {
    fn acquire(monitor: HMONITOR) -> Option<Self> {
        let mut count = 0u32;
        unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &raw mut count) }.ok()?;
        if count == 0 {
            return None;
        }
        let mut monitors = vec![PHYSICAL_MONITOR::default(); count as usize];
        if unsafe { GetPhysicalMonitorsFromHMONITOR(monitor, &mut monitors) }.is_err() {
            let _ = unsafe { DestroyPhysicalMonitors(&monitors) };
            return None;
        }
        Some(Self(monitors))
    }
}

impl Drop for PhysicalMonitors {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            let _ = unsafe { DestroyPhysicalMonitors(&self.0) };
        }
    }
}

fn read_ddc_brightness_native(monitor: HMONITOR) -> Option<u8> {
    let monitors = PhysicalMonitors::acquire(monitor)?;
    for entry in &monitors.0 {
        let mut min = 0u32;
        let mut current = 0u32;
        let mut max = 0u32;
        let ok = unsafe {
            GetMonitorBrightness(
                entry.hPhysicalMonitor,
                &raw mut min,
                &raw mut current,
                &raw mut max,
            )
        } != 0;
        if ok && let Some(percent) = ddc_raw_to_percent(min, current, max) {
            return Some(percent);
        }
    }
    None
}

fn set_ddc_brightness_native(monitor: HMONITOR, percent: u8) -> bool {
    let Some(monitors) = PhysicalMonitors::acquire(monitor) else {
        return false;
    };
    let mut updated = false;
    for entry in &monitors.0 {
        let mut min = 0u32;
        let mut current = 0u32;
        let mut max = 0u32;
        let ok = unsafe {
            GetMonitorBrightness(
                entry.hPhysicalMonitor,
                &raw mut min,
                &raw mut current,
                &raw mut max,
            )
        } != 0;
        if ok
            && let Some(raw) = ddc_percent_to_raw(percent, min, max)
            && unsafe { SetMonitorBrightness(entry.hPhysicalMonitor, raw) } != 0
        {
            updated = true;
        }
    }
    updated
}

fn ddc_raw_to_percent(min: u32, current: u32, max: u32) -> Option<u8> {
    if max <= min {
        return None;
    }
    let span = u64::from(max - min);
    let offset = u64::from(current.saturating_sub(min).min(max - min));
    let rounded = (offset * 100 + span / 2) / span;
    u8::try_from(rounded.min(100)).ok()
}

fn ddc_percent_to_raw(percent: u8, min: u32, max: u32) -> Option<u32> {
    if max <= min {
        return None;
    }
    let span = u64::from(max - min);
    let clamped = u64::from(percent.min(100));
    let raw = u64::from(min) + (clamped * span + 50) / 100;
    u32::try_from(raw.min(u64::from(max))).ok()
}

fn open_lcd_device() -> Option<HANDLE> {
    if let Ok(absent) = lcd_absent_cache().lock()
        && let Some(since) = *absent
        && since.elapsed() < LCD_ABSENT_TTL
    {
        return None;
    }
    let opened = unsafe {
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
    };
    if opened.is_none()
        && let Ok(mut absent) = lcd_absent_cache().lock()
    {
        *absent = Some(Instant::now());
    }
    opened
}

fn nearest_brightness_level(levels: &[u8], returned: u32, target: u8) -> u8 {
    if returned == 0 {
        return target;
    }
    let count = (returned as usize).min(levels.len());
    levels[..count]
        .iter()
        .copied()
        .min_by_key(|&lvl| (i32::from(lvl) - i32::from(target)).abs())
        .unwrap_or(target)
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
    if !ok {
        return target;
    }
    nearest_brightness_level(&levels, returned, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_lang_from_lang_id_resolves_english_and_rejects_zero() {
        assert!(input_lang_from_lang_id(0).is_empty());
        assert_eq!(input_lang_from_lang_id(0x0409).as_str(), "EN");
    }

    #[test]
    fn ddc_brightness_maps_vendor_range_to_percent_and_back() {
        assert_eq!(ddc_raw_to_percent(0, 50, 100), Some(50));
        assert_eq!(ddc_raw_to_percent(20, 50, 80), Some(50));
        assert_eq!(ddc_raw_to_percent(20, 20, 80), Some(0));
        assert_eq!(ddc_raw_to_percent(20, 80, 80), Some(100));
        assert_eq!(ddc_raw_to_percent(50, 50, 50), None);
        assert_eq!(ddc_percent_to_raw(0, 20, 80), Some(20));
        assert_eq!(ddc_percent_to_raw(50, 20, 80), Some(50));
        assert_eq!(ddc_percent_to_raw(100, 20, 80), Some(80));
        assert_eq!(ddc_percent_to_raw(150, 20, 80), Some(80));
        assert_eq!(ddc_percent_to_raw(50, 50, 50), None);
    }

    #[test]
    fn ddc_probe_does_not_overwrite_newer_set_brightness() {
        let now = Instant::now();
        let mut cache = DdcCacheState::default();
        let probe_gen = cache.generation;
        cache.store_set(42, 85, true, now);
        assert!(cache.generation > probe_gen);

        assert!(!cache.store_probe(probe_gen, 42, Some(20), now + Duration::from_secs(1)));
        assert_eq!(cache.percent, Some(85));
        assert!(cache.store_probe(cache.generation, 42, Some(60), now + Duration::from_secs(2)));
        assert_eq!(cache.percent, Some(60));

        let stale_gen = cache.generation;
        cache.store_set(42, 90, true, now + Duration::from_secs(3));
        cache.mark_async_set_failed(stale_gen, 42, now + Duration::from_secs(4));
        assert_eq!(cache.supported, Some(true));
        assert_eq!(cache.percent, Some(90));
        cache.mark_async_set_failed(cache.generation, 42, now + Duration::from_secs(5));
        assert_eq!(cache.supported, Some(false));
        assert_eq!(cache.percent, None);
    }

    #[test]
    fn ddc_probe_guard_resets_probing_flag_on_unwind() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        DDC_PROBING.store(true, Ordering::Release);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _guard = DdcProbeGuard;
            panic!("simulated panic during DDC probe");
        }));
        assert!(outcome.is_err());
        assert!(!DDC_PROBING.load(Ordering::Acquire));
    }

    #[test]
    fn shutdown_blocks_ddc_probe_until_resumed() {
        struct ResumeStatusGuard;
        impl Drop for ResumeStatusGuard {
            fn drop(&mut self) {
                shutdown_status_task();
                resume_status_task();
            }
        }

        let _resume = ResumeStatusGuard;
        shutdown_status_task();
        assert!(DDC_SHUTTING_DOWN.load(Ordering::Acquire));
        assert_eq!(read_ddc_brightness_cached(), None);
        assert!(DDC_PROBE_TASK.lock().unwrap().is_none());
    }

    #[test]
    fn nearest_brightness_level_clamps_returned_length_to_buffer() {
        let levels = [15, 50, 90];
        assert_eq!(nearest_brightness_level(&levels, 512, 48), 50);
        assert_eq!(nearest_brightness_level(&levels, 2, 85), 50);
        assert_eq!(nearest_brightness_level(&levels, 0, 77), 77);
    }

    #[test]
    fn dnd_state_combines_focus_session_and_quns_without_guessing_busy() {
        assert_eq!(quns_to_dnd(QUNS_QUIET_TIME), Some(true));
        assert_eq!(quns_to_dnd(QUNS_ACCEPTS_NOTIFICATIONS), Some(false));
        assert_eq!(quns_to_dnd(windows::Win32::UI::Shell::QUNS_BUSY), None);

        assert_eq!(combine_dnd_state(Some(true), Some(false)), Some(true));
        assert_eq!(combine_dnd_state(Some(false), Some(true)), Some(true));
        assert_eq!(combine_dnd_state(Some(false), None), Some(false));
        assert_eq!(combine_dnd_state(None, Some(false)), Some(false));
        assert_eq!(combine_dnd_state(None, None), None);
    }
}
