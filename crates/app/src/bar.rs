use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::bar_controls::{BarControlAction, BarControlsController};
use crate::host::RuntimeContext;
use crate::menu::TopBarMenuRequest;
use crate::notch::{NotchController, NotchEffect, NotchHotkey};
use otto_core::{
    Appearance, BarFocus, BarFocusOutcome, BarHideMode, BarKey, BarPointer, BarVisibility,
    BrandIcon, ChangeGate, ControlCenterAction, ControlCenterPage, HitTarget, Layout, LyricsStatus,
    MenuBarPreferences, Metrics, MotionPreference, NotchAction, NotchExpandedTab, NotchHudKind,
    NotchLayoutInput, NotchPort, NotchStyle, NowPlaying, SHELF_THUMBNAIL_MAX_PX, ShelfPayload,
    ShelfThumbnail, TickCadence, build_bar_accessible_tree, compute_layout_with_notch,
    decode_bar_hit_target, detect_hud_event, is_top_bar_menu_target, scale,
};
use otto_core::{BACKDROP_PADDING_PT, CaptureRegion, Rect, capture_interval_for, capture_wanted};
use otto_platform::is_running_on_battery;
use otto_platform::release_capture_if_owned;
use otto_platform::{
    AppBar, ClipboardListenerGuard, ClipboardRead, LyricsOutcome, LyricsRequest, LyricsService,
    PowerNotificationGuard, PrimaryScreen, QuickCaptureInput, ShelfDropEvent, ShelfDropQueue,
    ShelfDropRegion, ShelfDropTargetGuard, TrayOverflowOutcome, WM_APP_SHELF_EVENT,
    WM_APP_UIA_INVOKE, WM_CLIPBOARDUPDATE, WindowsNotchPort, acknowledge_idle_wake, adjust_volume,
    clipboard_sequence_number, close_tray_overflow, current_process_id, cursor_position,
    foreground_application, handle_wm_getobject, is_escape_down, load_tasks, motion_preference,
    notify_bluetooth_hardware_change, primary_screen, read_clipboard_payload, read_system_status,
    register_bluetooth_hud_target, register_capture_hotkey, register_idle_wake,
    register_shelf_drop_target, route_bar_keys, routes_keys_to, save_tasks, signal_command_failure,
    step_display_brightness, stop_routing_bar_keys, stop_routing_if_targets, suppress_native_osd,
    take_bluetooth_hud_event, toggle_tray_overflow, unregister_bluetooth_hud_target,
    unregister_capture_hotkey, unregister_idle_wake, watch_top_edge, window_bounds,
    window_change_generations,
};
use otto_render::Renderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, HWND_TOPMOST, KillTimer, MA_NOACTIVATE, PostQuitMessage, SM_CXDRAG,
    SM_CYDRAG, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer, SetWindowPos, ShowWindow,
    WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_DWMCOLORIZATIONCOLORCHANGED,
    WM_ENDSESSION, WM_HOTKEY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCDESTROY, WM_QUERYENDSESSION, WM_RBUTTONUP, WM_SETTINGCHANGE,
    WM_SYSCOLORCHANGE, WM_THEMECHANGED, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_GETOBJECT: u32 = 0x003D;
const WM_POINTERUPDATE: u32 = 0x0245;
const WM_POINTERDOWN: u32 = 0x0246;
const WM_POINTERUP: u32 = 0x0247;
const WM_POINTERCAPTURECHANGED: u32 = 0x024C;
const SWIPE_COLLAPSE_THRESHOLD_PT: i32 = 24;
const STATUS_TIMER: usize = 1;
const HUD_ANIM_TIMER: usize = 2;
const DEADLINE_TIMER: usize = 3;
const WM_POWERBROADCAST: u32 = 0x0218;
const WM_DEVICECHANGE: u32 = 0x0219;
const DBT_DEVNODES_CHANGED: usize = 0x0007;
const DBT_DEVICEARRIVAL: usize = 0x8000;
const DBT_DEVICEREMOVECOMPLETE: usize = 0x8004;
const PBT_APMSUSPEND: usize = 0x0004;
const PBT_APMRESUMESUSPEND: usize = 0x0007;
const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
const MIN_DEADLINE_MS: u128 = 10;
const STATUS_POLL_MS: u32 = 120;
const FOREGROUND_FALLBACK_TICKS: u32 = 8;
const WM_APP_BAR_KEY: u32 = 0x8000 + 21;
const WM_APP_IDLE_WAKE: u32 = 0x8000 + 22;
const WM_APP_BAR_BACKDROP: u32 = 0x8000 + 24;
const WM_APP_BAR_BLUETOOTH_HUD: u32 = 0x8000 + 28;
const WM_APP_BAR_CAPTURE: u32 = 0x8000 + 60;
const STATUS_IDLE_MS: u32 = 1000;
const STATUS_IDLE_AFTER_TICKS: u32 = 16;
const MEDIA_EVERY_TICKS: u8 = 4;
const MAINTAIN_EVERY_TICKS: u8 = 8;
const HUD_ANIM_MS: u32 = 8;

struct PendingShelfDrag {
    slot: u8,
    source_hwnd: isize,
    start_x: i32,
    start_y: i32,
    payload: ShelfPayload,
}

pub(crate) enum DeferredModalAction {
    PickShelfFiles,
    StartShelfDrag {
        capture_hwnd: isize,
        payload: ShelfPayload,
    },
}

pub(crate) enum PreparedModalAction {
    PickShelfFiles {
        port: WindowsNotchPort,
    },
    StartShelfDrag {
        capture_hwnd: isize,
        payload: ShelfPayload,
        local_drag: Arc<AtomicBool>,
        port: WindowsNotchPort,
    },
}

pub(crate) enum CompletedModalAction {
    PickedShelfFiles(Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>),
    FinishedShelfDrag,
}

struct LocalDragGuard(Arc<AtomicBool>);

impl Drop for LocalDragGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Bar {
    hwnd: isize,
    panel_hwnd: isize,
    panel_visible: bool,
    last_panel_rect: Option<Rect>,
    own_pid: u32,
    appbar: AppBar,
    renderer: Renderer,
    screen: PrimaryScreen,
    metrics: Metrics,
    layout: Layout,
    controls: BarControlsController,
    notch_port: WindowsNotchPort,
    notch: NotchController,
    shelf_queue: ShelfDropQueue,
    shelf_local_drag: Arc<AtomicBool>,
    notch_shelf_region: Arc<ShelfDropRegion>,
    notch_drop_target: Option<ShelfDropTargetGuard>,
    panel_drop_target: Option<ShelfDropTargetGuard>,
    clipboard_listener: Option<ClipboardListenerGuard>,
    quick_capture: Option<QuickCaptureInput>,
    power_listener: Option<PowerNotificationGuard>,
    last_clipboard_seq: u32,
    suppressed_clipboard_seq: Option<u32>,
    pending_shelf_drag: Option<PendingShelfDrag>,
    touch_swipe: Option<(u32, i32, i32)>,
    hovered: Option<HitTarget>,
    tracking_bar_mouse: bool,
    tracking_panel_mouse: bool,
    maintain_ticks: u8,
    foreground_gate: ChangeGate,
    cadence: TickCadence,
    focus: BarFocus,
    focus_notch_tab_on_expand: bool,
    seek_drag_hwnd: Option<isize>,
    lyrics: LyricsService,
    lyrics_track: Option<(String, String)>,
    lyrics_generation: u64,
    online_lyrics_enabled: bool,
    animating: bool,
    settle_ticks: u32,
    top_bar_enabled: bool,
    notch_enabled: bool,
    control_center_enabled: bool,
    menu_prefs: MenuBarPreferences,
    motion: MotionPreference,
    visibility: BarVisibility,
    shell_full_screen: bool,
    backdrop: Option<BackdropFeed>,
}

#[allow(clippy::too_many_lines)]
pub fn start(
    runtime: &RuntimeContext,
    hwnd: isize,
    panel_hwnd: isize,
    _blur: bool,
    control_center_enabled: bool,
) -> Result<()> {
    let mut appbar = AppBar::register(hwnd);
    let own_pid = current_process_id();
    let (active_app, active_window) = foreground_application(own_pid).map_or_else(
        || (String::from("Finder"), None),
        |foreground| (foreground.name, foreground.window),
    );
    let metrics = Metrics::for_style(runtime.notch_style());
    let menu_bar = runtime.menu_bar.get();
    let top_bar_enabled = runtime.top_bar_enabled.get();
    let notch_enabled = runtime.notch_enabled.get();
    let screen = primary_screen();
    let notch_port = WindowsNotchPort;
    let media_enabled = runtime.media_module_enabled.get();
    let privacy_enabled = runtime.privacy_indicators_enabled.get();
    let focus_enabled = runtime.focus_module_enabled.get();
    let shelf_enabled = runtime.drop_shelf_enabled.get();
    let clipboard_enabled = runtime.clipboard_history_enabled.get();
    let quick_capture_enabled = runtime.quick_capture_enabled.get();
    let hud_enabled = runtime.notch_hud_enabled.get();
    let media = if media_enabled {
        notch_port.read_now_playing()
    } else {
        None
    };
    let notch_tab = if media.as_ref().is_some_and(NowPlaying::is_active) {
        NotchExpandedTab::Media
    } else if focus_enabled {
        NotchExpandedTab::Pomodoro
    } else if shelf_enabled {
        NotchExpandedTab::Shelf
    } else {
        NotchExpandedTab::Clipboard
    };
    let mut notch = NotchController::new(media, notch_tab);
    notch.set_focus_enabled(focus_enabled);
    notch.set_shelf_enabled(shelf_enabled);
    notch.set_clipboard_enabled(clipboard_enabled);
    if quick_capture_enabled {
        notch.replace_tasks(load_tasks());
        notch.set_tasks_enabled(true);
    }
    notch.set_media_enabled(media_enabled);
    notch.set_privacy_enabled(privacy_enabled);
    notch.set_hud_enabled(hud_enabled);
    let status = read_system_status();
    if notch_enabled && privacy_enabled {
        let _ = notch.sync_privacy_state(notch_port.read_privacy_state());
    }
    let controls = BarControlsController::new(status, active_app.clone(), active_window);
    let layout = compute_layout_with_notch(
        &metrics,
        screen.width,
        screen.dpi,
        &active_app,
        NotchLayoutInput {
            top_bar_enabled,
            notch_enabled,
            hidden_items: menu_bar.hidden_items,
            ..notch.layout_input(control_center_enabled)
        },
    );
    let (max_panel_w, max_panel_h) = max_panel_surface_size(screen.dpi);
    let renderer = Renderer::new(
        layout.bar.width,
        layout.bar.height,
        max_panel_w,
        max_panel_h,
        screen.dpi,
        0.0,
    )?;
    renderer.set_brand_icon(runtime.brand_icon());

    if runtime.dock_enabled.get() && runtime.dock_preferences.get().hide_taskbar {
        appbar.hide_system_taskbar();
    }
    if top_bar_enabled {
        appbar.reserve_top(screen.width, screen.height, layout.bar.height);
    }
    let quick_capture = (notch_enabled && quick_capture_enabled)
        .then(|| QuickCaptureInput::create(hwnd, WM_APP_BAR_CAPTURE))
        .flatten();
    if runtime.hotkeys_enabled.get() && quick_capture.is_some() {
        register_capture_hotkey(hwnd);
    }

    let lyrics = LyricsService::start();
    let shelf_queue = ShelfDropQueue::new();
    let shelf_local_drag = Arc::new(AtomicBool::new(false));
    let notch_pad = scale(12, screen.dpi);
    let notch_shelf_region = ShelfDropRegion::restricted(
        layout.notch.x - notch_pad,
        0,
        layout.notch.right() + notch_pad,
        layout.bar.height + notch_pad,
    );
    let (notch_drop_target, panel_drop_target) = if notch_enabled && shelf_enabled {
        register_drop_targets(
            hwnd,
            panel_hwnd,
            &shelf_queue,
            &notch_shelf_region,
            &shelf_local_drag,
        )
    } else {
        (None, None)
    };
    let clipboard_listener = (notch_enabled && clipboard_enabled)
        .then(|| ClipboardListenerGuard::register(hwnd))
        .flatten();
    let power_listener = PowerNotificationGuard::register(hwnd);
    let last_clipboard_seq = clipboard_sequence_number();

    let mut bar = Bar {
        hwnd,
        panel_hwnd,
        panel_visible: false,
        last_panel_rect: None,
        own_pid,
        appbar,
        renderer,
        screen,
        metrics,
        layout,
        controls,
        notch_port,
        notch,
        shelf_queue,
        shelf_local_drag,
        notch_shelf_region,
        notch_drop_target,
        panel_drop_target,
        clipboard_listener,
        quick_capture,
        power_listener,
        last_clipboard_seq,
        suppressed_clipboard_seq: None,
        pending_shelf_drag: None,
        touch_swipe: None,
        hovered: None,
        tracking_bar_mouse: false,
        tracking_panel_mouse: false,
        maintain_ticks: 0,
        foreground_gate: ChangeGate::new(FOREGROUND_FALLBACK_TICKS),
        cadence: TickCadence::new(STATUS_POLL_MS, STATUS_IDLE_MS, STATUS_IDLE_AFTER_TICKS),
        focus: BarFocus::default(),
        focus_notch_tab_on_expand: false,
        seek_drag_hwnd: None,
        lyrics,
        lyrics_track: None,
        lyrics_generation: 0,
        online_lyrics_enabled: runtime.online_lyrics_enabled.get(),
        animating: false,
        settle_ticks: 0,
        top_bar_enabled,
        notch_enabled,
        control_center_enabled,
        menu_prefs: menu_bar,
        motion: motion_preference(),
        visibility: initial_visibility(menu_bar, top_bar_enabled),
        shell_full_screen: false,
        backdrop: runtime.glass_enabled.get().then(|| {
            BackdropFeed::start(
                hwnd,
                WM_APP_BAR_BACKDROP,
                vec![hwnd, panel_hwnd],
                runtime.capture_visible.get(),
            )
        }),
    };
    bar.notch.set_clock(status.clock);
    bar.notch
        .set_lyrics_in_compact(bar.online_lyrics_enabled && bar.notch.media_enabled());
    bar.notch.set_presented(bar.notch_enabled && bar.shown());
    if !bar.notch_enabled {
        otto_platform::withdraw_notch_rect(bar.hwnd);
    } else if !runtime.onboarding_seen.get() && hud_enabled {
        bar.notch.trigger_hud(NotchHudKind::Welcome);
        runtime.pending_onboarding_seen.set(Some(true));
    }
    bar.sync_bar_shelf_region();
    bar.sync_backdrop(runtime, true);
    bar.place();
    bar.present(runtime);
    if !bar.shown() || !bar.visibility.is_visible() {
        bar.apply_visibility(runtime);
    }
    bar.schedule_deadline();

    unsafe {
        SetTimer(
            Some(HWND(hwnd as *mut _)),
            STATUS_TIMER,
            STATUS_POLL_MS,
            None,
        );
    }
    bar.ensure_anim_timer();
    register_idle_wake(hwnd, WM_APP_IDLE_WAKE);
    if notch_enabled && hud_enabled {
        register_bluetooth_hud_target(hwnd, WM_APP_BAR_BLUETOOTH_HUD);
    }

    *runtime.bar.borrow_mut() = Some(bar);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    unregister_idle_wake();
    unregister_bluetooth_hud_target();
    if let Ok(mut guard) = runtime.bar.try_borrow_mut()
        && let Some(mut bar) = guard.take()
    {
        bar.stop_anim_timer();
        unsafe {
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), STATUS_TIMER);
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), HUD_ANIM_TIMER);
            let _ = KillTimer(Some(HWND(bar.hwnd as *mut _)), DEADLINE_TIMER);
            let _ = ShowWindow(HWND(bar.hwnd as *mut _), SW_HIDE);
            let _ = ShowWindow(HWND(bar.panel_hwnd as *mut _), SW_HIDE);
        }
        watch_top_edge(false, bar.screen.width, bar.screen.height);
        unregister_capture_hotkey(bar.hwnd);
        bar.quick_capture = None;
        stop_routing_bar_keys();
        bar.lyrics.shutdown();
    }
}

fn is_primary_bar_window(runtime: &RuntimeContext, hwnd: isize) -> bool {
    hwnd != 0 && hwnd == runtime.bar_hwnd.get()
}

#[allow(clippy::too_many_lines)]
pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    wake_for_message(runtime, message);
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_QUERYENDSESSION => Some(LRESULT(1)),
        WM_CLOSE => {
            if is_primary_bar_window(runtime, hwnd) {
                unsafe { PostQuitMessage(0) };
            } else {
                unsafe {
                    let _ = ShowWindow(HWND(hwnd as *mut _), SW_HIDE);
                }
                with_bar(runtime, |bar| {
                    bar.panel_visible = false;
                    bar.last_panel_rect = None;
                });
            }
            Some(LRESULT(0))
        }
        WM_APP_IDLE_WAKE => Some(LRESULT(0)),
        WM_APP_SHELF_EVENT => {
            handle_shelf_drop_events(runtime);
            Some(LRESULT(0))
        }
        WM_CLIPBOARDUPDATE => {
            with_bar(runtime, |bar| bar.on_clipboard_update(runtime));
            Some(LRESULT(0))
        }
        WM_APP_BAR_CAPTURE => {
            with_bar(runtime, |bar| bar.on_quick_capture_submitted(runtime));
            Some(LRESULT(0))
        }
        WM_NCDESTROY => {
            with_bar(runtime, Bar::revoke_drop_targets);
            None
        }
        WM_APP_BAR_BACKDROP => {
            with_bar(runtime, |bar| bar.on_backdrop(runtime));
            Some(LRESULT(0))
        }
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                runtime.pending_end_session.set(true);
            }
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == STATUS_TIMER => {
            with_bar(runtime, |bar| {
                bar.tick(runtime);
            });
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == HUD_ANIM_TIMER => {
            with_bar(runtime, |bar| bar.step_hud_animation(runtime));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == DEADLINE_TIMER => {
            with_bar(runtime, |bar| bar.on_deadline(runtime));
            Some(LRESULT(0))
        }
        otto_platform::APPBAR_CALLBACK_MESSAGE => {
            if let Some(notification) =
                otto_platform::classify_shell_notification(wparam.0, lparam.0)
            {
                with_bar(runtime, |bar| {
                    bar.on_shell_notification(runtime, notification);
                });
            }
            Some(LRESULT(0))
        }
        WM_POWERBROADCAST => {
            with_bar(runtime, |bar| bar.on_power_event(runtime, wparam.0));
            crate::theme_state::refresh_system_appearance(runtime);
            Some(LRESULT(1))
        }
        WM_DEVICECHANGE
            if matches!(
                wparam.0,
                DBT_DEVNODES_CHANGED | DBT_DEVICEARRIVAL | DBT_DEVICEREMOVECOMPLETE
            ) =>
        {
            notify_bluetooth_hardware_change();
            Some(LRESULT(0))
        }
        WM_APP_BAR_BLUETOOTH_HUD => {
            if let Some(kind) = take_bluetooth_hud_event() {
                with_bar(runtime, |bar| bar.on_bluetooth_hud(runtime, kind));
            }
            Some(LRESULT(0))
        }
        WM_HOTKEY => {
            with_bar(runtime, |bar| bar.on_hotkey(runtime, wparam.0 as i32));
            Some(LRESULT(0))
        }
        WM_APP_BAR_KEY => {
            if let Some(key) = BarKey::from_code(wparam.0) {
                let modal = with_bar_result(runtime, |bar| bar.on_bar_key(runtime, key)).flatten();
                if let Some(modal) = modal {
                    queue_modal_action(runtime, modal);
                }
            }
            Some(LRESULT(0))
        }
        WM_GETOBJECT => with_bar_result(runtime, |bar| {
            let notch_panel_only = hwnd == bar.panel_hwnd;
            bar.accessible_wm_getobject(hwnd, wparam.0, lparam.0, notch_panel_only)
        })
        .flatten()
        .map(LRESULT),
        WM_APP_UIA_INVOKE => {
            if let Some(target) = i32::try_from(wparam.0).ok().and_then(decode_bar_hit_target) {
                let modal =
                    with_bar_result(runtime, |bar| bar.invoke_accessible_target(runtime, target))
                        .flatten();
                if let Some(modal) = modal {
                    queue_modal_action(runtime, modal);
                }
            }
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            let modal = with_bar_result(runtime, |bar| {
                if bar.notch.is_seek_dragging() {
                    bar.drag_seek(runtime, hwnd, x, y);
                    None
                } else if let Some(modal) = bar.maybe_start_shelf_drag(hwnd, x, y) {
                    Some(modal)
                } else {
                    bar.on_mouse_move(runtime, hwnd, x, y);
                    None
                }
            })
            .flatten();
            if let Some(modal) = modal {
                queue_modal_action(runtime, modal);
            }
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let pending = with_bar_result(runtime, |bar| {
                bar.release_seek(runtime, true);
                bar.pending_shelf_drag.take()
            })
            .flatten();
            if let Some(drag) = pending {
                let _ = release_capture_if_owned(drag.source_hwnd);
                with_bar(runtime, |bar| {
                    let _ = bar.run_notch_target(runtime, HitTarget::ShelfItem(drag.slot));
                });
            }
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_bar(runtime, |bar| {
                bar.release_seek(runtime, false);
                bar.pending_shelf_drag = None;
            });
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_bar(runtime, |bar| bar.on_mouse_leave(runtime, hwnd));
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_mouse_wheel(runtime, x, y, delta));
            Some(LRESULT(0))
        }
        WM_POINTERDOWN => {
            let pointer_id = (wparam.0 & 0xFFFF) as u32;
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_pointer_down(pointer_id, x, y));
            None
        }
        WM_POINTERUPDATE => {
            let pointer_id = (wparam.0 & 0xFFFF) as u32;
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| {
                bar.on_pointer_update(runtime, pointer_id, x, y);
            });
            None
        }
        WM_POINTERUP | WM_POINTERCAPTURECHANGED => {
            let pointer_id = (wparam.0 & 0xFFFF) as u32;
            with_bar(runtime, |bar| bar.clear_touch_swipe(pointer_id));
            None
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            let modal =
                with_bar_result(runtime, |bar| bar.on_left_click(runtime, hwnd, x, y)).flatten();
            if let Some(modal) = modal {
                queue_modal_action(runtime, modal);
            }
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| bar.on_right_click(runtime, hwnd, x, y));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_bar(runtime, |bar| bar.rebuild(runtime));
            Some(LRESULT(0))
        }
        WM_SETTINGCHANGE => {
            crate::theme_state::refresh_system_appearance(runtime);
            with_bar(runtime, Bar::refresh_motion);
            None
        }
        WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_DWMCOLORIZATIONCOLORCHANGED => {
            crate::theme_state::refresh_system_appearance(runtime);
            None
        }
        _ => None,
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

const fn wakes_status_tick(message: u32) -> bool {
    matches!(
        message,
        WM_APP_IDLE_WAKE
            | WM_HOTKEY
            | WM_APP_BAR_KEY
            | WM_LBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MOUSEWHEEL
            | WM_POWERBROADCAST
            | WM_APP_SHELF_EVENT
            | otto_platform::APPBAR_CALLBACK_MESSAGE
    )
}

fn wake_for_message(runtime: &RuntimeContext, message: u32) {
    if message == WM_APP_IDLE_WAKE {
        acknowledge_idle_wake();
        with_bar(runtime, |bar| bar.idle_wake(runtime));
        return;
    }
    if message == WM_MOUSEMOVE {
        with_bar(runtime, Bar::wake_timer_only);
        crate::dock::wake_timer_only(runtime);
        return;
    }
    if wakes_status_tick(message) {
        wake(runtime);
    }
}

pub fn wake(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| bar.wake(runtime));
    crate::dock::wake(runtime);
}

fn with_bar(runtime: &RuntimeContext, action: impl FnOnce(&mut Bar)) {
    if let Ok(mut guard) = runtime.bar.try_borrow_mut()
        && let Some(bar) = guard.as_mut()
    {
        action(bar);
    }
}

fn with_bar_result<T>(runtime: &RuntimeContext, action: impl FnOnce(&mut Bar) -> T) -> Option<T> {
    let mut guard = runtime.bar.try_borrow_mut().ok()?;
    guard.as_mut().map(action)
}

enum ProcessedShelfEvent {
    DragEnter,
    DragLeave,
    Drop(Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>),
}

fn handle_shelf_drop_events(runtime: &RuntimeContext) {
    let Some((shelf_enabled, queue, port)) = with_bar_result(runtime, |bar| {
        (
            bar.notch_enabled && bar.notch.shelf_enabled(),
            bar.shelf_queue.clone(),
            bar.notch_port,
        )
    }) else {
        return;
    };
    let events = queue.drain();
    if events.is_empty() || !shelf_enabled {
        return;
    }
    let processed: Vec<ProcessedShelfEvent> = events
        .into_iter()
        .map(|ev| match ev {
            ShelfDropEvent::DragEnter => ProcessedShelfEvent::DragEnter,
            ShelfDropEvent::DragLeave => ProcessedShelfEvent::DragLeave,
            ShelfDropEvent::Drop(payloads) => {
                let enriched = payloads
                    .into_iter()
                    .map(|payload| match &payload {
                        ShelfPayload::File(path) => {
                            let (is_dir, thumb) =
                                port.inspect_shelf_file(path, SHELF_THUMBNAIL_MAX_PX);
                            (payload, is_dir, thumb)
                        }
                        ShelfPayload::Text(_) | ShelfPayload::Url(_) => (payload, false, None),
                    })
                    .collect();
                ProcessedShelfEvent::Drop(enriched)
            }
        })
        .collect();
    with_bar(runtime, |bar| {
        bar.ensure_anim_timer();
        let now = Instant::now();
        let mut changed = false;
        for ev in processed {
            let update = match ev {
                ProcessedShelfEvent::DragEnter => bar.notch.handle_shelf_drag_enter(),
                ProcessedShelfEvent::DragLeave => bar.notch.handle_shelf_drag_leave(now),
                ProcessedShelfEvent::Drop(items) => bar.notch.handle_shelf_drop(items),
            };
            changed |= update.changed;
        }
        if changed {
            bar.recompute_layout();
            bar.sync_panel_visibility();
            bar.present(runtime);
        }
        bar.schedule_deadline();
    });
}

fn queue_modal_action(runtime: &RuntimeContext, action: DeferredModalAction) {
    if let Ok(mut pending) = runtime.pending_modal_action.try_borrow_mut()
        && pending.is_none()
    {
        *pending = Some(action);
    }
}

pub(crate) fn prepare_modal_action(
    runtime: &RuntimeContext,
    action: DeferredModalAction,
) -> Option<PreparedModalAction> {
    match action {
        DeferredModalAction::PickShelfFiles => {
            let port = with_bar_result(runtime, |bar| {
                if !bar.notch.begin_shelf_modal_picker() {
                    return None;
                }
                bar.recompute_layout();
                bar.sync_panel_visibility();
                bar.present(runtime);
                Some(bar.notch_port)
            })
            .flatten()?;
            Some(PreparedModalAction::PickShelfFiles { port })
        }
        DeferredModalAction::StartShelfDrag {
            capture_hwnd,
            payload,
        } => {
            let (local_drag, port) = with_bar_result(runtime, |bar| {
                let _ = bar.notch.set_shelf_drag_out(true);
                (Arc::clone(&bar.shelf_local_drag), bar.notch_port)
            })?;
            Some(PreparedModalAction::StartShelfDrag {
                capture_hwnd,
                payload,
                local_drag,
                port,
            })
        }
    }
}

pub(crate) fn run_prepared_modal_action(action: PreparedModalAction) -> CompletedModalAction {
    match action {
        PreparedModalAction::PickShelfFiles { port } => {
            let picked = port.pick_shelf_files();
            let enriched = picked
                .into_iter()
                .map(|path| {
                    let (is_dir, thumb) = port.inspect_shelf_file(&path, SHELF_THUMBNAIL_MAX_PX);
                    (ShelfPayload::File(path), is_dir, thumb)
                })
                .collect();
            CompletedModalAction::PickedShelfFiles(enriched)
        }
        PreparedModalAction::StartShelfDrag {
            capture_hwnd,
            payload,
            local_drag,
            port,
        } => {
            let _ = release_capture_if_owned(capture_hwnd);
            local_drag.store(true, Ordering::Release);
            let guard = LocalDragGuard(local_drag);
            let _ = port.start_shelf_drag(capture_hwnd, &payload);
            drop(guard);
            CompletedModalAction::FinishedShelfDrag
        }
    }
}

pub(crate) fn finish_modal_action(runtime: &RuntimeContext, completion: CompletedModalAction) {
    with_bar(runtime, |bar| {
        bar.ensure_anim_timer();
        match completion {
            CompletedModalAction::PickedShelfFiles(enriched) => {
                let update = bar.notch.finish_shelf_modal_picker(enriched);
                if update.changed {
                    bar.recompute_layout();
                    bar.sync_panel_visibility();
                    bar.present(runtime);
                }
            }
            CompletedModalAction::FinishedShelfDrag => {
                let _ = bar.notch.set_shelf_drag_out(false);
                bar.recompute_layout();
                bar.sync_panel_visibility();
                bar.present(runtime);
            }
        }
        bar.schedule_deadline();
    });
}

pub fn refresh_plugin_layout(runtime: &RuntimeContext, control_center_enabled: bool) {
    with_bar(runtime, |bar| {
        bar.control_center_enabled = control_center_enabled;
        bar.recompute_layout();
        bar.present(runtime);
    });
}

pub fn activate_hidden_target(runtime: &RuntimeContext, target: HitTarget) {
    with_bar(runtime, |bar| {
        if let Some(action) = BarControlsController::click_action(target) {
            bar.perform(runtime, action);
        }
    });
}

pub fn apply_appearance(runtime: &RuntimeContext, _appearance: Appearance) {
    with_bar(runtime, |bar| {
        bar.sync_backdrop(runtime, true);
        bar.present(runtime);
    });
}

pub fn set_online_lyrics_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.online_lyrics_enabled = enabled;
        bar.notch
            .set_lyrics_in_compact(enabled && bar.notch.media_enabled());
        bar.lyrics_track = None;
        if bar.notch.media_enabled() {
            bar.request_lyrics_for_current_track();
        }
        bar.recompute_layout();
        bar.present(runtime);
        bar.schedule_deadline();
    });
}

pub fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if enabled {
            if bar.backdrop.is_none() {
                bar.backdrop = Some(BackdropFeed::start(
                    bar.hwnd,
                    WM_APP_BAR_BACKDROP,
                    vec![bar.hwnd, bar.panel_hwnd],
                    runtime.capture_visible.get(),
                ));
            }
        } else {
            bar.backdrop = None;
            bar.renderer.clear_bar_backdrop();
            bar.present(runtime);
        }
        bar.sync_backdrop(runtime, true);
    });
}

pub fn set_top_bar_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if bar.top_bar_enabled == enabled {
            return;
        }
        bar.top_bar_enabled = enabled;
        if enabled {
            bar.appbar
                .reserve_top(bar.screen.width, bar.screen.height, bar.layout.bar.height);
            let _ = bar.visibility.set_mode(bar.menu_prefs.hide_mode);
        } else {
            bar.appbar.release_top();
            bar.shell_full_screen = false;
            let _ = bar.visibility.set_mode(BarHideMode::FullScreenOnly);
            let _ = bar
                .visibility
                .set_full_screen_app(bar.foreground_covers_primary());
            bar.hovered = None;
            if bar.focus.is_active() {
                bar.leave_keyboard_focus(runtime);
            }
        }
        bar.recompute_layout();
        bar.sync_backdrop(runtime, true);
        bar.apply_visibility(runtime);
        bar.sync_panel_visibility();
        watch_top_edge(
            bar.hidden_for_full_screen(),
            bar.screen.width,
            bar.screen.height,
        );
        if bar.shown() {
            bar.present(runtime);
        }
        bar.schedule_deadline();
    });
}

pub fn set_notch_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if bar.notch_enabled == enabled {
            return;
        }
        bar.notch_enabled = enabled;
        if enabled {
            if bar.notch.shelf_enabled() {
                bar.ensure_shelf_drop_targets();
            }
            if bar.notch.clipboard_enabled() && bar.clipboard_listener.is_none() {
                bar.clipboard_listener = ClipboardListenerGuard::register(bar.hwnd);
                bar.last_clipboard_seq = clipboard_sequence_number();
                bar.suppressed_clipboard_seq = None;
            }
            if bar.notch.hud_enabled() {
                register_bluetooth_hud_target(bar.hwnd, WM_APP_BAR_BLUETOOTH_HUD);
            }
            if runtime.quick_capture_enabled.get() {
                if bar.quick_capture.is_none() {
                    bar.quick_capture = QuickCaptureInput::create(bar.hwnd, WM_APP_BAR_CAPTURE);
                }
                if runtime.hotkeys_enabled.get() && bar.quick_capture.is_some() {
                    register_capture_hotkey(bar.hwnd);
                }
            }
            if bar.notch.privacy_enabled() {
                let _ = bar
                    .notch
                    .sync_privacy_state(bar.notch_port.read_privacy_state());
            }
        } else {
            bar.notch_drop_target = None;
            bar.panel_drop_target = None;
            bar.pending_shelf_drag = None;
            let _ = bar.shelf_queue.drain();
            bar.clipboard_listener = None;
            unregister_bluetooth_hud_target();
            unregister_capture_hotkey(bar.hwnd);
            bar.quick_capture = None;
            bar.seek_drag_hwnd = None;
            bar.touch_swipe = None;
            bar.notch.collapse_immediately();
            if bar.hovered.is_some_and(is_notch_target) {
                bar.hovered = None;
            }
            otto_platform::withdraw_notch_rect(bar.hwnd);
        }
        bar.recompute_layout();
        bar.apply_visibility(runtime);
        bar.sync_panel_visibility();
        watch_top_edge(
            bar.hidden_for_full_screen(),
            bar.screen.width,
            bar.screen.height,
        );
        if bar.shown() {
            bar.present(runtime);
        }
        bar.schedule_deadline();
    });
}

pub fn leave_keyboard_focus(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| {
        if bar.focus.is_active() {
            bar.leave_keyboard_focus(runtime);
        }
    });
}

#[must_use]
pub fn control_center_anchor(
    runtime: &RuntimeContext,
    trigger: HitTarget,
) -> Option<(i32, i32, MotionPreference)> {
    with_bar_result(runtime, |bar| {
        bar.top_bar_enabled.then(|| {
            (
                bar.anchor_right(trigger),
                bar.layout.popup_anchor_y(bar.screen.dpi),
                bar.motion,
            )
        })
    })
    .flatten()
}

#[must_use]
pub fn search_slot_trigger(runtime: &RuntimeContext) -> Option<Rect> {
    with_bar_result(runtime, |bar| {
        (bar.top_bar_enabled && bar.layout.search_slot.width > 0).then_some(bar.layout.search_slot)
    })
    .flatten()
}

pub fn set_privacy_indicators_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_privacy_enabled(enabled);
        if enabled && bar.notch_enabled {
            let _ = bar
                .notch
                .sync_privacy_state(bar.notch_port.read_privacy_state());
        }
        bar.recompute_layout();
        bar.present(runtime);
    });
}

pub fn set_clipboard_history_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_clipboard_enabled(enabled);
        if enabled && bar.notch_enabled {
            if bar.clipboard_listener.is_none() {
                bar.clipboard_listener = ClipboardListenerGuard::register(bar.hwnd);
                bar.last_clipboard_seq = clipboard_sequence_number();
                bar.suppressed_clipboard_seq = None;
            }
        } else {
            bar.clipboard_listener = None;
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.ensure_anim_timer();
    });
}

pub fn set_drop_shelf_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_shelf_enabled(enabled);
        if enabled && bar.notch_enabled {
            bar.ensure_shelf_drop_targets();
        } else {
            bar.notch_drop_target = None;
            bar.panel_drop_target = None;
            bar.pending_shelf_drag = None;
            let _ = bar.shelf_queue.drain();
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.ensure_anim_timer();
    });
}

fn initial_visibility(menu_bar: MenuBarPreferences, top_bar_enabled: bool) -> BarVisibility {
    let mut visibility = BarVisibility::default();
    let mode = if top_bar_enabled {
        menu_bar.hide_mode
    } else {
        BarHideMode::FullScreenOnly
    };
    visibility.set_mode(mode);
    visibility
}

pub fn set_menu_bar_preferences(runtime: &RuntimeContext, preferences: MenuBarPreferences) {
    with_bar(runtime, |bar| {
        let items_changed = bar.menu_prefs.hidden_items != preferences.hidden_items;
        let bg_changed = bar.menu_prefs.show_background != preferences.show_background;
        let keep_changed = bar.menu_prefs.keep_notch != preferences.keep_notch;
        bar.menu_prefs = preferences;
        let visibility_changed = if bar.top_bar_enabled {
            bar.visibility.set_mode(preferences.hide_mode) || keep_changed
        } else {
            false
        };
        if items_changed {
            bar.recompute_layout();
        }
        if bg_changed {
            bar.sync_backdrop(runtime, true);
        }
        if visibility_changed {
            bar.apply_visibility(runtime);
        }
        watch_top_edge(
            bar.hidden_for_full_screen(),
            bar.screen.width,
            bar.screen.height,
        );
        bar.present(runtime);
        bar.schedule_deadline();
    });
}

pub fn set_notch_hud_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_hud_enabled(enabled);
        if enabled && bar.notch_enabled {
            register_bluetooth_hud_target(bar.hwnd, WM_APP_BAR_BLUETOOTH_HUD);
        } else {
            unregister_bluetooth_hud_target();
        }
        bar.recompute_layout();
        bar.present(runtime);
        bar.schedule_deadline();
    });
}

pub fn set_media_module_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_media_enabled(enabled);
        bar.notch
            .set_lyrics_in_compact(enabled && bar.online_lyrics_enabled);
        bar.lyrics_track = None;
        bar.lyrics_generation = bar.lyrics_generation.wrapping_add(1);
        if enabled {
            let latest_media = bar.notch_port.read_now_playing();
            let _ = bar.notch.set_media(latest_media);
            bar.request_lyrics_for_current_track();
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.ensure_anim_timer();
        bar.schedule_deadline();
    });
}

pub fn set_focus_module_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_focus_enabled(enabled);
        runtime.focus_active.set(bar.notch.focus_active());
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.ensure_anim_timer();
        bar.schedule_deadline();
    });
}

pub fn set_dock_taskbar_hidden(runtime: &RuntimeContext, hidden: bool) {
    with_bar(runtime, |bar| {
        if hidden {
            bar.appbar.hide_system_taskbar();
        } else {
            bar.appbar.show_system_taskbar();
        }
        bar.present(runtime);
    });
}

pub fn set_hotkeys_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if enabled && bar.notch_enabled && bar.quick_capture.is_some() {
            register_capture_hotkey(bar.hwnd);
        } else {
            unregister_capture_hotkey(bar.hwnd);
        }
    });
}

pub fn set_quick_capture_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if enabled {
            bar.notch.replace_tasks(load_tasks());
            if bar.notch_enabled && bar.quick_capture.is_none() {
                bar.quick_capture = QuickCaptureInput::create(bar.hwnd, WM_APP_BAR_CAPTURE);
            }
            if bar.notch_enabled && runtime.hotkeys_enabled.get() && bar.quick_capture.is_some() {
                register_capture_hotkey(bar.hwnd);
            }
            bar.notch
                .set_tasks_enabled(!bar.notch_enabled || bar.quick_capture.is_some());
        } else {
            unregister_capture_hotkey(bar.hwnd);
            bar.quick_capture = None;
            bar.notch.set_tasks_enabled(false);
            bar.notch.replace_tasks(otto_core::TasksState::default());
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.ensure_anim_timer();
    });
}

pub fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_bar(runtime, |bar| {
        if let Some(feed) = bar.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        bar.sync_backdrop(runtime, true);
        bar.present(runtime);
    });
}

pub fn set_brand_icon(runtime: &RuntimeContext, brand_icon: BrandIcon) {
    with_bar(runtime, |bar| {
        bar.renderer.set_brand_icon(brand_icon);
        bar.present(runtime);
    });
}

pub fn set_notch_style(runtime: &RuntimeContext, style: NotchStyle) {
    with_bar(runtime, |bar| {
        let next_metrics = Metrics::for_style(style);
        if next_metrics.bar_height == bar.metrics.bar_height {
            bar.metrics = next_metrics;
            bar.recompute_layout();
            bar.sync_panel_visibility();
            bar.present(runtime);
        } else {
            bar.rebuild_with_metrics(runtime, next_metrics);
            bar.sync_panel_visibility();
        }
    });
}

fn max_panel_surface_size(dpi: u32) -> (i32, i32) {
    let classic = Metrics::MACBOOK_PRO_14;
    let island = Metrics::DYNAMIC_ISLAND;
    let max_pts_w = (classic.notch_expanded_width + classic.notch_expanded_ear_radius * 2 + 8)
        .max(island.notch_expanded_width + island.notch_expanded_ear_radius * 2 + 8);
    let max_pts_h = (classic.notch_top_inset
        + classic
            .notch_expanded_height
            .max(classic.notch_lyrics_height)
        + 4)
    .max(island.notch_top_inset + island.notch_expanded_height.max(island.notch_lyrics_height) + 4);
    (scale(max_pts_w, dpi), scale(max_pts_h, dpi))
}

fn register_drop_targets(
    hwnd: isize,
    panel_hwnd: isize,
    shelf_queue: &ShelfDropQueue,
    notch_shelf_region: &Arc<ShelfDropRegion>,
    shelf_local_drag: &Arc<AtomicBool>,
) -> (Option<ShelfDropTargetGuard>, Option<ShelfDropTargetGuard>) {
    let notch_target = register_shelf_drop_target(
        hwnd,
        shelf_queue.clone(),
        Arc::clone(notch_shelf_region),
        Arc::clone(shelf_local_drag),
    );
    let panel_target = register_shelf_drop_target(
        panel_hwnd,
        shelf_queue.clone(),
        ShelfDropRegion::full_window(),
        Arc::clone(shelf_local_drag),
    );
    (notch_target, panel_target)
}

impl Bar {
    fn ensure_shelf_drop_targets(&mut self) {
        if self.notch_drop_target.is_some() && self.panel_drop_target.is_some() {
            return;
        }
        let (notch_target, panel_target) = register_drop_targets(
            self.hwnd,
            self.panel_hwnd,
            &self.shelf_queue,
            &self.notch_shelf_region,
            &self.shelf_local_drag,
        );
        self.notch_drop_target = notch_target;
        self.panel_drop_target = panel_target;
    }

    fn request_lyrics_for_current_track(&mut self) {
        if !self.notch.media_enabled() {
            self.lyrics_track = None;
            return;
        }
        let Some(media) = self.notch.media() else {
            self.lyrics_track = None;
            return;
        };
        let track = (media.title.clone(), media.artist.clone());
        if track.0.trim().is_empty() || self.lyrics_track.as_ref() == Some(&track) {
            return;
        }
        let generation = self.lyrics_generation.wrapping_add(1);
        let accepted = self.lyrics.request(LyricsRequest {
            generation,
            title: track.0.clone(),
            artist: track.1.clone(),
            duration_ms: media.duration_ms,
            allow_online: self.online_lyrics_enabled,
        });
        if accepted {
            self.lyrics_generation = generation;
            self.lyrics_track = Some(track);
            self.notch.begin_lyrics_request();
        }
    }

    fn notch_input(&self) -> NotchLayoutInput {
        NotchLayoutInput {
            top_bar_enabled: self.top_bar_enabled,
            notch_enabled: self.notch_enabled,
            hidden_items: self.menu_prefs.hidden_items,
            ..self.notch.layout_input(self.control_center_enabled)
        }
    }

    fn recompute_layout(&mut self) {
        let renderer = &self.renderer;
        if self.notch_enabled
            && self.notch.sync_compact_title_width(
                Instant::now(),
                crate::notch::unix_now_ms(),
                |text| renderer.compact_title_points(text),
            )
        {
            self.ensure_anim_timer();
        }
        let notch_input = self.notch_input();
        let settled = notch_input.panel_expansion <= 0.0 && notch_input.press_progress <= 0.0;
        self.layout = compute_layout_with_notch(
            &self.metrics,
            self.screen.width,
            self.screen.dpi,
            self.controls.active_app(),
            notch_input,
        );
        if !self.notch_enabled {
            otto_platform::withdraw_notch_rect(self.hwnd);
        } else if settled {
            let island = self.layout.notch.union(&self.layout.notch_secondary);
            otto_platform::publish_notch_rect(
                self.hwnd,
                otto_core::Rect {
                    y: self.layout.notch.y,
                    height: self.layout.notch.height,
                    ..island
                },
            );
        }
        self.sync_bar_shelf_region();
    }

    fn sync_bar_shelf_region(&self) {
        let pad = scale(12, self.screen.dpi);
        self.notch_shelf_region.set_rect(
            self.layout.notch.x - pad,
            0,
            self.layout.notch.right() + pad,
            self.layout.bar.height + pad,
        );
    }

    fn revoke_drop_targets(&mut self) {
        unregister_bluetooth_hud_target();
        self.notch_drop_target = None;
        self.panel_drop_target = None;
        self.clipboard_listener = None;
        self.power_listener = None;
    }

    fn on_bluetooth_hud(&mut self, runtime: &RuntimeContext, kind: NotchHudKind) {
        if !self.notch_enabled || !self.notch.hud_enabled() {
            return;
        }
        self.notch.trigger_hud(kind);
        self.ensure_anim_timer();
        self.present(runtime);
        self.schedule_deadline();
    }

    fn open_quick_capture(&self) {
        let Some(capture) = &self.quick_capture else {
            return;
        };
        let center = self.layout.notch.x + self.layout.notch.width / 2;
        capture.show(center, self.layout.notch.bottom(), self.screen.dpi);
    }

    fn on_quick_capture_submitted(&mut self, runtime: &RuntimeContext) {
        let Some(submission) = self
            .quick_capture
            .as_ref()
            .and_then(QuickCaptureInput::take_submission)
        else {
            return;
        };
        if !self.notch.add_task(&submission.text) {
            signal_command_failure();
            return;
        }
        if !save_tasks(self.notch.tasks_state()) {
            signal_command_failure();
        }
        self.recompute_layout();
        self.present(runtime);
    }

    fn on_clipboard_update(&mut self, runtime: &RuntimeContext) {
        if !self.notch_enabled || !self.notch.clipboard_enabled() {
            return;
        }
        if self.notch.clipboard_state().paused {
            self.last_clipboard_seq = clipboard_sequence_number();
            self.suppressed_clipboard_seq = None;
            return;
        }
        let seq = clipboard_sequence_number();
        if self.suppressed_clipboard_seq == Some(seq) {
            self.suppressed_clipboard_seq = None;
            self.last_clipboard_seq = seq;
            return;
        }
        if seq == self.last_clipboard_seq {
            return;
        }
        match read_clipboard_payload() {
            ClipboardRead::Captured { seq, payload } => {
                self.last_clipboard_seq = seq;
                self.suppressed_clipboard_seq = None;
                if self.notch.sync_clipboard_capture(payload) {
                    self.recompute_layout();
                    if self.panel_visible {
                        self.present(runtime);
                    }
                }
            }
            ClipboardRead::SensitiveOrEmpty { seq } => {
                self.last_clipboard_seq = seq;
                self.suppressed_clipboard_seq = None;
            }
            ClipboardRead::Busy => {}
        }
    }

    fn maybe_start_shelf_drag(
        &mut self,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) -> Option<DeferredModalAction> {
        let pending = self.pending_shelf_drag.as_ref()?;
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        let dx = (x - pending.start_x).abs();
        let dy = (y - pending.start_y).abs();
        let cx = unsafe { GetSystemMetrics(SM_CXDRAG) }.max(4);
        let cy = unsafe { GetSystemMetrics(SM_CYDRAG) }.max(4);
        if dx < cx && dy < cy {
            return None;
        }
        let pending = self.pending_shelf_drag.take()?;
        Some(DeferredModalAction::StartShelfDrag {
            capture_hwnd: pending.source_hwnd,
            payload: pending.payload,
        })
    }

    fn seek_ratio_at(&self, x: i32) -> Option<f32> {
        let seekbar = self.layout.media_seekbar;
        (seekbar.width > 0).then(|| ((x - seekbar.x) as f32 / seekbar.width as f32).clamp(0.0, 1.0))
    }

    fn drag_seek(&mut self, runtime: &RuntimeContext, source_hwnd: isize, raw_x: i32, raw_y: i32) {
        let (x, _) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        if let Some(ratio) = self.seek_ratio_at(x)
            && self.notch.update_seek_drag(ratio)
        {
            self.present(runtime);
        }
    }

    fn release_seek(&mut self, runtime: &RuntimeContext, commit: bool) {
        let Some(hwnd) = self.seek_drag_hwnd.take() else {
            return;
        };
        let update = if commit {
            self.notch.finish_seek_drag(crate::notch::unix_now_ms())
        } else {
            self.notch.cancel_seek_drag();
            crate::notch::NotchUpdate {
                changed: true,
                effect: None,
            }
        };
        let _ = release_capture_if_owned(hwnd);
        self.apply_notch_effect(update.effect);
        self.present(runtime);
    }

    fn translate_client_point(&self, source_hwnd: isize, x: i32, y: i32) -> (i32, i32) {
        if source_hwnd == self.panel_hwnd {
            (
                x + self.layout.panel_window.x,
                y + self.layout.panel_window.y,
            )
        } else {
            (x, y)
        }
    }

    fn on_shell_notification(
        &mut self,
        runtime: &RuntimeContext,
        notification: otto_platform::ShellNotification,
    ) {
        match notification {
            otto_platform::ShellNotification::PositionChanged => {
                if self.top_bar_enabled {
                    self.appbar.reserve_top(
                        self.screen.width,
                        self.screen.height,
                        self.layout.bar.height,
                    );
                }
            }
            otto_platform::ShellNotification::FullScreenApp(active) => {
                self.shell_full_screen = active;
                self.sync_full_screen(runtime);
            }
        }
    }

    fn foreground_covers_primary(&self) -> bool {
        self.controls.active_window().is_some_and(|window| {
            window.full_screen
                && window_bounds(window.identity.hwnd).is_some_and(|frame| {
                    frame.x <= 0
                        && frame.y <= 0
                        && frame.right() >= self.screen.width
                        && frame.bottom() >= self.screen.height
                })
        })
    }

    fn sync_full_screen(&mut self, runtime: &RuntimeContext) {
        let active = self.shell_full_screen || self.foreground_covers_primary();
        if self.visibility.set_full_screen_app(active) {
            self.apply_visibility(runtime);
        }
    }

    fn track_reveal(&mut self, runtime: &RuntimeContext) {
        let (x, y) = cursor_position();
        let pointer = BarPointer {
            on_bar_monitor: x >= 0 && x < self.screen.width && y >= 0 && y < self.screen.height,
            y,
        };
        let popup_open = crate::menu::active_top_bar_target(runtime).is_some()
            || crate::control_center::is_open(runtime)
            || crate::menu::is_calendar_open(runtime)
            || crate::menu::is_notification_open(runtime);
        if self
            .visibility
            .track_pointer(pointer, self.layout.bar.height, popup_open)
        {
            self.apply_visibility(runtime);
        }
    }

    fn apply_visibility(&mut self, runtime: &RuntimeContext) {
        self.notch.set_presented(self.notch_enabled && self.shown());
        if !self.visibility.is_visible() {
            crate::menu::close(runtime);
        }
        if self.shown() {
            self.place();
            self.present(runtime);
            self.schedule_deadline();
            return;
        }
        self.schedule_deadline();
        crate::menu::close(runtime);
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            let _ = ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE);
        }
        self.panel_visible = false;
        self.last_panel_rect = None;
    }

    const fn top_bar_chrome_visible(&self) -> bool {
        self.top_bar_enabled && self.visibility.is_visible()
    }

    fn shown(&self) -> bool {
        if self.top_bar_enabled {
            self.visibility.is_visible() || (self.notch_enabled && self.menu_prefs.keep_notch)
        } else {
            self.notch_enabled && self.visibility.is_visible()
        }
    }

    fn place(&self) {
        if !self.shown() {
            return;
        }
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                0,
                0,
                self.layout.bar.width,
                self.layout.bar.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    fn sync_panel_visibility(&mut self) {
        let should_show =
            self.notch_enabled && self.notch.panel_expansion() > 0.005 && self.shown();
        if should_show {
            if !self.panel_visible || self.last_panel_rect != Some(self.layout.panel_window) {
                let _ = unsafe {
                    SetWindowPos(
                        HWND(self.panel_hwnd as *mut _),
                        Some(HWND_TOPMOST),
                        self.layout.panel_window.x,
                        self.layout.panel_window.y,
                        self.layout.panel_window.width,
                        self.layout.panel_window.height,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    )
                };
                self.last_panel_rect = Some(self.layout.panel_window);
            }
            self.panel_visible = true;
        } else if self.panel_visible {
            let _ = unsafe { ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE) };
            self.panel_visible = false;
            self.last_panel_rect = None;
        }
    }

    fn ensure_anim_timer(&mut self) {
        if !self.animating {
            let timer = unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    HUD_ANIM_TIMER,
                    HUD_ANIM_MS,
                    None,
                )
            };
            if timer != 0 {
                otto_platform::begin_fine_timing();
                self.animating = true;
                self.settle_ticks = 10;
            }
        }
    }

    fn stop_anim_timer(&mut self) {
        if self.animating {
            self.animating = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), HUD_ANIM_TIMER);
            }
            otto_platform::end_fine_timing();
        }
    }

    fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.rebuild_with_metrics(runtime, Metrics::for_style(runtime.notch_style()));
    }

    fn rebuild_with_metrics(&mut self, runtime: &RuntimeContext, metrics: Metrics) {
        self.last_panel_rect = None;
        self.screen = primary_screen();
        self.metrics = metrics;
        self.recompute_layout();
        let (max_panel_w, max_panel_h) = max_panel_surface_size(self.screen.dpi);
        if let Ok(renderer) = Renderer::new(
            self.layout.bar.width,
            self.layout.bar.height,
            max_panel_w,
            max_panel_h,
            self.screen.dpi,
            0.0,
        ) {
            renderer.set_brand_icon(runtime.brand_icon());
            self.renderer = renderer;
            if let Some(feed) = self.backdrop.as_mut() {
                feed.reset();
            }
        }
        self.appbar.rebase(self.screen.width, self.screen.height);
        if self.top_bar_enabled {
            self.appbar.reserve_top(
                self.screen.width,
                self.screen.height,
                self.layout.bar.height,
            );
        }
        self.sync_backdrop(runtime, true);
        self.place();
        self.present(runtime);
        self.ensure_anim_timer();
    }

    fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = capture_wanted(&appearance, self.backdrop.is_some())
            && self.top_bar_enabled
            && self.menu_prefs.show_background;
        let visible = self.top_bar_chrome_visible();
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        if !wanted {
            if feed.is_active() {
                feed.reset();
                self.renderer.clear_bar_backdrop();
                self.present(runtime);
            }
            return;
        }
        if !visible {
            return;
        }
        let bar_rect = Rect {
            x: 0,
            y: 0,
            width: self.layout.bar.width,
            height: self.layout.bar.height,
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) =
            CaptureRegion::padded_to_screen(bar_rect, pad, self.screen.width, self.screen.height)
        else {
            return;
        };
        if !self.renderer.has_bar_backdrop()
            && let Some(pixels) =
                otto_platform::capture_backdrop_now(region.origin(), region.width, region.height)
        {
            self.renderer.set_bar_backdrop(pixels, region.origin());
        }
        let active = (self.panel_visible || self.hovered.is_some()) && feed.recently_changed();
        let interval = capture_interval_for(is_running_on_battery(), active);
        feed.request(region, interval, force);
    }

    fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        self.renderer.set_bar_backdrop(update.pixels, update.origin);
        self.present(runtime);
    }

    fn wake(&mut self, runtime: &RuntimeContext) {
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
            self.run_tick(runtime, true);
        }
    }

    fn wake_timer_only(&mut self) {
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
        }
    }

    fn idle_wake(&mut self, runtime: &RuntimeContext) {
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
        }
        self.run_tick(runtime, true);
    }

    fn set_status_interval(&self, interval: u32) {
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                STATUS_TIMER,
                interval,
                None,
            );
        }
    }

    const fn periodic_due(&self, every_ticks: u8) -> bool {
        periodic_tick_due(self.maintain_ticks, every_ticks)
    }

    fn hidden_for_full_screen(&self) -> bool {
        (self.top_bar_enabled || self.notch_enabled)
            && self.visibility.tracks_pointer()
            && !self.visibility.is_visible()
    }

    fn status_busy(&self, runtime: &RuntimeContext, dirty: bool) -> bool {
        let reveal_needs_polling = self.visibility.tracks_pointer()
            && (self.visibility.is_visible() || window_change_generations().is_none());
        dirty || reveal_needs_polling || self.needs_status_polling(runtime)
    }

    fn needs_status_polling(&self, runtime: &RuntimeContext) -> bool {
        self.animating
            || self.settle_ticks > 0
            || self.panel_visible
            || self.hovered.is_some()
            || self.focus.is_active()
            || self.notch.is_seek_dragging()
            || self.notch.hud_state().is_some()
            || self.notch.lyrics_view().status == LyricsStatus::Loading
            || crate::menu::active_top_bar_target(runtime).is_some()
            || crate::control_center::is_open(runtime)
            || crate::menu::is_calendar_open(runtime)
            || crate::menu::is_notification_open(runtime)
    }

    fn refresh_motion(&mut self) {
        let next_motion = motion_preference();
        if next_motion != self.motion {
            self.motion = next_motion;
            self.ensure_anim_timer();
        }
    }

    fn tick(&mut self, runtime: &RuntimeContext) {
        self.run_tick(runtime, false);
    }

    fn run_tick(&mut self, runtime: &RuntimeContext, force_media: bool) {
        self.maintain_ticks = self.maintain_ticks.wrapping_add(1);
        self.track_reveal(runtime);
        if !self.cadence.is_idle() || self.periodic_due(MAINTAIN_EVERY_TICKS) {
            crate::window_controls::poll_foreground(runtime);
        }
        if self.periodic_due(MAINTAIN_EVERY_TICKS) {
            if self.notch_enabled && self.notch.hud_enabled() {
                suppress_native_osd();
            }
            self.appbar.maintain(
                self.screen.width,
                self.screen.height,
                self.layout.bar.height,
            );
            if self.panel_visible && self.shown() {
                let _ = unsafe {
                    SetWindowPos(
                        HWND(self.panel_hwnd as *mut _),
                        Some(HWND_TOPMOST),
                        self.layout.panel_window.x,
                        self.layout.panel_window.y,
                        self.layout.panel_window.width,
                        self.layout.panel_window.height,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    )
                };
            }
        }
        let mut dirty = false;
        while let Some(response) = self.lyrics.poll() {
            if response.generation == self.lyrics_generation
                && let Some(media) = self.notch.media()
                && media.title == response.title
                && media.artist == response.artist
            {
                let (sheet, status) = match response.outcome {
                    LyricsOutcome::Found { sheet, .. } => (Some(sheet), LyricsStatus::Ready),
                    LyricsOutcome::NotFound => (
                        None,
                        LyricsStatus::NotFound {
                            searched_online: self.online_lyrics_enabled,
                        },
                    ),
                    LyricsOutcome::Failed => (None, LyricsStatus::Failed),
                };
                self.notch.finish_lyrics(sheet, status);
                self.recompute_layout();
                self.ensure_anim_timer();
                dirty = true;
            }
        }
        let foreground_changed = self
            .foreground_gate
            .should_refresh(window_change_generations().map(|changes| changes.geometry));
        if foreground_changed && let Some(foreground) = foreground_application(self.own_pid) {
            let change = self
                .controls
                .apply_foreground(foreground.name, foreground.window);
            if change.app {
                self.recompute_layout();
                dirty = true;
            }
            if change.window {
                if crate::menu::active_top_bar_target(runtime).is_some()
                    || crate::menu::is_calendar_open(runtime)
                    || crate::menu::is_notification_open(runtime)
                {
                    crate::menu::close(runtime);
                    dirty = true;
                }
                if self.notch.collapse_from_external_focus_change() {
                    self.recompute_layout();
                    self.sync_panel_visibility();
                    dirty = true;
                }
            }
        }
        self.sync_full_screen(runtime);
        self.sync_backdrop(runtime, foreground_changed);
        self.sync_keyboard_focus(runtime);
        if self.notch.is_seek_dragging() && is_escape_down() {
            self.release_seek(runtime, false);
        }
        if self.refresh_media(force_media) {
            dirty = true;
        }
        dirty |= self.sync_notification_feed();
        if dirty {
            self.ensure_anim_timer();
            self.present(runtime);
        }
        self.schedule_deadline();
        watch_top_edge(
            self.hidden_for_full_screen(),
            self.screen.width,
            self.screen.height,
        );
        let busy = self.status_busy(runtime, dirty);
        self.observe_cadence(busy);
    }

    fn observe_cadence(&mut self, busy: bool) {
        if let Some(interval) = self.cadence.observe(busy) {
            self.set_status_interval(interval);
            if self.cadence.is_idle() {
                otto_platform::trim_working_set();
            }
        }
    }

    fn sync_notification_feed(&mut self) -> bool {
        if !self.notch_enabled {
            return false;
        }
        let Some(snapshot) = otto_platform::take_notification_changes() else {
            return false;
        };
        if !self
            .notch
            .ingest_notifications(&snapshot.items, Instant::now())
        {
            return false;
        }
        self.recompute_layout();
        self.sync_panel_visibility();
        true
    }

    fn refresh_media(&mut self, force_media: bool) -> bool {
        let mut dirty = false;
        if (self.top_bar_enabled || self.notch_enabled)
            && should_refresh_system_status(
                force_media,
                self.cadence.is_idle(),
                self.maintain_ticks,
            )
            && self.refresh_system_status()
        {
            dirty = true;
        }
        if !self.notch_enabled {
            return dirty;
        }
        if self.notch.privacy_enabled()
            && self
                .notch
                .sync_privacy_state(self.notch_port.read_privacy_state())
        {
            self.recompute_layout();
            dirty = true;
        }
        if self.notch.media_enabled() && should_refresh_media(force_media, self.maintain_ticks) {
            let latest_media = self.notch_port.read_now_playing();
            if self.notch.set_media(latest_media) {
                self.recompute_layout();
                dirty = true;
            }
            self.request_lyrics_for_current_track();
        }
        dirty
    }

    fn refresh_system_status(&mut self) -> bool {
        let mut latest = read_system_status();
        let previous = self.controls.status();
        if latest.input_lang.is_empty() && !previous.input_lang.is_empty() {
            latest.input_lang = previous.input_lang;
        }
        let mut dirty = false;
        if self.notch_enabled
            && self.notch.hud_enabled()
            && let Some(event) = detect_hud_event(previous, latest)
        {
            self.notch.trigger_hud(event);
            dirty = true;
        }
        self.notch.set_clock(latest.clock);
        if self.controls.apply_status(latest) {
            dirty = true;
        }
        dirty
    }

    fn schedule_deadline(&self) {
        let hwnd = Some(HWND(self.hwnd as *mut _));
        if !self.notch_enabled {
            unsafe {
                let _ = KillTimer(hwnd, DEADLINE_TIMER);
            }
            return;
        }
        let now = Instant::now();
        match self.notch.next_deadline(now) {
            Some(deadline) => {
                let wait = deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .max(MIN_DEADLINE_MS);
                unsafe {
                    SetTimer(
                        hwnd,
                        DEADLINE_TIMER,
                        u32::try_from(wait).unwrap_or(u32::MAX),
                        None,
                    );
                }
            }
            None => unsafe {
                let _ = KillTimer(hwnd, DEADLINE_TIMER);
            },
        }
    }

    fn apply_notch_effect(&mut self, effect: Option<NotchEffect>) -> bool {
        let Some(effect) = effect else {
            return false;
        };
        match effect {
            NotchEffect::Platform(action) => {
                self.notch_port.execute(action.clone());
                if matches!(action, NotchAction::ToggleMicMute) {
                    let _ = self
                        .notch
                        .sync_privacy_state(self.notch_port.read_privacy_state());
                    true
                } else {
                    false
                }
            }
            NotchEffect::PickShelfFiles => false,
            NotchEffect::SaveTasks => {
                if !save_tasks(self.notch.tasks_state()) {
                    signal_command_failure();
                }
                false
            }
            NotchEffect::OpenQuickCapture => {
                self.open_quick_capture();
                false
            }
            NotchEffect::CopyClipboard(payload) => {
                if let Some(seq) = self.notch_port.copy_clipboard(&payload) {
                    self.suppressed_clipboard_seq = Some(seq);
                    self.last_clipboard_seq = seq;
                    false
                } else {
                    self.notch.clear_clipboard_copied_feedback()
                }
            }
        }
    }

    fn apply_notch_update(&mut self, runtime: &RuntimeContext, update: crate::notch::NotchUpdate) {
        let effect_changed = self.apply_notch_effect(update.effect);
        if update.changed || effect_changed {
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
            self.ensure_anim_timer();
        }
    }

    fn on_deadline(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let (cx, cy) = cursor_position();
        let hovered = self.layout.notch.contains(cx, cy);
        let mut update = self.notch.sync_clock(now);
        if self.notch.sync_notifications(now, hovered) {
            update.changed = true;
        }
        self.apply_notch_update(runtime, update);
        self.schedule_deadline();
    }

    fn on_power_event(&mut self, runtime: &RuntimeContext, event: usize) {
        let now = Instant::now();
        match event {
            PBT_APMSUSPEND => {
                let update = self.notch.suspend(now);
                self.apply_notch_update(runtime, update);
            }
            PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC => {
                self.notch.resume(now);
                notify_bluetooth_hardware_change();
                self.schedule_deadline();
            }
            _ => {}
        }
    }

    fn step_hud_animation(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let (cx, cy) = cursor_position();
        let update = self.notch.advance(
            now,
            self.layout.notch.contains(cx, cy),
            self.panel_visible,
            self.motion,
        );
        let effect_changed = self.apply_notch_effect(update.effect);
        if update.changed || effect_changed {
            self.settle_ticks = 10;
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
        } else if self.settle_ticks > 0 {
            self.settle_ticks -= 1;
        } else {
            self.sync_panel_visibility();
            self.stop_anim_timer();
        }
        self.schedule_deadline();
    }

    fn keyboard_targets(&self) -> Vec<HitTarget> {
        let media_present = self
            .notch
            .display_media(crate::notch::unix_now_ms())
            .is_some();
        self.layout.keyboard_targets_with_media(media_present)
    }

    fn enter_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if !self.shown() {
            return;
        }
        crate::menu::close(runtime);
        let targets = self.keyboard_targets();
        if self.focus.enter(&targets).is_some() && route_bar_keys(self.hwnd, WM_APP_BAR_KEY) {
            self.present(runtime);
        } else {
            self.focus.exit();
        }
    }

    fn enter_from_dock(&mut self, runtime: &RuntimeContext, from_end: bool) {
        if !self.shown() {
            stop_routing_if_targets(self.hwnd);
            return;
        }
        let targets = self.keyboard_targets();
        let entered = if from_end {
            self.focus.enter_last(&targets)
        } else {
            self.focus.enter(&targets)
        };
        if entered.is_some() && route_bar_keys(self.hwnd, WM_APP_BAR_KEY) {
            self.present(runtime);
        } else {
            self.focus.exit();
            stop_routing_if_targets(self.hwnd);
        }
    }

    fn leave_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        stop_routing_if_targets(self.hwnd);
        self.focus_notch_tab_on_expand = false;
        self.focus.exit();
        self.present(runtime);
    }

    fn sync_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if self.focus_notch_tab_on_expand && self.layout.panel_expansion > 0.55 {
            self.focus_notch_tab_on_expand = false;
            let tab_target = active_notch_tab_target(self.notch.active_tab());
            let targets = self.keyboard_targets();
            if targets.contains(&tab_target)
                && self.focus.enter(&[tab_target]).is_some()
                && route_bar_keys(self.hwnd, WM_APP_BAR_KEY)
            {
                self.present(runtime);
                return;
            }
        }
        if !self.focus.is_active() {
            return;
        }
        if !routes_keys_to(self.hwnd) || !self.shown() {
            self.leave_keyboard_focus(runtime);
        } else if self.focus.revalidate(&self.keyboard_targets()) {
            self.present(runtime);
        }
    }

    fn accessible_wm_getobject(
        &self,
        hwnd: isize,
        wparam: usize,
        lparam: isize,
        notch_panel_only: bool,
    ) -> Option<isize> {
        handle_wm_getobject(hwnd, wparam, lparam, WM_APP_UIA_INVOKE, || {
            let app_name = self.controls.active_app();
            let media_present = self
                .notch
                .display_media(crate::notch::unix_now_ms())
                .is_some();
            build_bar_accessible_tree(
                &self.layout,
                (0, 0),
                app_name,
                media_present,
                notch_panel_only,
            )
        })
    }

    fn invoke_accessible_target(
        &mut self,
        runtime: &RuntimeContext,
        target: HitTarget,
    ) -> Option<DeferredModalAction> {
        self.ensure_anim_timer();
        if is_notch_target(target) {
            let modal = self.run_notch_target(runtime, target);
            self.present(runtime);
            return modal;
        }
        if let Some(action) = BarControlsController::click_action(target) {
            match action {
                BarControlAction::ControlCenter { page, trigger } => {
                    self.toggle_control_center(runtime, page, trigger, true);
                }
                other => self.perform(runtime, other),
            }
            self.present(runtime);
        }
        None
    }

    fn on_bar_key(&mut self, runtime: &RuntimeContext, key: BarKey) -> Option<DeferredModalAction> {
        let targets = self.keyboard_targets();
        if let Some(from_end) = self.focus.dock_handoff(key, &targets)
            && crate::dock::keyboard_entry_count(runtime) > 0
        {
            self.focus.exit();
            crate::dock::enter_keyboard_focus(runtime, from_end);
            self.present(runtime);
            return None;
        }
        if self.focus.current() == Some(HitTarget::MediaSeekbar)
            && let Some(delta_ms) = keyboard_seek_delta(key)
        {
            self.seek_from_keyboard(runtime, delta_ms);
            return None;
        }
        match self.focus.handle(key, &targets) {
            BarFocusOutcome::Moved(_) => self.present(runtime),
            BarFocusOutcome::Exited => self.leave_keyboard_focus(runtime),
            BarFocusOutcome::Unchanged | BarFocusOutcome::Activate(HitTarget::MediaSeekbar) => {}
            BarFocusOutcome::Activate(target) if is_notch_target(target) => {
                let was_collapsed = self.layout.panel_expansion <= 0.55;
                let modal = self.run_notch_target(runtime, target);
                if matches!(target, HitTarget::Notch | HitTarget::PrivacyIndicator) && was_collapsed
                {
                    let tab_target = active_notch_tab_target(self.notch.active_tab());
                    let targets = self.keyboard_targets();
                    if self.layout.panel_expansion > 0.55 && targets.contains(&tab_target) {
                        let _ = self.focus.enter(&[tab_target]);
                        self.focus_notch_tab_on_expand = false;
                    } else if self.panel_visible {
                        let _ = self.focus.enter(&[HitTarget::Notch]);
                        self.focus_notch_tab_on_expand = true;
                    }
                } else {
                    self.focus_notch_tab_on_expand = false;
                    stop_routing_if_targets(self.hwnd);
                }
                self.present(runtime);
                return modal;
            }
            BarFocusOutcome::Activate(target) => {
                self.leave_keyboard_focus(runtime);
                self.present(runtime);
                if let Some(action) = BarControlsController::click_action(target) {
                    match action {
                        BarControlAction::ControlCenter { page, trigger } => {
                            self.toggle_control_center(runtime, page, trigger, true);
                        }
                        other => self.perform(runtime, other),
                    }
                }
            }
        }
        None
    }

    fn on_hotkey(&mut self, runtime: &RuntimeContext, hotkey_id: i32) {
        if hotkey_id == otto_platform::HOTKEY_FOCUS_BAR {
            if self.focus.is_active() {
                self.leave_keyboard_focus(runtime);
            } else {
                self.enter_keyboard_focus(runtime);
            }
            return;
        }
        if !self.notch_enabled {
            return;
        }
        if hotkey_id == otto_platform::HOTKEY_QUICK_CAPTURE {
            self.open_quick_capture();
            return;
        }
        self.ensure_anim_timer();
        let Some(hotkey) = (match hotkey_id {
            otto_platform::HOTKEY_TOGGLE_NOTCH => Some(NotchHotkey::ToggleNotch),
            otto_platform::HOTKEY_TOGGLE_POMODORO => Some(NotchHotkey::TogglePomodoro),
            _ => None,
        }) else {
            return;
        };
        let update = self.notch.handle_hotkey(hotkey);
        if update.changed {
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
        }
        self.schedule_deadline();
    }

    fn ensure_mouse_tracking(&mut self, source_hwnd: isize) {
        let already_tracking = if source_hwnd == self.panel_hwnd {
            self.tracking_panel_mouse
        } else {
            self.tracking_bar_mouse
        };
        if already_tracking {
            return;
        }
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: HWND(source_hwnd as *mut _),
            dwHoverTime: 0,
        };
        if unsafe { TrackMouseEvent(&mut track) }.is_ok() {
            if source_hwnd == self.panel_hwnd {
                self.tracking_panel_mouse = true;
            } else {
                self.tracking_bar_mouse = true;
            }
        }
    }

    fn on_mouse_move(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        self.ensure_mouse_tracking(source_hwnd);
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);

        if self.notch_enabled {
            self.notch
                .handle_pointer_move(self.layout.notch.contains(x, y));
        }

        let target = self.layout.hit_test(x, y);
        if target != self.hovered || self.layout.notch.contains(x, y) {
            self.ensure_anim_timer();
        }
        if let Some(open_target) = crate::menu::active_top_bar_target(runtime)
            && let Some(hit) = target
            && is_top_bar_menu_target(hit)
            && hit != open_target
        {
            self.open_menu(runtime, hit, None);
        }
        if target != self.hovered {
            self.hovered = target;
            self.present(runtime);
        }
    }

    fn on_mouse_leave(&mut self, runtime: &RuntimeContext, source_hwnd: isize) {
        self.ensure_anim_timer();
        if source_hwnd == self.panel_hwnd {
            self.tracking_panel_mouse = false;
        } else {
            self.tracking_bar_mouse = false;
        }
        let (cx, cy) = cursor_position();
        let cursor_inside_notch = self.layout.notch.contains(cx, cy);
        let notch_changed = self.notch.handle_pointer_leave(cursor_inside_notch);
        if cursor_inside_notch {
            return;
        }
        if self.hovered.is_some() {
            self.hovered = None;
            self.present(runtime);
        } else if notch_changed {
            self.present(runtime);
        }
    }

    fn on_pointer_down(&mut self, pointer_id: u32, x: i32, y: i32) {
        if self.notch.panel_expansion() <= 0.55 || !self.layout.notch.contains(x, y) {
            return;
        }
        if matches!(
            self.layout.hit_test(x, y),
            Some(HitTarget::ShelfItem(_) | HitTarget::MediaSeekbar)
        ) {
            return;
        }
        self.touch_swipe = Some((pointer_id, x, y));
    }

    fn on_pointer_update(&mut self, runtime: &RuntimeContext, pointer_id: u32, x: i32, y: i32) {
        let Some((active_id, sx, sy)) = self.touch_swipe else {
            return;
        };
        if active_id != pointer_id {
            return;
        }
        let threshold = scale(SWIPE_COLLAPSE_THRESHOLD_PT, self.screen.dpi);
        if is_upward_touch_swipe((sx, sy), (x, y), threshold) {
            self.touch_swipe = None;
            self.ensure_anim_timer();
            let update = self.notch.collapse_from_gesture();
            self.apply_notch_update(runtime, update);
        }
    }

    fn clear_touch_swipe(&mut self, pointer_id: u32) {
        if self
            .touch_swipe
            .is_some_and(|(active_id, _, _)| active_id == pointer_id)
        {
            self.touch_swipe = None;
        }
    }

    fn on_mouse_wheel(&mut self, runtime: &RuntimeContext, x: i32, y: i32, delta: i16) {
        self.ensure_anim_timer();
        let hit = self.hovered.or_else(|| self.layout.hit_test(x, y));
        if self.notch.panel_expansion() > 0.55 {
            let header_bottom = self
                .layout
                .notch_tab_strip
                .bottom()
                .max(self.layout.bar.height);
            let in_header_strip = self.layout.notch.contains(x, y) && y <= header_bottom;
            if should_wheel_collapse_notch(in_header_strip, hit, self.notch.active_tab(), delta) {
                let update = self.notch.collapse_from_gesture();
                self.apply_notch_update(runtime, update);
                return;
            }
        }
        if self.notch.panel_expansion() > 0.55
            && let Some(
                target @ (HitTarget::Notch
                | HitTarget::FocusMode(_)
                | HitTarget::PomodoroPreset(_)
                | HitTarget::PomodoroPlayPause
                | HitTarget::PomodoroReset
                | HitTarget::PomodoroSkip
                | HitTarget::NotchTabShelf
                | HitTarget::NotchTabClipboard
                | HitTarget::NotchTabTasks
                | HitTarget::ShelfAdd
                | HitTarget::ShelfClear
                | HitTarget::ShelfPager
                | HitTarget::ShelfItem(_)
                | HitTarget::ShelfCopyItem(_)
                | HitTarget::ShelfRemoveItem(_)
                | HitTarget::ClipboardPauseToggle
                | HitTarget::ClipboardClear
                | HitTarget::ClipboardPager
                | HitTarget::ClipboardItem(_)
                | HitTarget::ClipboardRemoveItem(_)
                | HitTarget::TasksAdd
                | HitTarget::TasksClearDone
                | HitTarget::TasksPager
                | HitTarget::TaskItem(_)
                | HitTarget::TaskCheck(_)
                | HitTarget::TaskRemove(_)),
            ) = hit
        {
            let update = self.notch.handle_wheel(target, delta);
            if update.changed {
                self.recompute_layout();
                self.present(runtime);
            }
            return;
        }
        if let Some(target) = hit
            && let Some(action) = BarControlsController::wheel_action(
                target,
                delta,
                self.notch.panel_expansion() > 0.55,
            )
        {
            match action {
                BarControlAction::AdjustVolume(delta) => {
                    adjust_volume(delta);
                    let latest = read_system_status();
                    self.controls.apply_status(latest);
                    self.notch.trigger_hud(NotchHudKind::Volume {
                        percent: if latest.muted { 0 } else { latest.volume },
                        muted: latest.muted,
                    });
                }
                BarControlAction::StepBrightness(delta) => {
                    if let Some(pct) = step_display_brightness(delta) {
                        let mut status = self.controls.status();
                        status.brightness = Some(pct);
                        self.controls.apply_status(status);
                        self.notch
                            .trigger_hud(NotchHudKind::Brightness { percent: pct });
                    }
                }
                _ => return,
            }
            self.present(runtime);
        }
    }

    fn seek_from_keyboard(&mut self, runtime: &RuntimeContext, delta_ms: i64) {
        let update = self
            .notch
            .handle_seek_step(delta_ms, crate::notch::unix_now_ms());
        let effect_changed = self.apply_notch_effect(update.effect);
        if update.changed || effect_changed {
            self.present(runtime);
        }
    }

    fn run_notch_target(
        &mut self,
        runtime: &RuntimeContext,
        target: HitTarget,
    ) -> Option<DeferredModalAction> {
        if target == HitTarget::PrivacyIndicator {
            let _ = self
                .notch
                .sync_privacy_state(self.notch_port.read_privacy_state());
        }
        let update = self.notch.handle_click(target);
        if matches!(update.effect, Some(NotchEffect::PickShelfFiles)) {
            return Some(DeferredModalAction::PickShelfFiles);
        }
        self.apply_notch_update(runtime, update);
        None
    }

    fn on_left_click(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) -> Option<DeferredModalAction> {
        self.ensure_anim_timer();
        self.leave_keyboard_focus(runtime);
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        let target = self.layout.hit_test(x, y)?;

        if target == HitTarget::MediaSeekbar {
            if let Some(ratio) = self.seek_ratio_at(x)
                && self.notch.begin_seek_drag(ratio)
            {
                unsafe {
                    let _ = SetCapture(HWND(source_hwnd as *mut _));
                }
                self.seek_drag_hwnd = Some(source_hwnd);
                self.present(runtime);
            }
            return None;
        }

        if let HitTarget::ShelfItem(slot) = target
            && let Some(payload) = self.notch.shelf_item_payload(slot)
        {
            unsafe {
                let _ = SetCapture(HWND(source_hwnd as *mut _));
            }
            self.pending_shelf_drag = Some(PendingShelfDrag {
                slot,
                source_hwnd,
                start_x: x,
                start_y: y,
                payload,
            });
            return None;
        }

        if target == HitTarget::Notch
            && self.notch.panel_expansion() > 0.55
            && self.notch.active_tab() == NotchExpandedTab::Shelf
            && self.notch.shelf_state().items.is_empty()
            && self.layout.shelf_dropzone.contains(x, y)
        {
            return self.run_notch_target(runtime, HitTarget::ShelfAdd);
        }

        if is_notch_target(target) {
            return self.run_notch_target(runtime, target);
        }

        if let Some(action) = BarControlsController::click_action(target) {
            self.perform(runtime, action);
        }
        None
    }

    fn open_menu(&self, runtime: &RuntimeContext, target: HitTarget, anchor_x: Option<i32>) {
        let Some(slot) = self.layout.menu_anchor_rect(target) else {
            return;
        };
        crate::menu::open_top_bar_menu(
            runtime,
            &TopBarMenuRequest {
                bar_hwnd: self.hwnd,
                target,
                anchor_x: anchor_x.unwrap_or(slot.x),
                anchor_y: self.layout.popup_anchor_y(self.screen.dpi),
                app_name: self.controls.active_app(),
                window: self.controls.active_window(),
                hidden: &self.layout.hidden_targets,
            },
        );
    }

    fn toggle_control_center(
        &self,
        runtime: &RuntimeContext,
        page: ControlCenterPage,
        trigger: HitTarget,
        focused: bool,
    ) {
        if !self.control_center_enabled {
            let action = match page {
                ControlCenterPage::Wifi => Some(ControlCenterAction::OpenWifi),
                ControlCenterPage::Sound => Some(ControlCenterAction::OpenSound),
                _ => None,
            };
            if let Some(action) = action {
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                crate::control_center::open_settings_action(action);
            }
            return;
        }
        let _ = close_tray_overflow();
        if crate::control_center::is_open(runtime)
            && crate::control_center::active_page(runtime) == Some(page)
        {
            crate::control_center::close(runtime);
            return;
        }
        crate::menu::close(runtime);
        let anchor_right = self.anchor_right(trigger);
        let anchor_y = self.layout.popup_anchor_y(self.screen.dpi);
        if focused {
            crate::control_center::open_at_page_focused(
                runtime,
                page,
                anchor_right,
                anchor_y,
                self.motion,
            );
        } else {
            crate::control_center::open_at_page(runtime, page, anchor_right, anchor_y, self.motion);
        }
    }

    fn anchor_right(&self, target: HitTarget) -> i32 {
        self.layout
            .menu_anchor_rect(target)
            .map_or(self.screen.width, |slot| slot.right())
    }

    fn perform(&mut self, runtime: &RuntimeContext, action: BarControlAction) {
        match action {
            BarControlAction::TopBarMenu(target) => {
                let _ = close_tray_overflow();
                if crate::menu::active_top_bar_target(runtime) == Some(target) {
                    crate::menu::close(runtime);
                } else {
                    self.open_menu(runtime, target, None);
                }
                self.present(runtime);
            }
            BarControlAction::Tray => {
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                let anchor = self
                    .layout
                    .menu_anchor_rect(HitTarget::Tray)
                    .unwrap_or(self.layout.tray_slot);
                if toggle_tray_overflow(
                    anchor.x + anchor.width / 2,
                    self.layout.bar.height,
                    self.screen.width,
                ) == TrayOverflowOutcome::Unavailable
                {
                    signal_command_failure();
                }
            }
            BarControlAction::ControlCenter { page, trigger } => {
                self.toggle_control_center(runtime, page, trigger, false);
            }
            BarControlAction::OpenSettings(action) => {
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                crate::control_center::open_settings_action(action);
            }
            BarControlAction::Search => {
                self.leave_keyboard_focus(runtime);
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                let trigger =
                    (self.layout.search_slot.width > 0).then_some(self.layout.search_slot);
                crate::spotlight::open_search(runtime, trigger);
            }
            BarControlAction::Notifications => {
                let _ = close_tray_overflow();
                if crate::menu::is_notification_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    crate::menu::open_notifications(
                        runtime,
                        self.anchor_right(HitTarget::Notifications),
                        self.layout.popup_anchor_y(self.screen.dpi),
                    );
                }
            }
            BarControlAction::Clock => {
                let _ = close_tray_overflow();
                if crate::menu::is_calendar_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    crate::menu::open_calendar(
                        runtime,
                        self.anchor_right(HitTarget::Clock),
                        self.layout.popup_anchor_y(self.screen.dpi),
                    );
                }
            }
            BarControlAction::AdjustVolume(_) | BarControlAction::StepBrightness(_) => {}
        }
    }

    fn on_right_click(
        &mut self,
        runtime: &RuntimeContext,
        source_hwnd: isize,
        raw_x: i32,
        raw_y: i32,
    ) {
        self.ensure_anim_timer();
        let (x, y) = self.translate_client_point(source_hwnd, raw_x, raw_y);
        if self.notch_enabled {
            if let Some(HitTarget::ShelfItem(slot)) = self.layout.hit_test(x, y) {
                let update = self.notch.handle_shelf_right_click(slot);
                self.apply_notch_update(runtime, update);
                return;
            }
            if self.layout.secondary_expansion > 0.25 && self.layout.notch_secondary.contains(x, y)
            {
                let update = self.notch.dismiss_secondary_compact(Instant::now());
                if update.changed {
                    self.recompute_layout();
                    self.present(runtime);
                    return;
                }
            }
            if self.layout.notch.contains(x, y) {
                let update = self.notch.dismiss_compact(Instant::now());
                if update.changed {
                    self.recompute_layout();
                    self.present(runtime);
                    return;
                }
            }
        }
        if self.top_bar_chrome_visible() {
            let anchor_x = if self.layout.hit_test(x, y) == Some(HitTarget::Start) {
                self.layout.start_slot.x
            } else {
                x
            };
            self.open_menu(runtime, HitTarget::Start, Some(anchor_x));
            self.present(runtime);
        } else if self.notch_enabled && self.layout.notch.contains(x, y) {
            crate::menu::open_tray_menu(
                runtime,
                x,
                self.layout.popup_anchor_y(self.screen.dpi),
                false,
            );
            self.present(runtime);
        }
    }

    fn present(&mut self, runtime: &RuntimeContext) {
        runtime
            .focus_active
            .set(self.notch_enabled && self.notch.focus_active());
        if !self.shown() {
            return;
        }
        let hovered = self
            .hovered
            .or_else(|| crate::menu::active_top_bar_target(runtime))
            .or_else(|| {
                crate::menu::is_notification_open(runtime).then_some(HitTarget::Notifications)
            })
            .or_else(|| crate::menu::is_calendar_open(runtime).then_some(HitTarget::Clock));
        self.renderer
            .set_bar_background(self.top_bar_enabled && self.menu_prefs.show_background);
        self.renderer
            .set_bar_chrome_hidden(!self.top_bar_chrome_visible());
        let appearance = crate::theme_state::appearance(runtime);
        let media = self
            .notch
            .media_scene(Instant::now(), crate::notch::unix_now_ms());
        let status = self.controls.status();
        let pomodoro = self.notch.pomodoro_scene(Instant::now());
        let privacy = self.notch.privacy_state();
        crate::host::log_present_error(
            "bar",
            self.renderer.present(
                self.hwnd,
                (0, 0),
                &self.layout,
                status,
                self.controls.active_app(),
                hovered,
                self.focus.current(),
                self.notch.hud_state(),
                &pomodoro,
                media.as_ref(),
                self.notch.anim_phase(),
                appearance,
                privacy,
                self.notch.call_scene(Instant::now()).as_ref(),
            ),
        );
        if self.panel_visible {
            crate::host::log_present_error(
                "bar-panel",
                self.renderer.present_panel(
                    self.panel_hwnd,
                    &self.layout,
                    hovered,
                    self.focus.current(),
                    &pomodoro,
                    media.as_ref(),
                    self.notch.anim_phase(),
                    self.notch.lyrics_view(),
                    self.notch.shelf_state(),
                    self.notch.clipboard_state(),
                    self.notch.tasks_state(),
                    privacy,
                    self.notch
                        .notification_presentation(Instant::now())
                        .as_ref(),
                ),
            );
        }
    }
}

fn keyboard_seek_delta(key: BarKey) -> Option<i64> {
    match key {
        BarKey::Previous => Some(-otto_core::KEYBOARD_SEEK_STEP_MS),
        BarKey::Next => Some(otto_core::KEYBOARD_SEEK_STEP_MS),
        _ => None,
    }
}

const fn active_notch_tab_target(tab: NotchExpandedTab) -> HitTarget {
    match tab {
        NotchExpandedTab::Pomodoro => HitTarget::NotchTabPomodoro,
        NotchExpandedTab::Shelf => HitTarget::NotchTabShelf,
        NotchExpandedTab::Clipboard => HitTarget::NotchTabClipboard,
        NotchExpandedTab::Tasks => HitTarget::NotchTabTasks,
        NotchExpandedTab::Media => HitTarget::NotchTabMedia,
    }
}

fn is_notch_target(target: HitTarget) -> bool {
    matches!(
        target,
        HitTarget::Notch
            | HitTarget::NotchSecondary
            | HitTarget::FocusMode(_)
            | HitTarget::PomodoroPreset(_)
            | HitTarget::PomodoroPlayPause
            | HitTarget::PomodoroReset
            | HitTarget::PomodoroSkip
            | HitTarget::PomodoroStartNext
            | HitTarget::PomodoroExtendFive
            | HitTarget::NotchTabPomodoro
            | HitTarget::NotchTabShelf
            | HitTarget::NotchTabClipboard
            | HitTarget::NotchTabTasks
            | HitTarget::NotchTabMedia
            | HitTarget::ShelfAdd
            | HitTarget::ShelfClear
            | HitTarget::ShelfPager
            | HitTarget::ShelfItem(_)
            | HitTarget::ShelfCopyItem(_)
            | HitTarget::ShelfRemoveItem(_)
            | HitTarget::ClipboardPauseToggle
            | HitTarget::ClipboardClear
            | HitTarget::ClipboardPager
            | HitTarget::ClipboardItem(_)
            | HitTarget::ClipboardRemoveItem(_)
            | HitTarget::TasksAdd
            | HitTarget::TasksClearDone
            | HitTarget::TasksPager
            | HitTarget::TaskItem(_)
            | HitTarget::TaskCheck(_)
            | HitTarget::TaskRemove(_)
            | HitTarget::PrivacyIndicator
            | HitTarget::PrivacyMicMute
            | HitTarget::MediaPlayPause
            | HitTarget::MediaPrevious
            | HitTarget::MediaNext
            | HitTarget::MediaLyricsToggle
            | HitTarget::NotificationCard
            | HitTarget::NotificationClose
            | HitTarget::NotificationAccept
            | HitTarget::NotificationDecline
    )
}

pub fn enter_keyboard_focus_from_dock(runtime: &RuntimeContext, from_end: bool) {
    with_bar(runtime, |bar| bar.enter_from_dock(runtime, from_end));
}

pub fn toggle_focus_mode(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| {
        if !bar.notch_enabled {
            return;
        }
        bar.ensure_anim_timer();
        let update = bar.notch.handle_hotkey(NotchHotkey::TogglePomodoro);
        bar.apply_notch_update(runtime, update);
        bar.schedule_deadline();
        runtime.focus_active.set(bar.notch.focus_active());
    });
}

pub fn open_notch_media(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| {
        if !bar.notch_enabled || !bar.notch.media_enabled() || bar.notch.media().is_none() {
            return;
        }
        bar.ensure_anim_timer();
        bar.notch.open_media_tab();
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.schedule_deadline();
    });
}

const fn periodic_tick_due(maintain_ticks: u8, every_ticks: u8) -> bool {
    maintain_ticks.is_multiple_of(every_ticks)
}

const fn should_refresh_system_status(force: bool, is_idle: bool, maintain_ticks: u8) -> bool {
    force || !is_idle || periodic_tick_due(maintain_ticks, MEDIA_EVERY_TICKS)
}

const fn should_refresh_media(force: bool, maintain_ticks: u8) -> bool {
    force || periodic_tick_due(maintain_ticks, MEDIA_EVERY_TICKS)
}

const fn should_wheel_collapse_notch(
    in_header_strip: bool,
    hit: Option<HitTarget>,
    active_tab: NotchExpandedTab,
    delta: i16,
) -> bool {
    if delta <= 0 {
        return false;
    }
    in_header_strip
        || matches!(
            hit,
            Some(
                HitTarget::NotchTabPomodoro
                    | HitTarget::NotchTabShelf
                    | HitTarget::NotchTabClipboard
                    | HitTarget::NotchTabTasks
                    | HitTarget::NotchTabMedia
            )
        )
        || (matches!(hit, Some(HitTarget::Notch)) && matches!(active_tab, NotchExpandedTab::Media))
}

const fn is_upward_touch_swipe(start: (i32, i32), current: (i32, i32), threshold: i32) -> bool {
    let dy = start.1 - current.1;
    let dx = (current.0 - start.0).abs();
    let min_dy = if threshold > 1 { threshold } else { 1 };
    dy >= min_dy && dy > dx
}

#[cfg(test)]
mod tests {
    use super::{
        DeferredModalAction, HitTarget, LPARAM, LRESULT, MAINTAIN_EVERY_TICKS, MEDIA_EVERY_TICKS,
        NotchExpandedTab, RuntimeContext, WM_CLOSE, WPARAM, handle_message, is_primary_bar_window,
        is_upward_touch_swipe, periodic_tick_due, queue_modal_action, should_refresh_media,
        should_refresh_system_status, should_wheel_collapse_notch,
    };

    #[test]
    fn periodic_maintenance_keeps_strided_intervals_while_idle() {
        assert!(periodic_tick_due(0, MAINTAIN_EVERY_TICKS));
        assert!(!periodic_tick_due(1, MAINTAIN_EVERY_TICKS));
        assert!(!periodic_tick_due(7, MAINTAIN_EVERY_TICKS));
        assert!(periodic_tick_due(8, MAINTAIN_EVERY_TICKS));
    }

    #[test]
    fn wake_events_bypass_media_and_status_stride_gates() {
        assert!(!should_refresh_media(false, 1));
        assert!(should_refresh_media(true, 1));
        assert!(should_refresh_media(false, MEDIA_EVERY_TICKS));
        assert!(!should_refresh_system_status(false, true, 1));
        assert!(should_refresh_system_status(true, true, 1));
        assert!(should_refresh_system_status(false, false, 1));
        assert!(should_refresh_system_status(false, true, MEDIA_EVERY_TICKS));
    }

    #[test]
    fn primary_bar_window_check_distinguishes_bar_and_panel_hwnds() {
        let runtime = RuntimeContext::new(false);
        assert!(!is_primary_bar_window(&runtime, 0));
        runtime.bar_hwnd.set(101);
        assert!(is_primary_bar_window(&runtime, 101));
        assert!(!is_primary_bar_window(&runtime, 202));
        assert_eq!(
            handle_message(&runtime, 202, WM_CLOSE, WPARAM(0), LPARAM(0)),
            Some(LRESULT(0))
        );
    }

    #[test]
    fn queue_modal_action_keeps_first_pending_modal_until_drained() {
        let runtime = RuntimeContext::new(false);
        queue_modal_action(&runtime, DeferredModalAction::PickShelfFiles);
        queue_modal_action(
            &runtime,
            DeferredModalAction::StartShelfDrag {
                capture_hwnd: 55,
                payload: otto_core::ShelfPayload::Text(String::from("ignored")),
            },
        );
        let first = runtime.pending_modal_action.borrow_mut().take();
        assert!(matches!(first, Some(DeferredModalAction::PickShelfFiles)));
    }

    #[test]
    fn wheel_collapse_preserves_pomodoro_and_shelf_body_scrolling() {
        assert!(should_wheel_collapse_notch(
            true,
            Some(HitTarget::Notch),
            NotchExpandedTab::Pomodoro,
            120,
        ));
        assert!(!should_wheel_collapse_notch(
            true,
            Some(HitTarget::Notch),
            NotchExpandedTab::Pomodoro,
            -120,
        ));
        assert!(should_wheel_collapse_notch(
            false,
            Some(HitTarget::NotchTabPomodoro),
            NotchExpandedTab::Pomodoro,
            120,
        ));
        assert!(!should_wheel_collapse_notch(
            false,
            Some(HitTarget::PomodoroPreset(0)),
            NotchExpandedTab::Pomodoro,
            120,
        ));
        assert!(!should_wheel_collapse_notch(
            false,
            Some(HitTarget::ShelfItem(0)),
            NotchExpandedTab::Shelf,
            120,
        ));
        assert!(should_wheel_collapse_notch(
            false,
            Some(HitTarget::Notch),
            NotchExpandedTab::Media,
            120,
        ));
    }

    #[test]
    fn upward_touch_swipe_requires_vertical_displacement_over_threshold() {
        assert!(is_upward_touch_swipe((500, 100), (505, 70), 24));
        assert!(!is_upward_touch_swipe((500, 100), (500, 80), 24));
        assert!(!is_upward_touch_swipe((500, 100), (500, 130), 24));
        assert!(!is_upward_touch_swipe((500, 100), (550, 70), 24));
    }

    #[test]
    fn end_session_message_queues_pending_end_session_when_wparam_nonzero() {
        let runtime = RuntimeContext::new(false);
        assert_eq!(
            handle_message(&runtime, 0, super::WM_ENDSESSION, WPARAM(0), LPARAM(0)),
            Some(LRESULT(0))
        );
        assert!(!runtime.pending_end_session.get());
        assert_eq!(
            handle_message(&runtime, 0, super::WM_ENDSESSION, WPARAM(1), LPARAM(0)),
            Some(LRESULT(0))
        );
        assert!(runtime.pending_end_session.get());
    }

    #[test]
    fn prepare_modal_action_returns_none_without_active_bar() {
        let runtime = RuntimeContext::new(false);
        assert!(
            super::prepare_modal_action(&runtime, DeferredModalAction::PickShelfFiles).is_none()
        );
    }

    #[test]
    fn mouse_move_uses_non_blocking_wake_while_clicks_and_hotkeys_wake_synchronously() {
        assert!(!super::wakes_status_tick(super::WM_MOUSEMOVE));
        assert!(super::wakes_status_tick(super::WM_LBUTTONDOWN));
        assert!(super::wakes_status_tick(super::WM_HOTKEY));
        assert!(super::wakes_status_tick(super::WM_POWERBROADCAST));
    }
}
