mod bar;
mod dock;
mod genie;
mod glass;
mod menu;
mod surface;
mod theme;
mod window_controls;

use std::cell::RefCell;
use std::f32::consts::{PI, TAU};

pub use dock::{DockRenderItem, DockRenderer};
pub use genie::GenieRenderer;
pub use menu::PopupMenuRenderer;
pub use theme::{BrushPalette, LiquidGlassPalette};
pub use window_controls::{TrafficLightRenderer, WindowControlsRenderer};

use otto_core::{
    Appearance, HitTarget, Layout, LyricsView, MediaCapabilities, MediaCommand, NotchExpandedTab,
    NotchHudKind, NotchHudState, NowPlaying, PomodoroPhase, PomodoroState, PomodoroStatus, Rect,
    ShapeTokens, SystemStatus, scale, unsupported_reason,
};
use surface::DibSurface;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F,
    D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
    D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_LINE_JOIN_ROUND,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE, D2D1CreateFactory, ID2D1Bitmap, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1PathGeometry, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
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
const FOCUS_ORANGE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.624,
    b: 0.039,
    a: 1.0,
};
const BREAK_GREEN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.188,
    g: 0.820,
    b: 0.345,
    a: 1.0,
};
const LONG_BREAK_BLUE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.039,
    g: 0.518,
    b: 1.0,
    a: 1.0,
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
const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

const fn rgb(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

const fn phase_accent_color(phase: PomodoroPhase) -> D2D1_COLOR_F {
    match phase {
        PomodoroPhase::Focus => FOCUS_ORANGE,
        PomodoroPhase::ShortBreak => BREAK_GREEN,
        PomodoroPhase::LongBreak => LONG_BREAK_BLUE,
    }
}

struct CachedNotchGeometry {
    rect: Rect,
    corner_radius_bits: u32,
    ear_radius_bits: u32,
    geometry: ID2D1PathGeometry,
}

struct CachedCoverArt {
    bytes_ptr: usize,
    bytes_len: usize,
    bitmap: ID2D1Bitmap,
}

pub struct Renderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    text_leading: IDWriteTextFormat,
    text_trailing: IDWriteTextFormat,
    text_app_bold: IDWriteTextFormat,
    text_menu: IDWriteTextFormat,
    text_timer_display: IDWriteTextFormat,
    text_badge: IDWriteTextFormat,
    text_caption: IDWriteTextFormat,
    text_alert_title: IDWriteTextFormat,
    bar_surface: DibSurface,
    panel_surface: DibSurface,
    dpi: u32,
    brushes: BrushPalette,
    cached_bar_notch: RefCell<Option<CachedNotchGeometry>>,
    cached_panel_notch: RefCell<Option<CachedNotchGeometry>>,
    cached_cover_art: RefCell<Option<CachedCoverArt>>,
    glass: glass::GlassPainter,
}

thread_local! {
    static D2D_FACTORY: RefCell<Option<ID2D1Factory>> = const { RefCell::new(None) };
    static DWRITE_FACTORY: RefCell<Option<IDWriteFactory>> = const { RefCell::new(None) };
}

pub fn release_shared_factories() {
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

fn decode_image_bytes(target: &ID2D1DCRenderTarget, bytes: &[u8]) -> Result<ID2D1Bitmap> {
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
        target.CreateBitmapFromWicBitmap(&converter, None)
    }
}

impl Renderer {
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
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: PIXEL_FORMAT,
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let target = factory.CreateDCRenderTarget(&properties)?;

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

            let dwrite = shared_dwrite_factory()?;
            let font_px = scale(12, dpi) as f32;
            let text_leading = create_text_format(
                &dwrite,
                font_px,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_trailing = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
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
                scale(27, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_badge = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_caption = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let text_alert_title = create_text_format(
                &dwrite,
                scale(14, dpi) as f32,
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
                text_leading,
                text_trailing,
                text_app_bold,
                text_menu,
                text_timer_display,
                text_badge,
                text_caption,
                text_alert_title,
                bar_surface,
                panel_surface,
                dpi,
                brushes,
                cached_bar_notch,
                cached_panel_notch,
                cached_cover_art: RefCell::new(None),
                glass: glass::GlassPainter::default(),
            })
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &self,
        hwnd: isize,
        origin: (i32, i32),
        layout: &Layout,
        status: SystemStatus,
        app_name: &str,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        hud: Option<NotchHudState>,
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
        appearance: Appearance,
    ) -> Result<()> {
        self.draw_bar(
            layout, status, app_name, hovered, focused, hud, pomodoro, media, anim_phase,
            appearance,
        )?;
        self.bar_surface.present(hwnd, origin)
    }

    pub fn set_bar_backdrop(&self, capture: otto_core::RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn clear_bar_backdrop(&self) {
        self.glass.clear_backdrop();
    }

    #[must_use]
    pub fn has_bar_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present_panel(
        &self,
        panel_hwnd: isize,
        layout: &Layout,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
    ) -> Result<()> {
        self.draw_panel(
            layout, hovered, focused, pomodoro, media, anim_phase, lyrics,
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
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
    ) -> Result<()> {
        let pomo_active = pomodoro.is_compact_active();
        let media_item = media.filter(|m| m.is_active());
        match (pomo_active, media_item) {
            (true, Some(m)) => self.draw_dual_compact(layout, pomodoro, m, anim_phase),
            (false, Some(m)) => {
                self.draw_compact_media(layout, m, anim_phase);
                Ok(())
            }
            (true, None) => self.draw_compact_pomodoro(layout, pomodoro),
            (false, None) => Ok(()),
        }
    }

    fn draw_compact_media(&self, layout: &Layout, media: &NowPlaying, anim_phase: f32) {
        let alpha = ((layout.compact_expansion - 0.15) / 0.85).clamp(0.0, 1.0);
        let n = layout.notch;
        let cy = layout.bar.height as f32 * 0.5;
        let note_cx = n.x as f32 + scale(18, self.dpi) as f32;
        let note_r = scale(6, self.dpi) as f32;

        let accent = &self.brushes.alpha_brush(MUSIC_ACCENT, 0.96 * alpha);
        let fg = &self.brushes.alpha_brush(FOREGROUND, 0.92 * alpha);
        let eq_brush = &self.brushes.alpha_brush(EQUALIZER_GREEN, 0.96 * alpha);

        self.draw_music_note_glyph(note_cx, cy, note_r, accent);

        let eq_cx = n.right() as f32 - scale(20, self.dpi) as f32;
        self.draw_audio_equalizer(eq_cx, cy, anim_phase, eq_brush);

        let text_rect = D2D_RECT_F {
            left: note_cx + scale(12, self.dpi) as f32,
            top: 0.0,
            right: eq_cx - scale(12, self.dpi) as f32,
            bottom: layout.bar.height as f32,
        };
        self.draw_text(&media.title, &self.text_leading, text_rect, fg);
    }

    fn draw_dual_compact(
        &self,
        layout: &Layout,
        pomodoro: PomodoroState,
        _media: &NowPlaying,
        anim_phase: f32,
    ) -> Result<()> {
        let alpha = ((layout.compact_expansion - 0.15) / 0.85).clamp(0.0, 1.0);
        let n = layout.notch;
        let cy = layout.bar.height as f32 * 0.5;
        let ring_cx = n.x as f32 + scale(18, self.dpi) as f32;
        let ring_r = scale(6, self.dpi) as f32;
        let stroke = (scale(2, self.dpi) as f32).max(1.6);
        let accent_color = phase_accent_color(pomodoro.phase);

        let accent = &self.brushes.alpha_brush(accent_color, 0.96 * alpha);
        let track = &self.brushes.alpha_brush(FOREGROUND, 0.22 * alpha);
        let eq_brush = &self.brushes.alpha_brush(EQUALIZER_GREEN, 0.96 * alpha);

        self.draw_progress_ring(
            ring_cx,
            cy,
            ring_r,
            stroke,
            pomodoro.progress(),
            accent,
            track,
        )?;

        let text = pomodoro.format_time();
        let text_rect = D2D_RECT_F {
            left: ring_cx + scale(10, self.dpi) as f32,
            top: 0.0,
            right: n.right() as f32 - scale(36, self.dpi) as f32,
            bottom: layout.bar.height as f32,
        };
        self.draw_text(&text, &self.text_leading, text_rect, accent);

        let eq_cx = n.right() as f32 - scale(18, self.dpi) as f32;
        self.draw_audio_equalizer(eq_cx, cy, anim_phase, eq_brush);
        Ok(())
    }

    fn draw_compact_pomodoro(&self, layout: &Layout, pomodoro: PomodoroState) -> Result<()> {
        let alpha = ((layout.compact_expansion - 0.15) / 0.85).clamp(0.0, 1.0);
        let n = layout.notch;
        let cy = layout.bar.height as f32 * 0.5;
        let ring_cx = n.x as f32 + scale(18, self.dpi) as f32;
        let ring_r = scale(6, self.dpi) as f32;
        let stroke = (scale(2, self.dpi) as f32).max(1.6);
        let accent_color = phase_accent_color(pomodoro.phase);

        unsafe {
            let accent = &self.brushes.alpha_brush(accent_color, 0.96 * alpha);
            let track = &self.brushes.alpha_brush(FOREGROUND, 0.22 * alpha);

            self.draw_progress_ring(
                ring_cx,
                cy,
                ring_r,
                stroke,
                pomodoro.progress(),
                accent,
                track,
            )?;

            if pomodoro.status == PomodoroStatus::Paused {
                let dot = D2D1_ELLIPSE {
                    point: Vector2::new(ring_cx, cy),
                    radiusX: stroke * 0.65,
                    radiusY: stroke * 0.65,
                };
                self.target.FillEllipse(&dot, track);
            } else {
                let dot = D2D1_ELLIPSE {
                    point: Vector2::new(ring_cx, cy),
                    radiusX: stroke * 0.75,
                    radiusY: stroke * 0.75,
                };
                self.target.FillEllipse(&dot, accent);
            }

            let text = pomodoro.format_time();
            let text_rect = D2D_RECT_F {
                left: n.right() as f32 - scale(54, self.dpi) as f32,
                top: 0.0,
                right: n.right() as f32 - scale(11, self.dpi) as f32,
                bottom: layout.bar.height as f32,
            };
            self.draw_text(&text, &self.text_trailing, text_rect, accent);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_expanded_tabs(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        active_tab: NotchExpandedTab,
        has_media: bool,
        pomodoro: PomodoroState,
    ) {
        let pomo_rect = translate_rect(layout.notch_tab_pomodoro, -offset_x);
        let media_rect = translate_rect(layout.notch_tab_media, -offset_x);

        unsafe {
            let pomo_active = active_tab == NotchExpandedTab::Pomodoro;
            let pomo_hover = hovered == Some(HitTarget::NotchTabPomodoro);
            let pomo_alpha = if pomo_active {
                0.22 * alpha
            } else if pomo_hover {
                0.14 * alpha
            } else {
                0.06 * alpha
            };
            let pomo_brush = &self.brushes.alpha_brush(FOREGROUND, pomo_alpha);
            self.draw_rounded_box(pomo_rect, pomo_rect.height as f32 * 0.5, pomo_brush);

            let accent_color = phase_accent_color(pomodoro.phase);
            let dot_x = pomo_rect.x as f32 + scale(10, self.dpi) as f32;
            let dot_mid_y = pomo_rect.y as f32 + pomo_rect.height as f32 * 0.5;
            let dot_r = scale(3, self.dpi) as f32;
            let accent = &self.brushes.alpha_brush(accent_color, alpha);
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(dot_x, dot_mid_y),
                    radiusX: dot_r,
                    radiusY: dot_r,
                },
                accent,
            );

            let pomo_text_brush = &self.brushes.alpha_brush(
                FOREGROUND,
                if pomo_active {
                    0.96 * alpha
                } else {
                    0.55 * alpha
                },
            );
            let pomo_label_rect = D2D_RECT_F {
                left: dot_x + scale(7, self.dpi) as f32,
                top: pomo_rect.y as f32,
                right: pomo_rect.right() as f32 - scale(6, self.dpi) as f32,
                bottom: pomo_rect.bottom() as f32,
            };
            self.draw_text(
                pomodoro.phase.badge_label(),
                &self.text_badge,
                pomo_label_rect,
                pomo_text_brush,
            );

            if has_media {
                let media_active = active_tab == NotchExpandedTab::Media;
                let media_hover = hovered == Some(HitTarget::NotchTabMedia);
                let media_alpha = if media_active {
                    0.22 * alpha
                } else if media_hover {
                    0.14 * alpha
                } else {
                    0.06 * alpha
                };
                let media_brush = &self.brushes.alpha_brush(FOREGROUND, media_alpha);
                self.draw_rounded_box(media_rect, media_rect.height as f32 * 0.5, media_brush);

                let m_dot_x = media_rect.x as f32 + scale(10, self.dpi) as f32;
                let m_dot_mid_y = media_rect.y as f32 + media_rect.height as f32 * 0.5;
                let m_accent = &self.brushes.alpha_brush(MUSIC_ACCENT, alpha);
                self.draw_music_note_glyph(
                    m_dot_x,
                    m_dot_mid_y,
                    scale(4, self.dpi) as f32,
                    m_accent,
                );

                let media_text_brush = &self.brushes.alpha_brush(
                    FOREGROUND,
                    if media_active {
                        0.96 * alpha
                    } else {
                        0.55 * alpha
                    },
                );
                let media_label_rect = D2D_RECT_F {
                    left: m_dot_x + scale(7, self.dpi) as f32,
                    top: media_rect.y as f32,
                    right: media_rect.right() as f32 - scale(6, self.dpi) as f32,
                    bottom: media_rect.bottom() as f32,
                };
                self.draw_text(
                    "NOW PLAYING",
                    &self.text_badge,
                    media_label_rect,
                    media_text_brush,
                );
            }
        }
    }

    fn get_or_decode_cover_art(&self, thumbnail: &[u8]) -> Option<ID2D1Bitmap> {
        let ptr = thumbnail.as_ptr() as usize;
        let len = thumbnail.len();
        if let Some(cached) = self.cached_cover_art.borrow().as_ref()
            && cached.bytes_ptr == ptr
            && cached.bytes_len == len
        {
            return Some(cached.bitmap.clone());
        }
        let bitmap = decode_image_bytes(&self.target, thumbnail).ok()?;
        *self.cached_cover_art.borrow_mut() = Some(CachedCoverArt {
            bytes_ptr: ptr,
            bytes_len: len,
            bitmap: bitmap.clone(),
        });
        Some(bitmap)
    }

    #[allow(
        clippy::similar_names,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    fn draw_expanded_media(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        media: &NowPlaying,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
    ) -> Result<()> {
        let art_rect = translate_rect(layout.media_cover_art, -offset_x);
        let seekbar_rect = translate_rect(layout.media_seekbar, -offset_x);

        let fg = &self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let muted = &self.brushes.alpha_brush(FOREGROUND, 0.55 * alpha);
        let tile_bg = &self.brushes.alpha_brush(MUSIC_ACCENT, 0.18 * alpha);
        let tile_icon = &self.brushes.alpha_brush(MUSIC_ACCENT, 0.96 * alpha);
        let eq_brush = &self.brushes.alpha_brush(EQUALIZER_GREEN, 0.96 * alpha);

        let mut drawn_art = false;
        if let Some(thumb) = media.thumbnail.as_ref()
            && let Some(bitmap) = self.get_or_decode_cover_art(thumb)
        {
            let dest = D2D_RECT_F {
                left: art_rect.x as f32,
                top: art_rect.y as f32,
                right: art_rect.right() as f32,
                bottom: art_rect.bottom() as f32,
            };
            let squircle = D2D1_ROUNDED_RECT {
                rect: dest,
                radiusX: scale(10, self.dpi) as f32,
                radiusY: scale(10, self.dpi) as f32,
            };
            unsafe {
                self.target.DrawBitmap(
                    &bitmap,
                    Some(&raw const dest),
                    alpha,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
                let rim_brush = &self.brushes.alpha_brush(FOREGROUND, 0.15 * alpha);
                self.target
                    .DrawRoundedRectangle(&squircle, rim_brush, 1.0, None);
            }
            drawn_art = true;
        }

        if !drawn_art {
            self.draw_rounded_box(art_rect, scale(10, self.dpi) as f32, tile_bg);
            self.draw_music_note_glyph(
                art_rect.x as f32 + art_rect.width as f32 * 0.5,
                art_rect.y as f32 + art_rect.height as f32 * 0.5,
                scale(11, self.dpi) as f32,
                tile_icon,
            );
        }

        let local_notch = Rect {
            x: layout.notch.x - offset_x,
            y: 0,
            width: layout.notch.width,
            height: layout.notch.height,
        };
        let eq_cx = local_notch.right() as f32 - scale(20, self.dpi) as f32;
        let eq_cy = art_rect.y as f32 + art_rect.height as f32 * 0.5;
        let eq_phase = if media.playing { anim_phase } else { 0.0 };
        self.draw_audio_equalizer(eq_cx, eq_cy, eq_phase, eq_brush);

        let toggle_rect = translate_rect(layout.media_lyrics_toggle, -offset_x);
        self.draw_lyrics_toggle_button(
            toggle_rect,
            alpha,
            hovered == Some(HitTarget::MediaLyricsToggle),
            layout.lyrics_expanded,
        );

        let text_left = art_rect.right() as f32 + scale(12, self.dpi) as f32;
        let text_right = toggle_rect.x as f32 - scale(6, self.dpi) as f32;

        let title_rect = D2D_RECT_F {
            left: text_left,
            top: art_rect.y as f32 + scale(2, self.dpi) as f32,
            right: text_right,
            bottom: art_rect.y as f32 + scale(24, self.dpi) as f32,
        };
        self.draw_text(&media.title, &self.text_alert_title, title_rect, fg);

        let artist_rect = D2D_RECT_F {
            left: text_left,
            top: art_rect.y as f32 + scale(25, self.dpi) as f32,
            right: text_right,
            bottom: art_rect.bottom() as f32,
        };

        let disabled_reason =
            hovered.and_then(|target| Self::disabled_transport_reason(target, media.capabilities));
        if let Some(reason) = disabled_reason {
            self.draw_text(reason, &self.text_caption, artist_rect, muted);
        } else if let Some(active_lyric) =
            lyrics.sheet.and_then(|l| l.active_line(media.position_ms))
        {
            let lyric_brush = &self.brushes.alpha_brush(MUSIC_ACCENT, 0.95 * alpha);
            self.draw_text(active_lyric, &self.text_caption, artist_rect, lyric_brush);
        } else {
            self.draw_text(
                media.display_artist(),
                &self.text_caption,
                artist_rect,
                muted,
            );
        }

        if layout.lyrics_expanded {
            let drawer_rect = translate_rect(layout.media_lyrics_drawer, -offset_x);
            self.draw_lyrics_drawer(drawer_rect, alpha, media, lyrics, anim_phase);
        }

        self.draw_media_seekbar(
            seekbar_rect,
            alpha,
            hovered == Some(HitTarget::MediaSeekbar),
            media,
            muted,
        );

        self.draw_media_action_buttons(layout, offset_x, alpha, hovered, media, fg)?;
        Ok(())
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

    fn disabled_transport_reason(
        target: HitTarget,
        capabilities: MediaCapabilities,
    ) -> Option<&'static str> {
        match target {
            HitTarget::MediaPlayPause if !capabilities.play_pause => {
                Some(unsupported_reason(MediaCommand::TogglePlayPause))
            }
            HitTarget::MediaPrevious if !capabilities.previous => {
                Some(unsupported_reason(MediaCommand::Previous))
            }
            HitTarget::MediaNext if !capabilities.next => {
                Some(unsupported_reason(MediaCommand::Next))
            }
            _ => None,
        }
    }

    fn draw_lyrics_toggle_button(&self, rect: Rect, alpha: f32, hovered: bool, expanded: bool) {
        let radius = scale(6, self.dpi) as f32;
        let bg_color = if expanded {
            MUSIC_ACCENT
        } else if hovered {
            brighten(FOREGROUND, 0.2)
        } else {
            FOREGROUND
        };
        let bg_alpha = if expanded {
            0.85 * alpha
        } else if hovered {
            0.20 * alpha
        } else {
            0.10 * alpha
        };
        let bg_brush = &self.brushes.alpha_brush(bg_color, bg_alpha);
        self.draw_rounded_box(rect, radius, bg_brush);

        let glyph_color = if expanded || hovered {
            FOREGROUND
        } else {
            rgb(200, 205, 215, 1.0)
        };
        let glyph_brush = &self.brushes.alpha_brush(glyph_color, 0.92 * alpha);
        let glyph_rect = D2D_RECT_F {
            left: rect.x as f32,
            top: rect.y as f32,
            right: rect.right() as f32,
            bottom: rect.bottom() as f32,
        };
        self.draw_text("”", &self.text_caption, glyph_rect, glyph_brush);
    }

    fn draw_lyrics_drawer(
        &self,
        drawer_rect: Rect,
        alpha: f32,
        media: &NowPlaying,
        lyrics: LyricsView<'_>,
        anim_phase: f32,
    ) {
        let radius = scale(12, self.dpi) as f32;
        let bg_brush = &self.brushes.alpha_brush(FOREGROUND, 0.07 * alpha);
        self.draw_rounded_box(drawer_rect, radius, bg_brush);

        let border_brush = &self.brushes.alpha_brush(FOREGROUND, 0.12 * alpha);
        let rounded = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: drawer_rect.x as f32,
                top: drawer_rect.y as f32,
                right: drawer_rect.right() as f32,
                bottom: drawer_rect.bottom() as f32,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&rounded, border_brush, 1.0, None);
        }

        if let Some(sheet) = lyrics.sheet.filter(|s| !s.is_empty()) {
            let (rel_active, lines) = sheet.visible_window(media.position_ms, 3);
            let line_count = lines.len().max(1);
            let line_h =
                (drawer_rect.height as f32 - scale(12, self.dpi) as f32) / line_count as f32;

            for (i, line) in lines.iter().enumerate() {
                let y_top = drawer_rect.y as f32 + scale(6, self.dpi) as f32 + i as f32 * line_h;
                let rect = D2D_RECT_F {
                    left: drawer_rect.x as f32 + scale(14, self.dpi) as f32,
                    top: y_top,
                    right: drawer_rect.right() as f32 - scale(14, self.dpi) as f32,
                    bottom: y_top + line_h,
                };
                if i == rel_active {
                    let pill_pad_x = scale(6, self.dpi) as f32;
                    let pill_pad_y = scale(2, self.dpi) as f32;
                    let pill_rect = Rect {
                        x: (rect.left - pill_pad_x) as i32,
                        y: (rect.top - pill_pad_y) as i32,
                        width: (rect.right - rect.left + pill_pad_x * 2.0) as i32,
                        height: (line_h + pill_pad_y * 2.0) as i32,
                    };
                    let pill_brush = &self.brushes.alpha_brush(FOREGROUND, 0.12 * alpha);
                    self.draw_rounded_box(pill_rect, scale(8, self.dpi) as f32, pill_brush);

                    let active_brush = &self.brushes.alpha_brush(FOREGROUND, 0.98 * alpha);
                    self.draw_text(&line.text, &self.text_alert_title, rect, active_brush);
                } else {
                    let inactive_brush = &self.brushes.alpha_brush(FOREGROUND, 0.35 * alpha);
                    self.draw_text(&line.text, &self.text_caption, rect, inactive_brush);
                }
            }
        } else {
            let empty_rect = D2D_RECT_F {
                left: drawer_rect.x as f32 + scale(12, self.dpi) as f32,
                top: drawer_rect.y as f32 + drawer_rect.height as f32 * 0.38,
                right: drawer_rect.right() as f32 - scale(12, self.dpi) as f32,
                bottom: drawer_rect.bottom() as f32,
            };
            if media.title.trim().is_empty() {
                let empty_brush = &self.brushes.alpha_brush(FOREGROUND, 0.45 * alpha);
                self.draw_text(
                    "Tidak ada media diputar",
                    &self.text_caption,
                    empty_rect,
                    empty_brush,
                );
            } else {
                let level = if lyrics.status.is_pending() {
                    0.35 + 0.30 * (anim_phase * TAU).sin().abs()
                } else {
                    0.45
                };
                let status_brush = &self.brushes.alpha_brush(FOREGROUND, level * alpha);
                self.draw_text(
                    lyrics.status.placeholder(),
                    &self.text_caption,
                    empty_rect,
                    status_brush,
                );
            }
        }
    }

    fn draw_media_seekbar(
        &self,
        seekbar_rect: Rect,
        alpha: f32,
        hovered: bool,
        media: &NowPlaying,
        muted_brush: &ID2D1SolidColorBrush,
    ) {
        let track_h = scale(4, self.dpi) as f32;
        let track_y = seekbar_rect.y as f32 + (seekbar_rect.height as f32 - track_h) * 0.5;
        let track_rect = D2D_RECT_F {
            left: seekbar_rect.x as f32,
            top: track_y,
            right: seekbar_rect.right() as f32,
            bottom: track_y + track_h,
        };

        let ratio = if media.has_known_duration() {
            media.progress_ratio()
        } else {
            0.0
        };
        let seekable = media.is_seekable();
        let fill_w = (seekbar_rect.width as f32 * ratio).max(0.0);
        let fill_rect = D2D_RECT_F {
            left: seekbar_rect.x as f32,
            top: track_y,
            right: (seekbar_rect.x as f32 + fill_w).min(seekbar_rect.right() as f32),
            bottom: track_y + track_h,
        };

        unsafe {
            let track_alpha = if hovered && seekable {
                0.28 * alpha
            } else {
                0.16 * alpha
            };
            let track_brush = &self.brushes.alpha_brush(FOREGROUND, track_alpha);
            let fill_color = if hovered {
                brighten(MUSIC_ACCENT, 0.14)
            } else {
                MUSIC_ACCENT
            };
            let fill_brush = &self.brushes.alpha_brush(fill_color, 0.95 * alpha);

            let track_radius = track_h * 0.5;
            let track_box = Rect {
                x: track_rect.left as i32,
                y: track_rect.top as i32,
                width: (track_rect.right - track_rect.left) as i32,
                height: track_h as i32,
            };
            self.draw_rounded_box(track_box, track_radius, track_brush);

            if fill_w > 1.0 {
                let fill_box = Rect {
                    x: fill_rect.left as i32,
                    y: fill_rect.top as i32,
                    width: (fill_rect.right - fill_rect.left) as i32,
                    height: track_h as i32,
                };
                self.draw_rounded_box(fill_box, track_radius, fill_brush);
            }

            if hovered && seekable && fill_w > 0.0 {
                let thumb_x = (seekbar_rect.x as f32 + fill_w).min(seekbar_rect.right() as f32);
                let thumb_y = track_y + track_h * 0.5;
                let thumb_ellipse = D2D1_ELLIPSE {
                    point: Vector2::new(thumb_x, thumb_y),
                    radiusX: scale(4, self.dpi) as f32,
                    radiusY: scale(4, self.dpi) as f32,
                };
                self.target.FillEllipse(&thumb_ellipse, fill_brush);
            }

            let label_y = track_y + track_h + scale(2, self.dpi) as f32;
            let label_h = scale(12, self.dpi) as f32;
            let elapsed_rect = D2D_RECT_F {
                left: seekbar_rect.x as f32,
                top: label_y,
                right: seekbar_rect.x as f32 + scale(48, self.dpi) as f32,
                bottom: label_y + label_h,
            };
            let remaining_rect = D2D_RECT_F {
                left: seekbar_rect.right() as f32 - scale(48, self.dpi) as f32,
                top: label_y,
                right: seekbar_rect.right() as f32,
                bottom: label_y + label_h,
            };
            self.draw_text(
                &media.elapsed_formatted(),
                &self.text_caption,
                elapsed_rect,
                muted_brush,
            );
            let remaining = if media.has_known_duration() {
                media.remaining_formatted()
            } else {
                String::from("LIVE")
            };
            self.draw_text(&remaining, &self.text_trailing, remaining_rect, muted_brush);
        }
    }

    fn draw_media_action_buttons(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        media: &NowPlaying,
        fg: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let prev_rect = translate_rect(layout.media_prev_button, -offset_x);
        let play_rect = translate_rect(layout.media_play_button, -offset_x);
        let next_rect = translate_rect(layout.media_next_button, -offset_x);

        let capabilities = media.capabilities;
        let disabled_glyph = &self
            .brushes
            .alpha_brush(FOREGROUND, DISABLED_CONTROL_ALPHA * alpha);
        let button_alpha = |target: HitTarget, enabled: bool| {
            if !enabled {
                0.06 * alpha
            } else if hovered == Some(target) {
                0.25 * alpha
            } else {
                0.14 * alpha
            }
        };
        let prev_fill = &self.brushes.alpha_brush(
            FOREGROUND,
            button_alpha(HitTarget::MediaPrevious, capabilities.previous),
        );
        self.draw_circle_button(prev_rect, prev_fill);
        self.draw_previous_glyph(
            prev_rect.x as f32 + prev_rect.width as f32 * 0.5,
            prev_rect.y as f32 + prev_rect.height as f32 * 0.5,
            scale(6, self.dpi) as f32,
            if capabilities.previous {
                fg
            } else {
                disabled_glyph
            },
        )?;

        let play_hover = hovered == Some(HitTarget::MediaPlayPause) && capabilities.play_pause;
        let play_color = if play_hover {
            brighten(MUSIC_ACCENT, 0.14)
        } else {
            MUSIC_ACCENT
        };
        let play_alpha = if capabilities.play_pause {
            alpha
        } else {
            DISABLED_CONTROL_ALPHA * alpha
        };
        let play_fill = &self.brushes.alpha_brush(play_color, play_alpha);
        let dark_icon = &self.brushes.alpha_brush(BLACK, 0.92 * alpha);
        self.draw_circle_button(play_rect, play_fill);
        let play_x = play_rect.x as f32 + play_rect.width as f32 * 0.5;
        let play_y = play_rect.y as f32 + play_rect.height as f32 * 0.5;
        if media.playing {
            self.draw_pause_glyph(play_x, play_y, scale(7, self.dpi) as f32, dark_icon);
        } else {
            self.draw_play_glyph(play_x, play_y, scale(7, self.dpi) as f32, dark_icon)?;
        }

        let next_fill = &self.brushes.alpha_brush(
            FOREGROUND,
            button_alpha(HitTarget::MediaNext, capabilities.next),
        );
        self.draw_circle_button(next_rect, next_fill);
        self.draw_skip_glyph(
            next_rect.x as f32 + next_rect.width as f32 * 0.5,
            next_rect.y as f32 + next_rect.height as f32 * 0.5,
            scale(6, self.dpi) as f32,
            if capabilities.next {
                fg
            } else {
                disabled_glyph
            },
        )?;
        Ok(())
    }

    fn draw_expanded_pomodoro(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        pomodoro: PomodoroState,
    ) -> Result<()> {
        let local_left = (layout.notch.x - offset_x) as f32;
        let local_right = (layout.notch.right() - offset_x) as f32;
        let accent_color = phase_accent_color(pomodoro.phase);

        let fg = &self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let muted = &self.brushes.alpha_brush(FOREGROUND, 0.48 * alpha);
        let track = &self.brushes.alpha_brush(FOREGROUND, 0.15 * alpha);
        let accent = &self.brushes.alpha_brush(accent_color, alpha);

        let dot_mid_y = scale(17, self.dpi) as f32;
        self.draw_session_dots(
            local_right - scale(18, self.dpi) as f32,
            dot_mid_y,
            pomodoro.completed_focus_sessions,
            accent,
            track,
        );

        let row_cy = scale(70, self.dpi) as f32;
        let ring_cx = local_left + scale(42, self.dpi) as f32;
        let ring_r = scale(20, self.dpi) as f32;
        let ring_stroke = (scale(4, self.dpi) as f32).max(2.6);
        self.draw_progress_ring(
            ring_cx,
            row_cy,
            ring_r,
            ring_stroke,
            pomodoro.progress(),
            accent,
            track,
        )?;
        self.draw_phase_center_glyph(ring_cx, row_cy, ring_r * 0.45, pomodoro.phase, accent);

        let play_rect = translate_rect(layout.pomodoro_play_button, -offset_x);
        let timer_left = local_left + scale(74, self.dpi) as f32;
        let timer_right = (play_rect.x - scale(8, self.dpi)) as f32;
        let timer_rect = D2D_RECT_F {
            left: timer_left,
            top: scale(43, self.dpi) as f32,
            right: timer_right,
            bottom: scale(79, self.dpi) as f32,
        };
        let time_str = pomodoro.format_time();
        self.draw_text(&time_str, &self.text_timer_display, timer_rect, fg);

        let hint_rect = D2D_RECT_F {
            left: timer_left + 1.0,
            top: scale(79, self.dpi) as f32,
            right: timer_right + scale(20, self.dpi) as f32,
            bottom: scale(97, self.dpi) as f32,
        };
        self.draw_text(pomodoro.status_hint(), &self.text_caption, hint_rect, muted);

        self.draw_pomodoro_action_buttons(
            layout,
            offset_x,
            alpha,
            hovered,
            pomodoro,
            accent_color,
            fg,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pomodoro_action_buttons(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        pomodoro: PomodoroState,
        accent_color: D2D1_COLOR_F,
        fg: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let play_rect = translate_rect(layout.pomodoro_play_button, -offset_x);
        let reset_rect = translate_rect(layout.pomodoro_reset_button, -offset_x);
        let skip_rect = translate_rect(layout.pomodoro_skip_button, -offset_x);

        let play_hover = hovered == Some(HitTarget::PomodoroPlayPause);
        let play_fill_color = if play_hover {
            brighten(accent_color, 0.14)
        } else {
            accent_color
        };
        let play_fill = &self.brushes.alpha_brush(play_fill_color, alpha);
        let dark_icon = &self.brushes.alpha_brush(BLACK, 0.92 * alpha);
        self.draw_circle_button(play_rect, play_fill);
        let play_x = play_rect.x as f32 + play_rect.width as f32 * 0.5;
        let play_mid_y = play_rect.y as f32 + play_rect.height as f32 * 0.5;
        if pomodoro.status == PomodoroStatus::Running {
            self.draw_pause_glyph(play_x, play_mid_y, scale(7, self.dpi) as f32, dark_icon);
        } else {
            self.draw_play_glyph(play_x, play_mid_y, scale(7, self.dpi) as f32, dark_icon)?;
        }

        let reset_alpha = if hovered == Some(HitTarget::PomodoroReset) {
            0.25 * alpha
        } else {
            0.14 * alpha
        };
        let reset_fill = &self.brushes.alpha_brush(FOREGROUND, reset_alpha);
        self.draw_circle_button(reset_rect, reset_fill);
        self.draw_reset_glyph(
            reset_rect.x as f32 + reset_rect.width as f32 * 0.5,
            reset_rect.y as f32 + reset_rect.height as f32 * 0.5,
            scale(6, self.dpi) as f32,
            fg,
        )?;

        let skip_alpha = if hovered == Some(HitTarget::PomodoroSkip) {
            0.25 * alpha
        } else {
            0.14 * alpha
        };
        let skip_fill = &self.brushes.alpha_brush(FOREGROUND, skip_alpha);
        self.draw_circle_button(skip_rect, skip_fill);
        self.draw_skip_glyph(
            skip_rect.x as f32 + skip_rect.width as f32 * 0.5,
            skip_rect.y as f32 + skip_rect.height as f32 * 0.5,
            scale(6, self.dpi) as f32,
            fg,
        )?;
        Ok(())
    }

    fn draw_expanded_alert(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        pomodoro: PomodoroState,
        alert: otto_core::PomodoroAlert,
    ) -> Result<()> {
        let local_left = (layout.notch.x - offset_x) as f32;
        let local_right = (layout.notch.right() - offset_x) as f32;
        let accent_color = phase_accent_color(alert.next_phase);

        unsafe {
            let fg = &self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
            let muted = &self.brushes.alpha_brush(FOREGROUND, 0.58 * alpha);
            let track = &self.brushes.alpha_brush(FOREGROUND, 0.16 * alpha);
            let accent = &self.brushes.alpha_brush(accent_color, alpha);
            let accent_soft = &self.brushes.alpha_brush(accent_color, 0.20 * alpha);

            let top_cy = scale(17, self.dpi) as f32;
            let badge_rect = D2D_RECT_F {
                left: local_left + scale(20, self.dpi) as f32,
                top: scale(7, self.dpi) as f32,
                right: local_left + scale(180, self.dpi) as f32,
                bottom: scale(27, self.dpi) as f32,
            };
            self.draw_text(
                "POMODORO NOTIFICATION",
                &self.text_badge,
                badge_rect,
                accent,
            );
            self.draw_session_dots(
                local_right - scale(18, self.dpi) as f32,
                top_cy,
                pomodoro.completed_focus_sessions,
                accent,
                track,
            );

            let bell_x = local_left + scale(38, self.dpi) as f32;
            let bell_mid_y = scale(68, self.dpi) as f32;
            let icon_r = scale(18, self.dpi) as f32;
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(bell_x, bell_mid_y),
                    radiusX: icon_r,
                    radiusY: icon_r,
                },
                accent_soft,
            );
            self.draw_bell_icon(bell_x, bell_mid_y, icon_r * 0.52, accent)?;

            let extend_rect = translate_rect(layout.pomodoro_alert_extend_button, -offset_x);
            let start_rect = translate_rect(layout.pomodoro_alert_start_button, -offset_x);

            let text_left = local_left + scale(64, self.dpi) as f32;
            let text_right = (extend_rect.x - scale(8, self.dpi)) as f32;
            let title_rect = D2D_RECT_F {
                left: text_left,
                top: scale(47, self.dpi) as f32,
                right: text_right,
                bottom: scale(69, self.dpi) as f32,
            };
            self.draw_text(alert.title(), &self.text_alert_title, title_rect, fg);

            let sub_rect = D2D_RECT_F {
                left: text_left,
                top: scale(69, self.dpi) as f32,
                right: text_right + scale(12, self.dpi) as f32,
                bottom: scale(90, self.dpi) as f32,
            };
            self.draw_text(alert.subtitle(), &self.text_caption, sub_rect, muted);

            let ext_alpha = if hovered == Some(HitTarget::PomodoroExtendFive) {
                0.26 * alpha
            } else {
                0.15 * alpha
            };
            let ext_brush = &self.brushes.alpha_brush(FOREGROUND, ext_alpha);
            self.draw_rounded_box(extend_rect, extend_rect.height as f32 * 0.5, ext_brush);
            self.draw_text("+5m", &self.text_app_bold, rect_to_d2d(extend_rect), fg);

            let start_color = if hovered == Some(HitTarget::PomodoroStartNext) {
                brighten(accent_color, 0.14)
            } else {
                accent_color
            };
            let start_brush = &self.brushes.alpha_brush(start_color, alpha);
            let dark_text = &self.brushes.alpha_brush(BLACK, 0.92 * alpha);
            self.draw_rounded_box(start_rect, start_rect.height as f32 * 0.5, start_brush);
            self.draw_text(
                alert.action_label(),
                &self.text_app_bold,
                rect_to_d2d(start_rect),
                dark_text,
            );
        }
        Ok(())
    }

    fn draw_session_dots(
        &self,
        right_x: f32,
        cy: f32,
        completed: u8,
        active_brush: &ID2D1SolidColorBrush,
        inactive_brush: &ID2D1SolidColorBrush,
    ) {
        let dot_r = scale(3, self.dpi) as f32;
        let gap = scale(10, self.dpi) as f32;
        for i in 0..4u8 {
            let cx = right_x - f32::from(3 - i) * gap;
            let brush = if i < completed {
                active_brush
            } else {
                inactive_brush
            };
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: dot_r,
                        radiusY: dot_r,
                    },
                    brush,
                );
            }
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

    fn draw_phase_center_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        phase: PomodoroPhase,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (r * 0.22).max(1.5);
        unsafe {
            match phase {
                PomodoroPhase::Focus => {
                    self.target.DrawLine(
                        Vector2::new(cx, cy),
                        Vector2::new(cx, cy - r * 0.72),
                        brush,
                        stroke,
                        &self.stroke_style,
                    );
                    self.target.DrawLine(
                        Vector2::new(cx, cy),
                        Vector2::new(cx + r * 0.52, cy + r * 0.22),
                        brush,
                        stroke,
                        &self.stroke_style,
                    );
                }
                PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak => {
                    let dot = D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: r * 0.42,
                        radiusY: r * 0.42,
                    };
                    self.target.FillEllipse(&dot, brush);
                }
            }
        }
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
        let pill = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(box_rect),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&pill, brush);
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
        let stroke = (r * 0.22).max(1.5);
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

    fn draw_audio_equalizer(
        &self,
        cx: f32,
        cy: f32,
        anim_phase: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let bar_w = (scale(2, self.dpi) as f32).max(1.8);
        let gap = (scale(2, self.dpi) as f32).max(1.8);
        let max_h = scale(12, self.dpi) as f32;
        let min_h = scale(3, self.dpi) as f32;
        let offsets = [-1.5f32, -0.5, 0.5, 1.5];
        let phases = [0.0f32, 1.6, 3.1, 4.7];
        let speeds = [5.5f32, 7.8, 6.2, 8.5];
        for (i, &dx_mult) in offsets.iter().enumerate() {
            let bx = cx + dx_mult * (bar_w + gap);
            let wave = ((anim_phase * speeds[i] + phases[i]).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            let h = min_h + wave * (max_h - min_h);
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
        let pad_y = shape.slot_inset_y as f32;
        let radius = shape.slot_radius as f32;
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: slot.x as f32,
                top: slot.y as f32 + pad_y,
                right: slot.right() as f32,
                bottom: slot.bottom() as f32 - pad_y,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&rect, brush);
        }
    }

    fn draw_slot_focus_ring(&self, slot: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let pad_y = shape.focus_ring_inset_y as f32;
        let radius = shape.focus_ring_radius as f32;
        let stroke = shape.focus_ring_width as f32;
        let inset = stroke * 0.5;
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: slot.x as f32 + inset,
                top: slot.y as f32 + pad_y + inset,
                right: slot.right() as f32 - inset,
                bottom: slot.bottom() as f32 - pad_y - inset,
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

        let bat_x = speaker_x + unit * 1.05;
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
        let base_y = cy + radius * 0.56;
        unsafe {
            let dot = D2D1_ELLIPSE {
                point: Vector2::new(cx, base_y),
                radiusX: stroke * 0.80,
                radiusY: stroke * 0.80,
            };
            self.target.FillEllipse(&dot, brush);

            for factor in [0.54_f32, 0.96_f32] {
                let r = radius * factor;
                let dx = r * 0.72;
                let dy = r * 0.72;
                let geometry = self.factory.CreatePathGeometry()?;
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
                self.target
                    .DrawGeometry(&geometry, brush, stroke, &self.stroke_style);
            }
        }
        Ok(())
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
        let box_h = half_h * 0.44;
        let box_w = half_h * 0.40;
        let cone_w = half_h * 0.92;
        unsafe {
            let cone = self.factory.CreatePathGeometry()?;
            let sink = cone.Open()?;
            sink.BeginFigure(Vector2::new(x, cy - box_h), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(x + box_w, cy - box_h));
            sink.AddLine(Vector2::new(x + cone_w, cy - half_h * 0.88));
            sink.AddLine(Vector2::new(x + cone_w, cy + half_h * 0.88));
            sink.AddLine(Vector2::new(x + box_w, cy + box_h));
            sink.AddLine(Vector2::new(x, cy + box_h));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&cone, brush, None);

            if muted || volume == 0 {
                self.target.DrawLine(
                    Vector2::new(x + cone_w + stroke * 0.65, cy - half_h * 0.55),
                    Vector2::new(x + cone_w + stroke * 2.55, cy + half_h * 0.55),
                    brush,
                    stroke * 0.95,
                    &self.stroke_style,
                );
                self.target.DrawLine(
                    Vector2::new(x + cone_w + stroke * 2.55, cy - half_h * 0.55),
                    Vector2::new(x + cone_w + stroke * 0.65, cy + half_h * 0.55),
                    brush,
                    stroke * 0.95,
                    &self.stroke_style,
                );
                return Ok(());
            }

            let wave_count = if volume <= 35 { 1 } else { 2 };
            for &(offset, scale_y) in &[(1.10_f32, 0.46_f32), (2.25_f32, 0.78_f32)][..wave_count] {
                let wave = self.factory.CreatePathGeometry()?;
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
                self.target
                    .DrawGeometry(&wave, brush, stroke * 0.90, &self.stroke_style);
            }
        }
        Ok(())
    }

    fn draw_notch_hud(&self, layout: &Layout, hud: NotchHudState) -> Result<()> {
        let alpha = ((hud.expansion - 0.12) / 0.88).clamp(0.0, 1.0);
        let n = layout.notch;
        let h = layout.bar.height as f32;
        let cy = h * 0.5;
        let unit = layout.icon_size as f32;
        let stroke = (unit * 0.105).max(1.35);
        let pad_x = scale(14, self.dpi) as f32;
        let left_x = n.x as f32 + pad_x;
        let right_x = n.right() as f32 - pad_x;

        let fg_brush = &self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let muted_brush = &self.brushes.alpha_brush(FOREGROUND, 0.30 * alpha);
        let green_brush = &self.brushes.alpha_brush(CHARGING_GREEN, 0.96 * alpha);
        let amber_brush = &self.brushes.alpha_brush(AMBER_WARN, 0.96 * alpha);

        match hud.kind {
            NotchHudKind::Volume { percent, muted } => {
                self.draw_speaker_icon(left_x, cy, unit * 0.46, stroke, percent, muted, fg_brush)?;
                let label = if muted || percent == 0 {
                    "Muted"
                } else {
                    "Sound"
                };
                self.draw_hud_text(left_x + unit * 1.45, h, label, fg_brush);
                self.draw_hud_bar(
                    right_x,
                    cy,
                    hud.level,
                    percent,
                    muted,
                    fg_brush,
                    muted_brush,
                );
            }
            NotchHudKind::Brightness { percent } => {
                self.draw_sun_icon(left_x + unit * 0.50, cy, unit * 0.44, stroke, fg_brush);
                self.draw_hud_text(left_x + unit * 1.45, h, "Display", fg_brush);
                self.draw_hud_bar(
                    right_x,
                    cy,
                    hud.level,
                    percent,
                    false,
                    fg_brush,
                    muted_brush,
                );
            }
            NotchHudKind::Charging {
                percent,
                plugged_in,
            } => {
                let accent = if plugged_in { green_brush } else { fg_brush };
                self.draw_charging_bolt(left_x + unit * 0.45, cy, unit * 0.46, accent)?;
                let label = if plugged_in { "Charging" } else { "Battery" };
                self.draw_hud_text(left_x + unit * 1.25, h, label, fg_brush);
                self.draw_hud_bar(right_x, cy, hud.level, percent, false, accent, muted_brush);
            }
            NotchHudKind::LowBattery { percent } => {
                self.draw_charging_bolt(left_x + unit * 0.45, cy, unit * 0.46, amber_brush)?;
                self.draw_hud_text(left_x + unit * 1.25, h, "Low Bat", amber_brush);
                self.draw_hud_bar(
                    right_x,
                    cy,
                    hud.level,
                    percent,
                    false,
                    amber_brush,
                    muted_brush,
                );
            }
            NotchHudKind::CapsLock { enabled } => {
                let accent = if enabled { green_brush } else { muted_brush };
                self.draw_capslock_icon(left_x + unit * 0.50, cy, unit * 0.42, stroke, accent)?;
                self.draw_hud_text(left_x + unit * 1.35, h, "Caps Lock", fg_brush);
                self.draw_capslock_pill(right_x, cy, enabled, accent, fg_brush);
            }
        }
        Ok(())
    }

    fn draw_capslock_pill(
        &self,
        right_x: f32,
        cy: f32,
        enabled: bool,
        accent: &ID2D1SolidColorBrush,
        fg: &ID2D1SolidColorBrush,
    ) {
        let state_text = if enabled { "ON" } else { "OFF" };
        let pill_w = scale(36, self.dpi) as f32;
        let pill_h = scale(18, self.dpi) as f32;
        let pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: right_x - pill_w,
                top: cy - pill_h * 0.5,
                right: right_x,
                bottom: cy + pill_h * 0.5,
            },
            radiusX: pill_h * 0.5,
            radiusY: pill_h * 0.5,
        };
        unsafe {
            self.target.FillRoundedRectangle(&pill, accent);
        }
        self.draw_text(state_text, &self.text_app_bold, pill.rect, fg);
    }

    fn draw_hud_text(&self, left: f32, height: f32, label: &str, brush: &ID2D1SolidColorBrush) {
        let rect = D2D_RECT_F {
            left,
            top: 0.0,
            right: left + scale(72, self.dpi) as f32,
            bottom: height,
        };
        self.draw_text(label, &self.text_leading, rect, brush);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_hud_bar(
        &self,
        right_x: f32,
        cy: f32,
        level: f32,
        percent: u8,
        muted: bool,
        fill_brush: &ID2D1SolidColorBrush,
        track_brush: &ID2D1SolidColorBrush,
    ) {
        let pct_w = scale(34, self.dpi) as f32;
        let bar_w = scale(68, self.dpi) as f32;
        let bar_h = scale(5, self.dpi) as f32;
        let gap = scale(6, self.dpi) as f32;
        let bar_left = right_x - pct_w - gap - bar_w;
        let seg_gap = 1.2_f32;
        let seg_w = (bar_w - seg_gap * 15.0) / 16.0;
        let active_segs = if muted {
            0
        } else {
            (level.clamp(0.0, 1.0) * 16.0).round() as i32
        };

        for i in 0..16 {
            let sx = bar_left + i as f32 * (seg_w + seg_gap);
            let seg = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: sx,
                    top: cy - bar_h * 0.5,
                    right: sx + seg_w,
                    bottom: cy + bar_h * 0.5,
                },
                radiusX: 1.2,
                radiusY: 1.2,
            };
            let brush = if i < active_segs {
                fill_brush
            } else {
                track_brush
            };
            unsafe {
                self.target.FillRoundedRectangle(&seg, brush);
            }
        }

        let pct_label = if muted {
            String::from("0%")
        } else {
            format!("{}%", percent.min(100))
        };
        let text_rect = D2D_RECT_F {
            left: right_x - pct_w,
            top: 0.0,
            right: right_x,
            bottom: cy * 2.0,
        };
        self.draw_text(&pct_label, &self.text_menu, text_rect, fill_brush);
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

    fn draw_clock_slot(
        &self,
        layout: &Layout,
        status: SystemStatus,
        fg: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let slot = layout.clock_slot;
        let unit = layout.icon_size as f32;
        let pad_x = scale(6, self.dpi) as f32;
        let bell_cx = slot.right() as f32 - pad_x - unit * 0.45;
        let cy = slot.height as f32 * 0.5;

        let text_rect = D2D_RECT_F {
            left: slot.x as f32 + pad_x,
            top: 0.0,
            right: bell_cx - unit * 0.45,
            bottom: slot.height as f32,
        };
        let text = status.clock.format();
        self.draw_text(&text, &self.text_leading, text_rect, fg);
        self.draw_bell_icon(bell_cx, cy, unit * 0.42, fg)?;
        Ok(())
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
        let bottom = n.height as f32;
        let cr = corner_radius.min(bottom * 0.5).max(0.0);
        let er = ear_radius.max(1.0);

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(Vector2::new(left - er, 0.0), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(right + er, 0.0));
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(right, er),
                size: D2D_SIZE_F {
                    width: er,
                    height: er,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.AddLine(Vector2::new(right, bottom - cr));
            if cr > 0.5 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: Vector2::new(right - cr, bottom),
                    size: D2D_SIZE_F {
                        width: cr,
                        height: cr,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            }
            sink.AddLine(Vector2::new(left + cr, bottom));
            if cr > 0.5 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: Vector2::new(left, bottom - cr),
                    size: D2D_SIZE_F {
                        width: cr,
                        height: cr,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            }
            sink.AddLine(Vector2::new(left, er));
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(left - er, 0.0),
                size: D2D_SIZE_F {
                    width: er,
                    height: er,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
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

const fn translate_rect(rect: Rect, dx: i32) -> Rect {
    Rect {
        x: rect.x + dx,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

const fn rect_to_d2d(rect: Rect) -> D2D_RECT_F {
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

unsafe fn create_text_format(
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
        DockRenderer, GenieRenderer, PIXEL_FORMAT, PopupMenuRenderer, Renderer,
        WindowControlsRenderer, release_shared_factories, shared_d2d_factory,
    };
    use crate::surface::{DibSurface, DrawSession, draw_on, upload_bgra_bitmap};
    use otto_core::{PixelLimits, Rect};
    use windows::Win32::Foundation::E_INVALIDARG;
    use windows::Win32::Graphics::Direct2D::{
        D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1_RENDER_TARGET_USAGE_NONE, ID2D1DCRenderTarget,
    };
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

    static SERIAL: Mutex<()> = Mutex::new(());

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
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: PIXEL_FORMAT,
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let factory = shared_d2d_factory().expect("shared factory");
        unsafe { factory.CreateDCRenderTarget(&properties) }.expect("DC render target")
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
}
