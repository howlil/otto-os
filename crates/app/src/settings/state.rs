use std::cell::Cell;

use crate::host::{PluginId, RuntimeContext};
use otto_core::{
    AccentColor, AppearanceStyle, BarItem, DockSettingsChange, OttoSettingKind,
    OttoSettingsSnapshot, PluginSettingsState, SystemSettingsState, ThemeMode, next_dock_icon_size,
};

fn toggle_flag(flag: &Cell<bool>, pending: &Cell<Option<bool>>) {
    pending.set(Some(!flag.get()));
}

fn toggle_dock_setting(runtime: &RuntimeContext, kind: OttoSettingKind) {
    let mut prefs = runtime.dock_preferences.get();
    match kind {
        OttoSettingKind::DockHideTaskbar => prefs.hide_taskbar = !prefs.hide_taskbar,
        OttoSettingKind::DockShowDownloads => prefs.show_downloads = !prefs.show_downloads,
        OttoSettingKind::DockShowTrash => prefs.show_trash = !prefs.show_trash,
        OttoSettingKind::DockAutohide => prefs = prefs.apply(DockSettingsChange::ToggleHiding),
        OttoSettingKind::DockReserveSpace => {
            prefs = prefs.apply(DockSettingsChange::ToggleReserveSpace);
        }
        OttoSettingKind::DockMagnification => prefs.magnification = !prefs.magnification,
        _ => return,
    }
    runtime.pending_dock_preferences.set(Some(prefs));
}

fn toggle_menu_bar_setting(runtime: &RuntimeContext, kind: OttoSettingKind) {
    let mut prefs = runtime.menu_bar.get();
    match kind {
        OttoSettingKind::MenuBarBackground => prefs.show_background = !prefs.show_background,
        OttoSettingKind::MenuBarKeepNotch => prefs.keep_notch = !prefs.keep_notch,
        OttoSettingKind::MenuBarTray => {
            prefs.hidden_items = prefs.hidden_items.toggled(BarItem::Tray);
        }
        OttoSettingKind::MenuBarStatus => {
            prefs.hidden_items = prefs.hidden_items.toggled(BarItem::Status);
        }
        OttoSettingKind::MenuBarSearch => {
            prefs.hidden_items = prefs.hidden_items.toggled(BarItem::Search);
        }
        OttoSettingKind::MenuBarNotifications => {
            prefs.hidden_items = prefs.hidden_items.toggled(BarItem::Notifications);
        }
        OttoSettingKind::MenuBarClock => {
            prefs.hidden_items = prefs.hidden_items.toggled(BarItem::Clock);
        }
        _ => return,
    }
    runtime.pending_menu_bar.set(Some(prefs));
}

pub(crate) fn settings_snapshot(
    runtime: &RuntimeContext,
    launch_at_login: bool,
) -> OttoSettingsSnapshot {
    let focus_activity = runtime
        .focus_activity
        .try_borrow()
        .map(|fa| fa.clone())
        .unwrap_or_default();
    OttoSettingsSnapshot {
        online_lyrics: runtime.online_lyrics_enabled.get(),
        dock: runtime.dock_preferences.get(),
        menu_bar: runtime.menu_bar.get(),
        focus_activity,
        process_audit: otto_core::GeneralPageState::default(),
        appearance: runtime.appearance_preferences.get(),
        system_appearance: runtime.system_appearance.get(),
        plugins: PluginSettingsState {
            top_bar: runtime.top_bar_enabled.get(),
            notch: runtime.notch_enabled.get(),
            dock: runtime.dock_enabled.get(),
            window_controls: runtime.window_controls_enabled.get(),
            genie: runtime.genie_enabled.get(),
        },
        system: SystemSettingsState {
            launch_at_login,
            hotkeys_enabled: runtime.hotkeys_enabled.get(),
            capture_visible: runtime.capture_visible.get(),
            classic_logo: runtime.classic_logo_enabled.get(),
            dynamic_island: runtime.dynamic_island_enabled.get(),
            privacy_indicators: runtime.privacy_indicators_enabled.get(),
            clipboard_history: runtime.clipboard_history_enabled.get(),
            drop_shelf: runtime.drop_shelf_enabled.get(),
            notch_hud: runtime.notch_hud_enabled.get(),
            media_module: runtime.media_module_enabled.get(),
            focus_module: runtime.focus_module_enabled.get(),
            background_cleanup: runtime.background_cleanup_enabled.get(),
            quick_capture: runtime.quick_capture_enabled.get(),
        },
    }
}

pub(super) fn cycle_dock_icon_size(runtime: &RuntimeContext) {
    let mut prefs = runtime.dock_preferences.get();
    prefs.icon_size = next_dock_icon_size(prefs.icon_size);
    runtime.pending_dock_preferences.set(Some(prefs));
}

pub(super) fn cycle_menu_bar_hide(runtime: &RuntimeContext) {
    let mut prefs = runtime.menu_bar.get();
    prefs.hide_mode = prefs.hide_mode.next();
    runtime.pending_menu_bar.set(Some(prefs));
}

pub(super) fn set_appearance_style(runtime: &RuntimeContext, style: AppearanceStyle) {
    let mut prefs = runtime
        .pending_appearance
        .get()
        .unwrap_or_else(|| runtime.appearance_preferences.get());
    prefs.style = style;
    runtime.pending_appearance.set(Some(prefs));
}

pub(super) fn set_appearance_theme(runtime: &RuntimeContext, theme: ThemeMode) {
    let mut prefs = runtime
        .pending_appearance
        .get()
        .unwrap_or_else(|| runtime.appearance_preferences.get());
    prefs.theme = theme;
    runtime.pending_appearance.set(Some(prefs));
}

pub(super) fn set_appearance_accent(runtime: &RuntimeContext, accent: AccentColor) {
    let mut prefs = runtime
        .pending_appearance
        .get()
        .unwrap_or_else(|| runtime.appearance_preferences.get());
    prefs.accent = accent;
    runtime.pending_appearance.set(Some(prefs));
}

#[allow(clippy::too_many_lines)]
pub(crate) fn toggle_setting(runtime: &RuntimeContext, kind: OttoSettingKind) {
    match kind {
        OttoSettingKind::DynamicIsland => {
            toggle_flag(
                &runtime.dynamic_island_enabled,
                &runtime.pending_dynamic_island,
            );
        }
        OttoSettingKind::TopBar => {
            runtime.queue_plugin_toggle(PluginId::TopBar, !runtime.top_bar_enabled.get());
        }
        OttoSettingKind::Notch => {
            toggle_flag(&runtime.notch_enabled, &runtime.pending_notch_enabled);
        }
        OttoSettingKind::Dock => {
            runtime.queue_plugin_toggle(PluginId::Dock, !runtime.dock_enabled.get());
        }
        OttoSettingKind::WindowControls => {
            runtime.queue_plugin_toggle(
                PluginId::WindowControls,
                !runtime.window_controls_enabled.get(),
            );
        }
        OttoSettingKind::Genie => {
            runtime.queue_plugin_toggle(PluginId::Genie, !runtime.genie_enabled.get());
        }
        OttoSettingKind::PrivacyIndicators => {
            toggle_flag(
                &runtime.privacy_indicators_enabled,
                &runtime.pending_privacy_indicators,
            );
        }
        OttoSettingKind::ClipboardHistory => {
            toggle_flag(
                &runtime.clipboard_history_enabled,
                &runtime.pending_clipboard_history,
            );
        }
        OttoSettingKind::DropShelf => {
            toggle_flag(&runtime.drop_shelf_enabled, &runtime.pending_drop_shelf);
        }
        OttoSettingKind::NotchHud => {
            toggle_flag(&runtime.notch_hud_enabled, &runtime.pending_notch_hud);
        }
        OttoSettingKind::BackgroundCleanup => {
            toggle_flag(
                &runtime.background_cleanup_enabled,
                &runtime.pending_background_cleanup,
            );
        }
        OttoSettingKind::MediaModule => {
            toggle_flag(&runtime.media_module_enabled, &runtime.pending_media_module);
        }
        OttoSettingKind::FocusModule => {
            toggle_flag(&runtime.focus_module_enabled, &runtime.pending_focus_module);
        }
        OttoSettingKind::QuickCapture => {
            toggle_flag(
                &runtime.quick_capture_enabled,
                &runtime.pending_quick_capture,
            );
        }
        OttoSettingKind::DockHideTaskbar
        | OttoSettingKind::DockShowDownloads
        | OttoSettingKind::DockShowTrash
        | OttoSettingKind::DockAutohide
        | OttoSettingKind::DockReserveSpace
        | OttoSettingKind::DockMagnification => toggle_dock_setting(runtime, kind),
        OttoSettingKind::CaptureVisible => {
            toggle_flag(&runtime.capture_visible, &runtime.pending_capture_visible);
        }
        OttoSettingKind::OnlineLyrics => {
            toggle_flag(
                &runtime.online_lyrics_enabled,
                &runtime.pending_online_lyrics,
            );
        }
        OttoSettingKind::LaunchAtLogin => {
            if !otto_platform::set_launch_at_login(!otto_platform::read_launch_at_login()) {
                otto_platform::signal_command_failure();
            }
        }
        OttoSettingKind::Hotkeys => {
            toggle_flag(&runtime.hotkeys_enabled, &runtime.pending_hotkeys_enabled);
        }
        OttoSettingKind::ClassicLogo => {
            toggle_flag(&runtime.classic_logo_enabled, &runtime.pending_classic_logo);
        }
        OttoSettingKind::MenuBarBackground
        | OttoSettingKind::MenuBarKeepNotch
        | OttoSettingKind::MenuBarTray
        | OttoSettingKind::MenuBarStatus
        | OttoSettingKind::MenuBarSearch
        | OttoSettingKind::MenuBarNotifications
        | OttoSettingKind::MenuBarClock => toggle_menu_bar_setting(runtime, kind),
    }
}
