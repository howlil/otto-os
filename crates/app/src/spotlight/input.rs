use crate::host::RuntimeContext;
use otto_core::{
    ClipboardPayload, NotchPort, SpotlightHit, SpotlightMode, SpotlightMove, SpotlightView,
    move_spotlight_selection,
};
use otto_platform::{
    ClipboardRead, WindowsNotchPort, cursor_position, launch_new_instance, read_clipboard_payload,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, ReleaseCapture, SetCapture, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE,
    VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP,
};

use super::PointerDrag;
use super::SPOTLIGHT_MAX_QUERY_CHARS;
use super::SpotlightSurface;

impl SpotlightSurface {
    pub(super) fn backspace(&mut self) {
        if SpotlightMode::backspace(
            &mut self.query,
            &mut self.query_state.cursor,
            &mut self.query_state.selected_all,
        ) {
            self.rebuild_results();
        }
    }

    pub(super) fn set_character(&mut self, unit: u16) {
        if (0xD800..=0xDBFF).contains(&unit) {
            self.pending_high_surrogate = Some(unit);
            return;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            if let Some(high) = self.pending_high_surrogate.take() {
                let high = u32::from(high - 0xD800);
                let low = u32::from(unit - 0xDC00);
                if let Some(character) = char::from_u32(0x1_0000 + (high << 10) + low) {
                    self.set_query(character);
                }
            }
            return;
        }
        self.pending_high_surrogate = None;
        if let Some(character) = char::from_u32(u32::from(unit)) {
            self.set_query(character);
        }
    }

    pub(super) fn on_ctrl_keydown(&mut self, runtime: &RuntimeContext, key: u16) -> bool {
        match key {
            0x41 => {
                if !self.query.is_empty() {
                    self.query_state.selected_all = true;
                    self.present(runtime);
                }
                true
            }
            0x56 => {
                if let ClipboardRead::Captured {
                    payload: ClipboardPayload::Text(text) | ClipboardPayload::Link(text),
                    ..
                } = read_clipboard_payload()
                    && SpotlightMode::insert_text(
                        &mut self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                        &text,
                        SPOTLIGHT_MAX_QUERY_CHARS,
                    )
                {
                    self.rebuild_results();
                    self.present(runtime);
                }
                true
            }
            0x43 => {
                if let Some(result) = &self.math_result {
                    let _ =
                        WindowsNotchPort.copy_clipboard(&ClipboardPayload::Text(result.clone()));
                } else if self.query_state.selected_all && !self.query.is_empty() {
                    let _ = WindowsNotchPort
                        .copy_clipboard(&ClipboardPayload::Text(self.query.clone()));
                }
                true
            }
            value if value == VK_BACK.0 => {
                if SpotlightMode::delete_word_backward(
                    &mut self.query,
                    &mut self.query_state.cursor,
                    &mut self.query_state.selected_all,
                ) {
                    self.rebuild_results();
                    self.present(runtime);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn on_keydown(&mut self, runtime: &RuntimeContext, key: u16) {
        let ctrl_down = (unsafe { GetAsyncKeyState(0x11) } as u16 & 0x8000) != 0;
        if ctrl_down && self.on_ctrl_keydown(runtime, key) {
            return;
        }
        let caret_nav_all =
            self.mode == SpotlightMode::Search && (self.filtered.is_empty() || ctrl_down);
        let caret_nav_horizontal =
            caret_nav_all || self.filtered.is_empty() || self.view == SpotlightView::List;
        match key {
            value if value == VK_ESCAPE.0 => self.request_close(true),
            value if value == VK_RETURN.0 => self.activate_selected(),
            value if value == VK_BACK.0 => {
                self.backspace();
                self.present(runtime);
            }
            value if value == VK_DELETE.0 => {
                if SpotlightMode::delete_forward(
                    &mut self.query,
                    &mut self.query_state.cursor,
                    &mut self.query_state.selected_all,
                ) {
                    self.rebuild_results();
                    self.present(runtime);
                }
            }
            value if value == VK_TAB.0 => {
                self.toggle_view(match self.view {
                    SpotlightView::Grid => SpotlightView::List,
                    SpotlightView::List => SpotlightView::Grid,
                });
                self.present(runtime);
            }
            value if value == VK_HOME.0 => {
                if caret_nav_all || self.filtered.is_empty() {
                    if SpotlightMode::move_cursor_home(
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::Home);
                    self.present(runtime);
                }
            }
            value if value == VK_END.0 => {
                if caret_nav_all || self.filtered.is_empty() {
                    if SpotlightMode::move_cursor_end(
                        &self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::End);
                    self.present(runtime);
                }
            }
            value if value == VK_LEFT.0 => {
                if caret_nav_horizontal {
                    if SpotlightMode::move_cursor_left(
                        &self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::Left);
                    self.present(runtime);
                }
            }
            value if value == VK_RIGHT.0 => {
                if caret_nav_horizontal {
                    if SpotlightMode::move_cursor_right(
                        &self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::Right);
                    self.present(runtime);
                }
            }
            value if value == VK_UP.0 => {
                self.move_selection(SpotlightMove::Up);
                self.present(runtime);
            }
            value if value == VK_DOWN.0 => {
                self.move_selection(SpotlightMove::Down);
                self.present(runtime);
            }
            _ => {}
        }
    }

    pub(super) fn move_selection(&mut self, movement: SpotlightMove) {
        self.selected = move_spotlight_selection(
            self.selected,
            self.filtered.len(),
            self.view,
            self.layout.as_ref().map_or(1, |layout| layout.columns),
            movement,
        );
        self.reveal_selection();
    }

    pub(super) fn reveal_selection(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        if layout.results.iter().any(|result| result.index == selected) {
            return;
        }
        if self.view == SpotlightView::Grid {
            self.first_visible = selected / layout.columns.max(1) * layout.columns.max(1);
        } else {
            let visible = layout.results.len().max(1);
            self.first_visible = selected.saturating_sub(visible.saturating_sub(1));
        }
    }

    pub(super) fn scroll(&mut self, delta: i16) {
        if self.filtered.is_empty() || delta == 0 {
            return;
        }
        let unit = if self.view == SpotlightView::Grid {
            self.layout.as_ref().map_or(6, |layout| layout.columns)
        } else {
            1
        };
        let current = self.first_visible as isize;
        let steps = -isize::from(delta.signum()) * unit as isize;
        let max_start = self.filtered.len().saturating_sub(1) as isize;
        let next = (current + steps).clamp(0, max_start) as usize;
        self.first_visible = if self.view == SpotlightView::Grid {
            next / unit * unit
        } else {
            next
        };
        self.selected = Some(self.first_visible.min(self.filtered.len() - 1));
    }

    pub(super) fn activate_selected(&mut self) {
        if let Some(selected) = self
            .selected
            .or_else(|| (!self.filtered.is_empty()).then_some(0))
        {
            self.launch_result(selected);
            return;
        }
        if let Some(result) = self.math_result.clone() {
            let _ = WindowsNotchPort.copy_clipboard(&ClipboardPayload::Text(result));
            self.request_close(true);
        }
    }

    pub(super) fn launch_result(&mut self, selected: usize) {
        let Some(app_index) = self.filtered.get(selected).copied() else {
            return;
        };
        let Some(entry) = self.apps.get(app_index).cloned() else {
            return;
        };
        self.hide();
        launch_new_instance(&entry);
    }

    pub(super) fn handle_click(&mut self, runtime: &RuntimeContext, hit: SpotlightHit) {
        match hit {
            SpotlightHit::SearchTab => {
                self.set_mode(SpotlightMode::Search);
                self.present(runtime);
            }
            SpotlightHit::AppsTab => {
                self.set_mode(SpotlightMode::Apps);
                self.present(runtime);
            }
            SpotlightHit::GridView => {
                self.toggle_view(SpotlightView::Grid);
                self.present(runtime);
            }
            SpotlightHit::ListView => {
                self.toggle_view(SpotlightView::List);
                self.present(runtime);
            }
            SpotlightHit::Result(index) => self.launch_result(index),
            SpotlightHit::SearchField | SpotlightHit::Drag | SpotlightHit::Resize => {}
        }
    }

    pub(super) fn begin_pointer_action(&mut self, hit: SpotlightHit) {
        if matches!(hit, SpotlightHit::Drag | SpotlightHit::Resize) {
            self.dragging = Some(PointerDrag {
                start_cursor: cursor_position(),
                start_window: self.window,
                resizing: hit == SpotlightHit::Resize,
            });
            unsafe {
                let _ = SetCapture(HWND(self.hwnd as *mut _));
            }
        } else if let SpotlightHit::Result(index) = hit {
            self.selected = Some(index);
            self.pending_click = Some(hit);
        } else {
            self.pending_click = Some(hit);
        }
    }

    pub(super) fn on_pointer_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if let Some(drag) = self.dragging {
            let cursor = cursor_position();
            let delta = (
                cursor.0 - drag.start_cursor.0,
                cursor.1 - drag.start_cursor.1,
            );
            let mut rect = drag.start_window;
            if drag.resizing {
                rect.width += delta.0;
                rect.height += delta.1;
            } else {
                rect.x += delta.0;
                rect.y += delta.1;
            }
            self.window = self.clamp_window(rect, self.work_area);
            self.target_window = self.window;
            self.source_window = self.window;
            self.morph_progress = None;
            self.place();
            self.sync_backdrop(runtime, false);
            self.present(runtime);
            return;
        }
        let local = (x, y);
        let hovered = self
            .layout
            .as_ref()
            .and_then(|layout| layout.hit_test(local.0, local.1))
            .and_then(|hit| match hit {
                SpotlightHit::Result(index) => Some(index),
                _ => None,
            });
        if hovered != self.hovered {
            self.hovered = hovered;
            self.present(runtime);
        }
    }

    pub(super) fn end_pointer_action(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if self.dragging.take().is_some() {
            unsafe {
                let _ = ReleaseCapture();
            }
            self.sync_backdrop(runtime, true);
            self.present(runtime);
            return;
        }
        let pressed = self.pending_click.take();
        let released = self
            .layout
            .as_ref()
            .and_then(|layout| layout.hit_test(x, y));
        if pressed == released
            && let Some(hit) = pressed
        {
            self.handle_click(runtime, hit);
        }
    }

    pub(super) fn wheel(&mut self, runtime: &RuntimeContext, delta: i16) {
        self.scroll(delta);
        self.present(runtime);
    }
}
