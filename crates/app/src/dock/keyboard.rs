use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_DOCK_KEY;
use otto_core::{BarKey, DockFocusOutcome};
use otto_platform::{route_bar_keys, routes_keys_to, stop_routing_if_targets};

use super::Dock;

impl Dock {
    pub(super) fn on_dock_key(&mut self, runtime: &RuntimeContext, key: BarKey) {
        let count = self.combined_entries().len();
        match self.focus.handle(key, count) {
            DockFocusOutcome::Moved(_) => self.present(runtime),
            DockFocusOutcome::Exited => self.leave_keyboard_focus(runtime),
            DockFocusOutcome::Activate(index) => {
                self.leave_keyboard_focus(runtime);
                self.activate_index(runtime, index);
            }
            DockFocusOutcome::ReturnToBar => {
                self.leave_keyboard_focus(runtime);
                crate::bar::enter_keyboard_focus_from_dock(runtime, key == BarKey::PreviousTab);
            }
            DockFocusOutcome::Unchanged => {}
        }
    }

    pub(super) fn enter_keyboard_focus(&mut self, runtime: &RuntimeContext, from_end: bool) {
        let count = self.combined_entries().len();
        if self.focus.enter(count, from_end).is_some() && route_bar_keys(self.hwnd, WM_APP_DOCK_KEY)
        {
            self.present(runtime);
        } else {
            self.focus.exit();
        }
    }

    pub(super) fn leave_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        stop_routing_if_targets(self.hwnd);
        self.focus.exit();
        self.present(runtime);
    }

    pub(super) fn sync_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if !self.focus.is_active() {
            return;
        }
        if !routes_keys_to(self.hwnd) {
            self.focus.exit();
            self.present(runtime);
        } else if self.focus.revalidate(self.combined_entries().len()) {
            self.present(runtime);
        }
    }
}
