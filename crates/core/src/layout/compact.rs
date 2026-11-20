use crate::notch::island::CompactSlotKind;
use crate::notch::privacy::PRIVACY_COMPACT_EXTRA_POINTS;

use super::geometry::Rect;
use super::geometry::lerp_points;
use super::geometry::scale;
use super::metrics::Metrics;

#[derive(Default)]
pub(super) struct CompactNotchGeometry {
    pub(super) ring: Rect,
    pub(super) peek: Rect,
    pub(super) cycle_steps: [Rect; 4],
    pub(super) time: Rect,
    pub(super) title: Rect,
    pub(super) trailing: Rect,
    pub(super) privacy_dots: Rect,
}

pub(super) fn compute_compact_notch_geometry(
    notch: Rect,
    bar_height: i32,
    dpi: u32,
    privacy_dots_visible: bool,
    media_live: bool,
) -> CompactNotchGeometry {
    let ring_size = scale(COMPACT_RING_POINTS, dpi);
    let band_y = notch.y;
    let band_height = notch.height.min(bar_height - notch.y * 2).max(ring_size);
    let ring = Rect {
        x: notch.x + scale(COMPACT_RING_INSET_POINTS, dpi),
        y: band_y + (band_height - ring_size) / 2,
        width: ring_size,
        height: ring_size,
    };
    let privacy_reserve = if privacy_dots_visible {
        scale(PRIVACY_COMPACT_EXTRA_POINTS, dpi)
    } else {
        0
    };
    let privacy_dots = if privacy_dots_visible {
        let dots_w = scale(COMPACT_PRIVACY_WIDTH_POINTS, dpi);
        let dots_inset = if notch.y > 0 {
            scale(COMPACT_FLOATING_PRIVACY_INSET_POINTS, dpi)
        } else {
            scale(COMPACT_PRIVACY_INSET_POINTS, dpi)
        };
        Rect {
            x: notch.right() - dots_inset - dots_w,
            y: band_y,
            width: dots_w,
            height: band_height,
        }
    } else {
        Rect::default()
    };
    let time_width = scale(COMPACT_TIME_WIDTH_POINTS, dpi);
    let time = Rect {
        x: notch.right() - scale(COMPACT_TIME_INSET_POINTS, dpi) - time_width - privacy_reserve,
        y: band_y,
        width: time_width,
        height: band_height,
    };
    let peek_gap = scale(COMPACT_PEEK_GAP_POINTS, dpi);
    let peek_left = ring.right() + peek_gap;
    let peek = Rect {
        x: peek_left,
        y: band_y,
        width: (time.x - peek_gap - peek_left).max(0),
        height: band_height,
    };
    let cycle_steps = compute_compact_cycle_steps(&peek, dpi);
    let trailing_width_pts = if media_live {
        COMPACT_LIVE_TRAILING_WIDTH_POINTS
    } else {
        COMPACT_TRAILING_WIDTH_POINTS
    };
    let trailing_width = scale(trailing_width_pts, dpi);
    let trailing = Rect {
        x: notch.right()
            - scale(COMPACT_TRAILING_INSET_POINTS, dpi)
            - trailing_width
            - privacy_reserve,
        y: band_y,
        width: trailing_width,
        height: band_height,
    };
    let title = Rect {
        x: peek_left,
        y: band_y,
        width: (trailing.x - peek_gap - peek_left).max(0),
        height: band_height,
    };
    CompactNotchGeometry {
        ring,
        peek,
        cycle_steps,
        time,
        title,
        trailing,
        privacy_dots,
    }
}

pub(super) const COMPACT_RING_POINTS: i32 = 20;
pub(super) const COMPACT_RING_INSET_POINTS: i32 = 8;
pub(super) const COMPACT_TIME_WIDTH_POINTS: i32 = 56;
pub(super) const COMPACT_TIME_INSET_POINTS: i32 = 12;
pub(super) const COMPACT_PEEK_GAP_POINTS: i32 = 8;
pub(super) const COMPACT_TRAILING_WIDTH_POINTS: i32 = 16;
pub(super) const COMPACT_LIVE_TRAILING_WIDTH_POINTS: i32 = 34;
pub(super) const COMPACT_TRAILING_INSET_POINTS: i32 = 12;
pub(super) const COMPACT_PRIVACY_INSET_POINTS: i32 = 6;
pub(super) const COMPACT_FLOATING_PRIVACY_INSET_POINTS: i32 = 8;
pub(super) const COMPACT_PRIVACY_WIDTH_POINTS: i32 = 14;
pub(super) const COMPACT_TITLE_SLACK_POINTS: i32 = 6;
pub(super) const COMPACT_MAX_WIDTH_POINTS: i32 = 520;
pub(super) const COMPACT_CYCLE_STEP_WIDTH_POINTS: i32 = 8;
pub(super) const COMPACT_CYCLE_STEP_GAP_POINTS: i32 = 4;
pub(super) const COMPACT_CYCLE_STEP_HEIGHT_POINTS: i32 = 3;
pub(super) const SECONDARY_NOTCH_SIZE_POINTS: i32 = 28;
pub(super) const SECONDARY_NOTCH_GAP_POINTS: i32 = 8;
pub(super) const SECONDARY_NOTCH_MIN_SIZE_POINTS: i32 = 12;
pub(super) const SECONDARY_NOTCH_SWAP_PULL_POINTS: i32 = 4;

pub(super) fn compute_compact_cycle_steps(peek: &Rect, dpi: u32) -> [Rect; 4] {
    let gap = scale(COMPACT_CYCLE_STEP_GAP_POINTS, dpi).min(peek.width / 7);
    let width = scale(COMPACT_CYCLE_STEP_WIDTH_POINTS, dpi).min((peek.width - gap * 3).max(0) / 4);
    let height = scale(COMPACT_CYCLE_STEP_HEIGHT_POINTS, dpi).min(peek.height.max(0));
    let total_width = width * 4 + gap * 3;
    let left = peek.x + (peek.width - total_width) / 2;
    std::array::from_fn(|index| Rect {
        x: left + index as i32 * (width + gap),
        y: peek.y + (peek.height - height) / 2,
        width,
        height,
    })
}

pub(super) fn compute_secondary_notch_rect(
    notch: Rect,
    metrics: &Metrics,
    dpi: u32,
    secondary_expansion: f32,
    swap_blend: f32,
) -> Rect {
    let t = secondary_expansion.clamp(0.0, 1.0);
    if t <= 0.005 {
        return Rect::default();
    }
    let floating = metrics.notch_top_inset > 0;
    let max_size_points = if floating {
        metrics.notch_compact_height
    } else {
        metrics.notch_height.min(SECONDARY_NOTCH_SIZE_POINTS)
    };
    let target_size_pts = max_size_points.max(SECONDARY_NOTCH_MIN_SIZE_POINTS);
    let size_pts = lerp_points(SECONDARY_NOTCH_MIN_SIZE_POINTS, target_size_pts, t);
    let size = scale(size_pts, dpi);
    let swap_pull = (swap_blend.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
    let pull_px = (scale(SECONDARY_NOTCH_SWAP_PULL_POINTS, dpi) as f32 * swap_pull).round() as i32;
    let start_x = notch.right() - size;
    let target_x = notch.right() + scale(SECONDARY_NOTCH_GAP_POINTS, dpi) - pull_px;
    let x = start_x + ((target_x - start_x) as f32 * t).round() as i32;
    let y = if floating {
        let collapsed_notch_h = scale(metrics.bar_height, dpi) - notch.y * 2;
        notch.y + (collapsed_notch_h - size) / 2
    } else {
        (scale(metrics.bar_height, dpi) - size) / 2
    };
    Rect {
        x,
        y,
        width: size,
        height: size,
    }
}

pub(super) fn compact_width_points(
    metrics: &Metrics,
    title_points: i32,
    media_live: bool,
    primary: Option<CompactSlotKind>,
) -> i32 {
    let focus_sized = matches!(
        primary,
        Some(CompactSlotKind::Focus | CompactSlotKind::Call)
    );
    let base_width = if focus_sized {
        metrics.notch_focus_compact_width
    } else {
        metrics.notch_compact_width
    };
    if focus_sized || title_points <= 0 {
        return base_width;
    }
    let trailing_pts = if media_live {
        COMPACT_LIVE_TRAILING_WIDTH_POINTS
    } else {
        COMPACT_TRAILING_WIDTH_POINTS
    };
    let chrome = COMPACT_RING_INSET_POINTS
        + COMPACT_RING_POINTS
        + COMPACT_PEEK_GAP_POINTS * 2
        + trailing_pts
        + COMPACT_TRAILING_INSET_POINTS;
    (chrome + title_points + COMPACT_TITLE_SLACK_POINTS)
        .min(COMPACT_MAX_WIDTH_POINTS.max(base_width))
        .max(base_width)
}
