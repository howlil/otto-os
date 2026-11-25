use otto_platform::place_overlay_above;
use otto_render::{WindowControlsPresent, WindowControlsRenderer};

use super::Overlay;

impl Overlay {
    pub(super) fn ensure_renderer(&mut self) -> bool {
        if self.renderer.is_none() {
            match WindowControlsRenderer::new(self.layout.width, self.layout.height) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    crate::host::log_present_error("window-controls renderer", Err(error));
                    return false;
                }
            }
        }
        true
    }

    pub(super) fn present(&mut self) {
        let Some(target) = self.target else {
            return;
        };
        if !self.ensure_renderer() {
            return;
        }
        let cover = self.cover_style();
        let render_origin = self.render_origin();
        let pressed = self.pressed_visual();
        let state = self
            .state
            .with_alt_zoom_glyph(self.hover.hovered && self.hover.alt_down);
        if let Some(renderer) = self.renderer.as_mut() {
            crate::host::log_present_error(
                "window-controls",
                renderer.present(
                    self.hwnd,
                    render_origin,
                    &WindowControlsPresent {
                        layout: &self.layout,
                        state,
                        maximized: target.maximized,
                        hovered: self.hover.hovered,
                        pressed,
                        appearance: self.appearance,
                        cover: cover.as_ref(),
                    },
                ),
            );
        }
        let _ = place_overlay_above(self.hwnd, target.hwnd);
    }
}
