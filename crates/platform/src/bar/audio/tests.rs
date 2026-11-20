use std::sync::atomic::Ordering;
use std::sync::{Mutex, PoisonError};

use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, eAll, eCapture, eCommunications, eConsole, eMultimedia, eRender,
};
use windows::core::BOOL;

use super::{
    LIVE_UPDATES, RUNTIME, decode_notification, is_available, percent_from_scalar,
    read_audio_outputs, read_mic_state, read_volume_and_mute, resume_audio_task, set_audio_output,
    shutdown_audio_task, stepped_percent, stop_runtime, tracks_console_capture,
    tracks_console_render, volume_step,
};

static RUNTIME_TEST_LOCK: Mutex<()> = Mutex::new(());

struct AudioResumeGuard;

impl Drop for AudioResumeGuard {
    fn drop(&mut self) {
        let _ = stop_runtime();
        resume_audio_task();
    }
}

#[test]
fn wheel_direction_maps_to_a_fixed_step() {
    assert_eq!(volume_step(120), Some(5));
    assert_eq!(volume_step(-120), Some(-5));
    assert_eq!(volume_step(0), None);
}

#[test]
fn stepping_clamps_to_the_percent_range() {
    assert_eq!(stepped_percent(98, 5), 100);
    assert_eq!(stepped_percent(3, -5), 0);
    assert_eq!(stepped_percent(50, 5), 55);
}

#[test]
fn scalar_conversion_rounds_and_clamps() {
    assert_eq!(percent_from_scalar(0.554), 55);
    assert_eq!(percent_from_scalar(1.4), 100);
    assert_eq!(percent_from_scalar(-0.2), 0);
}

#[test]
fn endpoint_notifications_decode_to_percent_and_mute() {
    let data = AUDIO_VOLUME_NOTIFICATION_DATA {
        bMuted: BOOL::from(true),
        fMasterVolume: 0.376,
        nChannels: 1,
        ..Default::default()
    };
    assert_eq!(decode_notification(&data), (38, true));
}

#[test]
fn only_the_console_render_default_is_tracked() {
    assert!(tracks_console_render(eRender, eConsole));
    assert!(!tracks_console_render(eRender, eMultimedia));
    assert!(!tracks_console_render(eRender, eCommunications));
    assert!(!tracks_console_render(eCapture, eConsole));
    assert!(!tracks_console_render(eAll, eConsole));
}

#[test]
fn only_the_console_capture_default_is_tracked_for_mic() {
    assert!(tracks_console_capture(eCapture, eConsole));
    assert!(!tracks_console_capture(eCapture, eMultimedia));
    assert!(!tracks_console_capture(eCapture, eCommunications));
    assert!(!tracks_console_capture(eRender, eConsole));
    assert!(!tracks_console_capture(eAll, eConsole));
}

#[test]
fn runtime_starts_on_first_read_and_its_worker_exits_on_shutdown() {
    let _serial = RUNTIME_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let _resume = AudioResumeGuard;
    resume_audio_task();
    let _ = read_volume_and_mute();
    let handle_alive = RUNTIME
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|runtime| !runtime.handle.is_finished());
    assert!(handle_alive);
    assert!(stop_runtime());
    assert!(RUNTIME.lock().unwrap().is_none());
    assert!(!is_available());
    assert!(!LIVE_UPDATES.load(Ordering::Acquire));
}

#[test]
fn shutdown_prevents_audio_worker_respawn_until_resumed() {
    let _serial = RUNTIME_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let _resume = AudioResumeGuard;
    resume_audio_task();
    let _ = read_volume_and_mute();
    shutdown_audio_task();
    assert!(RUNTIME.lock().unwrap().is_none());

    let _ = read_volume_and_mute();
    let _ = read_mic_state();
    let _ = read_audio_outputs();
    assert!(RUNTIME.lock().unwrap().is_none());
}

#[test]
#[ignore = "needs a default audio output device"]
fn live_updates_register_on_the_default_output_device() {
    let _serial = RUNTIME_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let _resume = AudioResumeGuard;
    resume_audio_task();
    let _ = read_volume_and_mute();
    assert!(is_available());
    assert!(LIVE_UPDATES.load(Ordering::Acquire));
    assert!(stop_runtime());
    assert!(!LIVE_UPDATES.load(Ordering::Acquire));
}

#[test]
fn audio_output_queries_are_safe_and_blank_id_is_rejected() {
    let _serial = RUNTIME_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let _resume = AudioResumeGuard;
    resume_audio_task();
    let outputs = read_audio_outputs();
    if let Some(default_dev) = outputs.iter().find(|dev| dev.is_default) {
        assert!(!default_dev.id.trim().is_empty());
        assert!(!default_dev.name.trim().is_empty());
    }
    assert!(!set_audio_output(""));
    assert!(!set_audio_output("   "));
    assert!(stop_runtime());
}
