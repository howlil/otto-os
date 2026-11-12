use crate::Rect;

const STRIP_COUNT: usize = 128;
const BOUNDS_PADDING: i32 = 2;
const ENVELOPE_SAMPLES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenieDirection {
    Minimize,
    Restore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenieGeometry {
    pub strips: Vec<GenieStrip>,
}

impl GenieGeometry {
    #[must_use]
    pub const fn empty() -> Self {
        Self { strips: Vec::new() }
    }

    #[must_use]
    pub fn destination_bounds(&self) -> Option<Rect> {
        let covered = self.covered_bounds()?;
        Some(Rect {
            x: covered.x - BOUNDS_PADDING,
            y: covered.y - BOUNDS_PADDING,
            width: covered.width + 2 * BOUNDS_PADDING,
            height: covered.height + 2 * BOUNDS_PADDING,
        })
    }

    fn covered_bounds(&self) -> Option<Rect> {
        let first = self.strips.first()?;
        let mut left = first.destination_left;
        let mut top = first.destination_top;
        let mut right = first.destination_left + first.destination_width;
        let mut bottom = first.destination_top + first.destination_height;

        for strip in self.strips.iter().skip(1) {
            left = left.min(strip.destination_left);
            top = top.min(strip.destination_top);
            right = right.max(strip.destination_left + strip.destination_width);
            bottom = bottom.max(strip.destination_top + strip.destination_height);
        }

        let x = left.floor() as i32;
        let y = top.floor() as i32;
        let right = right.ceil() as i32;
        let bottom = bottom.ceil() as i32;
        Some(Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenieStrip {
    pub source_top: f32,
    pub source_height: f32,
    pub destination_left: f32,
    pub destination_top: f32,
    pub destination_width: f32,
    pub destination_height: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy)]
struct WarpBoundary {
    y: f32,
    left: f32,
    right: f32,
}

#[must_use]
pub fn compute_genie_geometry(
    window: Rect,
    dock: Rect,
    transition_progress: f32,
    direction: GenieDirection,
) -> GenieGeometry {
    if window.width <= 0 || window.height <= 0 || dock.width <= 0 || dock.height <= 0 {
        return GenieGeometry::empty();
    }

    let transition_progress = normalized_transition(transition_progress);
    let warp_progress = match direction {
        GenieDirection::Minimize => transition_progress,
        GenieDirection::Restore => 1.0 - transition_progress,
    };
    compute_warp_geometry(window, dock, warp_progress)
}

fn compute_warp_geometry(source: Rect, target: Rect, progress: f32) -> GenieGeometry {
    let source_width = source.width as f32;
    let source_height = source.height as f32;
    let target_width = target.width as f32;
    let target_height = target.height as f32;
    let source_center_x = source.x as f32 + source_width * 0.5;
    let target_center_x = target.x as f32 + target_width * 0.5;
    let source_center_y = source.y as f32 + source_height * 0.5;
    let target_center_y = target.y as f32 + target_height * 0.5;
    let target_is_below = target_center_y >= source_center_y;
    let opacity = 1.0 - smooth_step(((progress - 0.90) / 0.10).clamp(0.0, 1.0));
    let boundaries = (0..=STRIP_COUNT)
        .map(|index| {
            let position = index as f32 / STRIP_COUNT as f32;
            compute_boundary(
                source,
                target,
                source_width,
                target_width,
                source_height,
                target_height,
                source_center_x,
                target_center_x,
                target_is_below,
                position,
                progress,
            )
        })
        .collect::<Vec<_>>();

    let mut strips = Vec::with_capacity(STRIP_COUNT);
    for (index, pair) in boundaries.windows(2).enumerate() {
        let top = pair[0];
        let bottom = pair[1];
        let destination_left = top.left.min(bottom.left);
        let destination_right = top.right.max(bottom.right);
        let source_top = source_height * index as f32 / STRIP_COUNT as f32;
        let source_bottom = source_height * (index + 1) as f32 / STRIP_COUNT as f32;
        strips.push(GenieStrip {
            source_top,
            source_height: source_bottom - source_top,
            destination_left,
            destination_top: top.y,
            destination_width: (destination_right - destination_left).max(1.0),
            destination_height: (bottom.y - top.y).max(0.5),
            opacity,
        });
    }

    GenieGeometry { strips }
}

#[allow(clippy::too_many_arguments)]
fn compute_boundary(
    source: Rect,
    target: Rect,
    source_width: f32,
    target_width: f32,
    source_height: f32,
    target_height: f32,
    source_center_x: f32,
    target_center_x: f32,
    target_is_below: bool,
    position: f32,
    progress: f32,
) -> WarpBoundary {
    let near_target = if target_is_below {
        position
    } else {
        1.0 - position
    };
    let delay = (1.0 - near_target).powf(1.8) * 0.40;
    let local_progress = normalized_progress(progress, delay);
    let eased = smooth_step(local_progress);

    let source_y = source.y as f32 + source_height * position;
    let target_y = target.y as f32 + target_height * position;
    let y = lerp(source_y, target_y, eased);

    let total_y_dist = (target_y - source_y).abs().max(1.0);
    let v = ((y - source_y).abs() / total_y_dist).clamp(0.0, 1.0);

    let s_curve = v * v * (3.0 - 2.0 * v);
    let mut center_x = lerp(source_center_x, target_center_x, s_curve);
    let bow = ((1.0 - v) * v * 4.0)
        * (target_center_x - source_center_x)
        * 0.14
        * (progress * std::f32::consts::PI).sin();
    center_x += bow;

    let trailing_progress = normalized_progress(progress, 0.40);
    let top_width = lerp(source_width, target_width, smooth_step(trailing_progress));
    let funnel = (1.0 - v).powf(1.6);
    let base_width = lerp(target_width, top_width, funnel);
    let pinch = ((1.0 - v) * v * 4.0) * 0.16 * (progress * std::f32::consts::PI).sin();
    let min_allowed = target_width.min(source_width);
    let max_allowed = source_width.max(target_width);
    let width = (base_width * (1.0 - pinch)).clamp(min_allowed, max_allowed);

    WarpBoundary {
        y,
        left: center_x - width * 0.5,
        right: center_x + width * 0.5,
    }
}

fn normalized_progress(progress: f32, delay: f32) -> f32 {
    if progress <= delay {
        0.0
    } else {
        ((progress - delay) / (1.0 - delay)).clamp(0.0, 1.0)
    }
}

fn normalized_transition(progress: f32) -> f32 {
    if progress.is_nan() {
        0.0
    } else {
        progress.clamp(0.0, 1.0)
    }
}

fn smooth_step(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn lerp(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress
}

#[must_use]
pub fn is_blank_frame(width: u32, height: u32, pixels: &[u8]) -> bool {
    const X_RATIOS: [f32; 5] = [0.15, 0.30, 0.50, 0.70, 0.85];
    const Y_RATIOS: [f32; 5] = [0.20, 0.35, 0.50, 0.65, 0.80];

    let total_width = width as usize;
    let total_height = height as usize;
    if width < 10
        || height < 10
        || pixels.len() < total_width.saturating_mul(total_height).saturating_mul(4)
    {
        return true;
    }
    let width_f = width as f32;
    let height_f = height as f32;
    let stride = total_width * 4;

    for &ratio_y in &Y_RATIOS {
        let sample_y = (height_f * ratio_y) as usize;
        let row_offset = sample_y * stride;
        for &ratio_x in &X_RATIOS {
            let sample_x = (width_f * ratio_x) as usize;
            let offset = row_offset + sample_x * 4;
            if offset + 3 < pixels.len() {
                let blue = pixels[offset];
                let green = pixels[offset + 1];
                let red = pixels[offset + 2];
                if red >= 3 || green >= 3 || blue >= 3 {
                    return false;
                }
            }
        }
    }
    true
}

#[must_use]
pub fn genie_canvas(window: Rect, dock: Rect, virtual_screen: Rect) -> Option<Rect> {
    if window.is_empty() || dock.is_empty() {
        return None;
    }
    warp_envelope(window, dock).intersection(&virtual_screen)
}

fn warp_envelope(window: Rect, dock: Rect) -> Rect {
    (0..=ENVELOPE_SAMPLES)
        .map(|step| compute_warp_geometry(window, dock, step as f32 / ENVELOPE_SAMPLES as f32))
        .filter_map(|geometry| geometry.covered_bounds())
        .fold(window.union(&dock), |envelope, bounds| {
            envelope.union(&bounds)
        })
}

#[cfg(test)]
mod canvas_tests {
    use super::{ENVELOPE_SAMPLES, GenieDirection, compute_genie_geometry, genie_canvas};
    use crate::layout::Rect;

    const fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    const VIRTUAL: Rect = rect(-1920, -400, 1920 + 2560, 1440 + 400);

    #[test]
    fn canvas_spans_a_window_on_a_left_monitor_and_the_primary_dock() {
        let window = rect(-1700, 120, 900, 700);
        let dock = rect(1200, 1340, 64, 64);
        let canvas = genie_canvas(window, dock, VIRTUAL).unwrap();
        assert!(canvas.contains_rect(&window));
        assert!(canvas.contains_rect(&dock));
        assert_eq!(canvas.x, -1700);
    }

    #[test]
    fn canvas_covers_a_monitor_above_the_primary() {
        let window = rect(200, -380, 800, 300);
        let dock = rect(900, 1340, 64, 64);
        let canvas = genie_canvas(window, dock, VIRTUAL).unwrap();
        assert_eq!(canvas.y, -380);
        assert_eq!(canvas.bottom(), 1404);
    }

    #[test]
    fn canvas_is_clipped_to_the_virtual_screen_after_a_monitor_is_removed() {
        let window = rect(-1700, 120, 900, 700);
        let dock = rect(1200, 1340, 64, 64);
        let primary_only = rect(0, 0, 2560, 1440);
        let canvas = genie_canvas(window, dock, primary_only).unwrap();
        assert!(primary_only.contains_rect(&canvas));
        assert!(
            genie_canvas(
                rect(-1700, 120, 900, 700),
                rect(-900, 900, 64, 64),
                primary_only
            )
            .is_none()
        );
    }

    #[test]
    fn empty_window_or_dock_has_no_canvas() {
        assert!(genie_canvas(rect(0, 0, 0, 10), rect(0, 0, 10, 10), VIRTUAL).is_none());
        assert!(genie_canvas(rect(0, 0, 10, 10), rect(0, 0, 10, 0), VIRTUAL).is_none());
    }

    #[test]
    fn canvas_contains_every_warped_strip_in_both_directions() {
        let cases = [
            (rect(-1700, 120, 900, 700), rect(1200, 1340, 64, 64)),
            (rect(120, 80, 900, 640), rect(930, 900, 72, 72)),
            (rect(200, -380, 800, 300), rect(900, 1340, 64, 64)),
        ];
        for (window, dock) in cases {
            let canvas = genie_canvas(window, dock, VIRTUAL).unwrap();
            for direction in [GenieDirection::Minimize, GenieDirection::Restore] {
                for step in 0..=ENVELOPE_SAMPLES {
                    let progress = step as f32 / ENVELOPE_SAMPLES as f32;
                    let bounds = compute_genie_geometry(window, dock, progress, direction)
                        .covered_bounds()
                        .unwrap();
                    assert!(
                        canvas.contains_rect(&bounds),
                        "progress {progress} {direction:?} bounds {bounds:?} canvas {canvas:?}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: Rect = Rect {
        x: 120,
        y: 80,
        width: 900,
        height: 640,
    };
    const TARGET: Rect = Rect {
        x: 930,
        y: 900,
        width: 72,
        height: 72,
    };

    #[test]
    fn minimize_starts_as_source_window() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 0.0, GenieDirection::Minimize);
        assert_eq!(geometry.strips.len(), 128);
        let first = geometry.strips.first().expect("strips are present");
        assert!((first.destination_left - SOURCE.x as f32).abs() < 0.01);
        assert!((first.destination_top - SOURCE.y as f32).abs() < 0.01);
        assert!((first.destination_width - SOURCE.width as f32).abs() < 0.01);
    }

    #[test]
    fn minimize_finishes_at_target_without_extra_offset() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 1.0, GenieDirection::Minimize);
        let first = geometry.strips.first().expect("strips are present");
        let last = geometry.strips.last().expect("strips are present");
        assert!((first.destination_left - TARGET.x as f32).abs() < 0.01);
        assert!((first.destination_top - TARGET.y as f32).abs() < 0.01);
        assert!((last.destination_left - TARGET.x as f32).abs() < 0.01);
        assert!(
            (last.destination_top + last.destination_height - TARGET.bottom() as f32).abs() < 0.01
        );
    }

    #[test]
    fn restore_is_the_exact_inverse_of_minimize() {
        let minimize = compute_genie_geometry(SOURCE, TARGET, 0.37, GenieDirection::Minimize);
        let restore = compute_genie_geometry(SOURCE, TARGET, 0.63, GenieDirection::Restore);
        assert_eq!(minimize, restore);
    }

    #[test]
    fn restore_samples_the_full_window_snapshot() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 0.0, GenieDirection::Restore);
        let last = geometry.strips.last().expect("strips are present");
        assert!((last.source_top + last.source_height - SOURCE.height as f32).abs() < 0.01);
    }

    #[test]
    fn leading_edge_moves_before_trailing_edge() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 0.35, GenieDirection::Minimize);
        let top = geometry.strips.first().expect("strips are present");
        let bottom = geometry.strips.last().expect("strips are present");
        let top_distance = (top.destination_top - SOURCE.y as f32).abs();
        let bottom_distance = (bottom.destination_top - SOURCE.y as f32).abs();
        assert!(bottom_distance > top_distance);
    }

    #[test]
    fn adjacent_strips_share_vertical_boundaries() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 0.5, GenieDirection::Minimize);
        for pair in geometry.strips.windows(2) {
            let previous_bottom = pair[0].destination_top + pair[0].destination_height;
            assert!((previous_bottom - pair[1].destination_top).abs() < 0.01);
        }
    }

    #[test]
    fn bounds_track_only_the_warped_surface() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 0.5, GenieDirection::Minimize);
        let bounds = geometry.destination_bounds().expect("bounds are present");
        assert!(bounds.width < 1_500);
        assert!(bounds.height < 1_400);
        assert!(bounds.x > 0);
        assert!(bounds.y > 0);
    }

    #[test]
    fn non_finite_progress_starts_the_transition() {
        let start = compute_genie_geometry(SOURCE, TARGET, 0.0, GenieDirection::Minimize);
        let nan = compute_genie_geometry(SOURCE, TARGET, f32::NAN, GenieDirection::Minimize);
        assert_eq!(nan, start);
        assert!(nan.strips.iter().all(|strip| {
            [
                strip.source_top,
                strip.source_height,
                strip.destination_left,
                strip.destination_top,
                strip.destination_width,
                strip.destination_height,
                strip.opacity,
            ]
            .iter()
            .all(|value| value.is_finite())
        }));
    }

    #[test]
    fn progress_outside_the_unit_interval_is_clamped() {
        let below = compute_genie_geometry(SOURCE, TARGET, -3.0, GenieDirection::Minimize);
        let above = compute_genie_geometry(SOURCE, TARGET, 7.0, GenieDirection::Minimize);
        assert_eq!(
            below,
            compute_genie_geometry(SOURCE, TARGET, 0.0, GenieDirection::Minimize)
        );
        assert_eq!(
            above,
            compute_genie_geometry(SOURCE, TARGET, 1.0, GenieDirection::Minimize)
        );
    }

    #[test]
    fn strip_source_ranges_tile_a_short_snapshot_exactly() {
        let short = Rect {
            height: 50,
            ..SOURCE
        };
        let geometry = compute_genie_geometry(short, TARGET, 0.5, GenieDirection::Minimize);
        let mut cursor = 0.0_f32;
        for strip in &geometry.strips {
            assert!((strip.source_top - cursor).abs() < 0.001);
            assert!(strip.source_height > 0.0);
            cursor = strip.source_top + strip.source_height;
        }
        assert!((cursor - short.height as f32).abs() < 0.001);
    }

    #[test]
    fn invalid_rects_produce_no_geometry() {
        let invalid = Rect { width: 0, ..SOURCE };
        assert!(
            compute_genie_geometry(invalid, TARGET, 0.5, GenieDirection::Minimize)
                .strips
                .is_empty()
        );
    }

    #[test]
    fn is_blank_frame_detects_all_black_buffer() {
        let pixels = vec![0u8; 100 * 100 * 4];
        assert!(is_blank_frame(100, 100, &pixels));
    }

    #[test]
    fn is_blank_frame_detects_rendered_window() {
        let mut pixels = vec![0u8; 100 * 100 * 4];
        let offset = (50 * 100 + 50) * 4;
        pixels[offset] = 200;
        pixels[offset + 1] = 150;
        pixels[offset + 2] = 100;
        assert!(!is_blank_frame(100, 100, &pixels));
    }

    #[test]
    fn is_blank_frame_detects_chromium_titlebar_only_black_box() {
        let mut pixels = vec![0u8; 100 * 100 * 4];
        for y in 0..15 {
            for x in 0..100 {
                let offset = (y * 100 + x) * 4;
                pixels[offset] = 220;
                pixels[offset + 1] = 220;
                pixels[offset + 2] = 220;
            }
        }
        assert!(is_blank_frame(100, 100, &pixels));
    }

    #[test]
    fn is_blank_frame_handles_small_or_invalid_buffers() {
        assert!(is_blank_frame(0, 0, &[]));
        assert!(is_blank_frame(5, 5, &[255u8; 100]));
        assert!(is_blank_frame(100, 100, &[255u8; 10]));
    }
}
