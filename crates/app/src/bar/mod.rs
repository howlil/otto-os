use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::bar_controls::BarControlsController;
use crate::host::RuntimeContext;
use crate::notch::{NotchController, NotchHotkey};
use crate::notch_io::NotchIo;
use crate::win_msg::{
    DBT_DEVICEARRIVAL, DBT_DEVICEREMOVECOMPLETE, DBT_DEVNODES_CHANGED, DEADLINE_TIMER,
    HUD_ANIM_TIMER, STATUS_TIMER, WM_APP_BAR_BACKDROP, WM_APP_BAR_BLUETOOTH_HUD,
    WM_APP_BAR_CAPTURE, WM_APP_BAR_KEY, WM_APP_IDLE_WAKE, WM_APP_NATIVE_MINIMIZE, WM_DEVICECHANGE,
    WM_GETOBJECT, WM_MOUSELEAVE, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERUP,
    WM_POINTERUPDATE, WM_POWERBROADCAST,
};
use otto_core::Rect;
use otto_core::{
    BarFocus, BarHideMode, BarKey, BarVisibility, ChangeGate, HitTarget, Layout,
    MenuBarPreferences, Metrics, MotionPreference, NotchExpandedTab, NotchHudKind,
    NotchLayoutInput, NotchPort, NowPlaying, TickCadence, compute_layout_with_notch,
    decode_bar_hit_target,
};
use otto_platform::release_capture_if_owned;
use otto_platform::{
    AppBar, PowerNotificationGuard, PrimaryScreen, QuickCaptureInput, WM_APP_SHELF_EVENT,
    WM_APP_UIA_INVOKE, WM_CLIPBOARDUPDATE, WindowsNotchPort, acknowledge_idle_wake,
    current_process_id, foreground_application, hide_window, install_native_minimize_hook,
    kill_window_timer, load_tasks, motion_preference, notify_bluetooth_hardware_change,
    primary_screen, read_system_status, register_bluetooth_hud_target, register_capture_hotkey,
    save_focus_activity, set_native_minimize_intercept_enabled, stop_routing_bar_keys,
    take_bluetooth_hud_event, uninstall_native_minimize_hook, unregister_bluetooth_hud_target,
    unregister_capture_hotkey, unregister_idle_wake, watch_top_edge,
};
use otto_render::Renderer;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MA_NOACTIVATE, PostQuitMessage, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_DWMCOLORIZATIONCOLORCHANGED, WM_ENDSESSION, WM_HOTKEY, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_QUERYENDSESSION, WM_RBUTTONUP,
    WM_SETTINGCHANGE, WM_SYSCOLORCHANGE, WM_THEMECHANGED, WM_TIMER,
};
use windows::core::Result;

mod actions;
pub(crate) mod controls;
mod keyboard;
mod modal;
mod monitors;
mod notch_bridge;
pub(crate) mod notch_io;
mod pointer;
mod presentation;
mod secondary_input;
mod settings;
mod timing;

pub(crate) use self::monitors::on_taskbar_created;
use self::presentation::max_panel_surface_size;

pub(crate) use self::modal::DeferredModalAction;
pub(crate) use self::modal::PreparedModalAction;
pub(crate) use self::modal::finish_modal_action;
use self::modal::handle_shelf_drop_events;
pub(crate) use self::modal::prepare_modal_action;
use self::modal::queue_modal_action;
pub(crate) use self::modal::run_prepared_modal_action;
pub(crate) use self::settings::activate_hidden_target;
pub(crate) use self::settings::apply_appearance;
pub(crate) use self::settings::control_center_anchor;
pub(crate) use self::settings::focus_session_status;
pub(crate) use self::settings::leave_keyboard_focus;
pub(crate) use self::settings::refresh_plugin_layout;
pub(crate) use self::settings::search_slot_trigger;
pub(crate) use self::settings::set_brand_icon;
pub(crate) use self::settings::set_capture_visible;
pub(crate) use self::settings::set_clipboard_history_enabled;
pub(crate) use self::settings::set_dock_taskbar_hidden;
pub(crate) use self::settings::set_drop_shelf_enabled;
pub(crate) use self::settings::set_focus_module_enabled;
pub(crate) use self::settings::set_glass_enabled;
pub(crate) use self::settings::set_hotkeys_enabled;
pub(crate) use self::settings::set_media_module_enabled;
pub(crate) use self::settings::set_menu_bar_preferences;
pub(crate) use self::settings::set_notch_enabled;
pub(crate) use self::settings::set_notch_hud_enabled;
pub(crate) use self::settings::set_notch_style;
pub(crate) use self::settings::set_online_lyrics_enabled;
pub(crate) use self::settings::set_privacy_indicators_enabled;
pub(crate) use self::settings::set_quick_capture_enabled;
pub(crate) use self::settings::set_top_bar_enabled;
pub(crate) use self::settings::sync_focus_blocklist;

const SWIPE_COLLAPSE_THRESHOLD_PT: i32 = 24;
const MIN_DEADLINE_MS: u128 = 10;
const STATUS_POLL_MS: u32 = 120;
const FOREGROUND_FALLBACK_TICKS: u32 = 8;
const STATUS_IDLE_MS: u32 = 1000;
const STATUS_IDLE_AFTER_TICKS: u32 = 16;
const MEDIA_EVERY_TICKS: u8 = 4;
const MAINTAIN_EVERY_TICKS: u8 = 8;
const HUD_ANIM_MS: u32 = 8;

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Bar {
    hwnd: isize,
    panel_hwnd: isize,
    panel_visible: bool,
    last_panel_rect: Option<Rect>,
    own_pid: u32,
    appbar: AppBar,
    renderer: Option<Renderer>,
    screen: PrimaryScreen,
    metrics: Metrics,
    layout: Layout,
    controls: BarControlsController,
    notch_port: WindowsNotchPort,
    notch: NotchController,
    io: NotchIo,
    power_listener: Option<PowerNotificationGuard>,
    touch_swipe: Option<(u32, i32, i32)>,
    hovered: Option<HitTarget>,
    tracking_bar_mouse: bool,
    tracking_panel_mouse: bool,
    maintain_ticks: u8,
    foreground_gate: ChangeGate,
    title_gate: ChangeGate,
    cadence: TickCadence,
    focus: BarFocus,
    focus_notch_tab_on_expand: bool,
    animating: bool,
    status_timer_running: bool,
    settle_ticks: u32,
    top_bar_enabled: bool,
    notch_enabled: bool,
    control_center_enabled: bool,
    menu_prefs: MenuBarPreferences,
    motion: MotionPreference,
    visibility: BarVisibility,
    shell_full_screen: bool,
    backdrop: Option<BackdropFeed>,
    focus_epoch: Instant,
    last_focus_save: Instant,
    last_blocked_window: Option<(isize, Instant)>,
    last_browser_url: Option<(isize, String)>,
    last_focus_relief_active: bool,
    stats_icon_hints: std::collections::HashMap<String, String>,
    stats_catalog: Option<std::collections::HashMap<String, (String, String)>>,
    stats_icons_attempted: std::collections::HashSet<String>,
    secondary_bars: Vec<self::monitors::SecondaryBar>,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn start(
    runtime: &RuntimeContext,
    hwnd: isize,
    panel_hwnd: isize,
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
    let media = if notch_enabled && media_enabled {
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
    if let Ok(fa) = runtime.focus_activity.try_borrow() {
        notch.set_focus_blocklist(fa.blocklist.clone());
        notch.set_focus_categories(fa.categories.clone());
    }
    notch.set_shelf_enabled(shelf_enabled);
    notch.set_clipboard_enabled(clipboard_enabled);
    if quick_capture_enabled {
        if notch_enabled {
            notch.replace_tasks(load_tasks());
        }
        notch.set_tasks_enabled(true);
    }
    notch.set_media_enabled(media_enabled);
    notch.set_privacy_enabled(privacy_enabled);
    notch.set_hud_enabled(hud_enabled);
    let any_surface_enabled = top_bar_enabled || notch_enabled;
    let status = if any_surface_enabled {
        read_system_status()
    } else {
        otto_core::SystemStatus::default()
    };
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
    let renderer = if any_surface_enabled {
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
        Some(renderer)
    } else {
        None
    };

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

    let mut io = NotchIo::new(
        layout.notch,
        layout.bar.height,
        screen.dpi,
        runtime.online_lyrics_enabled.get(),
        quick_capture,
    );
    if notch_enabled && shelf_enabled {
        io.register_shelf_targets(hwnd, panel_hwnd);
    }
    if notch_enabled && clipboard_enabled {
        io.listen_clipboard(hwnd);
    }
    let power_listener = PowerNotificationGuard::register(hwnd);

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
        io,
        power_listener,
        touch_swipe: None,
        hovered: None,
        tracking_bar_mouse: false,
        tracking_panel_mouse: false,
        maintain_ticks: 0,
        foreground_gate: ChangeGate::new(FOREGROUND_FALLBACK_TICKS),
        title_gate: ChangeGate::new(FOREGROUND_FALLBACK_TICKS),
        cadence: TickCadence::new(STATUS_POLL_MS, STATUS_IDLE_MS, STATUS_IDLE_AFTER_TICKS),
        focus: BarFocus::default(),
        focus_notch_tab_on_expand: false,
        animating: false,
        status_timer_running: false,
        settle_ticks: 0,
        top_bar_enabled,
        notch_enabled,
        control_center_enabled,
        menu_prefs: menu_bar,
        motion: motion_preference(),
        visibility: initial_visibility(menu_bar, top_bar_enabled),
        shell_full_screen: false,
        backdrop: (runtime.glass_enabled.get() && top_bar_enabled && menu_bar.show_background)
            .then(|| {
                BackdropFeed::start(
                    hwnd,
                    WM_APP_BAR_BACKDROP,
                    vec![hwnd, panel_hwnd],
                    runtime.capture_visible.get(),
                )
            }),
        focus_epoch: Instant::now(),
        last_focus_save: Instant::now(),
        last_blocked_window: None,
        last_browser_url: None,
        last_focus_relief_active: false,
        stats_icon_hints: std::collections::HashMap::new(),
        stats_catalog: None,
        stats_icons_attempted: std::collections::HashSet::new(),
        secondary_bars: Vec::new(),
    };
    if let Some(win) = bar.controls.active_window().cloned() {
        bar.remember_app_icon_hint(&win.executable, &win.executable);
        bar.remember_app_icon_hint(&win.app.key(), &win.executable);
    }
    bar.notch.set_clock(status.clock);
    bar.notch
        .set_lyrics_in_compact(bar.io.online_lyrics_enabled && bar.notch.media_enabled());
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
    bar.sync_secondary_bars(runtime);
    if !bar.shown() || !bar.visibility.is_visible() {
        bar.apply_visibility(runtime);
    }
    bar.log_started();
    bar.schedule_deadline();

    if any_surface_enabled {
        bar.ensure_status_timer();
        bar.ensure_anim_timer();
    }
    if notch_enabled && hud_enabled {
        register_bluetooth_hud_target(hwnd, WM_APP_BAR_BLUETOOTH_HUD);
    }
    set_native_minimize_intercept_enabled(runtime.genie_enabled.get());
    let _ = install_native_minimize_hook(hwnd, WM_APP_NATIVE_MINIMIZE);

    let mut slot = runtime
        .bar
        .try_borrow_mut()
        .map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    *slot = Some(bar);
    Ok(())
}

pub(crate) fn stop(runtime: &RuntimeContext) {
    set_native_minimize_intercept_enabled(false);
    uninstall_native_minimize_hook();
    unregister_idle_wake();
    unregister_bluetooth_hud_target();
    if let Ok(mut fa) = runtime.focus_activity.try_borrow_mut()
        && fa.is_dirty()
        && save_focus_activity(&fa)
    {
        fa.mark_saved();
    }
    if let Ok(mut guard) = runtime.bar.try_borrow_mut()
        && let Some(mut bar) = guard.take()
    {
        bar.teardown_secondary_bars(runtime);
        bar.stop_anim_timer();
        kill_window_timer(bar.hwnd, STATUS_TIMER);
        kill_window_timer(bar.hwnd, HUD_ANIM_TIMER);
        kill_window_timer(bar.hwnd, DEADLINE_TIMER);
        hide_window(bar.hwnd);
        hide_window(bar.panel_hwnd);
        watch_top_edge(false, bar.screen.width, bar.screen.height);
        unregister_capture_hotkey(bar.hwnd);
        bar.io.disable_quick_capture();
        stop_routing_bar_keys();
        bar.io.shutdown();
    }
}

fn is_primary_bar_window(runtime: &RuntimeContext, hwnd: isize) -> bool {
    hwnd != 0 && hwnd == runtime.bar_hwnd.get()
}

#[allow(clippy::too_many_lines)]
pub(crate) fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    wake_for_message(runtime, message);
    if !is_primary_bar_window(runtime, hwnd)
        && with_bar_result(runtime, |bar| bar.has_secondary_hwnd(hwnd)).unwrap_or(false)
    {
        return self::secondary_input::handle_secondary_message(
            runtime, hwnd, message, wparam, lparam,
        );
    }
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_QUERYENDSESSION => Some(LRESULT(1)),
        WM_CLOSE => {
            if is_primary_bar_window(runtime, hwnd) {
                unsafe { PostQuitMessage(0) };
            } else {
                hide_window(hwnd);
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
            with_bar(runtime, |bar| bar.on_quick_capture_event(runtime));
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
        WM_APP_NATIVE_MINIMIZE => {
            crate::window_controls::minimize_from_native_button(runtime, wparam.0 as isize);
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
                bar.io.pending_shelf_drag.take()
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
                bar.io.pending_shelf_drag = None;
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
            if is_primary_bar_window(runtime, hwnd) {
                with_bar(runtime, |bar| bar.rebuild(runtime));
            }
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

pub(crate) fn wake(runtime: &RuntimeContext) {
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
        NotchExpandedTab::Stats => HitTarget::NotchTabStats,
        NotchExpandedTab::Shelf => HitTarget::NotchTabShelf,
        NotchExpandedTab::Clipboard => HitTarget::NotchTabClipboard,
        NotchExpandedTab::Tasks => HitTarget::NotchTabTasks,
        NotchExpandedTab::Media => HitTarget::NotchTabMedia,
    }
}

fn is_notch_target(target: HitTarget) -> bool {
    target.scope().is_notch_surface() && target != HitTarget::MediaSeekbar
}

pub(crate) fn enter_keyboard_focus_from_dock(runtime: &RuntimeContext, from_end: bool) {
    with_bar(runtime, |bar| bar.enter_from_dock(runtime, from_end));
}

pub(crate) fn toggle_focus_mode(runtime: &RuntimeContext) {
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

pub(crate) fn open_notch_media(runtime: &RuntimeContext) {
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
                    | HitTarget::NotchTabStats
                    | HitTarget::NotchTabShelf
                    | HitTarget::NotchTabClipboard
                    | HitTarget::NotchTabTasks
                    | HitTarget::NotchTabMedia
            )
        )
        || (matches!(hit, Some(HitTarget::Notch))
            && matches!(
                active_tab,
                NotchExpandedTab::Media | NotchExpandedTab::Stats
            ))
}

const fn is_upward_touch_swipe(start: (i32, i32), current: (i32, i32), threshold: i32) -> bool {
    let dy = start.1 - current.1;
    let dx = (current.0 - start.0).abs();
    let min_dy = if threshold > 1 { threshold } else { 1 };
    dy >= min_dy && dy > dx
}

#[cfg(test)]
mod tests;
