use std::collections::HashMap;
use std::f32::consts::TAU;
use std::rc::Rc;

use otto_core::{LyricsSheet, LyricsView, NowPlaying, Rect, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NO_SNAP;
use windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_HIT_TEST_METRICS, DWRITE_LINE_METRICS, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP, IDWriteTextLayout,
};
use windows_numerics::Vector2;

use super::{FOREGROUND, Renderer, shared_dwrite_factory};

const LYRIC_ROW_GAP_PT: i32 = 6;
const LYRIC_PAD_PT: i32 = 14;
const LYRIC_CARD_RADIUS_PT: i32 = 14;
const LYRIC_EDGE_FADE_PT: i32 = 16;
pub(crate) const LYRIC_FONT_PT: i32 = 16;
const LYRIC_MAX_LINES: f32 = 2.0;
const LYRIC_MIN_FIT: f32 = 0.86;
const LYRIC_WIPE_FEATHER_PT: i32 = 10;
const LYRIC_WIPE_STEPS: usize = 3;
const LYRIC_LINE_HEIGHT_RATIO: f32 = 1.35;
const LYRIC_ANCHOR_RATIO: f32 = 0.5;
const LYRIC_SMALL_SCALE: f32 = 0.9;
const LYRIC_CACHE_LIMIT: usize = 64;
const LYRIC_BLUR_MAX_PX: f32 = 2.2;
const LYRIC_BLUR_RANGE: f32 = 2.0;
const LYRIC_BLUR_MIN_PX: f32 = 0.12;
const LYRIC_BLUR_INNER_SCALE: f32 = 0.45;
const LYRIC_ACTIVE_BASE_ALPHA: f32 = 0.58;
const LYRIC_ACTIVE_BLUR_THRESHOLD: f32 = 0.18;
const LYRIC_BLUR_RING: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.707, 0.707),
    (0.0, 1.0),
    (-0.707, 0.707),
    (-1.0, 0.0),
    (-0.707, -0.707),
    (0.0, -1.0),
    (0.707, -0.707),
];

#[derive(Clone, Copy)]
struct LyricSegment {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

pub(crate) struct LyricLayout {
    layout: IDWriteTextLayout,
    height: f32,
    segments: Vec<LyricSegment>,
}

#[derive(Default)]
pub(crate) struct LyricLayoutCache {
    width_bits: u32,
    entries: HashMap<String, Rc<LyricLayout>>,
}

struct LyricRow {
    index: usize,
    layout: Option<Rc<LyricLayout>>,
    size: f32,
    height: f32,
}

pub(crate) fn lyric_level(distance: f32) -> f32 {
    const STOPS: [f32; 4] = [1.0, 0.38, 0.0, 0.0];
    let clamped = distance.clamp(0.0, 3.0);
    let lower = clamped.floor() as usize;
    let upper = (lower + 1).min(3);
    let u = clamped - lower as f32;
    let smooth = u * u * (3.0 - 2.0 * u);
    STOPS[lower] + (STOPS[upper] - STOPS[lower]) * smooth
}

pub(crate) fn lyric_blur_radius(distance: f32, dpi: u32) -> f32 {
    let spread = (distance / LYRIC_BLUR_RANGE).clamp(0.0, 1.0);
    LYRIC_BLUR_MAX_PX * (dpi as f32 / 96.0) * spread
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LyricFit {
    Single,
    Shrink(f32),
    Wrap,
}

pub(crate) fn lyric_fit(natural_width: f32, available: f32) -> LyricFit {
    if natural_width <= available {
        LyricFit::Single
    } else if natural_width * LYRIC_MIN_FIT <= available {
        LyricFit::Shrink(available / natural_width)
    } else {
        LyricFit::Wrap
    }
}

pub(crate) fn lyric_wipe_bands(filled: f32, feather: f32) -> Vec<(f32, f32, f32)> {
    let solid_end = (filled - feather).max(0.0);
    let mut bands = vec![(0.0, solid_end, 1.0)];
    let step = feather / LYRIC_WIPE_STEPS as f32;
    for k in 0..LYRIC_WIPE_STEPS {
        let left = (solid_end + step * k as f32).min(filled);
        let right = (solid_end + step * (k + 1) as f32).min(filled);
        let weight = 1.0 - (k + 1) as f32 / (LYRIC_WIPE_STEPS + 1) as f32;
        bands.push((left, right, weight));
    }
    bands.retain(|(left, right, _)| right - left > 0.01);
    bands
}

pub(crate) fn lyric_row_size(distance: f32) -> f32 {
    1.0 - (1.0 - LYRIC_SMALL_SCALE) * distance.clamp(0.0, 1.0)
}

pub(crate) fn lyric_scroll_origin(centers: &[f32], scroll: f32, first: usize) -> f32 {
    let lower = (scroll.floor().max(0.0) as usize).saturating_sub(first);
    let lower = lower.min(centers.len().saturating_sub(1));
    let upper = (lower + 1).min(centers.len().saturating_sub(1));
    let fraction = scroll - scroll.floor();
    centers
        .get(lower)
        .zip(centers.get(upper))
        .map_or(0.0, |(a, b)| a + (b - a) * fraction)
}

pub(crate) fn lyric_fill_widths(widths: &[f32], progress: f32) -> Vec<f32> {
    let mut remaining = widths.iter().sum::<f32>() * progress.clamp(0.0, 1.0);
    widths
        .iter()
        .map(|width| {
            let filled = remaining.clamp(0.0, *width);
            remaining -= filled;
            filled
        })
        .collect()
}

impl Renderer {
    pub(super) fn draw_lyrics_drawer(
        &self,
        drawer_rect: Rect,
        alpha: f32,
        media: &NowPlaying,
        lyrics: LyricsView<'_>,
        anim_phase: f32,
    ) {
        self.draw_lyrics_card(drawer_rect, alpha);
        if let Some(sheet) = lyrics.sheet.filter(|s| !s.is_empty()) {
            self.draw_lyric_lines(
                drawer_rect,
                alpha,
                sheet,
                media.position_ms,
                lyrics.animated,
            );
            return;
        }
        let pad = scale(LYRIC_PAD_PT, self.dpi) as f32;
        let empty_rect = D2D_RECT_F {
            left: drawer_rect.x as f32 + pad,
            top: drawer_rect.y as f32,
            right: drawer_rect.right() as f32 - pad,
            bottom: drawer_rect.bottom() as f32,
        };
        if media.title.trim().is_empty() {
            let empty_brush = &self.brushes.alpha_brush(FOREGROUND, 0.45 * alpha);
            self.draw_text(
                "Nothing playing",
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

    fn draw_lyrics_card(&self, drawer_rect: Rect, alpha: f32) {
        if drawer_rect.is_empty() {
            return;
        }
        let radius = scale(LYRIC_CARD_RADIUS_PT, self.dpi) as f32;
        let card = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: drawer_rect.x as f32 + 0.5,
                top: drawer_rect.y as f32 + 0.5,
                right: drawer_rect.right() as f32 - 0.5,
                bottom: drawer_rect.bottom() as f32 - 0.5,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let fill = self.brushes.alpha_brush(FOREGROUND, 0.05 * alpha);
        let frame = self.brushes.alpha_brush(FOREGROUND, 0.10 * alpha);
        unsafe {
            self.target.FillRoundedRectangle(&card, &fill);
            self.target.DrawRoundedRectangle(&card, &frame, 1.0, None);
        }
    }

    fn draw_lyric_lines(
        &self,
        drawer_rect: Rect,
        alpha: f32,
        sheet: &LyricsSheet,
        position_ms: u64,
        animated: bool,
    ) {
        let active = sheet.active_line_index(position_ms);
        let scroll = if animated {
            sheet.scroll_position(position_ms)
        } else {
            active.unwrap_or(0) as f32
        };
        let progress = if animated {
            sheet.line_progress(position_ms)
        } else {
            1.0
        };
        let pad = scale(LYRIC_PAD_PT, self.dpi) as f32;
        let gap = scale(LYRIC_ROW_GAP_PT, self.dpi) as f32;
        let fade = scale(LYRIC_EDGE_FADE_PT, self.dpi) as f32;
        let top = drawer_rect.y as f32;
        let bottom = drawer_rect.bottom() as f32;
        let width = (drawer_rect.width as f32 - pad * 2.0).max(1.0);
        let left = drawer_rect.x as f32 + pad;
        let anchor_y = top + drawer_rect.height as f32 * LYRIC_ANCHOR_RATIO;

        let first = (scroll.floor() as isize - 4).max(0) as usize;
        let last =
            ((scroll.ceil() as isize + 4).max(0) as usize).min(sheet.len().saturating_sub(1));

        let mut rows = Vec::new();
        for index in first..=last {
            let text = sheet.lines[index].text.as_str();
            let layout = if text.trim().is_empty() {
                None
            } else {
                self.lyric_layout(text, width)
            };
            let size = lyric_row_size((index as f32 - scroll).abs());
            let height = layout.as_ref().map_or(0.0, |row| row.height * size);
            rows.push(LyricRow {
                index,
                layout,
                size,
                height,
            });
        }

        let mut centers = Vec::with_capacity(rows.len());
        let mut cursor = 0.0;
        for row in &rows {
            centers.push(cursor + row.height * 0.5);
            cursor += row.height + if row.height > 0.0 { gap } else { 0.0 };
        }
        let origin = lyric_scroll_origin(&centers, scroll, first);

        self.push_clip(D2D_RECT_F {
            left: drawer_rect.x as f32,
            top,
            right: drawer_rect.right() as f32,
            bottom,
        });
        for (row, center) in rows.iter().zip(&centers) {
            let Some(layout) = row.layout.as_ref() else {
                continue;
            };
            let distance = (row.index as f32 - scroll).abs();
            let center_y = anchor_y + center - origin;
            let edge = ((center_y - top) / fade)
                .min((bottom - center_y) / fade)
                .clamp(0.0, 1.0);
            let level = alpha * lyric_level(distance) * edge;
            if level < 0.01 {
                continue;
            }
            self.set_content_scale(row.size, left, center_y);
            let text_origin = Vector2::new(left, center_y - layout.height * 0.5);
            let is_active = Some(row.index) == active;
            if is_active && (!animated || distance <= LYRIC_ACTIVE_BLUR_THRESHOLD) {
                let row_progress = if animated { progress } else { 1.0 };
                self.draw_lyric_with_fill(layout, text_origin, level, row_progress);
            } else {
                let soft_level = if !is_active && active.is_some_and(|idx| row.index > idx) {
                    level.min(alpha * LYRIC_ACTIVE_BASE_ALPHA * edge)
                } else {
                    level
                };
                let radius = lyric_blur_radius(distance, self.dpi);
                self.draw_lyric_soft(layout, text_origin, soft_level, radius);
            }
        }
        self.set_content_scale(1.0, 0.0, 0.0);
        self.pop_clip();
    }

    fn draw_lyric_with_fill(&self, row: &LyricLayout, origin: Vector2, level: f32, progress: f32) {
        let dim = &self
            .brushes
            .alpha_brush(FOREGROUND, LYRIC_ACTIVE_BASE_ALPHA * level);
        self.draw_lyric_layout(row, origin, dim);
        let widths: Vec<f32> = row.segments.iter().map(|segment| segment.width).collect();
        let feather = scale(LYRIC_WIPE_FEATHER_PT, self.dpi) as f32;
        for (segment, filled) in row
            .segments
            .iter()
            .zip(lyric_fill_widths(&widths, progress))
        {
            if filled <= 0.0 {
                continue;
            }
            for (from, to, weight) in lyric_wipe_bands(filled, feather) {
                self.push_clip(D2D_RECT_F {
                    left: origin.X + segment.left + from,
                    top: origin.Y + segment.top,
                    right: origin.X + segment.left + to,
                    bottom: origin.Y + segment.top + segment.height,
                });
                let band = &self.brushes.alpha_brush(FOREGROUND, 0.98 * level * weight);
                self.draw_lyric_layout(row, origin, band);
                self.pop_clip();
            }
        }
    }

    fn draw_lyric_soft(&self, row: &LyricLayout, origin: Vector2, level: f32, radius: f32) {
        if radius < LYRIC_BLUR_MIN_PX {
            let sharp = &self.brushes.alpha_brush(FOREGROUND, level);
            self.draw_lyric_layout(row, origin, sharp);
            return;
        }
        let core = &self.brushes.alpha_brush(FOREGROUND, 0.28 * level);
        self.draw_lyric_layout(row, origin, core);
        let inner = &self.brushes.alpha_brush(FOREGROUND, 0.055 * level);
        let outer = &self.brushes.alpha_brush(FOREGROUND, 0.035 * level);
        let inner_r = radius * LYRIC_BLUR_INNER_SCALE;
        for (dx, dy) in LYRIC_BLUR_RING {
            self.draw_lyric_layout(
                row,
                Vector2::new(origin.X + dx * inner_r, origin.Y + dy * inner_r),
                inner,
            );
            self.draw_lyric_layout(
                row,
                Vector2::new(origin.X + dx * radius, origin.Y + dy * radius),
                outer,
            );
        }
    }

    fn draw_lyric_layout(
        &self,
        row: &LyricLayout,
        origin: Vector2,
        brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    ) {
        unsafe {
            self.target
                .DrawTextLayout(origin, &row.layout, brush, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP);
        }
    }

    fn lyric_layout(&self, text: &str, width: f32) -> Option<Rc<LyricLayout>> {
        let mut cache = self.lyric_layouts.borrow_mut();
        if cache.width_bits != width.to_bits() || cache.entries.len() >= LYRIC_CACHE_LIMIT {
            cache.entries.clear();
            cache.width_bits = width.to_bits();
        }
        if let Some(found) = cache.entries.get(text) {
            return Some(Rc::clone(found));
        }
        let built = Rc::new(self.build_lyric_layout(text, width)?);
        cache.entries.insert(text.to_owned(), Rc::clone(&built));
        Some(built)
    }

    fn build_lyric_layout(&self, text: &str, width: f32) -> Option<LyricLayout> {
        let dwrite = shared_dwrite_factory().ok()?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let font_px = scale(LYRIC_FONT_PT, self.dpi) as f32;
        unsafe {
            let mut probe_metrics = DWRITE_TEXT_METRICS::default();
            dwrite
                .CreateTextLayout(&wide, &self.text_lyric, 4096.0, 4096.0)
                .ok()?
                .GetMetrics(&mut probe_metrics)
                .ok()?;
            let fit = lyric_fit(probe_metrics.width, width);
            let wrapped = fit == LyricFit::Wrap;
            let max_height = if wrapped {
                font_px * LYRIC_LINE_HEIGHT_RATIO * LYRIC_MAX_LINES
            } else {
                font_px * LYRIC_LINE_HEIGHT_RATIO
            };
            let layout = dwrite
                .CreateTextLayout(&wide, &self.text_lyric, width, max_height)
                .ok()?;
            if let LyricFit::Shrink(factor) = fit {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: wide.len() as u32,
                };
                layout.SetFontSize(font_px * factor, range).ok()?;
            }
            layout
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)
                .ok()?;
            layout
                .SetWordWrapping(if wrapped {
                    DWRITE_WORD_WRAPPING_WRAP
                } else {
                    DWRITE_WORD_WRAPPING_NO_WRAP
                })
                .ok()?;
            layout
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)
                .ok()?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            let segments = lyric_segments(&layout, &metrics).unwrap_or_else(|| {
                vec![LyricSegment {
                    left: metrics.left,
                    top: metrics.top,
                    width: metrics.width,
                    height: metrics.height,
                }]
            });
            Some(LyricLayout {
                layout,
                height: metrics.height,
                segments,
            })
        }
    }
}

unsafe fn lyric_segments(
    layout: &IDWriteTextLayout,
    metrics: &DWRITE_TEXT_METRICS,
) -> Option<Vec<LyricSegment>> {
    unsafe {
        let mut count = 0u32;
        let _ = layout.GetLineMetrics(None, &mut count);
        if count == 0 || count > metrics.lineCount.max(1) {
            return None;
        }
        let mut lines = vec![DWRITE_LINE_METRICS::default(); count as usize];
        layout.GetLineMetrics(Some(&mut lines), &mut count).ok()?;
        let mut segments = Vec::with_capacity(lines.len());
        let mut start = 0u32;
        for line in &lines {
            let visible = line.length.saturating_sub(line.trailingWhitespaceLength);
            if visible > 0 {
                let mut x = 0.0;
                let mut y = 0.0;
                let mut first = DWRITE_HIT_TEST_METRICS::default();
                layout
                    .HitTestTextPosition(start, false, &mut x, &mut y, &mut first)
                    .ok()?;
                let mut last = DWRITE_HIT_TEST_METRICS::default();
                layout
                    .HitTestTextPosition(start + visible - 1, false, &mut x, &mut y, &mut last)
                    .ok()?;
                segments.push(LyricSegment {
                    left: first.left,
                    top: first.top,
                    width: (last.left + last.width - first.left).max(0.0),
                    height: first.height,
                });
            }
            start += line.length;
        }
        (!segments.is_empty()).then_some(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LyricFit, lyric_blur_radius, lyric_fill_widths, lyric_fit, lyric_level, lyric_row_size,
        lyric_scroll_origin, lyric_wipe_bands,
    };

    #[test]
    fn lyric_level_fades_monotonically_from_the_active_line_outward() {
        assert!((lyric_level(0.0) - 1.0).abs() < f32::EPSILON);
        let mut previous = 1.0;
        for step in 1..=40 {
            let level = lyric_level(step as f32 * 0.1);
            assert!(level <= previous + f32::EPSILON);
            previous = level;
        }
        assert!(lyric_level(1.0) > 0.0);
        assert!(lyric_level(2.0).abs() < f32::EPSILON);
        assert!(lyric_level(9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_active_lyric_line_is_never_blurred_and_blur_grows_with_distance() {
        assert!(lyric_blur_radius(0.0, 96).abs() < f32::EPSILON);
        let mut previous = 0.0;
        for step in 1..=30 {
            let radius = lyric_blur_radius(step as f32 * 0.1, 96);
            assert!(radius + f32::EPSILON >= previous);
            previous = radius;
        }
        assert!((lyric_blur_radius(2.0, 96) - lyric_blur_radius(9.0, 96)).abs() < f32::EPSILON);
    }

    #[test]
    fn lyric_blur_scales_with_display_dpi() {
        let at_96 = lyric_blur_radius(1.0, 96);
        let at_192 = lyric_blur_radius(1.0, 192);
        assert!((at_192 - at_96 * 2.0).abs() < 0.001);
    }

    #[test]
    fn rows_shrink_from_full_size_at_the_active_line_to_the_neighbour_size() {
        assert!((lyric_row_size(0.0) - 1.0).abs() < f32::EPSILON);
        assert!(lyric_row_size(0.5) < 1.0 && lyric_row_size(0.5) > lyric_row_size(1.0));
        assert!((lyric_row_size(5.0) - lyric_row_size(1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn the_scroll_origin_keeps_the_active_row_on_the_anchor_even_when_rows_differ_in_height() {
        let centers = [20.0, 70.0, 130.0];
        assert!((lyric_scroll_origin(&centers, 1.0, 0) - 70.0).abs() < f32::EPSILON);
        assert!((lyric_scroll_origin(&centers, 1.5, 0) - 100.0).abs() < f32::EPSILON);
        assert!((lyric_scroll_origin(&centers, 3.0, 1) - 130.0).abs() < f32::EPSILON);
        assert!(lyric_scroll_origin(&[], 0.0, 0).abs() < f32::EPSILON);
    }

    #[test]
    fn karaoke_fill_runs_through_wrapped_lines_in_order() {
        let widths = [100.0, 50.0];
        assert_eq!(lyric_fill_widths(&widths, 0.0), vec![0.0, 0.0]);
        let half = lyric_fill_widths(&widths, 0.5);
        assert!((half[0] - 75.0).abs() < 0.001 && half[1].abs() < 0.001);
        let most = lyric_fill_widths(&widths, 0.9);
        assert!((most[0] - 100.0).abs() < 0.001 && (most[1] - 35.0).abs() < 0.001);
        assert_eq!(lyric_fill_widths(&widths, 1.0), vec![100.0, 50.0]);
    }

    #[test]
    fn a_line_that_fits_stays_single_a_slightly_long_one_shrinks_and_only_a_long_one_wraps() {
        assert_eq!(lyric_fit(300.0, 336.0), LyricFit::Single);
        assert_eq!(lyric_fit(336.0, 336.0), LyricFit::Single);
        match lyric_fit(370.0, 336.0) {
            LyricFit::Shrink(factor) => assert!((factor - 336.0 / 370.0).abs() < 0.001),
            other => panic!("expected a shrink, got {other:?}"),
        }
        assert_eq!(lyric_fit(420.0, 336.0), LyricFit::Wrap);
    }

    #[test]
    fn shrinking_never_goes_below_the_minimum_fit() {
        for natural in (340..600).step_by(5) {
            if let LyricFit::Shrink(factor) = lyric_fit(natural as f32, 336.0) {
                assert!(factor >= 0.86 - 0.001);
            }
        }
    }

    #[test]
    fn the_wipe_edge_is_feathered_and_covers_exactly_the_filled_width() {
        let bands = lyric_wipe_bands(100.0, 12.0);
        assert!((bands[0].0).abs() < f32::EPSILON && (bands[0].1 - 88.0).abs() < 0.001);
        assert!((bands.last().map_or(0.0, |band| band.1) - 100.0).abs() < 0.001);
        assert!(bands.windows(2).all(|pair| pair[1].2 < pair[0].2));
        assert!(lyric_wipe_bands(0.0, 12.0).is_empty());
        let short = lyric_wipe_bands(6.0, 12.0);
        assert!((short.last().map_or(0.0, |band| band.1) - 6.0).abs() < 0.001);
    }
}
