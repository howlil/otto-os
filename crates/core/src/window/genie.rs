use std::f32::consts::PI;

use crate::Rect;

const STRIP_COUNT: usize = 128;
const BOUNDS_PADDING: i32 = 4;
const ENVELOPE_SAMPLES: usize = 200;
const PHASE_ONE_SPLIT: f32 = 0.36;
const FUNNEL_CURVE_POWER: f32 = 1.12;
const DOCK_SINK_RATIO: f32 = 0.35;
const MIN_CHUTE_RATIO: f32 = 0.45;
const END_CHUTE_RATIO: f32 = 0.65;
const COLLAPSE_RAIL_THRESHOLD: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenieDirection {
    Minimize,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenieBoundary {
    pub y: f32,
    pub left: f32,
    pub right: f32,
    pub v: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenieClip {
    pub edge_y: f32,
    pub sink_y: f32,
    pub clip_below: bool,
    pub strength: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenieGeometry {
    pub strips: Vec<GenieStrip>,
    pub boundaries: Vec<GenieBoundary>,
    pub clip: GenieClip,
    pub opacity: f32,
}

impl GenieGeometry {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            strips: Vec::new(),
            boundaries: Vec::new(),
            clip: GenieClip {
                edge_y: 0.0,
                sink_y: 0.0,
                clip_below: true,
                strength: 0.0,
            },
            opacity: 0.0,
        }
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
        let first = self.boundaries.first()?;
        let mut left = first.left;
        let mut right = first.right;
        let mut top = self.clamp_visible_y(first.y);
        let mut bottom = top;

        for boundary in self.boundaries.iter().skip(1) {
            left = left.min(boundary.left);
            right = right.max(boundary.right);
            let y = self.clamp_visible_y(boundary.y);
            top = top.min(y);
            bottom = bottom.max(y);
        }

        let x = left.floor() as i32;
        let y = top.floor() as i32;
        let right = right.ceil() as i32;
        let bottom = bottom.ceil() as i32;
        Some(Rect {
            x,
            y,
            width: (right - x).max(1),
            height: (bottom - y).max(1),
        })
    }

    fn clamp_visible_y(&self, y: f32) -> f32 {
        if self.clip.strength <= 0.0 {
            return y;
        }
        let clamped = if self.clip.clip_below {
            y.min(self.clip.sink_y)
        } else {
            y.max(self.clip.sink_y)
        };
        lerp(y, clamped, self.clip.strength)
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
    let source_height = source.height as f32;
    let target_height = target.height as f32;
    let source_center_y = source.y as f32 + source_height * 0.5;
    let target_center_y = target.y as f32 + target_height * 0.5;
    let target_is_below = target_center_y >= source_center_y;
    let dir = if target_is_below { 1.0_f32 } else { -1.0_f32 };

    let w_trail = if target_is_below {
        source.y as f32
    } else {
        (source.y + source.height) as f32
    };
    let w_lead = if target_is_below {
        (source.y + source.height) as f32
    } else {
        source.y as f32
    };
    let d_entry = if target_is_below {
        target.y as f32
    } else {
        (target.y + target.height) as f32
    };
    let d_sink = d_entry + dir * (target_height * DOCK_SINK_RATIO).max(4.0);
    let entry_distance = (d_entry - w_trail) * dir;
    let rail_len = if entry_distance > COLLAPSE_RAIL_THRESHOLD {
        entry_distance
    } else {
        source_height.max(1.0)
    };

    let (trail_y, lead_y, funnel, clip_strength, collapse) = if progress <= PHASE_ONE_SPLIT {
        let t1 = (progress / PHASE_ONE_SPLIT).clamp(0.0, 1.0);
        let e_drop = 1.0 - (1.0 - t1) * (1.0 - t1);
        let e_funnel = smooth_step(t1);
        let clip_ramp = smooth_step(((t1 - 0.55) / 0.45).clamp(0.0, 1.0));
        (
            w_trail,
            lerp(w_lead, d_entry, e_drop),
            e_funnel,
            clip_ramp,
            0.0,
        )
    } else {
        let t2 = ((progress - PHASE_ONE_SPLIT) / (1.0 - PHASE_ONE_SPLIT)).clamp(0.0, 1.0);
        let e_slide = t2 * t2 * (2.2 - 1.2 * t2);
        let chute_span = entry_distance.max(source_height * MIN_CHUTE_RATIO);
        let trail_y = lerp(w_trail, d_sink, e_slide);
        let lead_y = trail_y + dir * lerp(chute_span, source_height * END_CHUTE_RATIO, e_slide);
        let collapse = if entry_distance <= COLLAPSE_RAIL_THRESHOLD {
            smooth_step(t2)
        } else {
            0.0
        };
        (trail_y, lead_y, 1.0, 1.0, collapse)
    };

    let opacity = 1.0 - smooth_step(((progress - 0.96) / 0.04).clamp(0.0, 1.0));
    let source_left = source.x as f32;
    let source_right = (source.x + source.width) as f32;
    let target_left = target.x as f32;
    let target_right = (target.x + target.width) as f32;

    let mut boundaries = Vec::with_capacity(STRIP_COUNT + 1);
    for index in 0..=STRIP_COUNT {
        let v = index as f32 / STRIP_COUNT as f32;
        let s = if target_is_below { v } else { 1.0 - v };
        let y = lerp(trail_y, lead_y, s);
        let u_rail = (((y - w_trail) * dir) / rail_len).clamp(0.0, 1.0);
        let s_curve = 0.5 - 0.5 * (PI * u_rail.powf(FUNNEL_CURVE_POWER)).cos();
        let profile = s_curve.max(collapse);
        let left = lerp(source_left, lerp(source_left, target_left, profile), funnel);
        let right = lerp(
            source_right,
            lerp(source_right, target_right, profile),
            funnel,
        );
        boundaries.push(GenieBoundary { y, left, right, v });
    }

    let strips = build_strips(&boundaries, source_height, opacity);

    GenieGeometry {
        strips,
        boundaries,
        clip: GenieClip {
            edge_y: d_entry,
            sink_y: d_sink,
            clip_below: target_is_below,
            strength: clip_strength,
        },
        opacity,
    }
}

fn build_strips(boundaries: &[GenieBoundary], source_height: f32, opacity: f32) -> Vec<GenieStrip> {
    let mut strips = Vec::with_capacity(STRIP_COUNT);
    for (index, pair) in boundaries.windows(2).enumerate() {
        let top = pair[0];
        let bottom = pair[1];
        let destination_left = top.left.min(bottom.left);
        let destination_right = top.right.max(bottom.right);
        let destination_top = top.y.min(bottom.y);
        let destination_bottom = top.y.max(bottom.y);
        let source_top = source_height * index as f32 / STRIP_COUNT as f32;
        let source_bottom = source_height * (index + 1) as f32 / STRIP_COUNT as f32;
        strips.push(GenieStrip {
            source_top,
            source_height: source_bottom - source_top,
            destination_left,
            destination_top,
            destination_width: (destination_right - destination_left).max(1.0),
            destination_height: (destination_bottom - destination_top).max(0.5),
            opacity,
        });
    }
    strips
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

    const fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

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
    fn minimize_finishes_inside_dock_entry_aligned_to_target_span() {
        let geometry = compute_genie_geometry(SOURCE, TARGET, 1.0, GenieDirection::Minimize);
        let first = geometry.boundaries.first().expect("boundaries are present");
        let last = geometry.boundaries.last().expect("boundaries are present");
        assert!((first.left - TARGET.x as f32).abs() < 0.01);
        assert!((first.right - TARGET.right() as f32).abs() < 0.01);
        assert!(first.y >= TARGET.y as f32);
        assert!(first.y <= TARGET.bottom() as f32);
        assert!((last.left - TARGET.x as f32).abs() < 0.01);
        assert!((last.right - TARGET.right() as f32).abs() < 0.01);
        assert!(geometry.clip.strength >= 0.99);
        assert!(geometry.opacity <= 0.001);
    }

    #[test]
    fn phase_one_holds_trailing_edge_while_funneling_leading_edge_to_dock() {
        let geometry =
            compute_genie_geometry(SOURCE, TARGET, PHASE_ONE_SPLIT, GenieDirection::Minimize);
        let top = geometry.boundaries.first().expect("boundaries are present");
        let bottom = geometry.boundaries.last().expect("boundaries are present");
        let middle = &geometry.boundaries[geometry.boundaries.len() / 2];
        assert!((top.y - SOURCE.y as f32).abs() < 0.01);
        assert!((top.left - SOURCE.x as f32).abs() < 0.01);
        assert!((top.right - SOURCE.right() as f32).abs() < 0.01);
        assert!((bottom.y - TARGET.y as f32).abs() < 0.01);
        assert!((bottom.left - TARGET.x as f32).abs() < 0.01);
        assert!((bottom.right - TARGET.right() as f32).abs() < 0.01);
        assert!(middle.left > SOURCE.x as f32 && middle.left < TARGET.x as f32);
        assert!(middle.right < SOURCE.right() as f32 && middle.right > TARGET.right() as f32);
    }

    #[test]
    fn phase_two_slides_down_fixed_funnel_rails() {
        let funnel =
            compute_genie_geometry(SOURCE, TARGET, PHASE_ONE_SPLIT, GenieDirection::Minimize);
        let sliding = compute_genie_geometry(SOURCE, TARGET, 0.65, GenieDirection::Minimize);
        let slide_top = sliding.boundaries.first().expect("boundaries are present");
        assert!(slide_top.y > SOURCE.y as f32);
        let nearest = funnel
            .boundaries
            .iter()
            .min_by(|a, b| {
                (a.y - slide_top.y)
                    .abs()
                    .total_cmp(&(b.y - slide_top.y).abs())
            })
            .expect("funnel boundary exists");
        let rail_step = (TARGET.y - SOURCE.y) as f32 / STRIP_COUNT as f32;
        let expected_left_tolerance =
            ((TARGET.x - SOURCE.x) as f32 * rail_step / (TARGET.y - SOURCE.y) as f32).abs() * 2.5
                + 1.0;
        assert!((slide_top.left - nearest.left).abs() <= expected_left_tolerance);
    }

    #[test]
    fn maximized_window_overlapping_dock_starts_unclipped_at_zero() {
        let maximized = rect(0, 32, 1920, 1048);
        let dock = rect(928, 1004, 64, 64);
        let start = compute_genie_geometry(maximized, dock, 0.0, GenieDirection::Minimize);
        assert!(start.clip.strength.abs() < 0.001);
        assert!((start.boundaries.last().unwrap().y - maximized.bottom() as f32).abs() < 0.001);
        let end = compute_genie_geometry(maximized, dock, 1.0, GenieDirection::Minimize);
        assert!((end.clip.strength - 1.0).abs() < 0.001);
    }

    #[test]
    fn target_above_window_mirrors_two_phase_funnel_cleanly() {
        let window_below = rect(200, 600, 700, 350);
        let dock_above = rect(500, 80, 64, 64);
        let phase1 = compute_genie_geometry(
            window_below,
            dock_above,
            PHASE_ONE_SPLIT,
            GenieDirection::Minimize,
        );
        let bottom = phase1.boundaries.last().unwrap();
        let top = phase1.boundaries.first().unwrap();
        assert!((bottom.y - window_below.bottom() as f32).abs() < 0.01);
        assert!((top.y - dock_above.bottom() as f32).abs() < 0.01);
        assert!(!phase1.clip.clip_below);
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
