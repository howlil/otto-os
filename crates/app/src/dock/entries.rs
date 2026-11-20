use crate::host::RuntimeContext;
use otto_core::{
    DockEntry, DockItemKind, DockLayout, DockPort, Rect, dock_capacity, scale, split_dock_overflow,
};

use super::Dock;
use super::actions::activate_entry;
use super::actions::count_entries;
use super::actions::entry_options;
use super::actions::sync_icons;

impl Dock {
    pub(super) fn refresh_entries(&mut self, runtime: &RuntimeContext) {
        self.entries = self.port.collect_entries(
            self.own_pid,
            &mut self.order,
            entry_options(self.preferences),
        );
        for entry in &self.entries {
            self.presence.entry(entry.key.clone()).or_insert(1.0);
        }
        let updated = self.combined_entries();
        sync_icons(&mut self.renderer, &updated);
        self.recompute_layout();
        self.present(runtime);
    }

    pub(super) fn combined_entries(&self) -> Vec<DockEntry> {
        self.displayed_entries().0
    }

    pub(super) fn base_displayed_entries(&self) -> (Vec<DockEntry>, Vec<DockEntry>) {
        let mut all = self.all_entries();
        let right_items = all
            .iter()
            .rev()
            .take_while(|entry| {
                matches!(
                    entry.kind,
                    DockItemKind::Downloads | DockItemKind::Trash { .. }
                )
            })
            .count();
        let app_count = all.len() - right_items;
        let capacity = dock_capacity(
            &self.metrics,
            self.screen.width,
            self.screen.dpi,
            right_items,
        );
        let split = split_dock_overflow(app_count, capacity);
        if split.hidden_apps == 0 {
            return (all, Vec::new());
        }
        let right = all.split_off(app_count);
        let hidden = all.split_off(split.visible_apps);
        all.push(self.port.overflow_entry(hidden.len()));
        all.extend(right);
        (all, hidden)
    }

    pub(super) fn displayed_entries(&self) -> (Vec<DockEntry>, Vec<DockEntry>) {
        let (mut visible, hidden) = self.base_displayed_entries();
        if self.drag.is_dragging
            && let (Some(from), Some(to)) = (self.drag.start_idx, self.drag.hover_idx)
        {
            let (app_count, _, _) = count_entries(&visible);
            if from < app_count && to < app_count && from != to {
                let order =
                    DockLayout::preview_reordered_indices(visible.len(), Some(from), Some(to));
                visible = order.into_iter().map(|i| visible[i].clone()).collect();
            }
        }
        (visible, hidden)
    }

    pub(super) fn all_entries(&self) -> Vec<DockEntry> {
        if self.closing_entries.is_empty() {
            return self.entries.clone();
        }
        let mut list = Vec::with_capacity(self.entries.len() + self.closing_entries.len());
        let has_trash = self
            .entries
            .last()
            .is_some_and(|e| matches!(e.kind, DockItemKind::Trash { .. }));
        let app_slice = if has_trash {
            &self.entries[..self.entries.len() - 1]
        } else {
            &self.entries[..]
        };
        list.extend_from_slice(app_slice);
        for closing in &self.closing_entries {
            if !list.iter().any(|e| e.key == closing.key) {
                list.push(closing.clone());
            }
        }
        if has_trash && let Some(trash) = self.entries.last() {
            list.push(trash.clone());
        }
        list
    }

    pub(super) fn refresh_entries_if_changed(&mut self, topology: Option<u64>) -> bool {
        if !self.entries_gate.should_refresh(topology) {
            return false;
        }
        let latest = self.port.collect_entries(
            self.own_pid,
            &mut self.order,
            entry_options(self.preferences),
        );
        if latest == self.entries {
            return false;
        }
        let mut need_anim = false;
        for new_e in &latest {
            if !self.entries.iter().any(|old_e| old_e.key == new_e.key) {
                self.presence.insert(new_e.key.clone(), 0.08);
                need_anim = true;
            }
            if new_e.running
                && !matches!(new_e.kind, DockItemKind::Trash { .. })
                && !self
                    .entries
                    .iter()
                    .any(|old_e| old_e.key == new_e.key && old_e.running)
            {
                self.anim.bounce_key = Some(new_e.key.clone());
                self.anim.bounce_phase = 0.01;
                need_anim = true;
            }
        }

        for old_e in &self.entries {
            if !matches!(
                old_e.kind,
                DockItemKind::Finder
                    | DockItemKind::Apps
                    | DockItemKind::Downloads
                    | DockItemKind::Trash { .. }
            ) && !latest.iter().any(|new_e| new_e.key == old_e.key)
                && !self.closing_entries.iter().any(|c| c.key == old_e.key)
            {
                let mut ghost = old_e.clone();
                ghost.running = false;
                ghost.active = false;
                self.closing_entries.push(ghost);
                need_anim = true;
            }
        }
        if need_anim {
            self.ensure_animating();
        }

        self.closing_entries
            .retain(|c| !latest.iter().any(|n| n.key == c.key));
        self.entries = latest;
        let all = self.combined_entries();
        sync_icons(&mut self.renderer, &all);
        self.recompute_layout();
        true
    }

    pub(super) fn activate_index(&mut self, runtime: &RuntimeContext, idx: usize) {
        let all = self.combined_entries();
        let Some(entry) = all.get(idx).cloned() else {
            return;
        };
        if matches!(
            entry.kind,
            DockItemKind::Downloads | DockItemKind::Overflow { .. }
        ) {
            let Some(slot) = self.layout.items.get(idx).copied() else {
                return;
            };
            let (origin_x, origin_y) = self.window_origin();
            let center_x = origin_x + (slot.x + slot.size * 0.5).round() as i32;
            let bottom_y = origin_y + slot.y.round() as i32 - scale(10, self.screen.dpi);
            if matches!(entry.kind, DockItemKind::Downloads) {
                crate::menu::open_downloads_stack_menu(runtime, center_x, bottom_y);
            } else {
                let hidden = self.displayed_entries().1;
                crate::menu::open_dock_overflow_menu(runtime, center_x, bottom_y, hidden);
            }
            return;
        }
        if self.motion.allows_motion()
            && !entry.running
            && matches!(entry.kind, DockItemKind::App { .. })
        {
            self.anim.bounce_key = Some(entry.key.clone());
            self.anim.bounce_phase = 0.01;
            self.ensure_animating();
        }
        let target = self.target_for_entry(&entry);
        activate_entry(runtime, &entry, target);
    }

    pub(super) fn target_for_entry(&self, entry: &DockEntry) -> Option<Rect> {
        let index = self
            .combined_entries()
            .iter()
            .position(|candidate| candidate.key == entry.key)?;
        let item = self.layout.items.get(index)?;
        let (origin_x, origin_y) = self.window_origin();
        let size = item.size.round().max(1.0) as i32;
        Some(Rect {
            x: origin_x + item.x.round() as i32,
            y: origin_y + item.y.round() as i32,
            width: size,
            height: size,
        })
    }
}
