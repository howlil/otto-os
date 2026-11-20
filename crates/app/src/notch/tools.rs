use std::time::{Duration, Instant};

use otto_core::{
    ClipboardPayload, ClipboardState, HitTarget, NotchAction, NotchExpandedTab,
    SHELF_HOVER_LEAVE_MS, ShelfPayload, ShelfState, ShelfThumbnail,
};

use super::CLIPBOARD_COPIED_BADGE_MS;
use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;
use super::PanelTrigger;
use super::sanitize_shelf_payload;

impl NotchController {
    pub(crate) const fn clipboard_state(&self) -> &ClipboardState {
        &self.clipboard
    }

    pub(crate) fn sync_clipboard_capture(&mut self, payload: ClipboardPayload) -> bool {
        self.clipboard.push_payload(payload).is_some()
    }

    pub(crate) fn clear_clipboard_copied_feedback(&mut self) -> bool {
        self.clipboard_copied_until = None;
        self.clipboard.clear_copied_feedback()
    }

    pub(crate) const fn shelf_state(&self) -> &ShelfState {
        &self.shelf
    }

    pub(crate) fn shelf_item_payload(&self, slot: u8) -> Option<ShelfPayload> {
        self.shelf
            .visible_item_at(slot)
            .map(|item| item.payload.clone())
    }

    pub(crate) const fn is_shelf_hold_open(&self) -> bool {
        self.shelf.drag_hover
            || self.opened_by_shelf_drag
            || self.shelf_drag_leave_until.is_some()
            || self.modal_picker_open
            || self.drag_out_active
    }

    pub(crate) fn begin_shelf_modal_picker(&mut self) -> bool {
        let was_open = self.modal_picker_open;
        self.modal_picker_open = true;
        self.hover_blocked = false;
        !was_open
    }

    pub(crate) fn finish_shelf_modal_picker(
        &mut self,
        items: Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>,
    ) -> NotchUpdate {
        self.modal_picker_open = false;
        self.opened_by_shelf_drag = false;
        self.hover_blocked = false;
        let mut changed = true;
        for (payload, is_dir, thumb) in items {
            if let Some(valid) = sanitize_shelf_payload(payload)
                && self.shelf.push_payload(valid, is_dir, thumb).is_some()
            {
                changed = true;
            }
        }
        changed |= self.switch_tab(NotchExpandedTab::Shelf);
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn set_shelf_drag_out(&mut self, active: bool) -> bool {
        if self.drag_out_active == active {
            return false;
        }
        self.drag_out_active = active;
        if active {
            self.hover_blocked = false;
        }
        true
    }

    pub(crate) fn handle_shelf_drag_enter(&mut self) -> NotchUpdate {
        self.shelf_drag_leave_until = None;
        self.hover_blocked = false;
        let mut changed = self.shelf.set_drag_hover(true);
        if !self.panel_trigger.is_open() && !self.opened_by_shelf_drag {
            self.opened_by_shelf_drag = true;
            changed = true;
        }
        changed |= self.switch_tab(NotchExpandedTab::Shelf);
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn handle_shelf_drag_leave(&mut self, now: Instant) -> NotchUpdate {
        let changed = self.shelf.set_drag_hover(false);
        self.shelf_drag_leave_until = if self.opened_by_shelf_drag || self.panel_trigger.is_open() {
            Some(now + Duration::from_millis(SHELF_HOVER_LEAVE_MS))
        } else {
            None
        };
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn handle_shelf_drop(
        &mut self,
        items: Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>,
    ) -> NotchUpdate {
        self.shelf_drag_leave_until = None;
        self.hover_blocked = false;
        let mut changed = self.shelf.set_drag_hover(false);
        let was_drag_opened = self.opened_by_shelf_drag;
        self.opened_by_shelf_drag = false;
        if was_drag_opened && self.panel_trigger == PanelTrigger::Closed {
            self.panel_trigger = PanelTrigger::Hovered;
            changed = true;
        }
        let mut added = false;
        for (payload, is_dir, thumb) in items {
            if let Some(valid) = sanitize_shelf_payload(payload)
                && self.shelf.push_payload(valid, is_dir, thumb).is_some()
            {
                added = true;
            }
        }
        if added {
            let _ = self.switch_tab(NotchExpandedTab::Shelf);
            changed = true;
        }
        NotchUpdate {
            changed,
            effect: None,
        }
    }

    pub(crate) fn handle_shelf_right_click(&self, slot: u8) -> NotchUpdate {
        let Some(item) = self.shelf.visible_item_at(slot) else {
            return NotchUpdate::default();
        };
        let action = match &item.payload {
            ShelfPayload::File(_) => NotchAction::ShelfReveal(item.payload.clone()),
            ShelfPayload::Url(_) | ShelfPayload::Text(_) => {
                NotchAction::ShelfCopy(item.payload.clone())
            }
        };
        NotchUpdate {
            changed: false,
            effect: Some(NotchEffect::Platform(action)),
        }
    }

    pub(super) fn step_shelf_drag_leave(&mut self, now: Instant) -> bool {
        if self
            .shelf_drag_leave_until
            .is_some_and(|until| now >= until)
        {
            self.shelf_drag_leave_until = None;
            let hover_cleared = self.shelf.set_drag_hover(false);
            let drag_closed = self.opened_by_shelf_drag;
            self.opened_by_shelf_drag = false;
            return hover_cleared || drag_closed;
        }
        false
    }

    pub(super) fn handle_shelf_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::ShelfAdd => NotchUpdate {
                changed: false,
                effect: Some(NotchEffect::PickShelfFiles),
            },
            HitTarget::ShelfClear => NotchUpdate {
                changed: self.shelf.clear(),
                effect: None,
            },
            HitTarget::ShelfPager => NotchUpdate {
                changed: self.shelf.step_page(1),
                effect: None,
            },
            HitTarget::ShelfItem(slot) => {
                let Some(item) = self.shelf.visible_item_at(slot) else {
                    return NotchUpdate::default();
                };
                let action = match &item.payload {
                    ShelfPayload::File(_) | ShelfPayload::Url(_) => {
                        NotchAction::ShelfOpen(item.payload.clone())
                    }
                    ShelfPayload::Text(_) => NotchAction::ShelfCopy(item.payload.clone()),
                };
                NotchUpdate {
                    changed: false,
                    effect: Some(NotchEffect::Platform(action)),
                }
            }
            HitTarget::ShelfCopyItem(slot) => {
                let Some(item) = self.shelf.visible_item_at(slot) else {
                    return NotchUpdate::default();
                };
                NotchUpdate {
                    changed: false,
                    effect: Some(NotchEffect::Platform(NotchAction::ShelfCopy(
                        item.payload.clone(),
                    ))),
                }
            }
            HitTarget::ShelfRemoveItem(slot) => NotchUpdate {
                changed: self
                    .shelf
                    .visible_item_at(slot)
                    .map(|item| item.id)
                    .is_some_and(|id| self.shelf.remove(id)),
                effect: None,
            },
            _ => NotchUpdate::default(),
        }
    }

    pub(super) fn handle_clipboard_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::ClipboardPauseToggle => {
                let changed = self.clipboard.toggle_pause();
                NotchUpdate {
                    changed,
                    effect: None,
                }
            }
            HitTarget::ClipboardClear => {
                let changed = self.clipboard.clear();
                if changed {
                    self.clipboard_copied_until = None;
                }
                NotchUpdate {
                    changed,
                    effect: None,
                }
            }
            HitTarget::ClipboardPager => NotchUpdate {
                changed: self.clipboard.step_page(1),
                effect: None,
            },
            HitTarget::ClipboardItem(slot) => {
                let Some(item) = self.clipboard.visible_item_at(slot).cloned() else {
                    return NotchUpdate::default();
                };
                self.clipboard.mark_copied(item.id);
                self.clipboard_copied_until =
                    Some(Instant::now() + Duration::from_millis(CLIPBOARD_COPIED_BADGE_MS));
                NotchUpdate {
                    changed: true,
                    effect: Some(NotchEffect::CopyClipboard(item.payload)),
                }
            }
            HitTarget::ClipboardRemoveItem(slot) => {
                let changed = self
                    .clipboard
                    .visible_item_at(slot)
                    .map(|item| item.id)
                    .is_some_and(|id| self.clipboard.remove(id));
                if self.clipboard.copied_feedback_id.is_none() {
                    self.clipboard_copied_until = None;
                }
                NotchUpdate {
                    changed,
                    effect: None,
                }
            }
            _ => NotchUpdate::default(),
        }
    }

    pub(super) fn handle_tasks_click(&mut self, target: HitTarget) -> NotchUpdate {
        let saved = |changed: bool| NotchUpdate {
            changed,
            effect: changed.then_some(NotchEffect::SaveTasks),
        };
        match target {
            HitTarget::TasksAdd => NotchUpdate {
                changed: false,
                effect: Some(NotchEffect::OpenQuickCapture),
            },
            HitTarget::TasksClearDone => saved(self.tasks.clear_done()),
            HitTarget::TasksPager => NotchUpdate {
                changed: self.tasks.step_page(1),
                effect: None,
            },
            HitTarget::TaskItem(slot) => {
                let id = self.tasks.visible_item_at(slot).map(|item| item.id);
                saved(id.is_some_and(|id| self.tasks.toggle_active(id)))
            }
            HitTarget::TaskCheck(slot) => {
                let id = self.tasks.visible_item_at(slot).map(|item| item.id);
                saved(id.is_some_and(|id| self.tasks.toggle_done(id)))
            }
            HitTarget::TaskRemove(slot) => {
                let id = self.tasks.visible_item_at(slot).map(|item| item.id);
                saved(id.is_some_and(|id| self.tasks.remove(id)))
            }
            _ => NotchUpdate::default(),
        }
    }

    pub(super) fn step_clipboard_feedback(&mut self, now: Instant) -> bool {
        if self
            .clipboard_copied_until
            .is_some_and(|until| now >= until)
        {
            self.clipboard_copied_until = None;
            return self.clipboard.clear_copied_feedback();
        }
        false
    }
}
