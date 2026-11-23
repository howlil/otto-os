use std::cell::RefCell;

use crate::appearance::glass::GlassPainter;
use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery,
};
use crate::{
    BrushPalette, CoverArt, LiquidGlassPalette, create_text_format, scale, shared_d2d_factory,
    shared_dwrite_factory,
};
use otto_core::{
    Appearance, ControlCenterHitTarget, ControlCenterLayout, ControlCenterPage,
    ControlCenterSnapshot, RgbBuffer,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_LINE_JOIN_ROUND,
    D2D1_STROKE_STYLE_PROPERTIES, ID2D1DCRenderTarget, ID2D1Factory, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::Matrix3x2;

const FOCUS_PURPLE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.43,
    g: 0.36,
    b: 0.96,
    a: 1.0,
};

mod glyphs;
mod main_page;
mod primitives;
mod subpanels;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ControlCenterPaintState {
    pub hovered: Option<ControlCenterHitTarget>,
    pub pressed: Option<ControlCenterHitTarget>,
    pub focused: Option<ControlCenterHitTarget>,
    pub morph_progress: Option<f32>,
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

struct CachedCover {
    hash: u64,
    len: usize,
    art: CoverArt,
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
    cached_cover: RefCell<Option<CachedCover>>,
    glass: GlassPainter,
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
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn clear_backdrop(&self) {
        self.glass.clear_backdrop();
    }

    #[must_use]
    pub fn has_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.cached_cover.borrow_mut().take();
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
        let factor = 0.92 + 0.08 * state.morph_progress.unwrap_or(1.0).clamp(0.0, 1.0);
        let transform = Matrix3x2 {
            M11: factor,
            M12: 0.0,
            M21: 0.0,
            M22: factor,
            M31: w as f32 * (1.0 - factor),
            M32: 0.0,
        };
        let identity = Matrix3x2::identity();

        let target = self.target.clone();
        draw_on(&target, &self.surface, (w, h), || {
            unsafe {
                self.target.SetTransform(&raw const transform);
            }
            let result = (|| {
                self.draw_shell(w, h, layout.shell_radius as f32, &appearance, &palette)?;
                match layout.page {
                    ControlCenterPage::Main => {
                        self.draw_main_page(layout, snapshot, state, &palette)?;
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

const fn d2d_rect_size(width: i32, height: i32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: width as f32,
        bottom: height as f32,
    }
}

const fn d2d_rect_inset(width: i32, height: i32, inset: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: inset,
        top: inset,
        right: width as f32 - inset,
        bottom: height as f32 - inset,
    }
}
