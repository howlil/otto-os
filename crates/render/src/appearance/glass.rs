use std::cell::RefCell;

use otto_core::{
    Appearance, GlassBackdrop, GlassBitmap, GlassParams, GlassShape, LIGHT_DIRECTION, PixelLimits,
    ResolvedTheme, RgbBuffer, compose_glass_lit, soften_backdrop,
};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, ID2D1Bitmap, ID2D1RenderTarget,
};
use windows::core::Result;

use crate::appearance::surface::{update_bgra_bitmap, upload_bgra_bitmap};

const BASE_SOFT_FACTOR: usize = 2;
const MIN_SOFT_FACTOR: usize = 2;
const BASE_BLUR_PT: usize = 12;
const MIN_BLUR_RADIUS: usize = 3;
const BASE_DPI: usize = 96;

const LIGHT_QUANT_STEPS: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SoftKey {
    theme: ResolvedTheme,
    dpi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ComposeKey {
    generation: u64,
    soft: SoftKey,
    shape: GlassShape,
    lens: bool,
    light: (i16, i16),
}

fn quantize_light(light: (f32, f32)) -> (i16, i16) {
    (
        (light.0 * LIGHT_QUANT_STEPS).round() as i16,
        (light.1 * LIGHT_QUANT_STEPS).round() as i16,
    )
}

use crate::appearance::glass_gpu::{GpuGlassComposer, SoftCacheKey, release_shared_gpu};

struct Capture {
    sharp: RgbBuffer,
    origin: (f32, f32),
}

#[derive(Default)]
enum GpuBackend {
    #[default]
    Uninitialized,
    Ready(GpuGlassComposer),
    Unavailable,
}

impl GpuBackend {
    fn get_mut(&mut self) -> Option<&mut GpuGlassComposer> {
        if matches!(self, Self::Uninitialized) {
            *self = match GpuGlassComposer::new() {
                Some(gpu) => Self::Ready(gpu),
                None => Self::Unavailable,
            };
        }
        match self {
            Self::Ready(gpu) => Some(gpu),
            Self::Uninitialized | Self::Unavailable => None,
        }
    }

    fn reset(&mut self) {
        *self = Self::Uninitialized;
        release_shared_gpu();
    }
}

#[derive(Default)]
pub(crate) struct GlassComposer {
    capture: Option<Capture>,
    generation: u64,
    soft: Option<(SoftKey, RgbBuffer)>,
    composed: Option<(ComposeKey, GlassBitmap)>,
    revision: u64,
    gpu: GpuBackend,
}

impl GlassComposer {
    pub(crate) fn set_backdrop(&mut self, sharp: RgbBuffer, origin: (i32, i32)) {
        self.generation += 1;
        self.capture = Some(Capture {
            sharp,
            origin: (origin.0 as f32, origin.1 as f32),
        });
        self.soft = None;
        self.composed = None;
    }

    pub(crate) fn clear(&mut self) {
        self.capture = None;
        self.soft = None;
        self.composed = None;
        self.gpu = GpuBackend::Uninitialized;
    }

    pub(crate) fn take_backdrop(&mut self) -> Option<(RgbBuffer, (i32, i32))> {
        let capture = self.capture.take()?;
        self.soft = None;
        self.composed = None;
        Some((
            capture.sharp,
            (
                capture.origin.0.round() as i32,
                capture.origin.1.round() as i32,
            ),
        ))
    }

    pub(crate) const fn has_backdrop(&self) -> bool {
        self.capture.is_some()
    }

    #[cfg(test)]
    pub(crate) fn compose(
        &mut self,
        appearance: &Appearance,
        dpi: u32,
        shape: GlassShape,
        lens: bool,
    ) -> Option<(u64, &GlassBitmap)> {
        self.compose_lit(appearance, dpi, shape, lens, LIGHT_DIRECTION)
    }

    pub(crate) fn compose_lit(
        &mut self,
        appearance: &Appearance,
        dpi: u32,
        shape: GlassShape,
        lens: bool,
        light_dir: (f32, f32),
    ) -> Option<(u64, &GlassBitmap)> {
        if !appearance.uses_backdrop() {
            return None;
        }
        let capture = self.capture.as_ref()?;
        let soft_key = SoftKey {
            theme: appearance.theme,
            dpi,
        };
        let key = ComposeKey {
            generation: self.generation,
            soft: soft_key,
            shape,
            lens,
            light: quantize_light(light_dir),
        };
        let reusable = self
            .composed
            .as_ref()
            .is_some_and(|(cached, _)| *cached == key);
        if !reusable {
            let base = GlassParams::for_theme(appearance.theme).scaled(dpi);
            let params = if lens { base } else { base.without_lens() };
            let factor = soft_factor(dpi);
            let radius = blur_radius(dpi);
            if self.soft.as_ref().is_none_or(|(held, _)| *held != soft_key) {
                self.soft = Some((
                    soft_key,
                    soften_backdrop(&capture.sharp, &params, factor, radius),
                ));
            }
            let soft = &self.soft.as_ref()?.1;
            let backdrop = GlassBackdrop {
                sharp: &capture.sharp,
                soft,
                soft_factor: factor,
                origin: capture.origin,
            };
            self.revision += 1;
            let gpu_soft_key = SoftCacheKey {
                generation: self.generation,
                dpi,
                theme: appearance.theme,
            };
            let mut lost = false;
            let mut gpu_bitmap = self.gpu.get_mut().and_then(|gpu| {
                let result = gpu.compose(
                    self.generation,
                    gpu_soft_key,
                    &params,
                    &shape,
                    &backdrop,
                    light_dir,
                );
                if result.is_none() && gpu.is_device_lost() {
                    lost = true;
                }
                result
            });
            if lost {
                self.gpu.reset();
                gpu_bitmap = self.gpu.get_mut().and_then(|gpu| {
                    gpu.compose(
                        self.generation,
                        gpu_soft_key,
                        &params,
                        &shape,
                        &backdrop,
                        light_dir,
                    )
                });
            }
            let bitmap = gpu_bitmap
                .unwrap_or_else(|| compose_glass_lit(&params, &shape, &backdrop, light_dir));
            self.composed = Some((key, bitmap));
        }
        self.composed
            .as_ref()
            .map(|(_, bitmap)| (self.revision, bitmap))
    }
}

fn soft_factor(dpi: u32) -> usize {
    (BASE_SOFT_FACTOR * dpi as usize / BASE_DPI).max(MIN_SOFT_FACTOR)
}

fn blur_radius(dpi: u32) -> usize {
    let factor = soft_factor(dpi);
    let target_px = (BASE_BLUR_PT * dpi as usize).div_ceil(BASE_DPI);
    (target_px / factor).max(MIN_BLUR_RADIUS)
}

#[derive(Default)]
pub(crate) struct GlassPainter {
    composer: RefCell<GlassComposer>,
    uploaded: RefCell<Option<(u64, ID2D1Bitmap)>>,
}

impl GlassPainter {
    #[allow(clippy::unused_self)]
    pub(crate) fn adaptive_appearance(&self, base: &Appearance) -> Appearance {
        if base.contrast.is_some() || !base.uses_backdrop() {
            return *base;
        }
        Appearance::glass(ResolvedTheme::Dark)
    }

    pub(crate) fn set_backdrop(&self, sharp: RgbBuffer, origin: (i32, i32)) {
        if let Ok(mut composer) = self.composer.try_borrow_mut() {
            composer.set_backdrop(sharp, origin);
        }
    }

    pub(crate) fn clear_backdrop(&self) {
        if let Ok(mut composer) = self.composer.try_borrow_mut() {
            composer.clear();
        }
        self.invalidate_gpu();
    }

    pub(crate) fn take_backdrop(&self) -> Option<(RgbBuffer, (i32, i32))> {
        let taken = self
            .composer
            .try_borrow_mut()
            .ok()
            .and_then(|mut composer| composer.take_backdrop());
        if taken.is_some() {
            self.invalidate_gpu();
        }
        taken
    }

    pub(crate) fn invalidate_gpu(&self) {
        if let Ok(mut uploaded) = self.uploaded.try_borrow_mut() {
            uploaded.take();
        }
    }

    pub(crate) fn has_backdrop(&self) -> bool {
        self.composer
            .try_borrow()
            .is_ok_and(|composer| composer.has_backdrop())
    }

    pub(crate) fn draw(
        &self,
        target: &ID2D1RenderTarget,
        appearance: &Appearance,
        dpi: u32,
        shape: GlassShape,
        lens: bool,
    ) -> Result<bool> {
        let Ok(mut composer) = self.composer.try_borrow_mut() else {
            return Ok(false);
        };
        let Some((revision, bitmap)) =
            composer.compose_lit(appearance, dpi, shape, lens, LIGHT_DIRECTION)
        else {
            return Ok(false);
        };
        let Ok(mut uploaded) = self.uploaded.try_borrow_mut() else {
            return Ok(false);
        };
        if uploaded.as_ref().is_none_or(|(held, _)| *held != revision) {
            let reused = if let Some((held, gpu)) = uploaded.as_mut()
                && update_bgra_bitmap(
                    gpu,
                    bitmap.width,
                    bitmap.height,
                    &bitmap.pixels,
                    PixelLimits::BACKDROP,
                )? {
                *held = revision;
                true
            } else {
                false
            };
            if !reused {
                let gpu = upload_bgra_bitmap(
                    target,
                    bitmap.width,
                    bitmap.height,
                    &bitmap.pixels,
                    PixelLimits::BACKDROP,
                )?;
                *uploaded = Some((revision, gpu));
            }
        }
        let Some((_, gpu)) = uploaded.as_ref() else {
            return Ok(false);
        };
        let destination = D2D_RECT_F {
            left: bitmap.x as f32,
            top: bitmap.y as f32,
            right: (bitmap.x + bitmap.width as i32) as f32,
            bottom: (bitmap.y + bitmap.height as i32) as f32,
        };
        unsafe {
            target.DrawBitmap(
                gpu,
                Some(&raw const destination),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use otto_core::{Appearance, ContrastColors, GlassShape, Rect, ResolvedTheme, RgbBuffer};

    use super::{GlassComposer, GlassPainter, blur_radius, soft_factor};

    fn capture() -> RgbBuffer {
        let mut buffer = RgbBuffer::filled(320, 120, [0.0; 3]);
        for y in 0..120 {
            for x in 0..320 {
                let value = if (x / 8) % 2 == 0 { 1.0 } else { 0.1 };
                buffer.set_pixel(x, y, [value, 0.4, 1.0 - value]);
            }
        }
        buffer
    }

    fn pill(width: i32) -> GlassShape {
        GlassShape::from_rect(
            Rect {
                x: 30,
                y: 30,
                width,
                height: 60,
            },
            30.0,
        )
    }

    fn dark() -> Appearance {
        Appearance::glass(ResolvedTheme::Dark)
    }

    fn revision(composer: &mut GlassComposer, appearance: &Appearance, shape: GlassShape) -> u64 {
        composer
            .compose(appearance, 96, shape, true)
            .expect("glass should compose")
            .0
    }

    #[test]
    fn nothing_composes_without_a_backdrop_or_with_the_wrong_material() {
        let mut composer = GlassComposer::default();
        assert!(composer.compose(&dark(), 96, pill(200), true).is_none());
        composer.set_backdrop(capture(), (0, 0));
        assert!(composer.compose(&dark(), 96, pill(200), true).is_some());
        let opaque = Appearance {
            transparency: false,
            ..dark()
        };
        let contrast = Appearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..dark()
        };
        assert!(composer.compose(&opaque, 96, pill(200), true).is_none());
        assert!(composer.compose(&contrast, 96, pill(200), true).is_none());
        composer.clear();
        assert!(!composer.has_backdrop());
        assert!(composer.compose(&dark(), 96, pill(200), true).is_none());
    }

    #[test]
    fn identical_requests_reuse_the_composed_bitmap() {
        let mut composer = GlassComposer::default();
        composer.set_backdrop(capture(), (0, 0));
        let first = revision(&mut composer, &dark(), pill(200));
        let second = revision(&mut composer, &dark(), pill(200));
        assert_eq!(first, second);
    }

    #[test]
    fn a_new_shape_theme_or_backdrop_composes_again() {
        let mut composer = GlassComposer::default();
        composer.set_backdrop(capture(), (0, 0));
        let base = revision(&mut composer, &dark(), pill(200));
        let wider = revision(&mut composer, &dark(), pill(210));
        assert!(wider > base);
        let light = revision(
            &mut composer,
            &Appearance::glass(ResolvedTheme::Light),
            pill(210),
        );
        assert!(light > wider);
        composer.set_backdrop(capture(), (0, 0));
        let fresh = revision(
            &mut composer,
            &Appearance::glass(ResolvedTheme::Light),
            pill(210),
        );
        assert!(fresh > light);
    }

    #[test]
    fn dark_and_light_produce_different_glass_from_the_same_backdrop() {
        let mut composer = GlassComposer::default();
        composer.set_backdrop(capture(), (0, 0));
        let dark_pixels = composer
            .compose(&dark(), 96, pill(200), true)
            .unwrap()
            .1
            .pixels
            .clone();
        let light_pixels = composer
            .compose(
                &Appearance::glass(ResolvedTheme::Light),
                96,
                pill(200),
                true,
            )
            .unwrap()
            .1
            .pixels
            .clone();
        assert_ne!(dark_pixels, light_pixels);
    }

    #[test]
    fn the_lens_only_changes_pixels_near_the_edge() {
        let mut composer = GlassComposer::default();
        composer.set_backdrop(capture(), (0, 0));
        let lensed = composer
            .compose(&dark(), 96, pill(200), true)
            .unwrap()
            .1
            .clone();
        let flat = composer
            .compose(&dark(), 96, pill(200), false)
            .unwrap()
            .1
            .clone();
        assert_ne!(lensed.pixels, flat.pixels);
        let centre = |bitmap: &otto_core::GlassBitmap| {
            let x = (130 - bitmap.x) as usize;
            let y = (60 - bitmap.y) as usize;
            let at = (y * bitmap.width as usize + x) * 4;
            bitmap.pixels[at..at + 4].to_vec()
        };
        assert_eq!(centre(&lensed), centre(&flat));
    }

    #[test]
    fn the_backdrop_origin_moves_the_sampled_content() {
        let mut composer = GlassComposer::default();
        composer.set_backdrop(capture(), (0, 0));
        let at_zero = composer
            .compose(&dark(), 96, pill(200), false)
            .unwrap()
            .1
            .pixels
            .clone();
        composer.set_backdrop(capture(), (4, 0));
        let shifted = composer
            .compose(&dark(), 96, pill(200), false)
            .unwrap()
            .1
            .pixels
            .clone();
        assert_ne!(at_zero, shifted);
    }

    #[test]
    fn the_soft_factor_grows_with_dpi_and_never_collapses() {
        assert_eq!(soft_factor(96), 2);
        assert_eq!(soft_factor(144), 3);
        assert_eq!(soft_factor(192), 4);
        assert_eq!(soft_factor(24), 2);
        assert_eq!(blur_radius(96), 6);
        assert_eq!(blur_radius(144), 6);
        assert_eq!(blur_radius(192), 6);
        assert_eq!(blur_radius(24), 3);
    }

    #[test]
    fn adaptive_appearance_locks_clear_glass_to_dark_across_backdrop_luminance() {
        let painter = GlassPainter::default();
        let base = Appearance::glass(ResolvedTheme::Dark);
        assert_eq!(
            painter.adaptive_appearance(&base).theme,
            ResolvedTheme::Dark
        );

        for luma in [0.8, 0.5, 0.2] {
            painter.set_backdrop(RgbBuffer::filled(8, 8, [luma; 3]), (0, 0));
            assert_eq!(
                painter.adaptive_appearance(&base).theme,
                ResolvedTheme::Dark
            );
            assert_eq!(
                painter
                    .adaptive_appearance(&Appearance::glass(ResolvedTheme::Light))
                    .theme,
                ResolvedTheme::Dark
            );
        }
    }

    #[test]
    fn adaptive_appearance_preserves_accessibility_fallbacks() {
        let painter = GlassPainter::default();
        painter.set_backdrop(RgbBuffer::filled(8, 8, [0.9; 3]), (0, 0));
        let opaque = Appearance {
            transparency: false,
            ..Appearance::glass(ResolvedTheme::Dark)
        };
        let contrast = Appearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..Appearance::glass(ResolvedTheme::Dark)
        };

        assert_eq!(painter.adaptive_appearance(&opaque), opaque);
        assert_eq!(painter.adaptive_appearance(&contrast), contrast);
    }

    mod native {
        use otto_core::{Appearance, ResolvedTheme};
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
        };
        use windows::Win32::Graphics::Direct2D::{
            D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
            D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, ID2D1DCRenderTarget,
        };
        use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

        use super::{GlassPainter, capture, pill};
        use crate::appearance::surface::{DibSurface, draw_on};
        use crate::shared_d2d_factory;

        fn target() -> ID2D1DCRenderTarget {
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            unsafe {
                shared_d2d_factory()
                    .unwrap()
                    .CreateDCRenderTarget(&properties)
            }
            .expect("a DC render target should be created")
        }

        #[test]
        fn drawn_glass_lands_in_the_surface_exactly_where_the_core_composed_it() {
            let _serial = crate::resource_tests::SERIAL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let target = target();
            let surface = DibSurface::new(320, 120).unwrap();
            let painter = GlassPainter::default();
            let appearance = Appearance::glass(ResolvedTheme::Light);
            painter.set_backdrop(capture(), (0, 0));
            let drawn = draw_on(&target, &surface, surface.size(), || {
                painter.draw(&target, &appearance, 96, pill(200), true)
            })
            .unwrap();
            assert!(drawn);
            let pixels = surface.read_pixels();
            let expected = painter
                .composer
                .borrow_mut()
                .compose(&appearance, 96, pill(200), true)
                .unwrap()
                .1
                .clone();
            let at = |x: i32, y: i32| ((y * 320 + x) * 4) as usize;
            assert_eq!(pixels[at(5, 5) + 3], 0);
            assert_eq!(pixels[at(130, 60) + 3], 255);
            let mut worst = 0u8;
            for row in 0..expected.height as i32 {
                for column in 0..expected.width as i32 {
                    let (x, y) = (expected.x + column, expected.y + row);
                    if !(0..320).contains(&x) || !(0..120).contains(&y) {
                        continue;
                    }
                    let want = (row as usize * expected.width as usize + column as usize) * 4;
                    for channel in 0..4 {
                        let got = pixels[at(x, y) + channel];
                        worst = worst.max(got.abs_diff(expected.pixels[want + channel]));
                    }
                }
            }
            assert!(
                worst <= 3,
                "surface differs from the composed glass by {worst}"
            );
            drop(painter);
            drop(target);
            drop(surface);
            crate::release_shared_factories();
        }

        #[test]
        fn drawing_uploads_once_per_revision_and_reports_whether_glass_was_drawn() {
            let _serial = crate::resource_tests::SERIAL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let target = target();
            let surface = DibSurface::new(320, 120).unwrap();
            let painter = GlassPainter::default();
            let appearance = Appearance::glass(ResolvedTheme::Dark);
            let drawn = |painter: &GlassPainter| {
                draw_on(&target, &surface, surface.size(), || {
                    painter.draw(&target, &appearance, 96, pill(200), true)
                })
                .unwrap()
            };
            assert!(!drawn(&painter));
            painter.set_backdrop(capture(), (0, 0));
            assert!(painter.has_backdrop());
            assert!(drawn(&painter));
            let first = painter
                .uploaded
                .borrow()
                .as_ref()
                .map(|(revision, _)| *revision);
            assert!(first.is_some());
            assert!(drawn(&painter));
            let second = painter
                .uploaded
                .borrow()
                .as_ref()
                .map(|(revision, _)| *revision);
            assert_eq!(first, second);
            let bitmap_ptr = painter
                .uploaded
                .borrow()
                .as_ref()
                .map(|(_, bitmap)| windows::core::Interface::as_raw(bitmap));
            painter.set_backdrop(capture(), (4, 0));
            assert!(drawn(&painter));
            let third = painter
                .uploaded
                .borrow()
                .as_ref()
                .map(|(revision, _)| *revision);
            let reused_ptr = painter
                .uploaded
                .borrow()
                .as_ref()
                .map(|(_, bitmap)| windows::core::Interface::as_raw(bitmap));
            assert!(third > second);
            assert_eq!(reused_ptr, bitmap_ptr);
            painter.clear_backdrop();
            assert!(!drawn(&painter));
            assert!(painter.uploaded.borrow().is_none());
            drop(painter);
            drop(target);
            drop(surface);
            crate::release_shared_factories();
        }
    }

    #[test]
    fn take_backdrop_returns_capture_and_resets_state() {
        let painter = GlassPainter::default();
        assert!(painter.take_backdrop().is_none());
        let pixels = RgbBuffer::filled(4, 3, [0.25, 0.5, 0.75]);
        painter.set_backdrop(pixels.clone(), (-12, 8));
        assert!(painter.has_backdrop());
        let taken = painter.take_backdrop();
        assert_eq!(taken, Some((pixels, (-12, 8))));
        assert!(!painter.has_backdrop());
        assert!(painter.take_backdrop().is_none());
    }
}
