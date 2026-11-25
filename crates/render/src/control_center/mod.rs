use std::cell::RefCell;

use crate::appearance::glass::GlassPainter;
use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery,
};
use crate::{
    BrushPalette, CachedCoverArt, LiquidGlassPalette, create_text_format, scale,
    shared_d2d_factory, shared_dwrite_factory,
};
use otto_core::{
    Appearance, ControlCenterHitTarget, ControlCenterLayout, ControlCenterPage,
    ControlCenterSnapshot, RgbBuffer,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_LINE_JOIN_ROUND,
    D2D1_STROKE_STYLE_PROPERTIES, ID2D1Bitmap, ID2D1DCRenderTarget, ID2D1Factory, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::Matrix3x2;

mod canvas;
mod gallery;
mod glyphs;
mod main_page;
mod primitives;
mod subpanels;
mod tile_widgets;
mod tiles;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ControlCenterPaintState {
    pub hovered: Option<ControlCenterHitTarget>,
    pub pressed: Option<ControlCenterHitTarget>,
    pub focused: Option<ControlCenterHitTarget>,
    pub morph_progress: Option<f32>,
    pub editing: bool,
    pub drag_preview: Option<otto_core::Rect>,
    pub dragging_tile: Option<u32>,
}

#[derive(Clone, Copy)]
struct ControlVisualState {
    active: bool,
    enabled: bool,
    pointer: PointerState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PointerState {
    Normal,
    Hovered,
    Pressed,
}

impl PointerState {
    const fn from_flags(hovered: bool, pressed: bool) -> Self {
        if pressed {
            Self::Pressed
        } else if hovered {
            Self::Hovered
        } else {
            Self::Normal
        }
    }

    const fn is_hovered(self) -> bool {
        matches!(self, Self::Hovered | Self::Pressed)
    }

    const fn is_pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }
}

fn connectivity_subtitle(supported: bool, enabled: bool, active_label: Option<&str>) -> &str {
    if !supported {
        "Unavailable"
    } else if enabled {
        active_label.unwrap_or("On")
    } else {
        "Off"
    }
}

pub struct ControlCenterRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    brushes: BrushPalette,
    surface: DibSurface,
    title_format: IDWriteTextFormat,
    body_format: IDWriteTextFormat,
    subtitle_format: IDWriteTextFormat,
    value_format: IDWriteTextFormat,
    center_format: IDWriteTextFormat,
    dpi: u32,
    cached_cover: RefCell<Option<CachedCoverArt>>,
    glass: GlassPainter,
    canvas_blur_pixels: RefCell<Option<(u32, u32, Vec<u8>)>>,
    canvas_blur: RefCell<Option<ID2D1Bitmap>>,
}

impl ControlCenterRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        let dpi = dpi.max(96);
        let factory = shared_d2d_factory()?;
        let dwrite = shared_dwrite_factory()?;
        unsafe {
            let target = create_dc_render_target(&factory)?;
            let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 2.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let stroke_style = factory.CreateStrokeStyle(&stroke_props, None)?;
            let brushes = BrushPalette::new(&target)?;
            let scale_f = dpi as f32 / 96.0;
            let title_format = create_text_format(
                &dwrite,
                13.0 * scale_f,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let body_format = create_text_format(
                &dwrite,
                12.0 * scale_f,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let subtitle_format = create_text_format(
                &dwrite,
                11.0 * scale_f,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let value_format = create_text_format(
                &dwrite,
                11.0 * scale_f,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;
            let center_format = create_text_format(
                &dwrite,
                12.0 * scale_f,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let surface = DibSurface::new(scale(384, dpi), scale(640, dpi))?;
            Ok(Self {
                factory,
                target,
                stroke_style,
                brushes,
                surface,
                title_format,
                body_format,
                subtitle_format,
                value_format,
                center_format,
                dpi,
                cached_cover: RefCell::new(None),
                glass: GlassPainter::default(),
                canvas_blur_pixels: RefCell::new(None),
                canvas_blur: RefCell::new(None),
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn set_canvas_backdrop(&self, mut capture: RgbBuffer) {
        capture.box_blur_in_place(10);
        let Ok(w) = u32::try_from(capture.width()) else {
            return;
        };
        let Ok(h) = u32::try_from(capture.height()) else {
            return;
        };
        let mut bgra = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..capture.height() {
            for x in 0..capture.width() {
                let rgb = capture.pixel(x, y);
                bgra.push((rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8);
                bgra.push((rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8);
                bgra.push((rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8);
                bgra.push(255);
            }
        }
        if let Ok(mut slot) = self.canvas_blur_pixels.try_borrow_mut() {
            *slot = Some((w, h, bgra));
        }
        if let Ok(mut gpu) = self.canvas_blur.try_borrow_mut() {
            *gpu = None;
        }
    }

    pub fn clear_backdrop(&self) {
        self.glass.clear_backdrop();
        if let Ok(mut slot) = self.canvas_blur_pixels.try_borrow_mut() {
            *slot = None;
        }
        if let Ok(mut gpu) = self.canvas_blur.try_borrow_mut() {
            *gpu = None;
        }
    }

    #[must_use]
    pub fn has_backdrop(&self) -> bool {
        self.glass.has_backdrop()
            || self
                .canvas_blur_pixels
                .try_borrow()
                .is_ok_and(|slot| slot.is_some())
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.cached_cover.borrow_mut().take();
        if let Ok(mut gpu) = self.canvas_blur.try_borrow_mut() {
            *gpu = None;
        }
        self.glass.invalidate_gpu();
        Ok(())
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        appearance: &Appearance,
    ) -> Result<()> {
        draw_with_recovery(
            self,
            |renderer| renderer.draw_to_surface(layout, snapshot, state, appearance),
            Self::recreate_target,
        )?;
        self.surface.present_sized(
            hwnd,
            (layout.window.x, layout.window.y),
            (layout.window.width, layout.window.height),
        )
    }

    pub fn draw_to_surface(
        &mut self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        appearance: &Appearance,
    ) -> Result<()> {
        let w = layout.window.width.max(1);
        let h = layout.window.height.max(1);
        let (sw, sh) = self.surface.size();
        if w > sw || h > sh {
            self.surface = DibSurface::new(w.max(sw), h.max(sh))?;
        }
        let appearance = self.glass.adaptive_appearance(appearance);
        let palette = LiquidGlassPalette::for_control_center(&appearance);
        self.brushes.update_theme(&palette);
        let progress = state.morph_progress.unwrap_or(1.0).clamp(0.0, 1.0);
        let factor = 0.94 + 0.06 * progress;
        let anchor_x = if layout.canvas_panel.is_empty() {
            w as f32
        } else {
            layout.canvas_panel.right() as f32
        };
        let anchor_y = if layout.canvas_panel.is_empty() {
            0.0
        } else {
            layout.canvas_panel.y as f32
        };
        let transform = Matrix3x2 {
            M11: factor,
            M12: 0.0,
            M21: 0.0,
            M22: factor,
            M31: anchor_x * (1.0 - factor),
            M32: anchor_y * (1.0 - factor),
        };
        let identity = Matrix3x2::identity();

        let target = self.target.clone();
        draw_on(&target, &self.surface, (w, h), || {
            let transparent = crate::CLEAR;
            unsafe {
                self.target.SetTransform(&raw const identity);
                self.target.Clear(Some(&raw const transparent));
            }
            self.draw_canvas_backdrop(w, h, &appearance, &palette, progress)?;
            unsafe {
                self.target.SetTransform(&raw const transform);
            }
            let result = (|| {
                match layout.page {
                    ControlCenterPage::Main => {
                        self.draw_canvas_toolbar(layout, snapshot, state, &palette);
                        if let Some(preview) = state.drag_preview {
                            self.draw_drag_preview(preview, scale(22, self.dpi) as f32, &palette);
                        }
                        self.draw_modular_tiles(layout, snapshot, state, &palette)?;
                    }
                    _ => self.draw_subpanel_page(layout, snapshot, state, &palette)?,
                }
                self.draw_focus_ring(layout, state.focused);
                Ok(())
            })();
            unsafe {
                self.target.SetTransform(&raw const identity);
            }
            result
        })
    }
}
