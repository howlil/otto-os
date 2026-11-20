use crate::backdrop_feed::BackdropFeed;
use crate::bar_controls::BarControlsController;
use crate::host::RuntimeContext;
use crate::win_msg::{WM_APP_BAR_BACKDROP, WM_APP_BAR_BLUETOOTH_HUD, WM_APP_BAR_CAPTURE};
use otto_core::Rect;
use otto_core::{
    Appearance, BarHideMode, BrandIcon, HitTarget, MenuBarPreferences, Metrics, MotionPreference,
    NotchPort, NotchStyle,
};
use otto_platform::{
    QuickCaptureInput, load_tasks, register_bluetooth_hud_target, register_capture_hotkey,
    unregister_bluetooth_hud_target, unregister_capture_hotkey, watch_top_edge,
};

use super::is_notch_target;
use super::with_bar;
use super::with_bar_result;

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
        bar.io.online_lyrics_enabled = enabled;
        bar.notch
            .set_lyrics_in_compact(enabled && bar.notch.media_enabled());
        bar.io.forget_lyrics_track();
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
            if bar.notch.clipboard_enabled() {
                bar.io.listen_clipboard(bar.hwnd);
            }
            if bar.notch.hud_enabled() {
                register_bluetooth_hud_target(bar.hwnd, WM_APP_BAR_BLUETOOTH_HUD);
            }
            if runtime.quick_capture_enabled.get() {
                bar.io.enable_quick_capture(bar.hwnd);
                if runtime.hotkeys_enabled.get() && bar.io.quick_capture.is_some() {
                    register_capture_hotkey(bar.hwnd);
                }
            }
            if bar.notch.privacy_enabled() {
                let _ = bar
                    .notch
                    .sync_privacy_state(bar.notch_port.read_privacy_state());
            }
        } else {
            bar.io.release_shelf();
            bar.io.stop_clipboard();
            unregister_bluetooth_hud_target();
            unregister_capture_hotkey(bar.hwnd);
            bar.io.disable_quick_capture();
            bar.io.seek_drag_hwnd = None;
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
        (bar.top_bar_enabled && bar.layout.slots.search_slot.width > 0)
            .then_some(bar.layout.slots.search_slot)
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
            bar.io.listen_clipboard(bar.hwnd);
        } else {
            bar.io.stop_clipboard();
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
            bar.io.release_shelf();
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        bar.ensure_anim_timer();
    });
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
            .set_lyrics_in_compact(enabled && bar.io.online_lyrics_enabled);
        bar.io.invalidate_lyrics();
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
        if enabled && bar.notch_enabled && bar.io.quick_capture.is_some() {
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
            if bar.notch_enabled && bar.io.quick_capture.is_none() {
                bar.io.quick_capture = QuickCaptureInput::create(bar.hwnd, WM_APP_BAR_CAPTURE);
            }
            if bar.notch_enabled && runtime.hotkeys_enabled.get() && bar.io.quick_capture.is_some()
            {
                register_capture_hotkey(bar.hwnd);
            }
            bar.notch
                .set_tasks_enabled(!bar.notch_enabled || bar.io.quick_capture.is_some());
        } else {
            unregister_capture_hotkey(bar.hwnd);
            bar.io.quick_capture = None;
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
