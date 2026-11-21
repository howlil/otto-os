use std::mem::size_of;
use std::time::Instant;

use crate::bar_controls::{BarControlAction, BarControlsController};
use crate::host::RuntimeContext;
use crate::notch_io::PendingShelfDrag;
use otto_core::{HitTarget, NotchExpandedTab, NotchHudKind, is_top_bar_menu_target, scale};
use otto_platform::{adjust_volume, cursor_position, read_system_status, step_display_brightness};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};

use super::Bar;
use super::SWIPE_COLLAPSE_THRESHOLD_PT;
use super::is_notch_target;
use super::is_upward_touch_swipe;
use super::modal::DeferredModalAction;
use super::should_wheel_collapse_notch;

impl Bar {
    pub(super) fn translate_client_point(&self, source_hwnd: isize, x: i32, y: i32) -> (i32, i32) {
        if source_hwnd == self.panel_hwnd {
            (
                x + self.layout.panel_window.x,
                y + self.layout.panel_window.y,
            )
        } else {
            (x, y)
        }
    }

    pub(super) fn ensure_mouse_tracking(&mut self, source_hwnd: isize) {
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

    pub(super) fn on_mouse_move(
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

    pub(super) fn on_mouse_leave(&mut self, runtime: &RuntimeContext, source_hwnd: isize) {
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

    pub(super) fn on_pointer_down(&mut self, pointer_id: u32, x: i32, y: i32) {
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

    pub(super) fn on_pointer_update(
        &mut self,
        runtime: &RuntimeContext,
        pointer_id: u32,
        x: i32,
        y: i32,
    ) {
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

    pub(super) fn clear_touch_swipe(&mut self, pointer_id: u32) {
        if self
            .touch_swipe
            .is_some_and(|(active_id, _, _)| active_id == pointer_id)
        {
            self.touch_swipe = None;
        }
    }

    pub(super) fn on_mouse_wheel(&mut self, runtime: &RuntimeContext, x: i32, y: i32, delta: i16) {
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
                | HitTarget::TaskEdit(_)
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

    pub(super) fn on_left_click(
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
                self.io.seek_drag_hwnd = Some(source_hwnd);
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
            self.io.pending_shelf_drag = Some(PendingShelfDrag {
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
            && self.layout.shelf.dropzone.contains(x, y)
        {
            return self.run_notch_target(runtime, HitTarget::ShelfAdd);
        }

        if target == HitTarget::Notch
            && self.notch.panel_expansion() > 0.55
            && self.notch.active_tab() == NotchExpandedTab::Tasks
            && self.notch.tasks_state().items.is_empty()
            && !self.notch.is_task_editor_open()
            && self.layout.tasks.body.contains(x, y)
        {
            return self.run_notch_target(runtime, HitTarget::TasksAdd);
        }

        if is_notch_target(target) {
            return self.run_notch_target(runtime, target);
        }

        if let Some(action) = BarControlsController::click_action(target) {
            self.perform(runtime, action);
        }
        None
    }

    pub(super) fn on_right_click(
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
            if let Some(HitTarget::TaskItem(slot)) = self.layout.hit_test(x, y) {
                let _ = self.run_notch_target(runtime, HitTarget::TaskEdit(slot));
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
                self.layout.slots.start_slot.x
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
}
