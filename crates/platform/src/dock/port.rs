use std::collections::HashMap;

use otto_core::{
    AppIdentity, AppInfo, CommandOutcome, DockEntry, DockEntryOptions, DockPort, IconBitmap, Rect,
    StableOrder, WindowCommand, WindowIdentity,
};

use super::{
    collect_dock_entries_with_options, force_quit_entry, fullscreen_on_monitor,
    fullscreen_on_primary, launch_new_instance, open_file_with_dock_entry, overflow_dock_entry,
    pin_entry_at_target, remove_pinned_app, reorder_pinned_apps, reveal_entry_in_explorer,
    toggle_pin_entry, window_overlaps_zone,
};
use super::{extract_dock_icon, extract_window_icon, read_downloads_active_progress};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsDockPort;

impl DockPort for WindowsDockPort {
    fn collect_entries(
        &self,
        own_pid: u32,
        order: &mut StableOrder,
        options: DockEntryOptions,
    ) -> Vec<DockEntry> {
        collect_dock_entries_with_options(own_pid, order, options)
    }

    fn overflow_entry(&self, hidden: usize) -> DockEntry {
        overflow_dock_entry(hidden)
    }

    fn icon_for_entry(&self, entry: &DockEntry) -> Option<IconBitmap> {
        extract_dock_icon(entry)
    }

    fn icon_for_window(&self, hwnd: isize) -> Option<IconBitmap> {
        extract_window_icon(hwnd)
    }

    fn downloads_progress(&self) -> Option<f32> {
        read_downloads_active_progress()
    }

    fn notification_counts(&self) -> HashMap<String, u32> {
        crate::notch::notifications::read_notification_app_counts()
    }

    fn is_window_minimized(&self, hwnd: isize) -> bool {
        crate::window::ops::is_window_minimized(hwnd)
    }

    fn window_overlaps_zone(&self, zone: Rect, own_pid: u32) -> bool {
        window_overlaps_zone(zone, own_pid)
    }

    fn fullscreen_on_primary(&self, width: i32, height: i32, own_pid: u32) -> bool {
        fullscreen_on_primary(width, height, own_pid)
    }

    fn fullscreen_on_monitor(&self, bounds: Rect, own_pid: u32) -> bool {
        fullscreen_on_monitor(bounds, own_pid)
    }

    fn toggle_pin(&self, entry: &DockEntry) {
        toggle_pin_entry(entry);
    }

    fn remove_pin(&self, key: &str) -> bool {
        remove_pinned_app(key)
    }

    fn reorder_pins(&self, moved_key: &str, target_key: &str) -> bool {
        reorder_pinned_apps(moved_key, target_key)
    }

    fn pin_at_target(&self, entry: &DockEntry, target_key: &str) -> bool {
        pin_entry_at_target(entry, target_key)
    }

    fn launch_new_instance(&self, entry: &DockEntry) {
        launch_new_instance(entry);
    }

    fn open_file_with(&self, entry: &DockEntry, path: &str) {
        open_file_with_dock_entry(entry, path);
    }

    fn reveal_in_explorer(&self, entry: &DockEntry) {
        reveal_entry_in_explorer(entry);
    }

    fn force_quit(&self, entry: &DockEntry, own_pid: u32) -> CommandOutcome {
        force_quit_entry(entry, own_pid)
    }

    fn window_command(
        &self,
        target: WindowIdentity,
        command: WindowCommand,
        own_pid: u32,
    ) -> CommandOutcome {
        crate::window::model::execute_window_command(target, command, own_pid)
    }

    fn hide_application(&self, app: &AppInfo, own_pid: u32) -> CommandOutcome {
        crate::window::model::hide_application(app, own_pid)
    }

    fn show_application(&self, app: &AppInfo, own_pid: u32) -> CommandOutcome {
        crate::window::model::show_application(app, own_pid)
    }

    fn quit_application(&self, app: &AppInfo, own_pid: u32) -> CommandOutcome {
        crate::window::model::quit_application(app, own_pid)
    }

    fn hide_other_applications(&self, keep: &AppIdentity, own_pid: u32) -> CommandOutcome {
        crate::window::model::hide_other_applications(keep, own_pid)
    }

    fn empty_recycle_bin(&self) {
        crate::dock::trash::empty_recycle_bin();
    }
}
