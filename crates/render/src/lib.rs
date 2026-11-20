mod bar;
mod clipboard;
mod control_center;
mod dock;
mod genie;
mod glass;
mod glass_gpu;
mod glow;
mod lyrics;
mod media;
mod menu;
mod notification;
mod palette;
mod pomodoro;
mod privacy;
mod shelf;
mod spotlight;
mod stack;
mod surface;
mod tasks;
mod theme;
mod window_controls;

use std::cell::{Cell, RefCell};
use std::f32::consts::PI;

pub use control_center::{ControlCenterPaintState, ControlCenterRenderer};
pub use dock::{DockRenderItem, DockRenderer};
pub use genie::{GenieRenderer, build_fallback_card_snapshot};
pub use menu::{MenuHover, PopupMenuRenderer};
pub use spotlight::{SpotlightPaintEntry, SpotlightPaintState, SpotlightRenderer};
pub use stack::DownloadsStackRenderer;
pub use theme::{BrushPalette, LiquidGlassPalette};
pub use window_controls::{TrafficLightRenderer, WindowControlsRenderer};

use otto_core::{
    Appearance, BrandIcon, ClipboardState, CompactSlotKind, HitTarget, HudGlyph, HudTone, HudValue,
    Layout, LyricsView, MediaScene, NotchHudState, PomodoroScene, PrivacyState, Rect, ShapeTokens,
    ShelfState, SystemStatus, TasksState, cover_accent, present_hud, scale,
};
use surface::{DibSurface, create_dc_render_target, draw_with_recovery, fnv1a_hash};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_GRADIENT_STOP,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL,
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_DASH, D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2, D2D1_LINE_JOIN_ROUND,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1_SWEEP_DIRECTION, D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
    D2D1CreateFactory, ID2D1Bitmap, ID2D1DCRenderTarget, ID2D1Factory, ID2D1GeometrySink,
    ID2D1LinearGradientBrush, ID2D1PathGeometry, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_FEATURE, DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_RANGE,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat, IDWriteTypography,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{PCWSTR, Result, w};
use windows_numerics::{Matrix3x2, Vector2};

const BLACK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const DISABLED_CONTROL_ALPHA: f32 = 0.35;
const PANEL_CONTENT_RISE_PT: i32 = 8;
const NOTCH_TAB_SLIDE_PT: i32 = 24;
pub(crate) const GLYPH_STROKE_SCALE: f32 = 0.18;
const FOREGROUND: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.95,
};
const CHARGING_GREEN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.19,
    g: 0.82,
    b: 0.35,
    a: 0.96,
};
const AMBER_WARN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.62,
    b: 0.04,
    a: 0.96,
};
const MUSIC_ACCENT: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.988,
    g: 0.235,
    b: 0.329,
    a: 1.0,
};
const EQUALIZER_GREEN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.188,
    g: 0.820,
    b: 0.345,
    a: 1.0,
};
const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

const fn rgb(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

struct CachedNotchGeometry {
    rect: Rect,
    corner_radius_bits: u32,
    ear_radius_bits: u32,
    geometry: ID2D1PathGeometry,
}

#[derive(Clone)]
pub(crate) struct CoverArt {
    pub bitmap: ID2D1Bitmap,
    pub accent: Option<D2D1_COLOR_F>,
}

struct CachedCoverArt {
    hash: u64,
    bytes_len: usize,
    art: CoverArt,
}

pub struct Renderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    dashed_stroke_style: ID2D1StrokeStyle,
    text_leading: IDWriteTextFormat,
    text_compact_center: IDWriteTextFormat,
    text_hud_label: IDWriteTextFormat,
    text_hud_value: IDWriteTextFormat,
    text_app_bold: IDWriteTextFormat,
    text_menu: IDWriteTextFormat,
    text_timer_display: IDWriteTextFormat,
    text_compact_time: IDWriteTextFormat,
    text_pomodoro_sub: IDWriteTextFormat,
    text_pomodoro_sub_trailing: IDWriteTextFormat,
    text_tab: IDWriteTextFormat,
    text_wrap: IDWriteTextFormat,
    text_caption: IDWriteTextFormat,
    text_caption_center: IDWriteTextFormat,
    text_alert_title: IDWriteTextFormat,
    text_lyric: IDWriteTextFormat,
    lyric_layouts: RefCell<lyrics::LyricLayoutCache>,
    content_offset: Cell<(f32, f32)>,
    bar_surface: DibSurface,
    panel_surface: DibSurface,
    dpi: u32,
    brushes: BrushPalette,
    cached_bar_notch: RefCell<Option<CachedNotchGeometry>>,
    cached_panel_notch: RefCell<Option<CachedNotchGeometry>>,
    cached_cover_art: RefCell<Option<CachedCoverArt>>,
    cached_shelf_bitmaps: RefCell<Vec<shelf::CachedShelfBitmap>>,
    last_bar_frame: RefCell<Option<bar::BarFrameKey>>,
    last_bar_target: Cell<Option<(isize, (i32, i32))>>,
    glass: glass::GlassPainter,
    brand_icon: Cell<BrandIcon>,
    bar_background: Cell<bool>,
    bar_chrome_hidden: Cell<bool>,
}

const SQUIRCLE_CACHE_CAPACITY: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
struct SquircleCacheKey {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    radius: u32,
}

impl SquircleCacheKey {
    const fn new(rect: D2D_RECT_F, radius: f32) -> Self {
        Self {
            left: rect.left.to_bits(),
            top: rect.top.to_bits(),
            right: rect.right.to_bits(),
            bottom: rect.bottom.to_bits(),
            radius: radius.to_bits(),
        }
    }
}

thread_local! {
    static D2D_FACTORY: RefCell<Option<ID2D1Factory>> = const { RefCell::new(None) };
    static DWRITE_FACTORY: RefCell<Option<IDWriteFactory>> = const { RefCell::new(None) };
    static TABULAR_TYPOGRAPHY: RefCell<Option<IDWriteTypography>> = const { RefCell::new(None) };
    static SQUIRCLE_CACHE: RefCell<Vec<(SquircleCacheKey, ID2D1PathGeometry)>> = const { RefCell::new(Vec::new()) };
}

pub fn release_shared_factories() {
    glass_gpu::release_shared_gpu();
    let _ = SQUIRCLE_CACHE.try_with(|cell| cell.borrow_mut().clear());
    let _ = TABULAR_TYPOGRAPHY.try_with(|cell| cell.borrow_mut().take());
    let _ = D2D_FACTORY.try_with(|cell| cell.borrow_mut().take());
    let _ = DWRITE_FACTORY.try_with(|cell| cell.borrow_mut().take());
}

pub(crate) fn shared_d2d_factory() -> Result<ID2D1Factory> {
    D2D_FACTORY.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            let factory: ID2D1Factory =
                unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
            *guard = Some(factory);
        }
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))
    })
}

pub(crate) fn shared_dwrite_factory() -> Result<IDWriteFactory> {
    DWRITE_FACTORY.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            let factory: IDWriteFactory =
                unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
            *guard = Some(factory);
        }
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))
    })
}

pub(crate) fn shared_tabular_typography() -> Result<IDWriteTypography> {
    TABULAR_TYPOGRAPHY.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            let dwrite = shared_dwrite_factory()?;
            let typography = unsafe {
                let typography: IDWriteTypography = dwrite.CreateTypography()?;
                typography.AddFontFeature(DWRITE_FONT_FEATURE {
                    nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                    parameter: 1,
                })?;
                typography
            };
            *guard = Some(typography);
        }
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))
    })
}

pub(crate) fn decode_image_bytes(target: &ID2D1DCRenderTarget, bytes: &[u8]) -> Result<CoverArt> {
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let mut original_w = 0u32;
        let mut original_h = 0u32;
        frame.GetSize(&mut original_w, &mut original_h)?;

        let converter = factory.CreateFormatConverter()?;
        if original_w > 128 || original_h > 128 {
            let max_dim = original_w.max(original_h).max(1) as f32;
            let scale_factor = (128.0 / max_dim).min(1.0);
            let target_w = ((original_w as f32 * scale_factor).round() as u32).max(1);
            let target_h = ((original_h as f32 * scale_factor).round() as u32).max(1);
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(&frame, target_w, target_h, WICBitmapInterpolationModeFant)?;
            converter.Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
        } else {
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
        }
        let mut width = 0u32;
        let mut height = 0u32;
        converter.GetSize(&mut width, &mut height)?;
        let stride = width.saturating_mul(4);
        let mut pixels = vec![0u8; stride as usize * height as usize];
        let accent = converter
            .CopyPixels(std::ptr::null(), stride, &mut pixels)
            .ok()
            .and_then(|()| cover_accent(&pixels, width, height))
            .map(|colour| D2D1_COLOR_F {
                r: colour.r,
                g: colour.g,
                b: colour.b,
                a: 1.0,
            });
        let bitmap = target.CreateBitmapFromWicBitmap(&converter, None)?;
        Ok(CoverArt { bitmap, accent })
    }
}

impl Renderer {
    #[allow(clippy::too_many_lines)]
    pub fn new(
        width: i32,
        height: i32,
        panel_width: i32,
        panel_height: i32,
        dpi: u32,
        _tint_alpha: f32,
    ) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
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
            let dashed_props = D2D1_STROKE_STYLE_PROPERTIES {
                dashStyle: D2D1_DASH_STYLE_DASH,
                ..stroke_props
            };
            let dashed_stroke_style = factory.CreateStrokeStyle(&dashed_props, None)?;

            let dwrite = shared_dwrite_factory()?;
            let font_px = scale(12, dpi) as f32;
            let text_leading = create_text_format(
                &dwrite,
                font_px,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_compact_center = create_text_format(
                &dwrite,
                font_px,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_hud_label = create_text_format(
                &dwrite,
                scale(13, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_hud_value = create_text_format(
                &dwrite,
                scale(13, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;
            let text_app_bold = create_text_format(
                &dwrite,
                font_px,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_menu = create_text_format(
                &dwrite,
                font_px,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_timer_display = create_text_format(
                &dwrite,
                scale(30, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_compact_time = create_text_format(
                &dwrite,
                scale(13, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;
            let text_pomodoro_sub = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_pomodoro_sub_trailing = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;
            let text_tab = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_wrap = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            text_wrap.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            text_wrap.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            let text_caption = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_caption_center = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_alert_title = create_text_format(
                &dwrite,
                scale(14, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_lyric = create_text_format(
                &dwrite,
                scale(lyrics::LYRIC_FONT_PT, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;

            let bar_surface = DibSurface::new(width, height)?;
            let panel_surface = DibSurface::new(panel_width, panel_height)?;
            let brushes = BrushPalette::new(&target)?;
            let cached_bar_notch = RefCell::new(None);
            let cached_panel_notch = RefCell::new(None);

            Ok(Self {
                factory,
                target,
                stroke_style,
                dashed_stroke_style,
                text_leading,
                text_compact_center,
                text_hud_label,
                text_hud_value,
                text_app_bold,
                text_menu,
                text_timer_display,
                text_compact_time,
                text_pomodoro_sub,
                text_pomodoro_sub_trailing,
                text_tab,
                text_wrap,
                text_caption,
                text_caption_center,
                text_alert_title,
                text_lyric,
                lyric_layouts: RefCell::default(),
                content_offset: Cell::new((0.0, 0.0)),
                bar_surface,
                panel_surface,
                dpi,
                brushes,
                cached_bar_notch,
                cached_panel_notch,
                cached_cover_art: RefCell::new(None),
                cached_shelf_bitmaps: RefCell::default(),
                last_bar_frame: RefCell::new(None),
                last_bar_target: Cell::new(None),
                glass: glass::GlassPainter::default(),
                brand_icon: Cell::new(BrandIcon::Otto),
                bar_background: Cell::new(true),
                bar_chrome_hidden: Cell::new(false),
            })
        }
    }

    pub fn set_bar_background(&self, shown: bool) {
        if self.bar_background.replace(shown) != shown {
            self.last_bar_frame.borrow_mut().take();
        }
    }

    pub fn set_bar_chrome_hidden(&self, hidden: bool) {
        if self.bar_chrome_hidden.replace(hidden) != hidden {
            self.last_bar_frame.borrow_mut().take();
        }
    }

    pub fn set_brand_icon(&self, brand_icon: BrandIcon) {
        self.brand_icon.set(brand_icon);
        self.last_bar_frame.borrow_mut().take();
    }

    fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.glass.invalidate_gpu();
        self.cached_cover_art.borrow_mut().take();
        self.cached_shelf_bitmaps.borrow_mut().clear();
        self.last_bar_frame.borrow_mut().take();
        self.last_bar_target.set(None);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &mut self,
        hwnd: isize,
        origin: (i32, i32),
        layout: &Layout,
        status: SystemStatus,
        app_name: &str,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        hud: Option<NotchHudState>,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
        appearance: Appearance,
        privacy: &PrivacyState,
        call: Option<&otto_core::CallScene>,
    ) -> Result<()> {
        if let Some(fields) =
            self.bar_frame_fields(layout, status, hovered, focused, hud, appearance)
            && self.last_bar_target.get() == Some((hwnd, origin))
            && self
                .last_bar_frame
                .borrow()
                .as_ref()
                .is_some_and(|cached| cached.matches(&fields, app_name))
        {
            return Ok(());
        }
        draw_with_recovery(
            self,
            |renderer| {
                renderer.draw_bar(
                    layout, status, app_name, hovered, focused, hud, pomodoro, media, anim_phase,
                    appearance, privacy, call,
                )
            },
            Self::recreate_target,
        )?;
        if let Err(err) = self.bar_surface.present(hwnd, origin) {
            self.last_bar_frame.borrow_mut().take();
            self.last_bar_target.set(None);
            return Err(err);
        }
        self.last_bar_target.set(Some((hwnd, origin)));
        Ok(())
    }

    pub fn set_bar_backdrop(&self, capture: otto_core::RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
        self.last_bar_frame.borrow_mut().take();
    }

    pub fn clear_bar_backdrop(&self) {
        self.glass.clear_backdrop();
        self.last_bar_frame.borrow_mut().take();
    }

    #[must_use]
    pub fn has_bar_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present_panel(
        &mut self,
        panel_hwnd: isize,
        layout: &Layout,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
        shelf_state: &ShelfState,
        clipboard_state: &ClipboardState,
        tasks_state: &TasksState,
        privacy: &PrivacyState,
        notification: Option<&otto_core::NotificationPresentation>,
    ) -> Result<()> {
        draw_with_recovery(
            self,
            |renderer| {
                renderer.draw_panel(
                    layout,
                    hovered,
                    focused,
                    pomodoro,
                    media,
                    anim_phase,
                    lyrics,
                    shelf_state,
                    clipboard_state,
                    tasks_state,
                    privacy,
                    notification,
                )
            },
            Self::recreate_target,
        )?;
        self.panel_surface.present_sized(
            panel_hwnd,
            (layout.panel_window.x, layout.panel_window.y),
            (layout.panel_window.width, layout.panel_window.height),
        )
    }

    fn draw_compact_notch(
        &self,
        layout: &Layout,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        call: Option<&otto_core::CallScene>,
        anim_phase: f32,
    ) -> Result<()> {
        let primary = layout.compact_primary.unwrap_or_else(|| {
            if pomodoro.state.is_compact_active()
                || pomodoro.timer.is_compact_active()
                || pomodoro.stopwatch.is_compact_active()
            {
                CompactSlotKind::Focus
            } else {
                CompactSlotKind::Media
            }
        });
        let blend = layout.compact_swap_blend.clamp(0.0, 1.0);
        let slide = scale(8, self.dpi) as f32;
        if blend < 0.995 {
            self.set_content_offset(blend * slide, 0.0);
            match primary {
                CompactSlotKind::Focus => {
                    if let Some(scene) = media {
                        self.draw_compact_media(layout, scene, anim_phase, 1.0 - blend);
                    }
                }
                CompactSlotKind::Media => {
                    self.draw_compact_pomodoro(layout, pomodoro, 1.0 - blend)?;
                }
                CompactSlotKind::Call => {}
            }
            self.set_content_offset((1.0 - blend) * slide, 0.0);
        }
        let result = match primary {
            CompactSlotKind::Focus => self.draw_compact_pomodoro(layout, pomodoro, blend),
            CompactSlotKind::Media => {
                if let Some(scene) = media {
                    self.draw_compact_media(layout, scene, anim_phase, blend);
                }
                Ok(())
            }
            CompactSlotKind::Call => match call {
                Some(scene) => self.draw_compact_call(layout, scene, anim_phase, 1.0),
                None => Ok(()),
            },
        };
        if blend < 0.995 {
            self.set_content_offset(0.0, 0.0);
        }
        result
    }

    fn draw_metaball_bridge(&self, layout: &Layout, cy: f32, r: f32) -> Result<()> {
        if layout.notch.y <= 0 || layout.notch_secondary.right() <= layout.notch.right() {
            return Ok(());
        }
        let max_gap = scale(10, self.dpi) as f32;
        let gap = (layout.notch_secondary.x - layout.notch.right()) as f32;
        if gap >= max_gap {
            return Ok(());
        }
        let bridge_inset = (r * 0.35).min(scale(4, self.dpi) as f32).max(1.0);
        let left_x = layout.notch.right() as f32 - bridge_inset;
        let right_x = layout.notch_secondary.x as f32 + bridge_inset;
        if right_x <= left_x + 0.5 {
            return Ok(());
        }
        let blend = (1.0 - gap.max(0.0) / max_gap.max(1.0)).clamp(0.0, 1.0);
        let dx = (r - bridge_inset).max(0.0);
        let circle_half_h = (r * r - dx * dx).max(1.0).sqrt();
        let notch_half_limit = (cy - layout.notch.y as f32 - 1.5)
            .min(layout.notch.bottom() as f32 - cy - 1.5)
            .max(1.0);
        let edge_half_h = circle_half_h.min(notch_half_limit);
        let waist_half_h = edge_half_h * (0.15 + 0.60 * blend);
        let mid_x = (left_x + right_x) * 0.5;
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(left_x, cy - edge_half_h),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(mid_x, cy - waist_half_h),
                point2: Vector2::new(mid_x, cy - waist_half_h),
                point3: Vector2::new(right_x, cy - edge_half_h),
            });
            sink.AddLine(Vector2::new(right_x, cy + edge_half_h));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(mid_x, cy + waist_half_h),
                point2: Vector2::new(mid_x, cy + waist_half_h),
                point3: Vector2::new(left_x, cy + edge_half_h),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target
                .FillGeometry(&geom, &self.brushes.notch_obsidian, None);
        }
        Ok(())
    }

    fn draw_secondary_notch_frame(
        &self,
        layout: &Layout,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        is_high_contrast: bool,
    ) {
        let rect = layout.notch_secondary;
        let r = rect.width.min(rect.height) as f32 * 0.5;
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let _ = self.draw_metaball_bridge(layout, cy, r);
        let ellipse = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: r,
            radiusY: r,
        };
        unsafe {
            self.target
                .FillEllipse(&ellipse, &self.brushes.notch_obsidian);
            if layout.notch.y > 0 && !is_high_contrast {
                let floating_rim = self.brushes.alpha_brush(FOREGROUND, 0.14);
                self.target
                    .DrawEllipse(&ellipse, &floating_rim, 1.0, &self.stroke_style);
            } else {
                self.target.DrawEllipse(
                    &ellipse,
                    &self.brushes.specular_rim,
                    1.0,
                    &self.stroke_style,
                );
            }
            if hovered == Some(HitTarget::NotchSecondary) {
                let hover_fill = self.brushes.alpha_brush(FOREGROUND, 0.10);
                let hover_rim = self.brushes.alpha_brush(FOREGROUND, 0.28);
                self.target.FillEllipse(&ellipse, &hover_fill);
                self.target
                    .DrawEllipse(&ellipse, &hover_rim, 1.2, &self.stroke_style);
            }
        }
        if focused == Some(HitTarget::NotchSecondary) {
            self.draw_control_focus_ring(rect, &self.brushes.text_primary);
        }
    }

    fn draw_secondary_notch_content(
        &self,
        layout: &Layout,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
    ) -> Result<()> {
        let primary = layout.compact_primary.unwrap_or(CompactSlotKind::Focus);
        let secondary = layout.compact_secondary.unwrap_or(match primary {
            CompactSlotKind::Focus | CompactSlotKind::Call => CompactSlotKind::Media,
            CompactSlotKind::Media => CompactSlotKind::Focus,
        });
        let blend = layout.compact_swap_blend.clamp(0.0, 1.0);
        let alpha_scale = 0.35 + 0.65 * blend;
        match secondary {
            CompactSlotKind::Focus => {
                self.draw_secondary_pomodoro_circle(layout, pomodoro, alpha_scale)
            }
            CompactSlotKind::Media => {
                if let Some(scene) = media {
                    self.draw_secondary_media_circle(layout, scene, anim_phase, alpha_scale)?;
                }
                Ok(())
            }
            CompactSlotKind::Call => Ok(()),
        }
    }

    fn get_or_decode_cover_art(&self, thumbnail: &[u8]) -> Option<CoverArt> {
        let hash = fnv1a_hash(thumbnail);
        let len = thumbnail.len();
        if let Some(cached) = self.cached_cover_art.borrow().as_ref()
            && cached.hash == hash
            && cached.bytes_len == len
        {
            return Some(cached.art.clone());
        }
        let art = decode_image_bytes(&self.target, thumbnail).ok()?;
        *self.cached_cover_art.borrow_mut() = Some(CachedCoverArt {
            hash,
            bytes_len: len,
            art: art.clone(),
        });
        Some(art)
    }

    fn set_content_offset(&self, dx: f32, dy: f32) {
        let (dx, dy) = (dx.round(), dy.round());
        self.content_offset.set((dx, dy));
        let transform = Matrix3x2::translation(dx, dy);
        unsafe {
            self.target.SetTransform(&raw const transform);
        }
    }

    fn draw_control_focus_ring(&self, rect: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let stroke = shape.focus_ring_width as f32;
        let grow = scale(3, self.dpi) as f32 + stroke * 0.5;
        let ring = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.x as f32 - grow,
                top: rect.y as f32 - grow,
                right: rect.right() as f32 + grow,
                bottom: rect.bottom() as f32 + grow,
            },
            radiusX: rect.width as f32 * 0.5 + grow,
            radiusY: rect.height as f32 * 0.5 + grow,
        };
        unsafe {
            self.target.DrawRoundedRectangle(&ring, brush, stroke, None);
        }
    }

    fn push_clip(&self, rect: D2D_RECT_F) {
        unsafe {
            self.target
                .PushAxisAlignedClip(&raw const rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
    }

    fn pop_clip(&self) {
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    fn set_content_scale(&self, factor: f32, anchor_x: f32, anchor_y: f32) {
        let (offset_x, offset_y) = self.content_offset.get();
        let transform = Matrix3x2 {
            M11: factor,
            M12: 0.0,
            M21: 0.0,
            M22: factor,
            M31: offset_x + anchor_x * (1.0 - factor),
            M32: offset_y + anchor_y * (1.0 - factor),
        };
        unsafe {
            self.target.SetTransform(&raw const transform);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_progress_ring(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        progress: f32,
        active_brush: &ID2D1SolidColorBrush,
        track_brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let circle = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawEllipse(&circle, track_brush, stroke, &self.stroke_style);

            let clamped = progress.clamp(0.0, 1.0);
            if clamped <= 0.005 {
                return Ok(());
            }
            if clamped >= 0.998 {
                self.target
                    .DrawEllipse(&circle, active_brush, stroke, &self.stroke_style);
                return Ok(());
            }

            let start_angle = -PI * 0.5;
            let sweep = clamped * PI * 2.0;
            let end_angle = start_angle + sweep;
            let start = Vector2::new(
                cx + radius * start_angle.cos(),
                cy + radius * start_angle.sin(),
            );
            let end = Vector2::new(cx + radius * end_angle.cos(), cy + radius * end_angle.sin());
            let arc_size = if sweep > PI {
                D2D1_ARC_SIZE_LARGE
            } else {
                D2D1_ARC_SIZE_SMALL
            };

            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: arc_size,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, active_brush, stroke, &self.stroke_style);
        }
        Ok(())
    }

    fn draw_circle_button(&self, rect: Rect, brush: &ID2D1SolidColorBrush) {
        let r = rect.width.min(rect.height) as f32 * 0.5;
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: r,
                    radiusY: r,
                },
                brush,
            );
        }
    }

    fn draw_rounded_box(&self, box_rect: Rect, radius: f32, brush: &ID2D1SolidColorBrush) {
        let rect = rect_to_d2d(box_rect);
        if radius >= 6.0
            && let Ok(geom) = build_squircle_path(&self.factory, rect, radius)
        {
            unsafe {
                self.target.FillGeometry(&geom, brush, None);
            }
            return;
        }
        let pill = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&pill, brush);
        }
    }

    pub(crate) fn draw_directional_rim(
        &self,
        geom: &ID2D1PathGeometry,
        top_y: f32,
        bottom_y: f32,
        top_alpha: f32,
        mid_alpha: f32,
        bottom_alpha: f32,
    ) {
        let stops = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: D2D1_COLOR_F {
                    a: top_alpha,
                    ..FOREGROUND
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.5,
                color: D2D1_COLOR_F {
                    a: mid_alpha,
                    ..FOREGROUND
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    a: bottom_alpha,
                    ..FOREGROUND
                },
            },
        ];
        let end_y = bottom_y.max(top_y + 1.0);
        let brush: Option<ID2D1LinearGradientBrush> = unsafe {
            self.target
                .CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                .and_then(|collection| {
                    self.target.CreateLinearGradientBrush(
                        &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                            startPoint: Vector2::new(0.0, top_y),
                            endPoint: Vector2::new(0.0, end_y),
                        },
                        None,
                        &collection,
                    )
                })
                .ok()
        };
        unsafe {
            if let Some(gradient) = brush {
                self.target
                    .DrawGeometry(geom, &gradient, 1.0, &self.stroke_style);
            } else {
                let fallback = self.brushes.alpha_brush(FOREGROUND, mid_alpha);
                self.target
                    .DrawGeometry(geom, &fallback, 1.0, &self.stroke_style);
            }
        }
    }

    fn draw_play_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - r * 0.55, cy - r * 0.85),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + r * 0.90, cy));
            sink.AddLine(Vector2::new(cx - r * 0.55, cy + r * 0.85));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geom, brush, None);
        }
        Ok(())
    }

    fn draw_pause_glyph(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) {
        let bar_w = (r * 0.42).max(2.2);
        let bar_h = r * 1.55;
        let gap = r * 0.36;
        for dx in [-gap - bar_w * 0.5, gap + bar_w * 0.5] {
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx + dx - bar_w * 0.5,
                    top: cy - bar_h * 0.5,
                    right: cx + dx + bar_w * 0.5,
                    bottom: cy + bar_h * 0.5,
                },
                radiusX: bar_w * 0.45,
                radiusY: bar_w * 0.45,
            };
            unsafe {
                self.target.FillRoundedRectangle(&pill, brush);
            }
        }
    }

    fn draw_reset_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.28).max(1.5);
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            let start = Vector2::new(cx, cy - r);
            let end = Vector2::new(cx - r * 0.86, cy - r * 0.50);
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_LARGE,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, brush, stroke, &self.stroke_style);

            self.target.DrawLine(
                start,
                Vector2::new(cx - r * 0.45, cy - r * 1.25),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                start,
                Vector2::new(cx - r * 0.40, cy - r * 0.65),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    fn draw_skip_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.30).max(1.6);
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - r * 0.75, cy - r * 0.72),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + r * 0.35, cy));
            sink.AddLine(Vector2::new(cx - r * 0.75, cy + r * 0.72));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geom, brush, None);

            self.target.DrawLine(
                Vector2::new(cx + r * 0.62, cy - r * 0.68),
                Vector2::new(cx + r * 0.62, cy + r * 0.68),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    fn draw_previous_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.30).max(1.6);
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx + r * 0.75, cy - r * 0.72),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx - r * 0.35, cy));
            sink.AddLine(Vector2::new(cx + r * 0.75, cy + r * 0.72));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geom, brush, None);

            self.target.DrawLine(
                Vector2::new(cx - r * 0.62, cy - r * 0.68),
                Vector2::new(cx - r * 0.62, cy + r * 0.68),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    fn draw_music_note_glyph(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (r * GLYPH_STROKE_SCALE).max(1.5);
        let head_radius_x = r * 0.38;
        let head_radius_y = r * 0.28;
        let gap = r * 0.75;
        let x1 = cx - gap * 0.5;
        let x2 = cx + gap * 0.5;
        let y_head1 = cy + r * 0.45;
        let y_head2 = cy + r * 0.25;
        let y_top = cy - r * 0.70;
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(x1, y_head1),
                    radiusX: head_radius_x,
                    radiusY: head_radius_y,
                },
                brush,
            );
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(x2, y_head2),
                    radiusX: head_radius_x,
                    radiusY: head_radius_y,
                },
                brush,
            );
            self.target.DrawLine(
                Vector2::new(x1 + head_radius_x * 0.6, y_head1),
                Vector2::new(x1 + head_radius_x * 0.6, y_top),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(x2 + head_radius_x * 0.6, y_head2),
                Vector2::new(x2 + head_radius_x * 0.6, y_top - r * 0.2),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(x1 + head_radius_x * 0.6, y_top),
                Vector2::new(x2 + head_radius_x * 0.6, y_top - r * 0.2),
                brush,
                stroke * 1.6,
                &self.stroke_style,
            );
        }
    }

    fn draw_equalizer_bars(
        &self,
        cx: f32,
        cy: f32,
        max_h: f32,
        bars: [f32; 4],
        brush: &ID2D1SolidColorBrush,
    ) {
        let bar_w = (scale(2, self.dpi) as f32).max(1.8);
        let gap = (scale(2, self.dpi) as f32).max(1.8);
        let min_h = (scale(3, self.dpi) as f32).min(max_h * 0.45);
        let offsets = [-1.5f32, -0.5, 0.5, 1.5];
        for (i, &dx_mult) in offsets.iter().enumerate() {
            let bx = cx + dx_mult * (bar_w + gap);
            let level = bars[i].clamp(0.0, 1.0);
            let h = min_h + level * (max_h - min_h).max(0.0);
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: bx - bar_w * 0.5,
                    top: cy - h * 0.5,
                    right: bx + bar_w * 0.5,
                    bottom: cy + h * 0.5,
                },
                radiusX: bar_w * 0.45,
                radiusY: bar_w * 0.45,
            };
            unsafe {
                self.target.FillRoundedRectangle(&pill, brush);
            }
        }
    }

    fn draw_audio_equalizer(
        &self,
        cx: f32,
        cy: f32,
        anim_phase: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let phases = [0.0f32, 1.6, 3.1, 4.7];
        let speeds = [5.5f32, 7.8, 6.2, 8.5];
        let bars = std::array::from_fn(|i| {
            ((anim_phase * speeds[i] + phases[i]).sin() * 0.5 + 0.5).clamp(0.0, 1.0)
        });
        let max_h = scale(12, self.dpi) as f32;
        self.draw_equalizer_bars(cx, cy, max_h, bars, brush);
    }

    fn draw_left_menu_bar(&self, layout: &Layout, app_name: &str, fg: &ID2D1SolidColorBrush) {
        self.draw_slot_label(layout.app_slot, app_name, &self.text_app_bold, fg);
        for (slot, label) in [
            (layout.file_slot, "File"),
            (layout.edit_slot, "Edit"),
            (layout.view_slot, "View"),
            (layout.window_slot, "Window"),
            (layout.help_slot, "Help"),
        ] {
            self.draw_slot_label(slot, label, &self.text_menu, fg);
        }
    }

    fn draw_slot_label(
        &self,
        slot: Rect,
        label: &str,
        format: &IDWriteTextFormat,
        brush: &ID2D1SolidColorBrush,
    ) {
        if slot.width <= 0 {
            return;
        }
        let rect = D2D_RECT_F {
            left: slot.x as f32,
            top: 0.0,
            right: slot.right() as f32,
            bottom: slot.bottom() as f32,
        };
        self.draw_text(label, format, rect, brush);
    }

    fn draw_search_slot(&self, layout: &Layout, fg: &ID2D1SolidColorBrush) {
        let slot = layout.search_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let r = unit * 0.34;
        let stroke = (unit * 0.11).max(1.4);
        let center = Vector2::new(cx - unit * 0.07, cy - unit * 0.07);

        unsafe {
            let circle = D2D1_ELLIPSE {
                point: center,
                radiusX: r,
                radiusY: r,
            };
            self.target
                .DrawEllipse(&circle, fg, stroke, &self.stroke_style);

            let diag = 0.707_106_77_f32;
            let start = Vector2::new(cx - unit * 0.07 + r * diag, cy - unit * 0.07 + r * diag);
            let end = Vector2::new(
                cx - unit * 0.07 + (r + unit * 0.28) * diag,
                cy - unit * 0.07 + (r + unit * 0.28) * diag,
            );
            self.target
                .DrawLine(start, end, fg, stroke * 1.15, &self.stroke_style);
        }
    }

    fn draw_control_center_slot(
        &self,
        layout: &Layout,
        fg: &ID2D1SolidColorBrush,
        muted: &ID2D1SolidColorBrush,
    ) {
        let slot = layout.control_center_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let pill_w = unit * 0.94;
        let pill_h = unit * 0.36;
        let gap_y = unit * 0.24;
        let stroke = (unit * 0.095).max(1.25);

        for (dy, knob_right) in [(-gap_y, false), (gap_y, true)] {
            let row_cy = cy + dy;
            let left = cx - pill_w * 0.5;
            let right = cx + pill_w * 0.5;
            let radius = pill_h * 0.5;
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left,
                    top: row_cy - radius,
                    right,
                    bottom: row_cy + radius,
                },
                radiusX: radius,
                radiusY: radius,
            };
            let knob_r = (radius - stroke * 0.45).max(1.8);
            let knob_x = if knob_right {
                right - radius
            } else {
                left + radius
            };
            let knob = D2D1_ELLIPSE {
                point: Vector2::new(knob_x, row_cy),
                radiusX: knob_r,
                radiusY: knob_r,
            };
            unsafe {
                self.target
                    .DrawRoundedRectangle(&pill, muted, stroke, &self.stroke_style);
                self.target.FillEllipse(&knob, fg);
            }
        }
    }

    fn draw_slot_pill(&self, slot: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let pill = ShapeTokens::GOLDEN_GATE.bar_slot_pill_rect(slot, self.dpi);
        let radius = shape.slot_radius as f32;
        let rect = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(pill),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&rect, brush);
        }
    }

    fn draw_slot_focus_ring(&self, slot: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let ring = ShapeTokens::GOLDEN_GATE.bar_slot_focus_rect(slot, self.dpi);
        let radius = shape.focus_ring_radius as f32;
        let stroke = shape.focus_ring_width as f32;
        let inset = stroke * 0.5;
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: ring.x as f32 + inset,
                top: ring.y as f32 + inset,
                right: ring.right() as f32 - inset,
                bottom: ring.bottom() as f32 - inset,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&rect, brush, stroke, &self.stroke_style);
        }
    }

    fn draw_apple_logo(&self, layout: &Layout, brush: &ID2D1SolidColorBrush) -> Result<()> {
        let size = layout.icon_size as f32 * 1.08;
        let x0 = layout.start_slot.x as f32 + (layout.start_slot.width as f32 - size) * 0.5;
        let y0 = (layout.start_slot.height as f32 - size) * 0.5;
        let pt = |nx: f32, ny: f32| Vector2::new(x0 + nx * size, y0 + ny * size);
        let bez = |x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32| D2D1_BEZIER_SEGMENT {
            point1: pt(x1, y1),
            point2: pt(x2, y2),
            point3: pt(x3, y3),
        };

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;

            sink.BeginFigure(pt(0.67, 0.02), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&bez(0.55, 0.03, 0.46, 0.12, 0.48, 0.24));
            sink.AddBezier(&bez(0.59, 0.24, 0.68, 0.15, 0.67, 0.02));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);

            sink.BeginFigure(pt(0.49, 0.29), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&bez(0.43, 0.29, 0.38, 0.24, 0.32, 0.24));
            sink.AddBezier(&bez(0.19, 0.24, 0.10, 0.36, 0.12, 0.54));
            sink.AddBezier(&bez(0.14, 0.72, 0.24, 0.94, 0.33, 0.94));
            sink.AddBezier(&bez(0.39, 0.94, 0.42, 0.89, 0.49, 0.89));
            sink.AddBezier(&bez(0.56, 0.89, 0.59, 0.94, 0.65, 0.94));
            sink.AddBezier(&bez(0.73, 0.94, 0.81, 0.81, 0.86, 0.69));
            sink.AddBezier(&bez(0.73, 0.63, 0.71, 0.43, 0.82, 0.36));
            sink.AddBezier(&bez(0.77, 0.28, 0.70, 0.24, 0.64, 0.24));
            sink.AddBezier(&bez(0.57, 0.24, 0.54, 0.29, 0.49, 0.29));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);

            sink.Close()?;
            self.target.FillGeometry(&geometry, brush, None);
        }
        Ok(())
    }

    fn draw_tray_chevron(&self, layout: &Layout, brush: &ID2D1SolidColorBrush) -> Result<()> {
        let slot = layout.tray_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let dx = unit * 0.30;
        let dy = unit * 0.16;
        let stroke = (unit * 0.11).max(1.4);

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(Vector2::new(cx - dx, cy + dy), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(Vector2::new(cx, cy - dy));
            sink.AddLine(Vector2::new(cx + dx, cy + dy));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geometry, brush, stroke, &self.stroke_style);
        }
        Ok(())
    }

    fn draw_status_slot(
        &self,
        layout: &Layout,
        status: SystemStatus,
        fg: &ID2D1SolidColorBrush,
        muted: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let slot = layout.status_slot;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let stroke = (unit * 0.105).max(1.35);

        let wifi_cx = slot.x as f32 + unit * 0.80;
        self.draw_wifi_icon(wifi_cx, cy, unit * 0.48, stroke, fg)?;

        let speaker_x = wifi_cx + unit * 0.88;
        self.draw_speaker_icon(
            speaker_x,
            cy,
            unit * 0.44,
            stroke,
            status.volume,
            status.muted,
            fg,
        )?;

        let bat_x = speaker_x + unit * 1.38;
        let bat_w = unit * 1.40;
        let bat_h = unit * 0.72;
        let bat_top = cy - bat_h * 0.5;
        let corner = (bat_h * 0.28).max(2.0);

        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bat_x,
                top: bat_top,
                right: bat_x + bat_w,
                bottom: bat_top + bat_h,
            },
            radiusX: corner,
            radiusY: corner,
        };
        let cap_h = bat_h * 0.40;
        let cap_w = (stroke * 1.15).max(1.5);
        let cap = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bat_x + bat_w + 1.2,
                top: cy - cap_h * 0.5,
                right: bat_x + bat_w + 1.2 + cap_w,
                bottom: cy + cap_h * 0.5,
            },
            radiusX: cap_w * 0.5,
            radiusY: cap_w * 0.5,
        };

        let inset = (stroke * 1.30).max(1.8);
        let max_fill_w = (bat_w - inset * 2.0).max(0.0);
        let fill_w = (max_fill_w * status.battery.fill_fraction()).max(if max_fill_w > 0.0 {
            1.5
        } else {
            0.0
        });
        let fill_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bat_x + inset,
                top: bat_top + inset,
                right: bat_x + inset + fill_w,
                bottom: bat_top + bat_h - inset,
            },
            radiusX: (corner - 1.2).max(1.0),
            radiusY: (corner - 1.2).max(1.0),
        };

        unsafe {
            self.target
                .DrawRoundedRectangle(&body, muted, stroke * 0.9, &self.stroke_style);
            self.target.FillRoundedRectangle(&cap, muted);
            if status.battery.charging {
                let green = &self.brushes.color_brush(CHARGING_GREEN);
                self.target.FillRoundedRectangle(&fill_rect, green);
                self.draw_charging_bolt(bat_x + bat_w * 0.5, cy, bat_h * 0.42, fg)?;
            } else {
                self.target.FillRoundedRectangle(&fill_rect, fg);
            }
        }

        let text_left = bat_x + bat_w + cap_w + unit * 0.34;
        let text_rect = D2D_RECT_F {
            left: text_left,
            top: 0.0,
            right: slot.right() as f32,
            bottom: slot.height as f32,
        };
        let label = status.battery.label();
        self.draw_text(&label, &self.text_leading, text_rect, fg);
        Ok(())
    }

    fn draw_charging_bolt(
        &self,
        cx: f32,
        cy: f32,
        half_h: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let w = half_h * 0.65;
        unsafe {
            let bolt = self.factory.CreatePathGeometry()?;
            let sink = bolt.Open()?;
            sink.BeginFigure(
                Vector2::new(cx + w * 0.15, cy - half_h),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx - w, cy + half_h * 0.10));
            sink.AddLine(Vector2::new(cx - w * 0.05, cy + half_h * 0.10));
            sink.AddLine(Vector2::new(cx - w * 0.15, cy + half_h));
            sink.AddLine(Vector2::new(cx + w, cy - half_h * 0.10));
            sink.AddLine(Vector2::new(cx + w * 0.05, cy - half_h * 0.10));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&bolt, brush, None);
        }
        Ok(())
    }

    fn draw_wifi_icon(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = stroke.max(1.0);
        draw_wifi_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx,
            cy,
            radius,
            stroke,
            brush,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_speaker_icon(
        &self,
        x: f32,
        cy: f32,
        half_h: f32,
        stroke: f32,
        volume: u8,
        muted: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        draw_speaker_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            x,
            cy,
            half_h,
            stroke,
            volume,
            muted,
            brush,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_wifi_glyph(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    cx: f32,
    cy: f32,
    radius: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let base_y = cy + radius * 0.56;
    unsafe {
        let dot = D2D1_ELLIPSE {
            point: Vector2::new(cx, base_y),
            radiusX: stroke * 0.80,
            radiusY: stroke * 0.80,
        };
        target.FillEllipse(&dot, brush);

        for factor in [0.54_f32, 0.96_f32] {
            let r = radius * factor;
            let dx = r * 0.72;
            let dy = r * 0.72;
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(Vector2::new(cx - dx, base_y - dy), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(cx + dx, base_y - dy),
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            target.DrawGeometry(&geometry, brush, stroke, stroke_style);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_speaker_glyph(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    x: f32,
    cy: f32,
    half_h: f32,
    stroke: f32,
    volume: u8,
    muted: bool,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let box_h = half_h * 0.44;
    let box_w = half_h * 0.40;
    let cone_w = half_h * 0.92;
    unsafe {
        let cone = factory.CreatePathGeometry()?;
        let sink = cone.Open()?;
        sink.BeginFigure(Vector2::new(x, cy - box_h), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(Vector2::new(x + box_w, cy - box_h));
        sink.AddLine(Vector2::new(x + cone_w, cy - half_h * 0.88));
        sink.AddLine(Vector2::new(x + cone_w, cy + half_h * 0.88));
        sink.AddLine(Vector2::new(x + box_w, cy + box_h));
        sink.AddLine(Vector2::new(x, cy + box_h));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        target.FillGeometry(&cone, brush, None);

        if muted || volume == 0 {
            target.DrawLine(
                Vector2::new(x + cone_w + stroke * 0.65, cy - half_h * 0.55),
                Vector2::new(x + cone_w + stroke * 2.55, cy + half_h * 0.55),
                brush,
                stroke * 0.95,
                stroke_style,
            );
            target.DrawLine(
                Vector2::new(x + cone_w + stroke * 2.55, cy - half_h * 0.55),
                Vector2::new(x + cone_w + stroke * 0.65, cy + half_h * 0.55),
                brush,
                stroke * 0.95,
                stroke_style,
            );
            return Ok(());
        }

        let wave_count = if volume <= 35 { 1 } else { 2 };
        for &(offset, scale_y) in &[(1.10_f32, 0.46_f32), (2.25_f32, 0.78_f32)][..wave_count] {
            let wave = factory.CreatePathGeometry()?;
            let wsink = wave.Open()?;
            let wx = x + cone_w + stroke * offset;
            let wy = half_h * scale_y;
            wsink.BeginFigure(Vector2::new(wx, cy - wy), D2D1_FIGURE_BEGIN_HOLLOW);
            wsink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(wx, cy + wy),
                size: D2D_SIZE_F {
                    width: wy * 1.15,
                    height: wy * 1.15,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            wsink.EndFigure(D2D1_FIGURE_END_OPEN);
            wsink.Close()?;
            target.DrawGeometry(&wave, brush, stroke * 0.90, stroke_style);
        }
    }
    Ok(())
}

impl Renderer {
    fn hud_tone_brush(&self, tone: HudTone, alpha: f32) -> ID2D1SolidColorBrush {
        match tone {
            HudTone::Neutral => self.brushes.alpha_brush(FOREGROUND, 0.95 * alpha),
            HudTone::Positive => self.brushes.alpha_brush(CHARGING_GREEN, 0.96 * alpha),
            HudTone::Warning => self.brushes.alpha_brush(AMBER_WARN, 0.96 * alpha),
            HudTone::Muted => self.brushes.alpha_brush(FOREGROUND, 0.58 * alpha),
        }
    }

    #[expect(clippy::too_many_lines)]
    fn draw_notch_hud(&self, layout: &Layout, hud: NotchHudState) -> Result<()> {
        let presentation = present_hud(hud.kind);
        let alpha = hud.content_alpha.clamp(0.0, 1.0);
        let geometry = layout.hud;
        let stroke = (layout.icon_size as f32 * 0.105).max(1.35);
        let tone = self.hud_tone_brush(presentation.tone, alpha);
        let label_brush = self.brushes.alpha_brush(FOREGROUND, 0.95 * alpha);
        let track = self.brushes.alpha_brush(FOREGROUND, 0.16 * alpha);

        let glyph = geometry.glyph;
        let gcx = glyph.x as f32 + glyph.width as f32 * 0.5;
        let gcy = glyph.y as f32 + glyph.height as f32 * 0.5;
        let half = glyph.width as f32 * 0.5;
        let volume = match presentation.value {
            HudValue::Percent(percent) => percent,
            HudValue::Muted | HudValue::Pill { .. } | HudValue::Text(_) | HudValue::Badge(_) => 0,
        };
        match presentation.glyph {
            HudGlyph::Speaker => {
                self.draw_speaker_icon(
                    glyph.x as f32,
                    gcy,
                    half * 0.9,
                    stroke,
                    volume,
                    false,
                    &tone,
                )?;
            }
            HudGlyph::SpeakerMuted => {
                self.draw_speaker_icon(glyph.x as f32, gcy, half * 0.9, stroke, 0, true, &tone)?;
            }
            HudGlyph::Sun => self.draw_sun_icon(gcx, gcy, half * 0.85, stroke, &tone),
            HudGlyph::Bolt => self.draw_charging_bolt(gcx, gcy, half * 0.9, &tone)?,
            HudGlyph::Battery => {
                let level = presentation.level.unwrap_or(0.0);
                self.draw_battery_glyph(gcx, gcy, half, stroke, level, &tone);
            }
            HudGlyph::CapsLock => self.draw_capslock_icon(gcx, gcy, half * 0.9, stroke, &tone)?,
            HudGlyph::NumLock => self.draw_numlock_icon(gcx, gcy, half * 0.9, stroke, &tone),
            HudGlyph::Keyboard => self.draw_keyboard_icon(gcx, gcy, half * 0.9, stroke, &tone),
            HudGlyph::BatterySaver => {
                self.draw_battery_saver_icon(gcx, gcy, half * 0.9, stroke, &tone)?;
            }
            HudGlyph::Moon => {
                draw_crescent_moon(
                    &self.factory,
                    &self.target,
                    Vector2::new(gcx, gcy),
                    half * 0.85,
                    &tone,
                )?;
            }
            HudGlyph::Bluetooth => {
                draw_bluetooth_symbol(
                    &self.factory,
                    &self.target,
                    &self.stroke_style,
                    gcx,
                    gcy,
                    half * 0.85,
                    &tone,
                )?;
            }
            HudGlyph::Headphones => {
                self.draw_headphones_icon(gcx, gcy, half * 0.88, stroke, &tone)?;
            }
            HudGlyph::Otto => {
                self.draw_otto_mark(gcx, gcy, half * 0.88, stroke, &tone);
            }
        }

        let label_rect = if presentation.level.is_some() {
            rect_to_d2d(geometry.label)
        } else {
            let max_right = match presentation.value {
                HudValue::Badge(text) => {
                    let pill_w = self.hud_badge_width(text);
                    let gap = scale(8, self.dpi) as f32;
                    (geometry.value.right() as f32 - pill_w - gap)
                        .min(geometry.meter.right() as f32)
                        .max(geometry.label.x as f32)
                }
                _ => geometry.meter.right() as f32,
            };
            D2D_RECT_F {
                left: geometry.label.x as f32,
                top: geometry.label.y as f32,
                right: max_right,
                bottom: geometry.label.bottom() as f32,
            }
        };
        self.draw_text(
            presentation.label.as_str(),
            &self.text_hud_label,
            label_rect,
            &label_brush,
        );

        if let Some(level) = presentation.level {
            self.draw_hud_meter(geometry.meter, level, &tone, &track);
        }

        match presentation.value {
            HudValue::Percent(percent) => self.draw_tabular_text(
                &format!("{percent}%"),
                &self.text_hud_value,
                rect_to_d2d(geometry.value),
                &tone,
            ),
            HudValue::Muted => self.draw_text(
                "Muted",
                &self.text_hud_value,
                rect_to_d2d(geometry.value),
                &tone,
            ),
            HudValue::Pill { on } => self.draw_hud_pill(geometry.value, on, alpha),
            HudValue::Text(code) => self.draw_hud_text_badge(geometry.value, code.as_str(), alpha),
            HudValue::Badge(text) => self.draw_hud_text_badge(geometry.value, text, alpha),
        }
        Ok(())
    }

    fn draw_hud_meter(
        &self,
        meter: Rect,
        level: f32,
        fill: &ID2D1SolidColorBrush,
        track: &ID2D1SolidColorBrush,
    ) {
        let radius = meter.height as f32 * 0.5;
        let track_shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(meter),
            radiusX: radius,
            radiusY: radius,
        };
        let filled_w = meter.width as f32 * level.clamp(0.0, 1.0);
        unsafe {
            self.target.FillRoundedRectangle(&track_shape, track);
            if filled_w > 0.0 {
                let filled = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: meter.x as f32,
                        top: meter.y as f32,
                        right: meter.x as f32 + filled_w,
                        bottom: meter.bottom() as f32,
                    },
                    radiusX: radius,
                    radiusY: radius,
                };
                self.target.FillRoundedRectangle(&filled, fill);
            }
        }
    }

    fn draw_hud_pill(&self, value: Rect, on: bool, alpha: f32) {
        let pill_w = scale(34, self.dpi) as f32;
        let pill_h = scale(16, self.dpi) as f32;
        let cy = value.y as f32 + value.height as f32 * 0.5;
        let rect = D2D_RECT_F {
            left: value.right() as f32 - pill_w,
            top: cy - pill_h * 0.5,
            right: value.right() as f32,
            bottom: cy + pill_h * 0.5,
        };
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: pill_h * 0.5,
            radiusY: pill_h * 0.5,
        };
        if on {
            let fill = self.brushes.alpha_brush(CHARGING_GREEN, 0.96 * alpha);
            let ink = self.brushes.alpha_brush(BLACK, 0.92 * alpha);
            unsafe {
                self.target.FillRoundedRectangle(&rounded, &fill);
            }
            self.draw_text("ON", &self.text_app_bold, rect, &ink);
        } else {
            let outline = self.brushes.alpha_brush(FOREGROUND, 0.58 * alpha);
            unsafe {
                self.target
                    .DrawRoundedRectangle(&rounded, &outline, 1.0, None);
            }
            self.draw_text("OFF", &self.text_app_bold, rect, &outline);
        }
    }

    fn hud_badge_width(&self, text: &str) -> f32 {
        let min_w = scale(34, self.dpi) as f32;
        let char_w = scale(6, self.dpi) as f32;
        let pad_w = scale(12, self.dpi) as f32;
        (text.chars().count() as f32 * char_w + pad_w).max(min_w)
    }

    fn draw_hud_text_badge(&self, value: Rect, text: &str, alpha: f32) {
        let pill_w = self.hud_badge_width(text);
        let pill_h = scale(16, self.dpi) as f32;
        let cy = value.y as f32 + value.height as f32 * 0.5;
        let rect = D2D_RECT_F {
            left: value.right() as f32 - pill_w,
            top: cy - pill_h * 0.5,
            right: value.right() as f32,
            bottom: cy + pill_h * 0.5,
        };
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: pill_h * 0.5,
            radiusY: pill_h * 0.5,
        };
        let fill = self.brushes.alpha_brush(FOREGROUND, 0.14 * alpha);
        let outline = self.brushes.alpha_brush(FOREGROUND, 0.42 * alpha);
        let ink = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        unsafe {
            self.target.FillRoundedRectangle(&rounded, &fill);
            self.target
                .DrawRoundedRectangle(&rounded, &outline, 1.0, None);
        }
        self.draw_text(text, &self.text_app_bold, rect, &ink);
    }

    pub(crate) fn draw_otto_mark(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let ring = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        let eye_width = (radius * 0.27).max(stroke * 0.65);
        let eye_height = radius * 0.62;
        unsafe {
            self.target
                .DrawEllipse(&ring, brush, stroke, Some(&self.stroke_style));
            for offset in [-radius * 0.35, radius * 0.35] {
                let eye = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + offset - eye_width * 0.5,
                        top: cy - eye_height * 0.5,
                        right: cx + offset + eye_width * 0.5,
                        bottom: cy + eye_height * 0.5,
                    },
                    radiusX: eye_width * 0.5,
                    radiusY: eye_width * 0.5,
                };
                self.target.FillRoundedRectangle(&eye, brush);
            }
        }
    }

    fn draw_battery_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        stroke: f32,
        level: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let body_w = half * 1.44;
        let body_h = half * 0.88;
        let left = cx - body_w * 0.5 - half * 0.1;
        let top = cy - body_h * 0.5;
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + body_w,
                bottom: top + body_h,
            },
            radiusX: half * 0.22,
            radiusY: half * 0.22,
        };
        let inset = stroke * 1.6;
        let inner_w = (body_w - inset * 2.0).max(0.0);
        let fill_w = inner_w * level.clamp(0.04, 1.0);
        let fill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + inset,
                top: top + inset,
                right: left + inset + fill_w,
                bottom: top + body_h - inset,
            },
            radiusX: half * 0.08,
            radiusY: half * 0.08,
        };
        let terminal = D2D_RECT_F {
            left: left + body_w + stroke * 0.5,
            top: cy - half * 0.14,
            right: left + body_w + stroke * 0.5 + half * 0.18,
            bottom: cy + half * 0.14,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&body, brush, stroke * 0.95, None);
            self.target.FillRectangle(&terminal, brush);
            self.target.FillRoundedRectangle(&fill, brush);
        }
    }

    fn draw_sun_icon(&self, cx: f32, cy: f32, r: f32, stroke: f32, brush: &ID2D1SolidColorBrush) {
        let core_r = r * 0.46;
        let inner = r * 0.68;
        let outer = r * 1.04;
        let diag = 0.707_106_77_f32;
        unsafe {
            let core = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: core_r,
                radiusY: core_r,
            };
            self.target.FillEllipse(&core, brush);
            for (dx, dy) in [
                (1.0_f32, 0.0_f32),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (diag, diag),
                (-diag, diag),
                (diag, -diag),
                (-diag, -diag),
            ] {
                self.target.DrawLine(
                    Vector2::new(cx + dx * inner, cy + dy * inner),
                    Vector2::new(cx + dx * outer, cy + dy * outer),
                    brush,
                    stroke * 0.92,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_capslock_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            let arrow = self.factory.CreatePathGeometry()?;
            let sink = arrow.Open()?;
            sink.BeginFigure(Vector2::new(cx, cy - r * 0.85), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(cx + r * 0.75, cy - r * 0.05));
            sink.AddLine(Vector2::new(cx + r * 0.32, cy - r * 0.05));
            sink.AddLine(Vector2::new(cx + r * 0.32, cy + r * 0.35));
            sink.AddLine(Vector2::new(cx - r * 0.32, cy + r * 0.35));
            sink.AddLine(Vector2::new(cx - r * 0.32, cy - r * 0.05));
            sink.AddLine(Vector2::new(cx - r * 0.75, cy - r * 0.05));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&arrow, brush, None);

            self.target.DrawLine(
                Vector2::new(cx - r * 0.32, cy + r * 0.68),
                Vector2::new(cx + r * 0.32, cy + r * 0.68),
                brush,
                stroke * 1.15,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    fn draw_numlock_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let half_w = r * 0.78;
        let half_h = r * 0.74;
        let cap = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - half_w,
                top: cy - half_h,
                right: cx + half_w,
                bottom: cy + half_h,
            },
            radiusX: r * 0.24,
            radiusY: r * 0.24,
        };
        let line_w = (stroke * 0.85).max(1.1);
        let vx = r * 0.21;
        let vy = r * 0.42;
        let hx = r * 0.42;
        let hy = r * 0.19;
        unsafe {
            self.target
                .DrawRoundedRectangle(&cap, brush, stroke * 0.92, None);
            self.target.DrawLine(
                Vector2::new(cx - vx, cy - vy),
                Vector2::new(cx - vx, cy + vy),
                brush,
                line_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + vx, cy - vy),
                Vector2::new(cx + vx, cy + vy),
                brush,
                line_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - hx, cy - hy),
                Vector2::new(cx + hx, cy - hy),
                brush,
                line_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - hx, cy + hy),
                Vector2::new(cx + hx, cy + hy),
                brush,
                line_w,
                &self.stroke_style,
            );
        }
    }

    fn draw_keyboard_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let half_w = r * 0.94;
        let half_h = r * 0.64;
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - half_w,
                top: cy - half_h,
                right: cx + half_w,
                bottom: cy + half_h,
            },
            radiusX: r * 0.22,
            radiusY: r * 0.22,
        };
        let key_h = r * 0.16;
        let row1_y = cy - r * 0.26;
        let row2_y = cy + r * 0.10;
        let key_w = r * 0.22;
        unsafe {
            self.target
                .DrawRoundedRectangle(&body, brush, stroke * 0.92, None);
            for offset in [-0.54_f32, -0.18, 0.18, 0.54] {
                let kx = cx + r * offset - key_w * 0.5;
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left: kx,
                        top: row1_y,
                        right: kx + key_w,
                        bottom: row1_y + key_h,
                    },
                    brush,
                );
            }
            let space_hw = r * 0.44;
            self.target.FillRectangle(
                &D2D_RECT_F {
                    left: cx - space_hw,
                    top: row2_y,
                    right: cx + space_hw,
                    bottom: row2_y + key_h,
                },
                brush,
            );
        }
    }

    fn draw_battery_saver_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let base_x = cx - r * 0.54;
        let base_y = cy + r * 0.48;
        let tip_x = cx + r * 0.72;
        let tip_y = cy - r * 0.68;
        unsafe {
            let leaf = self.factory.CreatePathGeometry()?;
            let sink = leaf.Open()?;
            sink.BeginFigure(Vector2::new(base_x, base_y), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx - r * 0.72, cy - r * 0.52),
                point2: Vector2::new(cx + r * 0.08, cy - r * 0.78),
                point3: Vector2::new(tip_x, tip_y),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx + r * 0.78, cy - r * 0.02),
                point2: Vector2::new(cx + r * 0.42, cy + r * 0.68),
                point3: Vector2::new(base_x, base_y),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&leaf, brush, None);
            self.target.DrawLine(
                Vector2::new(cx - r * 0.80, cy + r * 0.76),
                Vector2::new(cx - r * 0.16, cy + r * 0.12),
                brush,
                (stroke * 0.95).max(1.2),
                &self.stroke_style,
            );
        }
        Ok(())
    }

    #[expect(clippy::similar_names)]
    fn draw_headphones_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let arch_rx = r * 0.72;
        let arch_ry = r * 0.76;
        let cup_y = cy + r * 0.08;
        let cup_w = r * 0.34;
        let cup_h = r * 0.64;
        let cup_bottom = cup_y + cup_h;
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(Vector2::new(cx - arch_rx, cup_y), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(cx + arch_rx, cup_y),
                size: D2D_SIZE_F {
                    width: arch_rx,
                    height: arch_ry,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, brush, (stroke * 0.95).max(1.2), &self.stroke_style);
            for side in [-1.0_f32, 1.0_f32] {
                let left = cx + side * arch_rx - cup_w * 0.5;
                let cup = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left,
                        top: cup_y - r * 0.08,
                        right: left + cup_w,
                        bottom: cup_bottom,
                    },
                    radiusX: cup_w * 0.45,
                    radiusY: cup_w * 0.45,
                };
                self.target.FillRoundedRectangle(&cup, brush);
            }
        }
        Ok(())
    }

    fn draw_clock_slot(&self, layout: &Layout, status: SystemStatus, fg: &ID2D1SolidColorBrush) {
        let slot = layout.clock_slot;
        let pad_x = scale(6, self.dpi) as f32;
        let text_rect = D2D_RECT_F {
            left: slot.x as f32 + pad_x,
            top: 0.0,
            right: slot.right() as f32 - pad_x,
            bottom: slot.height as f32,
        };
        let text = status.clock.format();
        self.draw_tabular_text(&text, &self.text_leading, text_rect, fg);
    }

    fn draw_notification_slot(&self, layout: &Layout, fg: &ID2D1SolidColorBrush) -> Result<()> {
        let slot = layout.notification_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        self.draw_bell_icon(cx, cy, layout.icon_size as f32 * 0.42, fg)
    }

    fn draw_bell_icon(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) -> Result<()> {
        let top_y = cy - r * 0.85;
        let base_y = cy + r * 0.48;
        let dome_w = r * 0.72;
        let rim_w = r * 0.96;

        unsafe {
            let bell = self.factory.CreatePathGeometry()?;
            let sink = bell.Open()?;
            sink.BeginFigure(Vector2::new(cx, top_y), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx + dome_w * 0.85, top_y),
                point2: Vector2::new(cx + dome_w, cy - r * 0.10),
                point3: Vector2::new(cx + rim_w, base_y),
            });
            sink.AddLine(Vector2::new(cx - rim_w, base_y));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx - dome_w, cy - r * 0.10),
                point2: Vector2::new(cx - dome_w * 0.85, top_y),
                point3: Vector2::new(cx, top_y),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&bell, brush, None);

            let clapper = D2D1_ELLIPSE {
                point: Vector2::new(cx, base_y + r * 0.28),
                radiusX: r * 0.24,
                radiusY: r * 0.22,
            };
            self.target.FillEllipse(&clapper, brush);
        }
        Ok(())
    }

    fn draw_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    pub(crate) fn draw_tabular_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        draw_dwrite_tabular_text(&self.target, text, format, rect, brush);
    }

    fn notch_geometry_rect(
        &self,
        n: Rect,
        corner_radius: f32,
        ear_radius: f32,
        cache: &RefCell<Option<CachedNotchGeometry>>,
    ) -> Result<ID2D1PathGeometry> {
        let cr_bits = corner_radius.to_bits();
        let er_bits = ear_radius.to_bits();
        if let Some(cached) = cache.borrow().as_ref()
            && cached.rect == n
            && cached.corner_radius_bits == cr_bits
            && cached.ear_radius_bits == er_bits
        {
            return Ok(cached.geometry.clone());
        }

        let (left, right) = (n.x as f32, n.right() as f32);
        let top = n.y as f32;
        let bottom = n.bottom() as f32;

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            if ear_radius <= 0.5 {
                let cr = corner_radius
                    .min(n.width as f32 * 0.5)
                    .min(n.height as f32 * 0.5)
                    .max(0.0);
                write_floating_notch_figure(&sink, left, top, right, bottom, cr);
            } else {
                write_attached_notch_figure(
                    &sink,
                    left,
                    top,
                    right,
                    bottom,
                    corner_radius.min(n.height as f32 * 0.5).max(0.0),
                    ear_radius.max(1.0),
                );
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            *cache.borrow_mut() = Some(CachedNotchGeometry {
                rect: n,
                corner_radius_bits: cr_bits,
                ear_radius_bits: er_bits,
                geometry: geometry.clone(),
            });
            Ok(geometry)
        }
    }
}

fn notch_arc_segment(x: f32, y: f32, radius: f32, sweep: D2D1_SWEEP_DIRECTION) -> D2D1_ARC_SEGMENT {
    D2D1_ARC_SEGMENT {
        point: Vector2::new(x, y),
        size: D2D_SIZE_F {
            width: radius,
            height: radius,
        },
        rotationAngle: 0.0,
        sweepDirection: sweep,
        arcSize: D2D1_ARC_SIZE_SMALL,
    }
}

struct SquircleCorner {
    reach: f32,
    smoothing: f32,
    arc_start: (f32, f32),
    arc_end: (f32, f32),
    edge_ctrl_1: f32,
    edge_ctrl_2: f32,
    arc_ctrl_1: (f32, f32),
    arc_ctrl_2: (f32, f32),
}

impl SquircleCorner {
    fn new(radius: f32, width: f32, height: f32) -> Self {
        let max_reach = 1.528_665 * radius;
        let reach_x = max_reach.min(width * 0.5);
        let reach_y = max_reach.min(height * 0.5);
        let smooth_x = ((reach_x / radius - 1.0) / 0.528_665).clamp(0.0, 1.0);
        let smooth_y = ((reach_y / radius - 1.0) / 0.528_665).clamp(0.0, 1.0);
        let smoothing = smooth_x.min(smooth_y);
        if smoothing <= 1.0e-4 {
            let kappa = (4.0 / 3.0) * (PI * 0.125).tan();
            let handle = radius * (1.0 - kappa);
            return Self {
                reach: radius,
                smoothing: 0.0,
                arc_start: (0.0, radius),
                arc_end: (radius, 0.0),
                edge_ctrl_1: radius,
                edge_ctrl_2: radius,
                arc_ctrl_1: (0.0, handle),
                arc_ctrl_2: (handle, 0.0),
            };
        }
        let reach = radius * (1.0 + 0.528_665 * smoothing);
        let alpha = (PI * 0.125) * smoothing;
        let sin_a = alpha.sin();
        let cos_a = alpha.cos();
        let tan_half = (alpha * 0.5).tan();
        let cos_half = (alpha * 0.5).cos();
        let arc_start = (radius * (1.0 - cos_a), radius * (1.0 - sin_a));
        let arc_end = (arc_start.1, arc_start.0);
        let edge_ctrl_2 = radius * (1.0 - tan_half);
        let edge_ctrl_1 = edge_ctrl_2 + 0.75 * radius * tan_half / (cos_half * cos_half);
        let arc_handle = (4.0 / 3.0) * (PI * 0.125 - alpha * 0.5).tan() * radius;
        let arc_ctrl_1 = (
            arc_start.0 + arc_handle * sin_a,
            arc_start.1 - arc_handle * cos_a,
        );
        let arc_ctrl_2 = (
            arc_end.0 - arc_handle * cos_a,
            arc_end.1 + arc_handle * sin_a,
        );
        Self {
            reach,
            smoothing,
            arc_start,
            arc_end,
            edge_ctrl_1,
            edge_ctrl_2,
            arc_ctrl_1,
            arc_ctrl_2,
        }
    }

    unsafe fn emit_v_to_u(&self, sink: &ID2D1GeometrySink, map: impl Fn(f32, f32) -> Vector2) {
        unsafe {
            if self.smoothing <= 1.0e-4 {
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: map(self.arc_ctrl_1.0, self.arc_ctrl_1.1),
                    point2: map(self.arc_ctrl_2.0, self.arc_ctrl_2.1),
                    point3: map(self.reach, 0.0),
                });
                return;
            }
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: map(0.0, self.edge_ctrl_1),
                point2: map(0.0, self.edge_ctrl_2),
                point3: map(self.arc_start.0, self.arc_start.1),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: map(self.arc_ctrl_1.0, self.arc_ctrl_1.1),
                point2: map(self.arc_ctrl_2.0, self.arc_ctrl_2.1),
                point3: map(self.arc_end.0, self.arc_end.1),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: map(self.edge_ctrl_2, 0.0),
                point2: map(self.edge_ctrl_1, 0.0),
                point3: map(self.reach, 0.0),
            });
        }
    }

    unsafe fn emit_u_to_v(&self, sink: &ID2D1GeometrySink, map: impl Fn(f32, f32) -> Vector2) {
        unsafe {
            if self.smoothing <= 1.0e-4 {
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: map(self.arc_ctrl_2.0, self.arc_ctrl_2.1),
                    point2: map(self.arc_ctrl_1.0, self.arc_ctrl_1.1),
                    point3: map(0.0, self.reach),
                });
                return;
            }
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: map(self.edge_ctrl_1, 0.0),
                point2: map(self.edge_ctrl_2, 0.0),
                point3: map(self.arc_end.0, self.arc_end.1),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: map(self.arc_ctrl_2.0, self.arc_ctrl_2.1),
                point2: map(self.arc_ctrl_1.0, self.arc_ctrl_1.1),
                point3: map(self.arc_start.0, self.arc_start.1),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: map(0.0, self.edge_ctrl_2),
                point2: map(0.0, self.edge_ctrl_1),
                point3: map(0.0, self.reach),
            });
        }
    }
}

pub(crate) unsafe fn write_squircle_figure(
    sink: &ID2D1GeometrySink,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    cr: f32,
) {
    let width = (right - left).max(0.0);
    let height = (bottom - top).max(0.0);
    let radius = cr.clamp(0.0, width.min(height) * 0.5);
    unsafe {
        if radius <= 0.5 {
            sink.BeginFigure(Vector2::new(left, top), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(right, top));
            sink.AddLine(Vector2::new(right, bottom));
            sink.AddLine(Vector2::new(left, bottom));
            return;
        }
        let corner = SquircleCorner::new(radius, width, height);
        let reach = corner.reach;
        sink.BeginFigure(Vector2::new(left + reach, top), D2D1_FIGURE_BEGIN_FILLED);
        if width > reach * 2.0 + 0.001 {
            sink.AddLine(Vector2::new(right - reach, top));
        }
        corner.emit_u_to_v(sink, |u, v| Vector2::new(right - u, top + v));
        if height > reach * 2.0 + 0.001 {
            sink.AddLine(Vector2::new(right, bottom - reach));
        }
        corner.emit_v_to_u(sink, |u, v| Vector2::new(right - u, bottom - v));
        if width > reach * 2.0 + 0.001 {
            sink.AddLine(Vector2::new(left + reach, bottom));
        }
        corner.emit_u_to_v(sink, |u, v| Vector2::new(left + u, bottom - v));
        if height > reach * 2.0 + 0.001 {
            sink.AddLine(Vector2::new(left, top + reach));
        }
        corner.emit_v_to_u(sink, |u, v| Vector2::new(left + u, top + v));
    }
}

pub(crate) fn build_squircle_path(
    factory: &ID2D1Factory,
    rect: D2D_RECT_F,
    radius: f32,
) -> Result<ID2D1PathGeometry> {
    let key = SquircleCacheKey::new(rect, radius);
    if let Ok(Some(cached)) = SQUIRCLE_CACHE.try_with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some(pos) = cache.iter().position(|(k, _)| *k == key) {
            if pos + 1 != cache.len() {
                let entry = cache.remove(pos);
                cache.push(entry);
            }
            return cache.last().map(|(_, geom)| geom.clone());
        }
        None
    }) {
        return Ok(cached);
    }
    let geometry = unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        write_squircle_figure(&sink, rect.left, rect.top, rect.right, rect.bottom, radius);
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        geometry
    };
    let _ = SQUIRCLE_CACHE.try_with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.len() >= SQUIRCLE_CACHE_CAPACITY {
            cache.remove(0);
        }
        cache.push((key, geometry.clone()));
    });
    Ok(geometry)
}

unsafe fn write_floating_notch_figure(
    sink: &ID2D1GeometrySink,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    cr: f32,
) {
    unsafe {
        write_squircle_figure(sink, left, top, right, bottom, cr);
    }
}

unsafe fn write_attached_notch_figure(
    sink: &ID2D1GeometrySink,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    cr: f32,
    er: f32,
) {
    unsafe {
        sink.BeginFigure(Vector2::new(left - er, top), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(Vector2::new(right + er, top));
        sink.AddArc(&notch_arc_segment(
            right,
            top + er,
            er,
            D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
        ));
        sink.AddLine(Vector2::new(right, bottom - cr));
        if cr > 0.5 {
            sink.AddArc(&notch_arc_segment(
                right - cr,
                bottom,
                cr,
                D2D1_SWEEP_DIRECTION_CLOCKWISE,
            ));
        }
        sink.AddLine(Vector2::new(left + cr, bottom));
        if cr > 0.5 {
            sink.AddArc(&notch_arc_segment(
                left,
                bottom - cr,
                cr,
                D2D1_SWEEP_DIRECTION_CLOCKWISE,
            ));
        }
        sink.AddLine(Vector2::new(left, top + er));
        sink.AddArc(&notch_arc_segment(
            left - er,
            top,
            er,
            D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
        ));
    }
}

const fn translate_rect(rect: Rect, dx: i32) -> Rect {
    Rect {
        x: rect.x + dx,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

pub(crate) const fn rect_to_d2d(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: rect.right() as f32,
        bottom: rect.bottom() as f32,
    }
}

const fn brighten(color: D2D1_COLOR_F, amount: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

unsafe fn resolve_apple_font_family(dwrite: &IDWriteFactory) -> PCWSTR {
    let candidates = [w!("SF Pro Text"), w!("SF Pro Display"), w!("SF Pro")];
    unsafe {
        let mut collection: Option<IDWriteFontCollection> = None;
        if dwrite
            .GetSystemFontCollection(&raw mut collection, true)
            .is_ok()
            && let Some(coll) = collection
        {
            for family in candidates {
                let mut index = 0u32;
                let mut exists = windows::core::BOOL(0);
                if coll
                    .FindFamilyName(family, &raw mut index, &raw mut exists)
                    .is_ok()
                    && exists.as_bool()
                {
                    return family;
                }
            }
        }
    }
    w!("Segoe UI Variable Display")
}

pub(crate) fn draw_bluetooth_symbol(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    cx: f32,
    cy: f32,
    half: f32,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let w = half * 0.65;
    let stroke = (half * 0.22).max(1.2);
    unsafe {
        let geom = factory.CreatePathGeometry()?;
        let sink = geom.Open()?;
        sink.BeginFigure(
            Vector2::new(cx - w, cy - half * 0.48),
            D2D1_FIGURE_BEGIN_HOLLOW,
        );
        sink.AddLine(Vector2::new(cx + w, cy + half * 0.46));
        sink.AddLine(Vector2::new(cx, cy + half));
        sink.AddLine(Vector2::new(cx, cy - half));
        sink.AddLine(Vector2::new(cx + w, cy - half * 0.46));
        sink.AddLine(Vector2::new(cx - w, cy + half * 0.48));
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.Close()?;
        target.DrawGeometry(&geom, brush, stroke, stroke_style);
    }
    Ok(())
}

pub(crate) fn draw_crescent_moon(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    center: Vector2,
    g: f32,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let top = Vector2::new(center.X + g * 0.7, center.Y - g * 0.714);
    let bottom = Vector2::new(center.X + g * 0.7, center.Y + g * 0.714);
    unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        sink.BeginFigure(top, D2D1_FIGURE_BEGIN_FILLED);
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: bottom,
            size: D2D_SIZE_F {
                width: g,
                height: g,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_LARGE,
        });
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: top,
            size: D2D_SIZE_F {
                width: g * 0.85,
                height: g * 0.85,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_LARGE,
        });
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        target.FillGeometry(&geometry, brush, None);
    }
    Ok(())
}

pub(crate) fn draw_crescent_moon_outline(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    center: Vector2,
    g: f32,
    stroke: f32,
    stroke_style: &ID2D1StrokeStyle,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let top = Vector2::new(center.X + g * 0.7, center.Y - g * 0.714);
    let bottom = Vector2::new(center.X + g * 0.7, center.Y + g * 0.714);
    unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        sink.BeginFigure(top, D2D1_FIGURE_BEGIN_HOLLOW);
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: bottom,
            size: D2D_SIZE_F {
                width: g,
                height: g,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_LARGE,
        });
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.BeginFigure(bottom, D2D1_FIGURE_BEGIN_HOLLOW);
        sink.AddArc(&D2D1_ARC_SEGMENT {
            point: top,
            size: D2D_SIZE_F {
                width: g * 0.85,
                height: g * 0.85,
            },
            rotationAngle: 0.0,
            sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
            arcSize: D2D1_ARC_SIZE_LARGE,
        });
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.Close()?;
        target.DrawGeometry(&geometry, brush, stroke, Some(stroke_style));
    }
    Ok(())
}

fn try_draw_dwrite_tabular_layout(
    target: &ID2D1DCRenderTarget,
    wide: &[u16],
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let dwrite = shared_dwrite_factory()?;
    let typography = shared_tabular_typography()?;
    let max_width = (rect.right - rect.left).max(0.0);
    let max_height = (rect.bottom - rect.top).max(0.0);
    unsafe {
        let layout = dwrite.CreateTextLayout(wide, format, max_width, max_height)?;
        let range = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: u32::try_from(wide.len()).unwrap_or(u32::MAX),
        };
        layout.SetTypography(&typography, range)?;
        target.DrawTextLayout(
            Vector2::new(rect.left, rect.top),
            &layout,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
        );
    }
    Ok(())
}

pub(crate) fn draw_dwrite_tabular_text(
    target: &ID2D1DCRenderTarget,
    text: &str,
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    if try_draw_dwrite_tabular_layout(target, &wide, format, rect, brush).is_err() {
        unsafe {
            target.DrawText(
                &wide,
                format,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

pub(crate) unsafe fn create_text_format(
    dwrite: &IDWriteFactory,
    font_px: f32,
    weight: DWRITE_FONT_WEIGHT,
    alignment: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    unsafe {
        let family = resolve_apple_font_family(dwrite);
        let format = dwrite.CreateTextFormat(
            family,
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_px,
            w!("en-us"),
        )?;
        format.SetTextAlignment(alignment)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        let trimming_sign = dwrite.CreateEllipsisTrimmingSign(&format).ok();
        let _ = format.SetTrimming(&trimming, trimming_sign.as_ref());
        Ok(format)
    }
}

#[cfg(test)]
mod resource_tests {
    use std::sync::{Mutex, MutexGuard, PoisonError};

    use super::{
        DockRenderItem, DockRenderer, DownloadsStackRenderer, GenieRenderer, PopupMenuRenderer,
        Renderer, WindowControlsRenderer, release_shared_factories, shared_d2d_factory,
    };
    use crate::surface::{
        DibSurface, DrawSession, create_dc_render_target, draw_on, upload_bgra_bitmap,
    };
    use otto_core::{PixelLimits, Rect};
    use windows::Win32::Foundation::E_INVALIDARG;
    use windows::Win32::Graphics::Direct2D::ID2D1DCRenderTarget;
    use windows::Win32::System::Threading::{
        GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetGuiResources,
    };
    use windows::core::{IUnknown, Interface};

    const ROUNDS: usize = 3;
    const IMPOSSIBLE: i32 = 1 << 20;
    const SURFACE_SIZE: (i32, i32) = (64, 48);
    const BITMAP_SIDE: u32 = 16;
    const SHORT_RUN_CYCLES: usize = 100;
    const LONG_RUN_CYCLES: usize = 10_000;

    pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

    struct SerialResourceTest {
        _lock: MutexGuard<'static, ()>,
    }

    impl SerialResourceTest {
        fn begin() -> Self {
            Self {
                _lock: SERIAL.lock().unwrap_or_else(PoisonError::into_inner),
            }
        }
    }

    impl Drop for SerialResourceTest {
        fn drop(&mut self) {
            release_shared_factories();
        }
    }

    #[derive(Clone, Copy)]
    struct GuiCounts {
        gdi: u32,
        user: u32,
    }

    impl GuiCounts {
        fn now() -> Self {
            let process = unsafe { GetCurrentProcess() };
            unsafe {
                Self {
                    gdi: GetGuiResources(process, GR_GDIOBJECTS),
                    user: GetGuiResources(process, GR_USEROBJECTS),
                }
            }
        }

        fn assert_no_growth(self, label: &str) {
            let after = Self::now();
            assert!(
                after.gdi <= self.gdi,
                "{label}: GDI objects grew from {} to {}",
                self.gdi,
                after.gdi
            );
            assert!(
                after.user <= self.user,
                "{label}: USER objects grew from {} to {}",
                self.user,
                after.user
            );
        }
    }

    fn observed_refs<T: Interface>(object: &T) -> u32 {
        let unknown: IUnknown = object.cast().expect("IUnknown view");
        let vtable = unknown.vtable();
        unsafe {
            let count = (vtable.AddRef)(unknown.as_raw());
            (vtable.Release)(unknown.as_raw());
            count
        }
    }

    fn dc_target() -> ID2D1DCRenderTarget {
        let factory = shared_d2d_factory().expect("shared factory");
        create_dc_render_target(&factory).expect("DC render target")
    }

    fn exercise_successful_constructions() {
        for _ in 0..ROUNDS {
            construct_each_renderer_once();
        }
    }

    const RENDERER_KINDS: usize = 6;

    fn construct_renderer(kind: usize) {
        match kind % RENDERER_KINDS {
            0 => drop(DibSurface::new(320, 200).expect("surface")),
            1 => drop(WindowControlsRenderer::new(108, 42).expect("window controls")),
            2 => drop(DockRenderer::new(800, 160, 96).expect("dock")),
            3 => drop(PopupMenuRenderer::new(144).expect("popup menu")),
            4 => drop(
                GenieRenderer::new(Rect {
                    x: -200,
                    y: 0,
                    width: 640,
                    height: 480,
                })
                .expect("genie"),
            ),
            _ => drop(Renderer::new(1920, 48, 600, 432, 144, 0.0).expect("bar")),
        }
    }

    fn construct_each_renderer_once() {
        (0..RENDERER_KINDS).for_each(construct_renderer);
    }

    fn exercise_failed_constructions() {
        for _ in 0..ROUNDS {
            assert!(DibSurface::new(IMPOSSIBLE, IMPOSSIBLE).is_err());
            assert!(WindowControlsRenderer::new(IMPOSSIBLE, IMPOSSIBLE).is_err());
            assert!(DockRenderer::new(IMPOSSIBLE, IMPOSSIBLE, 96).is_err());
            assert!(
                GenieRenderer::new(Rect {
                    x: 0,
                    y: 0,
                    width: IMPOSSIBLE,
                    height: IMPOSSIBLE,
                })
                .is_err()
            );
            assert!(Renderer::new(1920, 48, IMPOSSIBLE, IMPOSSIBLE, 144, 0.0).is_err());
        }
    }

    fn upload_one_bitmap(target: &ID2D1DCRenderTarget, pixels: &[u8]) {
        drop(
            upload_bgra_bitmap(target, BITMAP_SIDE, BITMAP_SIDE, pixels, PixelLimits::ICON)
                .expect("bitmap upload"),
        );
    }

    fn bitmap_pixels() -> Vec<u8> {
        vec![0xFFu8; (BITMAP_SIDE * BITMAP_SIDE * 4) as usize]
    }

    fn surface_draw_and_upload_cycle(target: &ID2D1DCRenderTarget, pixels: &[u8]) {
        let surface = DibSurface::new(SURFACE_SIZE.0, SURFACE_SIZE.1).expect("surface");
        draw_on(target, &surface, SURFACE_SIZE, || Ok(())).expect("draw");
        upload_one_bitmap(target, pixels);
    }

    fn steady_across_cycles(
        label: &str,
        target: &ID2D1DCRenderTarget,
        cycles: usize,
        cycle: impl Fn(&ID2D1DCRenderTarget),
    ) {
        cycle(target);
        let baseline = GuiCounts::now();
        let refs = observed_refs(target);
        for _ in 0..cycles {
            cycle(target);
        }
        baseline.assert_no_growth(label);
        assert_eq!(
            observed_refs(target),
            refs,
            "{label} changed the target reference count"
        );
    }

    #[test]
    fn constructing_and_dropping_renderers_releases_every_gdi_object() {
        let _serial = SerialResourceTest::begin();
        exercise_successful_constructions();
        exercise_failed_constructions();
        let baseline = GuiCounts::now();
        exercise_successful_constructions();
        baseline.assert_no_growth("successful constructions");
        exercise_failed_constructions();
        baseline.assert_no_growth("failed constructions");
        release_shared_factories();
        baseline.assert_no_growth("factory release");
    }

    #[test]
    fn abandoned_draw_session_ends_the_draw_and_leaves_the_target_usable() {
        let _serial = SerialResourceTest::begin();
        let target = dc_target();
        let surface = DibSurface::new(SURFACE_SIZE.0, SURFACE_SIZE.1).expect("surface");
        drop(DrawSession::begin(&target, &surface, SURFACE_SIZE).expect("warm-up session"));
        draw_on(&target, &surface, SURFACE_SIZE, || Ok(())).expect("warm-up draw");
        let baseline = GuiCounts::now();
        let refs = observed_refs(&target);
        for _ in 0..ROUNDS {
            drop(DrawSession::begin(&target, &surface, SURFACE_SIZE).expect("session"));
        }
        draw_on(&target, &surface, SURFACE_SIZE, || Ok(()))
            .expect("target draws after abandoned sessions");
        baseline.assert_no_growth("abandoned draw sessions");
        assert_eq!(
            observed_refs(&target),
            refs,
            "abandoned draw sessions changed the target reference count"
        );
    }

    #[test]
    fn rejected_bitmap_uploads_fail_before_any_resource_is_created() {
        let _serial = SerialResourceTest::begin();
        let target = dc_target();
        let limits = PixelLimits {
            max_dimension: 16,
            max_bytes: 64,
        };
        let pixels = vec![0u8; 256];
        let rejected = [(0, 4, 64), (4, 0, 64), (17, 1, 68), (4, 4, 63), (8, 4, 128)];
        let baseline = GuiCounts::now();
        let refs = observed_refs(&target);
        for _ in 0..ROUNDS {
            for (width, height, length) in rejected {
                let error = upload_bgra_bitmap(&target, width, height, &pixels[..length], limits)
                    .expect_err("oversized or undersized upload must be rejected");
                assert_eq!(
                    error.code(),
                    E_INVALIDARG,
                    "{width}x{height} with {length} bytes"
                );
            }
        }
        baseline.assert_no_growth("rejected bitmap uploads");
        assert_eq!(
            observed_refs(&target),
            refs,
            "rejected bitmap uploads changed the target reference count"
        );
    }

    #[test]
    fn shelf_render_and_thumbnail_churn_releases_bitmaps_and_gdi_handles() {
        let _serial = SerialResourceTest::begin();
        let metrics = otto_core::Metrics::default();
        let make_layout = |shelf_input: otto_core::ShelfLayoutInput| {
            otto_core::compute_layout_with_notch(
                &metrics,
                1920,
                144,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: otto_core::NotchExpandedTab::Shelf,
                    previous_tab: otto_core::NotchExpandedTab::Shelf,
                    tab_blend: 1.0,
                    shelf: shelf_input,
                    ..otto_core::NotchLayoutInput::default()
                },
            )
        };
        let empty_layout = make_layout(otto_core::ShelfLayoutInput::default());
        let renderer = Renderer::new(
            1920,
            48,
            empty_layout.panel_window.width,
            empty_layout.panel_window.height,
            144,
            0.0,
        )
        .expect("renderer");
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        let lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        let mut shelf = otto_core::ShelfState::default();
        let make_thumb = |seed: u8| otto_core::ShelfThumbnail {
            width: BITMAP_SIDE,
            height: BITMAP_SIDE,
            bgra: vec![seed; (BITMAP_SIDE * BITMAP_SIDE * 4) as usize],
        };

        let clipboard = otto_core::ClipboardState::default();

        let draw_once = |layout: &otto_core::Layout, state: &otto_core::ShelfState| {
            renderer
                .draw_panel(
                    layout,
                    Some(otto_core::HitTarget::ShelfItem(0)),
                    None,
                    &pomodoro,
                    None,
                    0.0,
                    lyrics,
                    state,
                    &clipboard,
                    &otto_core::TasksState::default(),
                    &privacy,
                    None,
                )
                .expect("shelf draw");
        };

        for i in 0..otto_core::MAX_SHELF_ITEMS {
            shelf.push_payload(
                otto_core::ShelfPayload::File(format!(r"C:\Temp\img_{i}.png")),
                false,
                Some(make_thumb((i as u8).saturating_add(1))),
            );
        }
        let populated_layout = make_layout(otto_core::ShelfLayoutInput {
            visible_count: u8::try_from(shelf.visible_items().len()).unwrap_or(0),
            has_pager: shelf.page_count() > 1,
            has_items: true,
        });
        draw_once(&populated_layout, &shelf);
        let baseline = GuiCounts::now();

        for cycle in 0..30 {
            let next_page = usize::from(cycle % 2 == 1);
            shelf.page = next_page;
            draw_once(&populated_layout, &shelf);
            shelf.push_payload(
                otto_core::ShelfPayload::File(format!(r"C:\Temp\extra_{cycle}.png")),
                false,
                Some(make_thumb(((cycle % 200) + 1) as u8)),
            );
            draw_once(&populated_layout, &shelf);
            assert!(renderer.cached_shelf_bitmaps.borrow().len() <= otto_core::MAX_SHELF_ITEMS);
        }

        shelf.clear();
        draw_once(&empty_layout, &shelf);
        assert!(renderer.cached_shelf_bitmaps.borrow().is_empty());
        baseline.assert_no_growth("shelf thumbnail churn");
    }

    fn sample_batch_shelf(drag_hover: bool, count: usize) -> otto_core::ShelfState {
        let mut shelf = otto_core::ShelfState {
            drag_hover,
            ..otto_core::ShelfState::default()
        };
        for i in 0..count {
            let payload = match i % 4 {
                0 => otto_core::ShelfPayload::File(format!(r"C:\Temp\clip_{i}.png")),
                1 => otto_core::ShelfPayload::File(format!(r"C:\Projects\repo_{i}")),
                2 => otto_core::ShelfPayload::Url(format!("https://example.com/{i}")),
                _ => otto_core::ShelfPayload::Text(format!("Snippet line {i}")),
            };
            let is_dir = i % 4 == 1;
            let thumb = (!is_dir && i % 4 == 0).then(|| otto_core::ShelfThumbnail {
                width: BITMAP_SIDE,
                height: BITMAP_SIDE,
                bgra: vec![
                    u8::try_from((i % 200) + 1).unwrap_or(1);
                    (BITMAP_SIDE * BITMAP_SIDE * 4) as usize
                ],
            });
            shelf.push_payload(payload, is_dir, thumb);
        }
        shelf
    }

    #[expect(clippy::too_many_lines)]
    fn exercise_batch_1_2_bars_for_dpi(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        media: &otto_core::MediaScene,
        privacy: &otto_core::PrivacyState,
    ) {
        let status = otto_core::SystemStatus {
            caps_lock: true,
            num_lock: true,
            power_mode: otto_core::PowerMode::BatterySaver,
            input_lang: otto_core::InputLangCode::from_tag("US"),
            ..otto_core::SystemStatus::default()
        };
        let privacy_layout = otto_core::PrivacyLayoutState {
            compact_dots: true,
            panel_mic_control: true,
        };
        let compact_layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                compact_expansion: 1.0,
                media_present: true,
                privacy: privacy_layout,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let huds = [
            None,
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::NumLock { enabled: true },
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::Welcome,
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::KeyboardLayout {
                    code: otto_core::InputLangCode::from_tag("EN"),
                },
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::BatterySaver { enabled: true },
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::Dnd { enabled: true },
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::BluetoothConnected {
                    name: otto_core::HudDeviceLabel::from_name("AirPods Pro"),
                    kind: otto_core::BluetoothDeviceKind::Headphones,
                    battery_percent: Some(85),
                },
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
            Some(otto_core::NotchHudState {
                kind: otto_core::NotchHudKind::BluetoothConnected {
                    name: otto_core::HudDeviceLabel::from_name("MX Master 3S"),
                    kind: otto_core::BluetoothDeviceKind::Mouse,
                    battery_percent: None,
                },
                expansion: 1.0,
                content_alpha: 1.0,
                level: 1.0,
            }),
        ];
        for theme in [
            otto_core::ResolvedTheme::Dark,
            otto_core::ResolvedTheme::Light,
        ] {
            let appearance = otto_core::Appearance {
                theme,
                transparency: false,
                contrast: None,
            };
            for hud in huds {
                let mut input = otto_core::NotchLayoutInput {
                    hud_expansion: if hud.is_some() { 1.0 } else { 0.0 },
                    compact_expansion: if hud.is_none() { 1.0 } else { 0.0 },
                    media_present: true,
                    privacy: privacy_layout,
                    ..otto_core::NotchLayoutInput::default()
                };
                let layout = if hud.is_some() {
                    input.hud_expansion = 1.0;
                    otto_core::compute_layout_with_notch(metrics, 1920, dpi, "Finder", input)
                } else {
                    compact_layout.clone()
                };
                renderer
                    .draw_bar(
                        &layout,
                        status,
                        "Finder",
                        Some(otto_core::HitTarget::PrivacyIndicator),
                        Some(otto_core::HitTarget::Notch),
                        hud,
                        pomodoro,
                        Some(media),
                        0.45,
                        appearance,
                        privacy,
                        None,
                    )
                    .expect("batch 1-2 bar draw");
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exercise_batch_1_focus_and_alert(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        media: &otto_core::MediaScene,
        privacy: &otto_core::PrivacyState,
    ) {
        let privacy_layout = otto_core::PrivacyLayoutState {
            compact_dots: true,
            panel_mic_control: true,
        };
        let empty_shelf = otto_core::ShelfState::default();
        let empty_clipboard = otto_core::ClipboardState::default();
        let idle_lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        let mut running_pomo = pomodoro.clone();
        running_pomo.state.status = otto_core::PomodoroStatus::Running;
        for focus_mode in [
            otto_core::FocusSubMode::Pomodoro,
            otto_core::FocusSubMode::Timer,
            otto_core::FocusSubMode::Stopwatch,
        ] {
            running_pomo.focus_mode = focus_mode;
            let layout = otto_core::compute_layout_with_notch(
                metrics,
                1920,
                dpi,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    focus_mode,
                    pomodoro_surface: running_pomo.view().surface,
                    media_present: true,
                    privacy: privacy_layout,
                    ..otto_core::NotchLayoutInput::default()
                },
            );
            renderer
                .draw_panel(
                    &layout,
                    Some(otto_core::HitTarget::PrivacyMicMute),
                    Some(otto_core::HitTarget::FocusMode(focus_mode)),
                    &running_pomo,
                    Some(media),
                    0.5,
                    idle_lyrics,
                    &empty_shelf,
                    &empty_clipboard,
                    &otto_core::TasksState::default(),
                    privacy,
                    None,
                )
                .expect("focus submode draw");
        }

        let mut alert_pomo = pomodoro.clone();
        alert_pomo.state.status = otto_core::PomodoroStatus::Alert;
        alert_pomo.state.alert = Some(otto_core::PomodoroAlert {
            completed_phase: otto_core::PomodoroPhase::Focus,
            next_phase: otto_core::PomodoroPhase::ShortBreak,
        });
        let alert_layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                pomodoro_surface: otto_core::PomodoroSurface::Alert,
                media_present: true,
                privacy: privacy_layout,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        renderer
            .draw_panel(
                &alert_layout,
                Some(otto_core::HitTarget::PomodoroStartNext),
                None,
                &alert_pomo,
                Some(media),
                0.5,
                idle_lyrics,
                &empty_shelf,
                &empty_clipboard,
                &otto_core::TasksState::default(),
                privacy,
                None,
            )
            .expect("pomodoro alert draw");
    }

    #[allow(clippy::too_many_arguments)]
    fn exercise_batch_2_shelf(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        media: &otto_core::MediaScene,
        privacy: &otto_core::PrivacyState,
    ) {
        let privacy_layout = otto_core::PrivacyLayoutState {
            compact_dots: true,
            panel_mic_control: true,
        };
        let empty_shelf = sample_batch_shelf(true, 0);
        let full_shelf = sample_batch_shelf(false, 9);
        let empty_clipboard = otto_core::ClipboardState::default();
        let idle_lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        for (shelf_state, shelf_input) in [
            (&empty_shelf, otto_core::ShelfLayoutInput::default()),
            (
                &full_shelf,
                otto_core::ShelfLayoutInput {
                    visible_count: u8::try_from(full_shelf.visible_items().len()).unwrap_or(0),
                    has_pager: full_shelf.page_count() > 1,
                    has_items: true,
                },
            ),
        ] {
            let layout = otto_core::compute_layout_with_notch(
                metrics,
                1920,
                dpi,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: otto_core::NotchExpandedTab::Shelf,
                    previous_tab: otto_core::NotchExpandedTab::Shelf,
                    shelf: shelf_input,
                    media_present: true,
                    privacy: privacy_layout,
                    ..otto_core::NotchLayoutInput::default()
                },
            );
            renderer
                .draw_panel(
                    &layout,
                    Some(otto_core::HitTarget::ShelfAdd),
                    None,
                    pomodoro,
                    Some(media),
                    0.5,
                    idle_lyrics,
                    shelf_state,
                    &empty_clipboard,
                    &otto_core::TasksState::default(),
                    privacy,
                    None,
                )
                .expect("shelf tab draw");
        }
    }

    fn sample_batch_clipboard(
        paused: bool,
        count: usize,
        copied_first: bool,
    ) -> otto_core::ClipboardState {
        let mut state = otto_core::ClipboardState::default();
        for index in 0..count {
            let payload = match index % 3 {
                0 => otto_core::ClipboardPayload::Text(format!(
                    "clipboard snippet line {index} with long preview text"
                )),
                1 => otto_core::ClipboardPayload::Link(format!("https://example.com/item/{index}")),
                _ => otto_core::ClipboardPayload::Files(vec![
                    format!(r"C:\Temp\clip_{index}.txt"),
                    format!(r"C:\Temp\extra_{index}.png"),
                ]),
            };
            state.push_payload(payload);
        }
        state.paused = paused;
        if copied_first && let Some(first) = state.items.first().map(|item| item.id) {
            state.mark_copied(first);
        }
        state
    }

    #[allow(clippy::too_many_arguments)]
    fn exercise_batch_3_clipboard(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        media: &otto_core::MediaScene,
        privacy: &otto_core::PrivacyState,
    ) {
        let privacy_layout = otto_core::PrivacyLayoutState {
            compact_dots: true,
            panel_mic_control: true,
        };
        let empty_shelf = otto_core::ShelfState::default();
        let idle_lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        let live_empty = sample_batch_clipboard(false, 0, false);
        let paused_empty = sample_batch_clipboard(true, 0, false);
        let populated = sample_batch_clipboard(false, 6, true);
        for (clipboard_state, hover) in [
            (
                &live_empty,
                Some(otto_core::HitTarget::ClipboardPauseToggle),
            ),
            (&paused_empty, None),
            (&populated, Some(otto_core::HitTarget::ClipboardItem(0))),
        ] {
            let layout = otto_core::compute_layout_with_notch(
                metrics,
                1920,
                dpi,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: otto_core::NotchExpandedTab::Clipboard,
                    previous_tab: otto_core::NotchExpandedTab::Clipboard,
                    clipboard: clipboard_state.layout_input(),
                    media_present: true,
                    privacy: privacy_layout,
                    ..otto_core::NotchLayoutInput::default()
                },
            );
            renderer
                .draw_panel(
                    &layout,
                    hover,
                    Some(otto_core::HitTarget::NotchTabClipboard),
                    pomodoro,
                    Some(media),
                    0.5,
                    idle_lyrics,
                    &empty_shelf,
                    clipboard_state,
                    &otto_core::TasksState::default(),
                    privacy,
                    None,
                )
                .expect("clipboard tab draw");
        }
    }

    fn exercise_tasks_tab(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        privacy: &otto_core::PrivacyState,
    ) {
        let empty_shelf = otto_core::ShelfState::default();
        let empty_clipboard = otto_core::ClipboardState::default();
        let idle_lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        let empty = otto_core::TasksState::default();
        let mut populated = otto_core::TasksState::default();
        for index in 0..6 {
            let outcome = populated.add(&format!("render task {index} with a fairly long title"));
            if let otto_core::TaskAddOutcome::Added(id) = outcome {
                if index == 1 {
                    populated.toggle_done(id);
                }
                if index == 2 {
                    populated.toggle_active(id);
                }
            }
        }
        for (tasks_state, hover) in [
            (&empty, Some(otto_core::HitTarget::TasksAdd)),
            (&populated, Some(otto_core::HitTarget::TaskItem(0))),
            (&populated, Some(otto_core::HitTarget::TaskCheck(1))),
            (&populated, Some(otto_core::HitTarget::TaskRemove(2))),
        ] {
            let layout = otto_core::compute_layout_with_notch(
                metrics,
                1920,
                dpi,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: otto_core::NotchExpandedTab::Tasks,
                    previous_tab: otto_core::NotchExpandedTab::Tasks,
                    tasks: tasks_state.layout_input(),
                    ..otto_core::NotchLayoutInput::default()
                },
            );
            renderer
                .draw_panel(
                    &layout,
                    hover,
                    Some(otto_core::HitTarget::NotchTabTasks),
                    pomodoro,
                    None,
                    0.0,
                    idle_lyrics,
                    &empty_shelf,
                    &empty_clipboard,
                    tasks_state,
                    privacy,
                    None,
                )
                .expect("tasks tab draw");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exercise_batch_4_notifications(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        privacy: &otto_core::PrivacyState,
    ) {
        let empty_shelf = otto_core::ShelfState::default();
        let empty_clipboard = otto_core::ClipboardState::default();
        let idle_lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        for (kind, stage, ringing) in [
            (
                otto_core::NotificationKind::Message,
                otto_core::NotificationStage::Banner,
                false,
            ),
            (
                otto_core::NotificationKind::VideoCall,
                otto_core::NotificationStage::Ringing,
                true,
            ),
        ] {
            let notification = otto_core::NotchNotification {
                id: 1,
                app_id: String::from("Teams.App"),
                app_name: String::from("Teams"),
                kind,
                title: String::from("Ayu Lestari"),
                body: String::from("Are we still meeting at three? I have the slides ready."),
                badge: otto_core::notification_badge("Teams"),
                count: 2,
            };
            let content = otto_core::present_notification(&notification, stage);
            let layout = otto_core::compute_layout_with_notch(
                metrics,
                1920,
                dpi,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    notification: Some(otto_core::NotificationLayoutInput { ringing }),
                    ..otto_core::NotchLayoutInput::default()
                },
            );
            let hovered = if ringing {
                otto_core::HitTarget::NotificationAccept
            } else {
                otto_core::HitTarget::NotificationClose
            };
            renderer
                .draw_panel(
                    &layout,
                    Some(hovered),
                    Some(otto_core::HitTarget::NotificationCard),
                    pomodoro,
                    None,
                    0.5,
                    idle_lyrics,
                    &empty_shelf,
                    &empty_clipboard,
                    &otto_core::TasksState::default(),
                    privacy,
                    Some(&content),
                )
                .expect("notification card draw");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exercise_batch_1_2_panels_for_dpi(
        renderer: &Renderer,
        metrics: &otto_core::Metrics,
        dpi: u32,
        pomodoro: &otto_core::PomodoroScene,
        media: &otto_core::MediaScene,
        lyrics_sheet: &otto_core::LyricsSheet,
        privacy: &otto_core::PrivacyState,
    ) {
        exercise_batch_1_focus_and_alert(renderer, metrics, dpi, pomodoro, media, privacy);
        exercise_batch_2_shelf(renderer, metrics, dpi, pomodoro, media, privacy);
        exercise_batch_3_clipboard(renderer, metrics, dpi, pomodoro, media, privacy);
        exercise_batch_4_notifications(renderer, metrics, dpi, pomodoro, privacy);
        let privacy_layout = otto_core::PrivacyLayoutState {
            compact_dots: true,
            panel_mic_control: true,
        };
        let empty_shelf = otto_core::ShelfState::default();
        let empty_clipboard = otto_core::ClipboardState::default();
        let idle_lyrics = otto_core::LyricsView {
            sheet: None,
            status: otto_core::LyricsStatus::Idle,
            animated: false,
        };
        let ready_lyrics = otto_core::LyricsView {
            sheet: Some(lyrics_sheet),
            status: otto_core::LyricsStatus::Ready,
            animated: true,
        };
        for (lyrics_progress, lyrics) in [(0.0_f32, idle_lyrics), (1.0_f32, ready_lyrics)] {
            let layout = otto_core::compute_layout_with_notch(
                metrics,
                1920,
                dpi,
                "Finder",
                otto_core::NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab: otto_core::NotchExpandedTab::Media,
                    previous_tab: otto_core::NotchExpandedTab::Media,
                    media_present: true,
                    lyrics_progress,
                    privacy: privacy_layout,
                    ..otto_core::NotchLayoutInput::default()
                },
            );
            renderer
                .draw_panel(
                    &layout,
                    Some(otto_core::HitTarget::MediaPlayPause),
                    None,
                    pomodoro,
                    Some(media),
                    0.5,
                    lyrics,
                    &empty_shelf,
                    &empty_clipboard,
                    &otto_core::TasksState::default(),
                    privacy,
                    None,
                )
                .expect("media tab draw");
        }
    }

    fn exercise_batch_1_2_for_dpi(dpi: u32) {
        let metrics = otto_core::Metrics::default();
        let max_layout = otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Media,
                previous_tab: otto_core::NotchExpandedTab::Media,
                media_present: true,
                lyrics_progress: 1.0,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let renderer = Renderer::new(
            1920,
            max_layout.bar.height,
            max_layout.panel_window.width,
            max_layout.panel_window.height,
            dpi,
            0.0,
        )
        .expect("renderer");
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let media = otto_core::MediaScene::steady(
            otto_core::NowPlaying {
                source_app: String::from("Spotify.exe"),
                title: String::from("Midnight Signal"),
                artist: String::from("Lumen Avenue"),
                playing: true,
                position_ms: 1_500,
                duration_ms: 180_000,
                ..otto_core::NowPlaying::default()
            },
            Some(String::from("First lyric line")),
        );
        let lyrics_sheet = otto_core::parse_lrc("[00:01.00]First lyric line\n[00:05.00]Next line");
        let privacy = otto_core::PrivacyState {
            mic: otto_core::MicState::Active { muted: false },
            camera_active: true,
            location_active: false,
            app_name: Some(String::from("meet.exe")),
        };
        exercise_batch_1_2_bars_for_dpi(&renderer, &metrics, dpi, &pomodoro, &media, &privacy);
        exercise_batch_1_2_panels_for_dpi(
            &renderer,
            &metrics,
            dpi,
            &pomodoro,
            &media,
            &lyrics_sheet,
            &privacy,
        );
    }

    fn exercise_tasks_for_dpi(dpi: u32) {
        let metrics = otto_core::Metrics::default();
        let layout = otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Tasks,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let renderer = Renderer::new(
            1920,
            layout.bar.height,
            layout.panel_window.width,
            layout.panel_window.height,
            dpi,
            0.0,
        )
        .expect("renderer");
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        exercise_tasks_tab(&renderer, &metrics, dpi, &pomodoro, &privacy);
    }

    #[test]
    fn the_tasks_tab_renders_cleanly_across_dpis_without_leaking() {
        let _serial = SerialResourceTest::begin();
        exercise_tasks_for_dpi(96);
        let baseline = GuiCounts::now();
        for dpi in [96u32, 144, 192] {
            exercise_tasks_for_dpi(dpi);
        }
        baseline.assert_no_growth("tasks tab multi-dpi offscreen render");
        release_shared_factories();
        baseline.assert_no_growth("tasks tab factory release");
    }

    #[test]
    fn batch_1_and_2_panels_and_huds_render_cleanly_across_dpis_without_leaking() {
        let _serial = SerialResourceTest::begin();
        exercise_batch_1_2_for_dpi(96);
        let baseline = GuiCounts::now();
        for dpi in [96u32, 144, 192] {
            exercise_batch_1_2_for_dpi(dpi);
        }
        baseline.assert_no_growth("batch 1 and 2 multi-dpi offscreen render");
        release_shared_factories();
        baseline.assert_no_growth("batch 1 and 2 factory release");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn control_center_renders_main_and_subpanels_across_dpis_without_leaking() {
        use otto_core::{
            Appearance, AudioOutputDevice, BluetoothDevice, BluetoothDeviceKind,
            ControlCenterMetrics, ControlCenterPage, ControlCenterPlacement, ControlCenterSnapshot,
            ResolvedTheme, WifiNetwork,
        };
        let _serial = SerialResourceTest::begin();
        let mut snap = ControlCenterSnapshot::default();
        snap.status.brightness = Some(72);
        snap.status.volume = 64;
        snap.settings.plugins.dock = true;
        snap.settings.plugins.top_bar = true;
        snap.settings.plugins.genie = true;
        snap.settings.plugins.window_controls = true;
        snap.settings.dock.hide_taskbar = true;
        snap.settings.dock.reserve_space = true;
        snap.settings.dock.autohide = true;
        snap.settings.system.media_module = true;
        snap.settings.system.focus_module = true;
        snap.settings.system.clipboard_history = true;
        snap.settings.system.drop_shelf = true;
        snap.settings.system.notch_hud = true;
        snap.settings.system.privacy_indicators = true;
        snap.settings.online_lyrics = true;
        snap.wifi.supported = true;
        snap.wifi.enabled = true;
        snap.wifi.connected_ssid = Some(String::from("Studio-5G"));
        snap.wifi.networks.push(WifiNetwork {
            ssid: String::from("Studio-5G"),
            signal_percent: 90,
            secured: true,
            connected: true,
            saved: true,
        });
        snap.bluetooth.supported = true;
        snap.bluetooth.enabled = true;
        snap.bluetooth.devices.push(BluetoothDevice {
            id: String::from("1"),
            name: String::from("AirPods Pro"),
            kind: BluetoothDeviceKind::Headphones,
            connected: true,
            battery_percent: Some(85),
        });
        snap.audio_outputs.push(AudioOutputDevice {
            id: String::from("out-1"),
            name: String::from("Speakers"),
            is_default: true,
        });
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        let mut exercise = |dpi: u32| {
            let mut renderer = super::ControlCenterRenderer::new(dpi).expect("cc renderer");
            let mut views: Vec<(ControlCenterPage, Option<otto_core::OttoSettingsArea>)> = [
                ControlCenterPage::Main,
                ControlCenterPage::Wifi,
                ControlCenterPage::Bluetooth,
                ControlCenterPage::Sound,
                ControlCenterPage::Display,
                ControlCenterPage::OttoSettings,
            ]
            .into_iter()
            .map(|page| (page, None))
            .collect();
            views.extend(
                otto_core::OttoSettingsArea::ALL
                    .into_iter()
                    .map(|area| (ControlCenterPage::OttoSettings, Some(area))),
            );
            for (page, area) in views {
                snap.page = page;
                snap.otto_area = area;
                let layout = otto_core::compute_control_center_layout_for_snapshot(
                    &ControlCenterMetrics::DEFAULT,
                    ControlCenterPlacement {
                        bounds,
                        dpi,
                        anchor_right: 1750,
                        anchor_y: 36,
                        page,
                        item_count: snap.subpanel_item_count(page),
                        scroll_offset: 0,
                    },
                    &snap,
                );
                let focused = (page == ControlCenterPage::Main)
                    .then_some(otto_core::ControlCenterHitTarget::WifiToggle);
                renderer
                    .draw_to_surface(
                        &layout,
                        &snap,
                        super::ControlCenterPaintState {
                            hovered: None,
                            pressed: None,
                            focused,
                            morph_progress: None,
                            scene_time: 2.3,
                        },
                        &Appearance::glass(ResolvedTheme::Dark),
                    )
                    .expect("cc draw");
                assert!(!renderer.surface_pixels().is_empty());
            }
        };
        exercise(96);
        let baseline = GuiCounts::now();
        for dpi in [96, 144, 192] {
            exercise(dpi);
        }
        baseline.assert_no_growth("control center render cycles");
        release_shared_factories();
        baseline.assert_no_growth("control center factory release");
    }

    #[test]
    fn repeated_bitmap_uploads_release_every_bitmap() {
        let _serial = SerialResourceTest::begin();
        let target = dc_target();
        let pixels = bitmap_pixels();
        steady_across_cycles("repeated bitmap uploads", &target, SHORT_RUN_CYCLES, |dc| {
            upload_one_bitmap(dc, &pixels);
        });
    }

    #[test]
    #[ignore = "10,000 renderer cycles (about 16 minutes); run with `cargo test -p otto-render -- --ignored`"]
    fn ten_thousand_renderer_cycles_keep_gdi_and_user_objects_flat() {
        let _serial = SerialResourceTest::begin();
        construct_each_renderer_once();
        let baseline = GuiCounts::now();
        for cycle in 0..LONG_RUN_CYCLES {
            construct_renderer(cycle);
        }
        baseline.assert_no_growth("10,000 renderer cycles");
        release_shared_factories();
        baseline.assert_no_growth("factory release after 10,000 renderer cycles");
    }

    #[test]
    #[ignore = "10,000 surface, draw and bitmap cycles; run with `cargo test -p otto-render -- --ignored`"]
    fn ten_thousand_surface_draw_and_bitmap_cycles_keep_gdi_and_user_objects_flat() {
        let _serial = SerialResourceTest::begin();
        let target = dc_target();
        let pixels = bitmap_pixels();
        steady_across_cycles(
            "10,000 surface, draw and bitmap cycles",
            &target,
            LONG_RUN_CYCLES,
            |dc| surface_draw_and_upload_cycle(dc, &pixels),
        );
    }

    fn one_by_one_bmp(blue: u8, green: u8, red: u8) -> Vec<u8> {
        vec![
            b'B', b'M', 58, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
            1, 0, 24, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            blue, green, red, 0,
        ]
    }

    #[test]
    fn cover_art_cache_detects_in_place_byte_mutation() {
        struct ComApartmentGuard(bool);

        impl ComApartmentGuard {
            fn enter() -> Self {
                use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

                Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
            }
        }

        impl Drop for ComApartmentGuard {
            fn drop(&mut self) {
                if self.0 {
                    unsafe {
                        windows::Win32::System::Com::CoUninitialize();
                    }
                }
            }
        }

        let _serial = SerialResourceTest::begin();
        let _com = ComApartmentGuard::enter();
        let renderer = Renderer::new(320, 64, 320, 240, 96, 0.25).expect("renderer");
        let mut bytes = one_by_one_bmp(20, 40, 220);
        let original_ptr = bytes.as_ptr();
        let first = renderer
            .get_or_decode_cover_art(&bytes)
            .expect("initial cover decode");
        let cached = renderer
            .get_or_decode_cover_art(&bytes)
            .expect("cached cover hit");
        assert_eq!(first.bitmap.as_raw(), cached.bitmap.as_raw());

        bytes[54] = 240;
        bytes[56] = 15;
        assert_eq!(bytes.as_ptr(), original_ptr);

        let updated = renderer
            .get_or_decode_cover_art(&bytes)
            .expect("mutated cover decode");
        assert_ne!(first.bitmap.as_raw(), updated.bitmap.as_raw());
    }

    #[test]
    fn shelf_bitmap_cache_detects_in_place_byte_mutation() {
        let _serial = SerialResourceTest::begin();
        let renderer = Renderer::new(320, 64, 320, 240, 96, 0.25).expect("renderer");
        let mut shelf = otto_core::ShelfState::default();
        shelf.push_payload(
            otto_core::ShelfPayload::File(String::from(r"C:\Temp\sample.png")),
            false,
            Some(otto_core::ShelfThumbnail {
                width: 2,
                height: 2,
                bgra: vec![
                    10, 20, 200, 255, 10, 20, 200, 255, 10, 20, 200, 255, 10, 20, 200, 255,
                ],
            }),
        );
        let mut item = shelf.items[0].clone();
        let original_ptr = item.thumbnail.as_ref().unwrap().bgra.as_ptr();
        let first = renderer
            .resolve_shelf_bitmap(&item)
            .expect("initial shelf bitmap");
        let cached = renderer
            .resolve_shelf_bitmap(&item)
            .expect("cached shelf bitmap");
        assert_eq!(first.as_raw(), cached.as_raw());

        item.thumbnail.as_mut().unwrap().bgra[0] = 250;
        item.thumbnail.as_mut().unwrap().bgra[2] = 15;
        assert_eq!(item.thumbnail.as_ref().unwrap().bgra.as_ptr(), original_ptr);

        let updated = renderer
            .resolve_shelf_bitmap(&item)
            .expect("mutated shelf bitmap");
        assert_ne!(first.as_raw(), updated.as_raw());
    }

    #[test]
    fn renderer_recreate_target_keeps_drawing_and_gui_counts_flat() {
        let _serial = SerialResourceTest::begin();
        let metrics = otto_core::Metrics::default();
        let layout = otto_core::compute_layout(&metrics, 1920, 96);
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        let status = otto_core::SystemStatus::default();
        let mut renderer = Renderer::new(
            layout.bar.width,
            layout.bar.height,
            layout.panel_window.width,
            layout.panel_window.height,
            96,
            0.25,
        )
        .expect("renderer");
        let draw_once = |r: &Renderer| {
            r.draw_bar(
                &layout,
                status,
                "Finder",
                None,
                None,
                None,
                &pomodoro,
                None,
                0.0,
                otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
                &privacy,
                None,
            )
            .expect("draw bar");
        };
        draw_once(&renderer);
        let baseline = GuiCounts::now();
        for _ in 0..ROUNDS {
            renderer.recreate_target().expect("recreate target");
            draw_once(&renderer);
        }
        baseline.assert_no_growth("renderer target recreate");
    }

    #[test]
    fn secondary_renderers_recreate_target_without_error() {
        let _serial = SerialResourceTest::begin();
        let pixels = bitmap_pixels();
        let mut control_center = super::ControlCenterRenderer::new(96).expect("control center");
        let mut dock = DockRenderer::new(400, 96, 96).expect("dock");
        dock.set_icon(String::from("app"), BITMAP_SIDE, BITMAP_SIDE, &pixels)
            .expect("dock icon");
        let mut menu = PopupMenuRenderer::new(96).expect("menu");
        let mut controls = WindowControlsRenderer::new(80, 24).expect("controls");

        let baseline = GuiCounts::now();
        for _ in 0..ROUNDS {
            control_center
                .recreate_target()
                .expect("control center recreate");
            dock.recreate_target().expect("dock recreate");
            assert!(!dock.has_icon("app"));
            menu.recreate_target().expect("menu recreate");
            controls.recreate_target().expect("controls recreate");
        }
        baseline.assert_no_growth("secondary renderers target recreate");
    }

    #[test]
    fn calendar_and_notification_menus_render_across_dpi_without_resource_growth() {
        let _serial = SerialResourceTest::begin();
        let snapshot = otto_core::NotificationCenterSnapshot {
            access: otto_core::NotificationAccess::Allowed,
            items: vec![
                otto_core::NotificationSummary {
                    id: 1,
                    app_id: String::from("Slack"),
                    app_name: String::from("Slack"),
                    title: String::from("Design review starting in 10 minutes"),
                    body: String::from("Thread in #otto-ui with updated spacing tokens"),
                    timestamp_label: String::from("2m"),
                },
                otto_core::NotificationSummary {
                    id: 2,
                    app_id: String::from("Mail"),
                    app_name: String::from("Mail"),
                    title: String::from("Weekly build summary"),
                    body: String::new(),
                    timestamp_label: String::from("1h"),
                },
            ],
        };
        let row_sets = [
            otto_core::build_calendar_rows(2026, 4, Some(15)),
            otto_core::build_notification_rows(&snapshot),
        ];
        let metrics = otto_core::PopupMenuMetrics::default();
        let placement_at = |dpi| otto_core::PopupMenuPlacement {
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            dpi,
            anchor: otto_core::MenuAnchor::BelowPoint { x: 1580, y: 36 },
            scroll_offset: 0,
        };

        let render_dpis = || {
            for &dpi in &[96, 144, 192] {
                let menu = PopupMenuRenderer::new(dpi).expect("popup menu");
                for rows in &row_sets {
                    let layout = otto_core::compute_popup_menu_layout(
                        &metrics,
                        placement_at(dpi),
                        rows,
                        &menu,
                    );
                    for round in 0..ROUNDS {
                        let hovered = if round % 2 == 0 {
                            rows.iter().position(|row| {
                                matches!(row, otto_core::MenuRow::Notification { .. })
                            })
                        } else {
                            None
                        };
                        menu.render_preview(
                            &layout,
                            rows,
                            crate::MenuHover {
                                row: hovered,
                                calendar: None,
                            },
                            otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
                        )
                        .expect("render calendar or notification menu");
                    }
                }
            }
        };

        render_dpis();
        let baseline = GuiCounts::now();
        render_dpis();
        baseline.assert_no_growth("calendar and notification menus");
    }

    #[test]
    #[expect(clippy::too_many_lines)]
    fn dock_badges_progress_and_downloads_stack_render_without_leaking() {
        let _serial = SerialResourceTest::begin();
        let pixels = bitmap_pixels();
        let thumbs = vec![
            Some(otto_core::ShelfThumbnail {
                width: BITMAP_SIDE,
                height: BITMAP_SIDE,
                bgra: pixels,
            }),
            None,
            None,
        ];
        let stack_items = vec![
            otto_core::DownloadsStackEntry {
                name: String::from("Wallpaper.png"),
                is_dir: false,
            },
            otto_core::DownloadsStackEntry {
                name: String::from("Builds"),
                is_dir: true,
            },
            otto_core::DownloadsStackEntry {
                name: String::from("Release Notes.pdf"),
                is_dir: false,
            },
        ];
        let bounds = Rect {
            x: 0,
            y: 32,
            width: 1920,
            height: 1048,
        };
        let render_once = || {
            for &dpi in &[96_u32, 144, 192] {
                let mut stack = DownloadsStackRenderer::new(dpi).expect("stack renderer");
                stack.set_thumbnails(&thumbs).expect("stack thumbs");
                let stack_layout =
                    otto_core::compute_downloads_stack_layout(1400, 960, bounds, dpi, &stack_items);
                stack
                    .render_preview(
                        &stack_layout,
                        &stack_items,
                        otto_core::DownloadsStackHit::Item(1),
                        otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
                    )
                    .expect("render downloads stack");
                stack.recreate_target().expect("stack recreate");
                stack.clear_thumbnails();

                let metrics = otto_core::DockMetrics::MACOS_SONOMA;
                let dock_layout = otto_core::compute_dock_layout(
                    &metrics,
                    otto_core::DockInput {
                        screen_width: 1920,
                        dpi,
                        app_count: 1,
                        right_count: 1,
                        has_trash: true,
                        cursor_x: None,
                        mag_strength: 0.0,
                        bounce_offsets: &[],
                        item_presence: &[],
                        hide_progress: 0.0,
                    },
                );
                let dock =
                    DockRenderer::new(dock_layout.window_width, dock_layout.window_height, dpi)
                        .expect("dock renderer");
                let items = [
                    DockRenderItem {
                        key: "finder",
                        title: "Finder",
                        apple_icon: Some(otto_core::AppleIconKind::Finder),
                        running: true,
                        active: true,
                        hidden: false,
                        focused: false,
                        badge_count: Some(12),
                        progress: None,
                    },
                    DockRenderItem {
                        key: "downloads",
                        title: "Downloads",
                        apple_icon: Some(otto_core::AppleIconKind::Folder),
                        running: false,
                        active: false,
                        hidden: false,
                        focused: false,
                        badge_count: Some(120),
                        progress: Some(0.65),
                    },
                ];
                dock.render_preview(
                    &dock_layout,
                    &items,
                    otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
                )
                .expect("render dock badges");
            }
        };

        render_once();
        let baseline = GuiCounts::now();
        render_once();
        baseline.assert_no_growth("dock badges and downloads stack");
    }

    #[test]
    fn squircle_corner_blends_cleanly_between_circular_pill_and_g2_spline() {
        let pill = super::SquircleCorner::new(18.0, 200.0, 36.0);
        assert!(pill.smoothing.abs() < f32::EPSILON);
        assert!((pill.reach - 18.0).abs() < 1.0e-4);
        assert!((pill.arc_ctrl_1.1 - 18.0 * (1.0 - 0.552_284_8)).abs() < 1.0e-3);

        let full = super::SquircleCorner::new(16.0, 120.0, 80.0);
        assert!((full.smoothing - 1.0).abs() < 1.0e-5);
        assert!((full.reach - 1.528_665 * 16.0).abs() < 1.0e-3);
        assert!((full.arc_start.0 - 0.076_12 * 16.0).abs() < 1.0e-2);
        assert!((full.arc_start.1 - 0.617_32 * 16.0).abs() < 1.0e-2);
        assert!((full.edge_ctrl_2 - 0.801_09 * 16.0).abs() < 1.0e-2);

        for (w, h, r) in [
            (200.0_f32, 24.0_f32, 12.0_f32),
            (24.0, 200.0, 12.0),
            (40.0, 40.0, 14.0),
            (120.0, 44.0, 16.0),
            (12.0, 12.0, 20.0),
        ] {
            let clamped = r.clamp(0.0, w.min(h) * 0.5);
            let corner = super::SquircleCorner::new(clamped, w, h);
            assert!(corner.reach <= w * 0.5 + 1.0e-4);
            assert!(corner.reach <= h * 0.5 + 1.0e-4);
            assert!(corner.reach >= corner.edge_ctrl_1 - 1.0e-4);
            assert!(corner.edge_ctrl_1 >= corner.edge_ctrl_2 - 1.0e-4);
            assert!(corner.edge_ctrl_2 >= corner.arc_start.1 - 1.0e-4);
            assert!(corner.arc_start.1 >= corner.arc_ctrl_1.1 - 1.0e-4);
            assert!(corner.arc_ctrl_1.1 >= corner.arc_ctrl_2.1 - 1.0e-4);
            assert!(corner.arc_ctrl_2.1 >= corner.arc_end.1 - 1.0e-4);
            assert!(corner.arc_end.1 >= 0.0);
            assert!((corner.arc_start.0 - corner.arc_end.1).abs() < 1.0e-4);
            assert!((corner.arc_ctrl_1.0 - corner.arc_ctrl_2.1).abs() < 1.0e-4);
        }
    }

    #[test]
    fn floating_notch_squircle_tabular_rim_and_metaball_do_not_leak() {
        let _serial = SerialResourceTest::begin();
        let metrics = otto_core::Metrics::default();
        let mut layout = otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            144,
            "Finder",
            otto_core::NotchLayoutInput {
                compact_expansion: 1.0,
                secondary_expansion: 0.45,
                media_present: true,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        layout.notch.y = 8;
        layout.notch.height = 42;
        layout.notch_corner_radius = 18;
        layout.notch_ear_radius = 0;
        layout.notch_secondary = Rect {
            x: layout.notch.right() + 4,
            y: layout.notch.y,
            width: 42,
            height: 42,
        };
        let renderer = Renderer::new(
            1920,
            layout.bar.height.max(64),
            layout.panel_window.width,
            layout.panel_window.height,
            144,
            0.0,
        )
        .expect("renderer");
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        let status = otto_core::SystemStatus::default();
        let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);

        let exercise = || {
            for _ in 0..20 {
                renderer
                    .draw_bar(
                        &layout,
                        status,
                        "Finder",
                        Some(otto_core::HitTarget::NotchSecondary),
                        None,
                        None,
                        &pomodoro,
                        None,
                        0.3,
                        appearance,
                        &privacy,
                        None,
                    )
                    .expect("floating bar draw");
                let rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: 8.0,
                    top: 8.0,
                    right: 120.0,
                    bottom: 48.0,
                };
                drop(
                    super::build_squircle_path(&renderer.factory, rect, 12.0)
                        .expect("squircle path"),
                );
            }
        };

        exercise();
        let baseline = GuiCounts::now();
        exercise();
        baseline.assert_no_growth("floating notch squircle tabular rim and metaball");
    }

    #[test]
    fn shared_tabular_typography_reuses_instance_and_releases_cleanly() {
        let _serial = SerialResourceTest::begin();
        let first = super::shared_tabular_typography().expect("first typography");
        let second = super::shared_tabular_typography().expect("second typography");
        assert_eq!(first.as_raw(), second.as_raw());
        release_shared_factories();
        assert!(super::TABULAR_TYPOGRAPHY.with(|cell| cell.borrow().is_none()));
        let _rebuilt = super::shared_tabular_typography().expect("rebuilt typography");
    }

    #[test]
    fn squircle_cache_reuses_geometry_and_caps_at_64_entries() {
        let _serial = SerialResourceTest::begin();
        let factory = shared_d2d_factory().expect("d2d factory");
        let rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
            left: 4.0,
            top: 4.0,
            right: 84.0,
            bottom: 44.0,
        };
        let first = super::build_squircle_path(&factory, rect, 12.0).expect("first squircle");
        let second = super::build_squircle_path(&factory, rect, 12.0).expect("cached squircle");
        assert_eq!(first.as_raw(), second.as_raw());

        for i in 0..70 {
            let r = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: i as f32,
                top: 0.0,
                right: i as f32 + 40.0,
                bottom: 40.0,
            };
            let _ = super::build_squircle_path(&factory, r, 10.0).expect("squircle");
        }
        let len = super::SQUIRCLE_CACHE.with(|cell| cell.borrow().len());
        assert_eq!(len, super::SQUIRCLE_CACHE_CAPACITY);

        release_shared_factories();
        let cleared_len = super::SQUIRCLE_CACHE.with(|cell| cell.borrow().len());
        assert_eq!(cleared_len, 0);
    }

    fn draw_test_bar(
        renderer: &Renderer,
        layout: &otto_core::Layout,
        hud: Option<otto_core::NotchHudState>,
    ) -> windows::core::Result<()> {
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        let status = otto_core::SystemStatus::default();
        let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);
        renderer.draw_bar(
            layout, status, "Finder", None, None, hud, &pomodoro, None, 0.0, appearance, &privacy,
            None,
        )
    }

    #[test]
    fn compact_call_activity_draws_for_voice_video_and_muted_calls() {
        let _serial = SerialResourceTest::begin();
        let metrics = otto_core::Metrics::default();
        let layout = otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            96,
            "Finder",
            otto_core::NotchLayoutInput {
                compact_expansion: 1.0,
                compact_primary: Some(otto_core::CompactSlotKind::Call),
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let renderer = Renderer::new(
            layout.bar.width,
            layout.bar.height,
            layout.panel_window.width,
            layout.panel_window.height,
            96,
            0.25,
        )
        .expect("renderer");
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        let status = otto_core::SystemStatus::default();
        let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);
        for (video, muted) in [(false, false), (true, false), (false, true)] {
            let scene = otto_core::CallScene {
                video,
                muted,
                elapsed: String::from("12:34"),
            };
            renderer
                .draw_bar(
                    &layout,
                    status,
                    "Finder",
                    None,
                    None,
                    None,
                    &pomodoro,
                    None,
                    0.4,
                    appearance,
                    &privacy,
                    Some(&scene),
                )
                .expect("call compact draw");
        }
    }

    #[test]
    fn bar_frame_key_memoizes_when_panel_open_and_skips_present() {
        let _serial = SerialResourceTest::begin();
        let metrics = otto_core::Metrics::default();
        let open_layout = otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            96,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let mut renderer = Renderer::new(
            open_layout.bar.width,
            open_layout.bar.height,
            open_layout.panel_window.width,
            open_layout.panel_window.height,
            96,
            0.25,
        )
        .expect("renderer");
        let pomodoro = otto_core::PomodoroScene::steady(
            otto_core::PomodoroState::new(),
            otto_core::ClockTime::default(),
        );
        let privacy = otto_core::PrivacyState::default();
        let status = otto_core::SystemStatus::default();
        let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);

        assert!(renderer.last_bar_frame.borrow().is_none());
        draw_test_bar(&renderer, &open_layout, None).expect("open draw_bar");
        assert!(renderer.last_bar_frame.borrow().is_some());

        let panel_hover_fields = renderer
            .bar_frame_fields(
                &open_layout,
                status,
                Some(otto_core::HitTarget::PomodoroPlayPause),
                Some(otto_core::HitTarget::PomodoroReset),
                None,
                appearance,
            )
            .expect("panel hover key");
        assert!(
            renderer
                .last_bar_frame
                .borrow()
                .as_ref()
                .is_some_and(|cached| cached.matches(&panel_hover_fields, "Finder"))
        );

        renderer.last_bar_target.set(Some((0, (0, 0))));
        renderer
            .present(
                0,
                (0, 0),
                &open_layout,
                status,
                "Finder",
                Some(otto_core::HitTarget::PomodoroPlayPause),
                Some(otto_core::HitTarget::PomodoroReset),
                None,
                &pomodoro,
                None,
                0.5,
                appearance,
                &privacy,
                None,
            )
            .expect("memoized present skips UpdateLayeredWindow");

        let bar_hover_fields = renderer
            .bar_frame_fields(
                &open_layout,
                status,
                Some(otto_core::HitTarget::Start),
                None,
                None,
                appearance,
            )
            .expect("bar hover key");
        assert!(
            !renderer
                .last_bar_frame
                .borrow()
                .as_ref()
                .is_some_and(|cached| cached.matches(&bar_hover_fields, "Finder"))
        );
        assert!(
            !renderer
                .last_bar_frame
                .borrow()
                .as_ref()
                .is_some_and(|cached| cached.matches(&panel_hover_fields, "Safari"))
        );
    }

    #[test]
    fn bar_frame_key_invalidates_on_backdrop_brand_recreate_closed_and_hud() {
        let _serial = SerialResourceTest::begin();
        let metrics = otto_core::Metrics::default();
        let open_layout = otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            96,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let closed_layout = otto_core::compute_layout(&metrics, 1920, 96);
        let mut renderer = Renderer::new(
            open_layout.bar.width,
            open_layout.bar.height,
            open_layout.panel_window.width,
            open_layout.panel_window.height,
            96,
            0.25,
        )
        .expect("renderer");
        let draw_open = |r: &Renderer| {
            draw_test_bar(r, &open_layout, None).expect("draw open bar");
            assert!(r.last_bar_frame.borrow().is_some());
        };

        draw_open(&renderer);
        renderer.set_brand_icon(otto_core::BrandIcon::ClassicApple);
        assert!(renderer.last_bar_frame.borrow().is_none());

        draw_open(&renderer);
        renderer.clear_bar_backdrop();
        assert!(renderer.last_bar_frame.borrow().is_none());

        draw_open(&renderer);
        renderer.set_bar_backdrop(otto_core::RgbBuffer::filled(2, 2, [0.1, 0.2, 0.3]), (0, 0));
        assert!(renderer.last_bar_frame.borrow().is_none());

        draw_open(&renderer);
        renderer.recreate_target().expect("recreate_target");
        assert!(renderer.last_bar_frame.borrow().is_none());

        draw_open(&renderer);
        draw_test_bar(&renderer, &closed_layout, None).expect("closed bar resets key");
        assert!(renderer.last_bar_frame.borrow().is_none());

        draw_open(&renderer);
        let active_hud = otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::Volume {
                percent: 50,
                muted: false,
            },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 0.5,
        };
        draw_test_bar(&renderer, &open_layout, Some(active_hud)).expect("active hud resets key");
        assert!(renderer.last_bar_frame.borrow().is_none());
    }
}
