use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_DOCK_BACKDROP;
use otto_core::{
    CommandOutcome, DockEffect, DockEntry, DockEntryOptions, DockItemKind, DockPort,
    DockPreferences, Rect, WindowCommand, WindowIdentity, dock_activation_effect,
    dock_command_effect,
};
use otto_platform::{
    WindowsDockPort, current_process_id, resume_recycle_bin_task, shutdown_recycle_bin_task,
    signal_command_failure,
};
use otto_render::DockRenderer;

use super::with_dock;

pub(super) fn report(outcome: CommandOutcome) {
    if !matches!(
        outcome,
        CommandOutcome::Completed | CommandOutcome::Cancelled
    ) {
        signal_command_failure();
    }
}

pub(super) fn show_window(runtime: &RuntimeContext, window: WindowIdentity, target: Option<Rect>) {
    if WindowsDockPort.is_window_minimized(window.hwnd)
        && crate::genie::restore_window_to(runtime, window.hwnd, target)
    {
        return;
    }
    report(WindowsDockPort.window_command(window, WindowCommand::Focus, current_process_id()));
}

pub fn apply_preferences(runtime: &RuntimeContext, preferences: DockPreferences) {
    with_dock(runtime, |dock| {
        let trash_changed = dock.preferences.show_trash != preferences.show_trash;
        let entries_changed =
            dock.preferences.show_downloads != preferences.show_downloads || trash_changed;
        dock.preferences = preferences;
        dock.metrics = preferences.metrics();
        if trash_changed {
            if preferences.show_trash {
                resume_recycle_bin_task();
            } else {
                shutdown_recycle_bin_task();
            }
        }
        if !preferences.show_downloads {
            dock.downloads_progress = None;
        }
        dock.ensure_animating();
        if entries_changed {
            dock.refresh_entries(runtime);
        }
        dock.rebuild(runtime);
    });
}

pub fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_dock(runtime, |dock| {
        if enabled {
            if dock.backdrop.is_none() {
                dock.backdrop = Some(BackdropFeed::start(
                    dock.hwnd,
                    WM_APP_DOCK_BACKDROP,
                    vec![dock.hwnd],
                    runtime.capture_visible.get(),
                ));
            }
        } else {
            dock.backdrop = None;
            dock.renderer.clear_backdrop();
            dock.present(runtime);
        }
        dock.sync_backdrop(runtime, true);
    });
}

pub fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_dock(runtime, |dock| {
        if let Some(feed) = dock.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        if visible {
            dock.renderer.clear_backdrop();
        }
        dock.sync_backdrop(runtime, true);
        dock.present(runtime);
    });
}

pub fn activate_overflow_entry(runtime: &RuntimeContext, entry: &DockEntry) {
    activate_entry(runtime, entry, None);
}

pub(super) fn activate_entry(runtime: &RuntimeContext, entry: &DockEntry, target: Option<Rect>) {
    let effect = dock_activation_effect(entry.app.as_ref());
    if !entry.active
        && matches!(effect, DockEffect::ShowWindow(window) if !WindowsDockPort.is_window_minimized(window.hwnd))
        && let Some(app) = entry.app.as_ref()
    {
        report(WindowsDockPort.show_application(app, current_process_id()));
        return;
    }
    execute_dock_effect(runtime, effect, entry, target);
}

pub fn execute_dock_menu_command(
    runtime: &RuntimeContext,
    cmd: usize,
    entry: &DockEntry,
    target: Option<Rect>,
) {
    if let Some(effect) = dock_command_effect(cmd, entry.app.as_ref()) {
        execute_dock_effect(runtime, effect, entry, target);
    }
}

pub(super) struct PortEffectResult {
    pub(super) outcome: Option<CommandOutcome>,
    pub(super) refresh_entries: bool,
}

pub(super) fn apply_port_effect<P: DockPort>(
    port: &P,
    effect: &DockEffect,
    entry: &DockEntry,
    own_pid: u32,
) -> PortEffectResult {
    let mut result = PortEffectResult {
        outcome: None,
        refresh_entries: false,
    };
    match effect {
        DockEffect::Launch => port.launch_new_instance(entry),
        DockEffect::TogglePin => {
            port.toggle_pin(entry);
            result.refresh_entries = true;
        }
        DockEffect::EmptyTrash => port.empty_recycle_bin(),
        DockEffect::RevealInExplorer => port.reveal_in_explorer(entry),
        DockEffect::ForceQuit => {
            result.outcome = Some(port.force_quit(entry, own_pid));
            result.refresh_entries = true;
        }
        DockEffect::Unhide | DockEffect::Hide | DockEffect::HideOthers | DockEffect::Quit => {
            if let Some(app) = entry.app.as_ref() {
                result.outcome = Some(match effect {
                    DockEffect::Unhide => port.show_application(app, own_pid),
                    DockEffect::Hide => port.hide_application(app, own_pid),
                    DockEffect::HideOthers => port.hide_other_applications(&app.identity, own_pid),
                    _ => port.quit_application(app, own_pid),
                });
            }
        }
        DockEffect::Fail(outcome) => result.outcome = Some(*outcome),
        DockEffect::ShowWindow(_) => {}
    }
    result
}

pub(super) fn execute_dock_effect(
    runtime: &RuntimeContext,
    effect: DockEffect,
    entry: &DockEntry,
    target: Option<Rect>,
) {
    match effect {
        DockEffect::Launch if matches!(entry.kind, DockItemKind::Apps) => {
            crate::spotlight::toggle_apps(runtime, target);
        }
        DockEffect::ShowWindow(window) => show_window(runtime, window, target),
        other => {
            let result = apply_port_effect(&WindowsDockPort, &other, entry, current_process_id());
            if let Some(outcome) = result.outcome {
                report(outcome);
            }
            if result.refresh_entries {
                with_dock(runtime, |dock| dock.refresh_entries(runtime));
            }
        }
    }
}

pub(super) fn count_entries(entries: &[DockEntry]) -> (usize, usize, bool) {
    let right_count = entries
        .iter()
        .filter(|e| matches!(e.kind, DockItemKind::Downloads | DockItemKind::Trash { .. }))
        .count();
    let has_trash = entries
        .last()
        .is_some_and(|e| matches!(e.kind, DockItemKind::Trash { .. }));
    let app_count = entries.len().saturating_sub(right_count);
    (app_count, right_count, has_trash)
}

pub(super) fn sync_icons(renderer: &mut DockRenderer, entries: &[DockEntry]) {
    for entry in entries {
        if entry.apple_icon.is_none()
            && !renderer.has_icon(&entry.key)
            && let Some(bmp) = WindowsDockPort.icon_for_entry(entry)
        {
            let _ = renderer.set_icon(entry.key.clone(), bmp.width, bmp.height, &bmp.pixels);
        }
    }
    let alive: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    renderer.retain_icons(&alive);
}

pub(super) const fn entry_options(preferences: DockPreferences) -> DockEntryOptions {
    DockEntryOptions {
        show_downloads: preferences.show_downloads,
        show_trash: preferences.show_trash,
    }
}
