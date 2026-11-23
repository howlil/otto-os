mod appearance;
mod bar;
mod control_center;
mod dock;
mod focus_shield;
mod notch;
mod popup;
mod settings;
mod spotlight;
mod window;

mod kit;
#[cfg(test)]
mod resource_tests;

use std::cell::{Cell, RefCell};
use std::f32::consts::PI;

pub use appearance::glass_gpu::release_shared_gpu;
pub use appearance::theme::{BrushPalette, LiquidGlassPalette};
pub use control_center::{ControlCenterPaintState, ControlCenterRenderer};
pub use dock::renderer::{DockRenderItem, DockRenderer};
pub use dock::stack::DownloadsStackRenderer;
pub use focus_shield::FocusShieldRenderer;
pub(crate) use kit::glyphs::{draw_bell_glyph, draw_speaker_glyph, draw_wifi_glyph};
pub use popup::menu::{MenuHover, PopupMenuRenderer};
pub use settings::{SettingsPaintState, SettingsRenderer};
pub use spotlight::{SpotlightPaintEntry, SpotlightPaintState, SpotlightRenderer};
pub use window::controls::WindowControlsRenderer;
pub use window::genie::{GenieRenderer, build_fallback_card_snapshot};

use appearance::surface::{
    DibSurface, create_dc_render_target, draw_with_recovery, upload_bgra_bitmap,
};
use otto_core::{
    Appearance, BrandIcon, ClipboardState, FocusStatsView, HitTarget, Layout, LyricsView,
    MediaScene, NotchHudState, PixelLimits, PomodoroScene, PrivacyState, Rect, ShelfState,
    SystemStatus, TasksState, bgra_layout, cover_accent, scale,
};
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_ROUND,
    D2D1_DASH_STYLE_DASH, D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_LINE_JOIN_ROUND,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE, D2D1CreateFactory, ID2D1Bitmap, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1GeometrySink, ID2D1PathGeometry, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_FEATURE, DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
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
use windows_numerics::Vector2;

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

pub(crate) struct CachedCoverArt {
    pub(crate) hash: u64,
    pub(crate) bytes_len: usize,
    pub(crate) art: CoverArt,
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
    text_notif_body: IDWriteTextFormat,
    text_notif_caller: IDWriteTextFormat,
    text_notif_badge: IDWriteTextFormat,
    text_notif_badge_sm: IDWriteTextFormat,
    text_lyric: IDWriteTextFormat,
    text_stats_hero: IDWriteTextFormat,
    text_stats_value: IDWriteTextFormat,
    text_stats_trailing: IDWriteTextFormat,
    text_stats_badge: IDWriteTextFormat,
    lyric_layouts: RefCell<notch::lyrics::LyricLayoutCache>,
    content_offset: Cell<(f32, f32)>,
    bar_surface: DibSurface,
    panel_surface: RefCell<Option<DibSurface>>,
    panel_surface_size: (i32, i32),
    dpi: u32,
    brushes: BrushPalette,
    cached_bar_notch: RefCell<Option<CachedNotchGeometry>>,
    cached_panel_notch: RefCell<Option<CachedNotchGeometry>>,
    cached_cover_art: RefCell<Option<CachedCoverArt>>,
    cached_notif_icons: RefCell<Vec<CachedCoverArt>>,
    cached_shelf_bitmaps: RefCell<Vec<notch::shelf::CachedShelfBitmap>>,
    pub(crate) cached_stats_icons: RefCell<Vec<(String, ID2D1Bitmap)>>,
    last_bar_frame: RefCell<Option<bar::frame::BarFrameKey>>,
    last_bar_target: Cell<Option<(isize, (i32, i32))>>,
    glass: appearance::glass::GlassPainter,
    brand_icon: Cell<BrandIcon>,
    bar_background: Cell<bool>,
    bar_chrome_hidden: Cell<bool>,
    notif_unread: Cell<bool>,
    reduced_motion: Cell<bool>,
    notif_coalesce: Cell<f32>,
}

const SQUIRCLE_CACHE_CAPACITY: usize = 64;
const MAX_STATS_APP_ICONS: usize = 24;

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
    static APPLE_FONT_FAMILY: Cell<Option<PCWSTR>> = const { Cell::new(None) };
    static SQUIRCLE_CACHE: RefCell<Vec<(SquircleCacheKey, ID2D1PathGeometry)>> = const { RefCell::new(Vec::new()) };
}

pub fn release_shared_factories() {
    appearance::glass_gpu::release_shared_gpu();
    let _ = SQUIRCLE_CACHE.try_with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            guard.clear();
        }
    });
    let _ = TABULAR_TYPOGRAPHY.try_with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            guard.take();
        }
    });
    let _ = APPLE_FONT_FAMILY.try_with(|cell| cell.set(None));
    let _ = D2D_FACTORY.try_with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            guard.take();
        }
    });
    let _ = DWRITE_FACTORY.try_with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            guard.take();
        }
    });
}

pub(crate) fn shared_d2d_factory() -> Result<ID2D1Factory> {
    D2D_FACTORY.with(|cell| {
        let mut guard = cell
            .try_borrow_mut()
            .map_err(|_| windows::core::Error::from_hresult(E_FAIL))?;
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
        let mut guard = cell
            .try_borrow_mut()
            .map_err(|_| windows::core::Error::from_hresult(E_FAIL))?;
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
        let mut guard = cell
            .try_borrow_mut()
            .map_err(|_| windows::core::Error::from_hresult(E_FAIL))?;
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

const RAW_BGRA_ICON_MAGIC: &[u8; 4] = b"BGRA";
const RAW_BGRA_HEADER_LEN: usize = 12;

fn decode_raw_bgra_bytes(target: &ID2D1DCRenderTarget, bytes: &[u8]) -> Option<Result<CoverArt>> {
    if bytes.len() < RAW_BGRA_HEADER_LEN || &bytes[..4] != RAW_BGRA_ICON_MAGIC {
        return None;
    }
    let width = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let height = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let pixels = &bytes[RAW_BGRA_HEADER_LEN..];
    Some(
        upload_bgra_bitmap(target, width, height, pixels, PixelLimits::ICON).map(|bitmap| {
            let accent = cover_accent(pixels, width, height).map(|colour| D2D1_COLOR_F {
                r: colour.r,
                g: colour.g,
                b: colour.b,
                a: 1.0,
            });
            CoverArt { bitmap, accent }
        }),
    )
}

pub(crate) fn decode_image_bytes(target: &ID2D1DCRenderTarget, bytes: &[u8]) -> Result<CoverArt> {
    if let Some(decoded) = decode_raw_bgra_bytes(target, bytes) {
        return decoded;
    }
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
            let text_notif_body = create_text_format(
                &dwrite,
                scale(12, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            text_notif_body.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            text_notif_body.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            let text_notif_caller = create_text_format(
                &dwrite,
                scale(15, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_notif_badge = create_text_format(
                &dwrite,
                scale(18, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_notif_badge_sm = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let text_lyric = create_text_format(
                &dwrite,
                scale(notch::lyrics::LYRIC_FONT_PT, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_stats_hero = create_text_format(
                &dwrite,
                scale(22, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_stats_value = create_text_format(
                &dwrite,
                scale(12, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_stats_trailing = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;
            let text_stats_badge = create_text_format(
                &dwrite,
                scale(9, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;

            let panel_surface_size = (panel_width.max(1), panel_height.max(1));
            bgra_layout(
                panel_surface_size.0 as u32,
                panel_surface_size.1 as u32,
                PixelLimits::BACKDROP,
            )
            .map_err(|_| windows::core::Error::from_hresult(E_INVALIDARG))?;
            let bar_surface = DibSurface::new(width, height)?;
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
                text_notif_body,
                text_notif_caller,
                text_notif_badge,
                text_notif_badge_sm,
                text_lyric,
                text_stats_hero,
                text_stats_value,
                text_stats_trailing,
                text_stats_badge,
                lyric_layouts: RefCell::default(),
                content_offset: Cell::new((0.0, 0.0)),
                bar_surface,
                panel_surface: RefCell::new(None),
                panel_surface_size,
                dpi,
                brushes,
                cached_bar_notch,
                cached_panel_notch,
                cached_cover_art: RefCell::new(None),
                cached_notif_icons: RefCell::default(),
                cached_shelf_bitmaps: RefCell::default(),
                cached_stats_icons: RefCell::default(),
                last_bar_frame: RefCell::new(None),
                last_bar_target: Cell::new(None),
                glass: appearance::glass::GlassPainter::default(),
                brand_icon: Cell::new(BrandIcon::Otto),
                bar_background: Cell::new(true),
                bar_chrome_hidden: Cell::new(false),
                notif_unread: Cell::new(false),
                reduced_motion: Cell::new(false),
                notif_coalesce: Cell::new(1.0),
            })
        }
    }

    pub fn set_bar_background(&self, shown: bool) {
        if self.bar_background.replace(shown) != shown
            && let Ok(mut frame) = self.last_bar_frame.try_borrow_mut()
        {
            frame.take();
        }
    }

    pub fn set_bar_chrome_hidden(&self, hidden: bool) {
        if self.bar_chrome_hidden.replace(hidden) != hidden
            && let Ok(mut frame) = self.last_bar_frame.try_borrow_mut()
        {
            frame.take();
        }
    }

    pub fn set_brand_icon(&self, brand_icon: BrandIcon) {
        self.brand_icon.set(brand_icon);
        if let Ok(mut frame) = self.last_bar_frame.try_borrow_mut() {
            frame.take();
        }
    }

    pub fn set_notification_unread(&self, unread: bool) {
        if self.notif_unread.replace(unread) != unread
            && let Ok(mut frame) = self.last_bar_frame.try_borrow_mut()
        {
            frame.take();
        }
    }

    pub fn set_reduced_motion(&self, reduced: bool) {
        self.reduced_motion.set(reduced);
    }

    pub fn set_notification_coalesce(&self, blend: f32) {
        self.notif_coalesce.set(blend.clamp(0.0, 1.0));
    }

    fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.glass.invalidate_gpu();
        if let Ok(mut cover) = self.cached_cover_art.try_borrow_mut() {
            cover.take();
        }
        if let Ok(mut icons) = self.cached_notif_icons.try_borrow_mut() {
            icons.clear();
        }
        if let Ok(mut shelf) = self.cached_shelf_bitmaps.try_borrow_mut() {
            shelf.clear();
        }
        if let Ok(mut stats_icons) = self.cached_stats_icons.try_borrow_mut() {
            stats_icons.clear();
        }
        if let Ok(mut frame) = self.last_bar_frame.try_borrow_mut() {
            frame.take();
        }
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
        notification: Option<&otto_core::NotificationPresentation>,
    ) -> Result<()> {
        if let Some(fields) =
            self.bar_frame_fields(layout, status, hovered, focused, hud, appearance)
            && self.last_bar_target.get() == Some((hwnd, origin))
            && self.last_bar_frame.try_borrow().ok().is_some_and(|guard| {
                guard
                    .as_ref()
                    .is_some_and(|cached| cached.matches(&fields, app_name))
            })
        {
            return Ok(());
        }
        draw_with_recovery(
            self,
            |renderer| {
                renderer.draw_bar(
                    layout,
                    status,
                    app_name,
                    hovered,
                    focused,
                    hud,
                    pomodoro,
                    media,
                    anim_phase,
                    appearance,
                    privacy,
                    call,
                    notification,
                )
            },
            Self::recreate_target,
        )?;
        if let Err(err) = self.bar_surface.present(hwnd, origin) {
            if let Ok(mut frame) = self.last_bar_frame.try_borrow_mut() {
                frame.take();
            }
            self.last_bar_target.set(None);
            return Err(err);
        }
        self.last_bar_target.set(Some((hwnd, origin)));
        Ok(())
    }

    pub fn set_bar_backdrop(&self, capture: otto_core::RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
        if let Ok(mut frame) = self.last_bar_frame.try_borrow_mut() {
            frame.take();
        }
    }

    pub fn clear_bar_backdrop(&self) {
        self.glass.clear_backdrop();
        if let Ok(mut frame) = self.last_bar_frame.try_borrow_mut() {
            frame.take();
        }
    }

    #[must_use]
    pub fn take_bar_backdrop(&self) -> Option<(otto_core::RgbBuffer, (i32, i32))> {
        let taken = self.glass.take_backdrop();
        if taken.is_some()
            && let Ok(mut frame) = self.last_bar_frame.try_borrow_mut()
        {
            frame.take();
        }
        taken
    }

    #[must_use]
    pub fn has_bar_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    pub(crate) fn ensure_panel_surface(&self) -> Result<()> {
        let mut slot = self
            .panel_surface
            .try_borrow_mut()
            .map_err(|_| windows::core::Error::from_hresult(E_FAIL))?;
        if slot.is_none() {
            let (width, height) = self.panel_surface_size;
            *slot = Some(DibSurface::new(width, height)?);
        }
        Ok(())
    }

    pub fn release_panel_surface(&self) {
        if let Ok(mut slot) = self.panel_surface.try_borrow_mut() {
            if slot.is_some() {
                let _ = self.bar_surface.bind(&self.target, self.bar_surface.size());
            }
            slot.take();
        }
        if let Ok(mut cached) = self.cached_panel_notch.try_borrow_mut() {
            cached.take();
        }
    }

    pub fn clear_media_caches(&self) {
        if let Ok(mut cover) = self.cached_cover_art.try_borrow_mut() {
            cover.take();
        }
        if let Ok(mut layouts) = self.lyric_layouts.try_borrow_mut() {
            layouts.clear();
        }
    }

    pub fn clear_shelf_caches(&self) {
        if let Ok(mut shelf) = self.cached_shelf_bitmaps.try_borrow_mut() {
            shelf.clear();
        }
    }

    pub fn release_notch_resources(&self) {
        self.release_panel_surface();
        self.clear_media_caches();
        self.clear_shelf_caches();
        if let Ok(mut icons) = self.cached_notif_icons.try_borrow_mut() {
            icons.clear();
        }
        if let Ok(mut stats_icons) = self.cached_stats_icons.try_borrow_mut() {
            stats_icons.clear();
        }
        if let Ok(mut cached) = self.cached_bar_notch.try_borrow_mut() {
            cached.take();
        }
    }

    #[must_use]
    pub fn has_stats_app_icon(&self, app_key: &str) -> bool {
        self.cached_stats_icons
            .try_borrow()
            .ok()
            .is_some_and(|cache| cache.iter().any(|(k, _)| k == app_key))
    }

    pub fn set_stats_app_icon(
        &self,
        app_key: String,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<()> {
        let bitmap = upload_bgra_bitmap(&self.target, width, height, pixels, PixelLimits::ICON)?;
        if let Ok(mut cache) = self.cached_stats_icons.try_borrow_mut() {
            if let Some(pos) = cache.iter().position(|(k, _)| k == &app_key) {
                cache.remove(pos);
            } else if cache.len() >= MAX_STATS_APP_ICONS {
                cache.remove(0);
            }
            cache.push((app_key, bitmap));
        }
        Ok(())
    }

    pub(crate) fn stats_app_icon(&self, app_key: &str) -> Option<ID2D1Bitmap> {
        self.cached_stats_icons
            .try_borrow()
            .ok()?
            .iter()
            .find(|(k, _)| k == app_key)
            .map(|(_, bmp)| bmp.clone())
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
        focus_stats: &FocusStatsView,
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
                    focus_stats,
                )
            },
            Self::recreate_target,
        )?;
        let guard = self
            .panel_surface
            .try_borrow()
            .map_err(|_| windows::core::Error::from_hresult(E_FAIL))?;
        let surface = guard
            .as_ref()
            .ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))?;
        surface.present_sized(
            panel_hwnd,
            (layout.panel_window.x, layout.panel_window.y),
            (layout.panel_window.width, layout.panel_window.height),
        )
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
        let mut cache = cell.try_borrow_mut().ok()?;
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
        if let Ok(mut cache) = cell.try_borrow_mut() {
            if cache.len() >= SQUIRCLE_CACHE_CAPACITY {
                cache.remove(0);
            }
            cache.push((key, geometry.clone()));
        }
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
    if let Some(cached) = APPLE_FONT_FAMILY.with(Cell::get) {
        return cached;
    }
    let candidates = [w!("SF Pro Text"), w!("SF Pro Display"), w!("SF Pro")];
    let mut resolved = w!("Segoe UI Variable Display");
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
                    resolved = family;
                    break;
                }
            }
        }
    }
    APPLE_FONT_FAMILY.with(|cell| cell.set(Some(resolved)));
    resolved
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
