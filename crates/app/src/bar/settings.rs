use crate::bar_controls::BarControlsController;
use crate::host::RuntimeContext;
use crate::win_msg::{WM_APP_BAR_BLUETOOTH_HUD, WM_APP_BAR_CAPTURE};
use otto_core::Rect;
use otto_core::{
    Appearance, BarHideMode, BrandIcon, HitTarget, MenuBarPreferences, Metrics, MotionPreference,
    NotchPort, NotchStyle,
};
use otto_platform::{
    QuickCaptureInput, load_tasks, register_bluetooth_hud_target, register_capture_hotkey,
    stop_idle_connectivity_task, unregister_bluetooth_hud_target, unregister_capture_hotkey,
};

use super::is_notch_target;
use super::with_bar;
use super::with_bar_result;

pub(crate) fn refresh_plugin_layout(runtime: &RuntimeContext, control_center_enabled: bool) {
    with_bar(runtime, |bar| {
        bar.control_center_enabled = control_center_enabled;
        bar.recompute_layout();
        bar.present(runtime);
    });
}

pub(crate) fn activate_hidden_target(runtime: &RuntimeContext, target: HitTarget) {
    with_bar(runtime, |bar| {
        if let Some(action) = BarControlsController::click_action(target) {
            bar.perform(runtime, action);
        }
    });
}

pub(crate) fn apply_appearance(runtime: &RuntimeContext, appearance: Appearance) {
    with_bar(runtime, |bar| {
        bar.sync_backdrop(runtime, true);
        bar.present(runtime);
    });
    crate::window_controls::apply_appearance(runtime, appearance);
}

pub(crate) fn set_online_lyrics_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.io.online_lyrics_enabled = enabled;
        bar.notch
            .set_lyrics_in_compact(enabled && bar.notch.media_enabled());
        if enabled {
            bar.io.forget_lyrics_track();
        } else {
            bar.io.stop_lyrics();
        }
        if bar.notch_enabled && bar.notch.media_enabled() {
            bar.request_lyrics_for_current_track();
        }
        bar.recompute_layout();
        bar.present(runtime);
        bar.schedule_deadline();
    });
}

pub(crate) fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if !enabled {
            bar.backdrop = None;
            if let Some(renderer) = bar.renderer.as_ref() {
                renderer.clear_bar_backdrop();
            }
            bar.clear_secondary_backdrops();
            bar.present(runtime);
        }
        bar.sync_backdrop(runtime, true);
    });
}

pub(crate) fn set_top_bar_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if bar.top_bar_enabled == enabled {
            return;
        }
        bar.top_bar_enabled = enabled;
        if enabled {
            bar.ensure_status_timer();
            let _ = bar.ensure_renderer(runtime);
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
        bar.sync_secondary_bars(runtime);
        bar.sync_backdrop(runtime, true);
        bar.apply_visibility(runtime);
        bar.sync_panel_visibility();
        bar.sync_top_edge_watches();
        if bar.shown() {
            bar.present(runtime);
        }
        bar.schedule_deadline();
        if !enabled {
            bar.release_renderer_if_unused(runtime);
        }
    });
    crate::window_controls::sync_bar_height(runtime);
}

pub(crate) fn set_notch_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if bar.notch_enabled == enabled {
            return;
        }
        bar.notch_enabled = enabled;
        if enabled {
            bar.ensure_status_timer();
            let _ = bar.ensure_renderer(runtime);
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
                bar.notch.replace_tasks(load_tasks());
                bar.io.enable_quick_capture(bar.hwnd);
                if runtime.hotkeys_enabled.get() && bar.io.quick_capture.is_some() {
                    register_capture_hotkey(bar.hwnd);
                }
                bar.notch.set_tasks_enabled(bar.io.quick_capture.is_some());
            }
            if bar.notch.privacy_enabled() {
                let _ = bar
                    .notch
                    .sync_privacy_state(bar.notch_port.read_privacy_state());
            }
            if bar.notch.media_enabled() {
                let latest_media = bar.notch_port.read_now_playing();
                let _ = bar.notch.set_media(latest_media);
                bar.request_lyrics_for_current_track();
            }
        } else {
            bar.io.release_shelf();
            bar.io.stop_clipboard();
            bar.io.stop_lyrics();
            unregister_bluetooth_hud_target();
            unregister_capture_hotkey(bar.hwnd);
            bar.io.disable_quick_capture();
            bar.io.seek_drag_hwnd = None;
            bar.touch_swipe = None;
            bar.notch.release_all_resources();
            bar.stats_icons_attempted.clear();
            bar.stats_catalog = None;
            if let Some(renderer) = bar.renderer.as_ref() {
                renderer.release_notch_resources();
            }
            if bar.hovered.is_some_and(is_notch_target) {
                bar.hovered = None;
            }
            otto_platform::withdraw_notch_rect(bar.hwnd);
            if !crate::control_center::is_open(runtime) {
                stop_idle_connectivity_task();
            }
        }
        bar.recompute_layout();
        bar.apply_visibility(runtime);
        bar.sync_panel_visibility();
        bar.sync_top_edge_watches();
        if bar.shown() {
            bar.present(runtime);
        }
        bar.schedule_deadline();
        if !enabled {
            bar.release_renderer_if_unused(runtime);
        }
    });
}

pub(crate) fn leave_keyboard_focus(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| {
        if bar.focus.is_active() {
            bar.leave_keyboard_focus(runtime);
        }
    });
}

#[must_use]
pub(crate) fn control_center_anchor(
    runtime: &RuntimeContext,
    trigger: HitTarget,
) -> Option<(i32, i32, MotionPreference)> {
    with_bar_result(runtime, |bar| {
        bar.top_bar_enabled.then(|| {
            bar.secondary_anchor_for_cursor(trigger).unwrap_or_else(|| {
                (
                    bar.anchor_right(trigger),
                    bar.layout.popup_anchor_y(bar.screen.dpi),
                    bar.motion,
                )
            })
        })
    })
    .flatten()
}

#[must_use]
pub(crate) fn search_slot_trigger(runtime: &RuntimeContext) -> Option<Rect> {
    with_bar_result(runtime, |bar| {
        if !bar.top_bar_enabled {
            return None;
        }
        bar.secondary_search_trigger_for_cursor().or_else(|| {
            (bar.layout.slots.search_slot.width > 0).then_some(bar.layout.slots.search_slot)
        })
    })
    .flatten()
}

pub(crate) fn set_privacy_indicators_enabled(runtime: &RuntimeContext, enabled: bool) {
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

pub(crate) fn set_clipboard_history_enabled(runtime: &RuntimeContext, enabled: bool) {
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
        if bar.shown() && (enabled || bar.panel_visible || bar.animating) {
            bar.ensure_anim_timer();
        }
    });
}

pub(crate) fn set_drop_shelf_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_shelf_enabled(enabled);
        if enabled && bar.notch_enabled {
            bar.ensure_shelf_drop_targets();
        } else {
            bar.io.release_shelf();
            if let Some(renderer) = bar.renderer.as_ref() {
                renderer.clear_shelf_caches();
            }
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        if bar.shown() && (enabled || bar.panel_visible || bar.animating) {
            bar.ensure_anim_timer();
        }
    });
}

pub(crate) fn set_menu_bar_preferences(runtime: &RuntimeContext, preferences: MenuBarPreferences) {
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
        bar.apply_secondary_menu_mode(runtime);
        bar.sync_top_edge_watches();
        bar.present(runtime);
        bar.schedule_deadline();
    });
    crate::window_controls::sync_bar_height(runtime);
}

pub(crate) fn set_notch_hud_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_hud_enabled(enabled);
        if enabled && bar.notch_enabled {
            register_bluetooth_hud_target(bar.hwnd, WM_APP_BAR_BLUETOOTH_HUD);
        } else {
            unregister_bluetooth_hud_target();
            if !crate::control_center::is_open(runtime) {
                stop_idle_connectivity_task();
            }
        }
        bar.recompute_layout();
        bar.present(runtime);
        bar.schedule_deadline();
    });
}

pub(crate) fn set_media_module_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_media_enabled(enabled);
        bar.notch
            .set_lyrics_in_compact(enabled && bar.io.online_lyrics_enabled);
        if enabled && bar.notch_enabled {
            bar.io.invalidate_lyrics();
            let latest_media = bar.notch_port.read_now_playing();
            let _ = bar.notch.set_media(latest_media);
            bar.request_lyrics_for_current_track();
        } else {
            bar.io.stop_lyrics();
            if let Some(renderer) = bar.renderer.as_ref() {
                renderer.clear_media_caches();
            }
        }
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        if bar.shown() && (enabled || bar.panel_visible || bar.animating) {
            bar.ensure_anim_timer();
        }
        bar.schedule_deadline();
    });
}

pub(crate) fn set_focus_module_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        bar.notch.set_focus_enabled(enabled);
        runtime.focus_active.set(bar.notch.focus_active());
        bar.recompute_layout();
        bar.sync_panel_visibility();
        bar.present(runtime);
        if bar.shown() && (enabled || bar.panel_visible || bar.animating) {
            bar.ensure_anim_timer();
        }
        bar.schedule_deadline();
    });
}

pub(crate) fn sync_focus_blocklist(runtime: &RuntimeContext) {
    let synced = runtime
        .focus_activity
        .try_borrow()
        .ok()
        .map(|fa| (fa.blocklist.clone(), fa.categories.clone()));
    if let Some((blocklist, categories)) = synced {
        with_bar(runtime, |bar| {
            bar.notch.set_focus_blocklist(blocklist);
            bar.notch.set_focus_categories(categories);
            bar.recompute_layout();
            bar.present(runtime);
        });
    }
}

#[must_use]
pub(crate) fn focus_session_status(runtime: &RuntimeContext) -> otto_core::FocusSessionStatus {
    with_bar_result(runtime, |bar| {
        if bar.notch_enabled {
            bar.notch.focus_session_status()
        } else {
            otto_core::FocusSessionStatus::NotRunning
        }
    })
    .unwrap_or_default()
}

pub(crate) fn set_dock_taskbar_hidden(runtime: &RuntimeContext, hidden: bool) {
    with_bar(runtime, |bar| {
        if hidden {
            bar.appbar.hide_system_taskbar();
        } else {
            bar.appbar.show_system_taskbar();
        }
        bar.present(runtime);
    });
}

pub(crate) fn set_hotkeys_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_bar(runtime, |bar| {
        if enabled && bar.notch_enabled && bar.io.quick_capture.is_some() {
            register_capture_hotkey(bar.hwnd);
        } else {
            unregister_capture_hotkey(bar.hwnd);
        }
    });
}

pub(crate) fn set_quick_capture_enabled(runtime: &RuntimeContext, enabled: bool) {
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
        if bar.shown() && (enabled || bar.panel_visible || bar.animating) {
            bar.ensure_anim_timer();
        }
    });
}

pub(crate) fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_bar(runtime, |bar| {
        if let Some(feed) = bar.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        if visible && let Some(renderer) = bar.renderer.as_ref() {
            renderer.clear_bar_backdrop();
        }
        bar.apply_secondary_capture_visible(visible);
        bar.sync_backdrop(runtime, true);
        bar.present(runtime);
    });
}

pub(crate) fn set_brand_icon(runtime: &RuntimeContext, brand_icon: BrandIcon) {
    with_bar(runtime, |bar| {
        if let Some(renderer) = bar.renderer.as_ref() {
            renderer.set_brand_icon(brand_icon);
        }
        bar.apply_secondary_brand_icon(brand_icon);
        bar.present(runtime);
    });
}

pub(crate) fn set_notch_style(runtime: &RuntimeContext, style: NotchStyle) {
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
    crate::window_controls::set_notch_style(runtime, style);
}
