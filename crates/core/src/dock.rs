use std::f32::consts::FRAC_PI_2;

use crate::layout::{Rect, scale};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppleIconKind {
    Finder,
    Launchpad,
    Trash,
    TrashFull,
    Terminal,
    Settings,
    Browser,
    Notes,
    Code,
    Music,
    Folder,
}

#[must_use]
pub fn select_dock_click_window(hwnds: &[isize], foreground_hwnd: Option<isize>) -> Option<isize> {
    let first = *hwnds.first()?;
    if hwnds.len() <= 1 {
        return Some(first);
    }
    if let Some(fg) = foreground_hwnd
        && let Some(idx) = hwnds.iter().position(|&h| h == fg)
    {
        return Some(hwnds[(idx + 1) % hwnds.len()]);
    }
    Some(first)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockMetrics {
    pub base_icon_size: i32,
    pub max_scale: f32,
    pub magnify_radius_slots: f32,
    pub icon_gap: i32,
    pub pad_x: i32,
    pub pad_top: i32,
    pub pad_bottom: i32,
    pub corner_radius: i32,
    pub bottom_margin: i32,
    pub divider_slot_width: i32,
    pub dot_radius: i32,
    pub tooltip_gap: i32,
    pub tooltip_height: i32,
    pub bounce_height: i32,
}

impl DockMetrics {
    pub const MACOS_SONOMA: Self = Self {
        base_icon_size: 48,
        max_scale: 1.45,
        magnify_radius_slots: 2.6,
        icon_gap: 8,
        pad_x: 10,
        pad_top: 8,
        pad_bottom: 12,
        corner_radius: 18,
        bottom_margin: 6,
        divider_slot_width: 14,
        dot_radius: 2,
        tooltip_gap: 10,
        tooltip_height: 26,
        bounce_height: 22,
    };
}

impl Default for DockMetrics {
    fn default() -> Self {
        Self::MACOS_SONOMA
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockInput<'a> {
    pub screen_width: i32,
    pub dpi: u32,
    pub app_count: usize,
    pub has_trash: bool,
    pub cursor_x: Option<f32>,
    pub mag_strength: f32,
    pub bounce_offsets: &'a [f32],
    pub item_presence: &'a [f32],
    pub hide_progress: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockItemLayout {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub scale: f32,
    pub opacity: f32,
    pub dot_center: (f32, f32),
}

impl DockItemLayout {
    #[must_use]
    pub fn contains(&self, px: f32, py: f32, pad: f32) -> bool {
        self.opacity > 0.4
            && px >= self.x - pad
            && px <= self.x + self.size + pad
            && py >= self.y - pad
            && py <= self.y + self.size + pad
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockLayout {
    pub window_width: i32,
    pub window_height: i32,
    pub pill: Rect,
    pub pill_radius: f32,
    pub divider_x: Option<f32>,
    pub dot_radius: f32,
    pub tooltip_gap: f32,
    pub tooltip_height: f32,
    pub items: Vec<DockItemLayout>,
    pub hovered_index: Option<usize>,
}

impl DockLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32, gap_pad: f32) -> Option<usize> {
        let (fx, fy) = (x as f32, y as f32);
        self.items
            .iter()
            .position(|item| item.contains(fx, fy, gap_pad))
    }
}

#[must_use]
pub fn step_toward(current: f32, target: f32, speed: f32) -> f32 {
    let diff = target - current;
    if diff.abs() < 0.002 {
        target
    } else {
        current + diff * speed.clamp(0.0, 1.0)
    }
}

#[must_use]
pub fn smooth_damp(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let diff = target - current;
    if diff.abs() < 0.0015 {
        target
    } else {
        let factor = 1.0 - (-rate * dt.clamp(0.001, 0.050)).exp();
        current + diff * factor
    }
}

#[must_use]
pub fn compute_dock_layout(metrics: &DockMetrics, input: DockInput<'_>) -> DockLayout {
    let dpi = input.dpi;
    let base_icon = scale(metrics.base_icon_size, dpi) as f32;
    let gap = scale(metrics.icon_gap, dpi) as f32;
    let pad_x = scale(metrics.pad_x, dpi) as f32;
    let pad_top = scale(metrics.pad_top, dpi) as f32;
    let pad_bottom = scale(metrics.pad_bottom, dpi) as f32;
    let corner_radius = scale(metrics.corner_radius, dpi) as f32;
    let bottom_margin = scale(metrics.bottom_margin, dpi) as f32;
    let divider_w = if input.has_trash && input.app_count > 0 {
        scale(metrics.divider_slot_width, dpi) as f32
    } else {
        0.0
    };
    let dot_radius = scale(metrics.dot_radius, dpi).max(2) as f32;
    let tooltip_gap = scale(metrics.tooltip_gap, dpi) as f32;
    let tooltip_height = scale(metrics.tooltip_height, dpi) as f32;
    let max_bounce = scale(metrics.bounce_height, dpi) as f32;

    let total_items = input.app_count + usize::from(input.has_trash);
    let max_icon = base_icon * metrics.max_scale;
    let unmag_pill_h = pad_top + base_icon + pad_bottom;
    let headroom = (max_icon - base_icon) + max_bounce + tooltip_gap + tooltip_height + 16.0;
    let window_height = (unmag_pill_h + bottom_margin + headroom).ceil() as i32;

    if total_items == 0 {
        return DockLayout {
            window_width: input.screen_width,
            window_height,
            pill: Rect {
                x: input.screen_width / 2,
                y: window_height,
                width: 0,
                height: 0,
            },
            pill_radius: corner_radius,
            divider_x: None,
            dot_radius,
            tooltip_gap,
            tooltip_height,
            items: Vec::new(),
            hovered_index: None,
        };
    }

    let (scales, hovered_index) = evaluate_scales(metrics, &input, base_icon, gap, divider_w);

    let mut content_w = divider_w;
    for (i, &s) in scales.iter().enumerate() {
        let p = item_presence_at(input.item_presence, i);
        content_w += base_icon * s * p;
        if i + 1 < total_items {
            content_w += gap * p;
        }
    }
    let pill_w = content_w + pad_x * 2.0;
    let pill_left = (input.screen_width as f32 - pill_w) * 0.5;

    let slide_y = input.hide_progress.clamp(0.0, 1.0) * (unmag_pill_h + bottom_margin + 12.0);
    let pill_bottom = window_height as f32 - bottom_margin + slide_y;
    let pill_top = pill_bottom - unmag_pill_h;
    let icon_baseline = pill_bottom - pad_bottom;
    let dot_y = pill_bottom - pad_bottom * 0.45;

    let mut items = Vec::with_capacity(total_items);
    let mut x = pill_left + pad_x;
    let mut divider_x = None;

    for (i, &s) in scales.iter().enumerate() {
        if input.has_trash && i == input.app_count && input.app_count > 0 {
            divider_x = Some(x - gap * 0.5 + divider_w * 0.5);
            x += divider_w;
        }
        let p = item_presence_at(input.item_presence, i);
        let full_size = base_icon * s;
        let size = full_size * p;
        let slot_offset = (full_size - size) * 0.5;
        let bounce = input.bounce_offsets.get(i).copied().unwrap_or(0.0) * max_bounce;
        let y = icon_baseline - size - bounce;
        items.push(DockItemLayout {
            x: x + slot_offset * 0.2,
            y,
            size,
            scale: s * p,
            opacity: p,
            dot_center: (x + size * 0.5, dot_y),
        });
        x += size + gap * p;
    }

    let pill = Rect {
        x: pill_left.round() as i32,
        y: pill_top.round() as i32,
        width: pill_w.round() as i32,
        height: unmag_pill_h.round() as i32,
    };

    DockLayout {
        window_width: input.screen_width,
        window_height,
        pill,
        pill_radius: corner_radius,
        divider_x,
        dot_radius,
        tooltip_gap,
        tooltip_height,
        items,
        hovered_index,
    }
}

fn item_presence_at(list: &[f32], index: usize) -> f32 {
    list.get(index).copied().unwrap_or(1.0).clamp(0.0, 1.0)
}

fn evaluate_scales(
    metrics: &DockMetrics,
    input: &DockInput<'_>,
    base_icon: f32,
    gap: f32,
    divider_w: f32,
) -> (Vec<f32>, Option<usize>) {
    let total_items = input.app_count + usize::from(input.has_trash);
    let base_step = base_icon + gap;
    let base_content_w =
        total_items as f32 * base_icon + total_items.saturating_sub(1) as f32 * gap + divider_w;
    let base_left = (input.screen_width as f32 - base_content_w) * 0.5;

    let mut base_centers = Vec::with_capacity(total_items);
    let mut cx = base_left;
    for i in 0..total_items {
        if input.has_trash && i == input.app_count && input.app_count > 0 {
            cx += divider_w;
        }
        base_centers.push(cx + base_icon * 0.5);
        cx += base_step;
    }

    let radius_px = metrics.magnify_radius_slots * base_step;
    let strength = input.mag_strength.clamp(0.0, 1.0);
    let mut scales = vec![1.0_f32; total_items];
    let mut hovered_index = None;

    if let Some(mx) = input.cursor_x
        && strength > 0.001
    {
        let mut best_dist = f32::MAX;
        for (i, &center) in base_centers.iter().enumerate() {
            let dist = (center - mx).abs();
            if dist < radius_px {
                let u = dist / radius_px;
                let wave = (u * FRAC_PI_2).cos();
                scales[i] = 1.0 + (metrics.max_scale - 1.0) * (wave * wave) * strength;
            }
            if dist <= base_step * 0.65 && dist < best_dist {
                best_dist = dist;
                hovered_index = Some(i);
            }
        }
    } else if let Some(mx) = input.cursor_x {
        for (i, &center) in base_centers.iter().enumerate() {
            if (center - mx).abs() <= base_step * 0.55 {
                hovered_index = Some(i);
                break;
            }
        }
    }

    (scales, hovered_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: DockMetrics = DockMetrics::MACOS_SONOMA;

    fn sample_input(cursor_x: Option<f32>, mag_strength: f32) -> DockInput<'static> {
        DockInput {
            screen_width: 1920,
            dpi: 96,
            app_count: 4,
            has_trash: true,
            cursor_x,
            mag_strength,
            bounce_offsets: &[],
            item_presence: &[],
            hide_progress: 0.0,
        }
    }

    #[test]
    fn unmagnified_dock_is_centered_with_divider() {
        let layout = compute_dock_layout(&M, sample_input(None, 0.0));
        assert_eq!(layout.items.len(), 5);
        assert!(layout.divider_x.is_some());
        let center = layout.pill.x + layout.pill.width / 2;
        assert!((center - 960).abs() <= 1);
    }

    #[test]
    fn magnification_enlarges_hovered_icon_and_widens_pill() {
        let unmag = compute_dock_layout(&M, sample_input(None, 0.0));
        let target_cx = unmag.items[2].x + unmag.items[2].size * 0.5;
        let mag = compute_dock_layout(&M, sample_input(Some(target_cx), 1.0));
        assert!(mag.items[2].size > unmag.items[2].size);
        assert!(mag.pill.width > unmag.pill.width);
        assert_eq!(mag.hovered_index, Some(2));
    }

    #[test]
    fn appearing_item_scales_slot_and_pill_width() {
        let full = compute_dock_layout(&M, sample_input(None, 0.0));
        let presence = [1.0, 1.0, 1.0, 0.25, 1.0];
        let mut partial_in = sample_input(None, 0.0);
        partial_in.item_presence = &presence;
        let partial = compute_dock_layout(&M, partial_in);
        assert!(partial.pill.width < full.pill.width);
        assert!(partial.items[3].size < full.items[3].size);
    }

    #[test]
    fn hit_test_finds_icon_under_cursor() {
        let layout = compute_dock_layout(&M, sample_input(None, 0.0));
        let first = layout.items[0];
        assert_eq!(
            layout.hit_test((first.x + 10.0) as i32, (first.y + 10.0) as i32, 4.0),
            Some(0)
        );
        assert_eq!(layout.hit_test(10, 10, 4.0), None);
    }

    #[test]
    fn select_dock_click_window_cycles_when_multi_window_app_is_active() {
        let windows = [101_isize, 202, 303];
        assert_eq!(select_dock_click_window(&windows, None), Some(101));
        assert_eq!(select_dock_click_window(&windows, Some(101)), Some(202));
        assert_eq!(select_dock_click_window(&windows, Some(202)), Some(303));
        assert_eq!(select_dock_click_window(&windows, Some(303)), Some(101));
        assert_eq!(select_dock_click_window(&[101], Some(101)), Some(101));
    }
}
