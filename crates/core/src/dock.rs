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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockPosition {
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct DockPreferences {
    pub position: DockPosition,
    pub magnification: bool,
    pub icon_size: i32,
    pub autohide: bool,
    pub reserve_space: bool,
    pub hide_taskbar: bool,
    pub show_downloads: bool,
    pub show_trash: bool,
}

impl Default for DockPreferences {
    fn default() -> Self {
        Self {
            position: DockPosition::Bottom,
            magnification: true,
            icon_size: DOCK_ICON_MEDIUM,
            autohide: false,
            reserve_space: false,
            hide_taskbar: true,
            show_downloads: true,
            show_trash: true,
        }
    }
}

pub const DOCK_ICON_SMALL: i32 = 40;
pub const DOCK_ICON_MEDIUM: i32 = 48;
pub const DOCK_ICON_LARGE: i32 = 64;
const DOCK_ICON_MIN: i32 = 32;
const DOCK_ICON_MAX: i32 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSettingsChange {
    ToggleHiding,
    ToggleReserveSpace,
    ToggleMagnification,
    ToggleHideTaskbar,
    ToggleShowDownloads,
    ToggleShowTrash,
    IconSize(i32),
}

impl DockPreferences {
    #[must_use]
    pub fn apply(self, change: DockSettingsChange) -> Self {
        match change {
            DockSettingsChange::ToggleHiding => Self {
                autohide: !self.autohide,
                reserve_space: false,
                ..self
            },
            DockSettingsChange::ToggleReserveSpace => Self {
                reserve_space: !self.reserve_space,
                autohide: false,
                ..self
            },
            DockSettingsChange::ToggleMagnification => Self {
                magnification: !self.magnification,
                ..self
            },
            DockSettingsChange::ToggleHideTaskbar => Self {
                hide_taskbar: !self.hide_taskbar,
                ..self
            },
            DockSettingsChange::ToggleShowDownloads => Self {
                show_downloads: !self.show_downloads,
                ..self
            },
            DockSettingsChange::ToggleShowTrash => Self {
                show_trash: !self.show_trash,
                ..self
            },
            DockSettingsChange::IconSize(size) => Self {
                icon_size: size.clamp(DOCK_ICON_MIN, DOCK_ICON_MAX),
                ..self
            },
        }
    }

    #[must_use]
    pub fn metrics(self) -> DockMetrics {
        DockMetrics {
            base_icon_size: self.icon_size.clamp(DOCK_ICON_MIN, DOCK_ICON_MAX),
            max_scale: if self.magnification {
                DockMetrics::MACOS_SONOMA.max_scale
            } else {
                1.0
            },
            ..DockMetrics::MACOS_SONOMA
        }
    }

    #[must_use]
    pub fn parse_config(contents: &str) -> Self {
        let defaults = Self::default();
        Self {
            position: DockPosition::Bottom,
            magnification: config_flag(contents, "dock_magnification", defaults.magnification),
            icon_size: config_value(contents, "dock_icon_size")
                .and_then(|text| text.parse::<i32>().ok())
                .map_or(defaults.icon_size, |size| {
                    size.clamp(DOCK_ICON_MIN, DOCK_ICON_MAX)
                }),
            autohide: config_flag(contents, "dock_autohide", defaults.autohide)
                && !config_flag(contents, "dock_reserve_space", defaults.reserve_space),
            reserve_space: config_flag(contents, "dock_reserve_space", defaults.reserve_space),
            hide_taskbar: config_flag(contents, "dock_hide_taskbar", defaults.hide_taskbar),
            show_downloads: config_flag(contents, "dock_show_downloads", defaults.show_downloads),
            show_trash: config_flag(contents, "dock_show_trash", defaults.show_trash),
        }
    }

    #[must_use]
    pub fn config_lines(self) -> String {
        format!(
            "dock_autohide={}\ndock_reserve_space={}\ndock_magnification={}\ndock_icon_size={}\ndock_hide_taskbar={}\ndock_show_downloads={}\ndock_show_trash={}\n",
            self.autohide,
            self.reserve_space,
            self.magnification,
            self.icon_size,
            self.hide_taskbar,
            self.show_downloads,
            self.show_trash
        )
    }
}

fn config_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then_some(value.trim())
    })
}

fn config_flag(contents: &str, key: &str, fallback: bool) -> bool {
    match config_value(contents, key) {
        Some(text) if text.eq_ignore_ascii_case("true") => true,
        Some(text) if text.eq_ignore_ascii_case("false") => false,
        _ => fallback,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockInput<'a> {
    pub screen_width: i32,
    pub dpi: u32,
    pub app_count: usize,
    pub right_count: usize,
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
    pub rest_height: i32,
}

impl DockLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32, gap_pad: f32) -> Option<usize> {
        let (fx, fy) = (x as f32, y as f32);
        self.items
            .iter()
            .position(|item| item.contains(fx, fy, gap_pad))
    }

    #[must_use]
    pub fn reorder_slot_index(&self, x: i32, app_count: usize) -> Option<usize> {
        let limit = app_count.min(self.items.len());
        if limit == 0 {
            return None;
        }
        let fx = x as f32;
        let first = &self.items[0];
        let last = &self.items[limit - 1];
        let pad = first.size * 0.5;
        if fx < first.x - pad || fx > last.x + last.size + pad {
            return None;
        }
        self.items[..limit]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (a.x + a.size * 0.5 - fx).abs();
                let db = (b.x + b.size * 0.5 - fx).abs();
                da.total_cmp(&db)
            })
            .map(|(idx, _)| idx)
    }

    #[must_use]
    pub fn preview_reordered_indices(
        count: usize,
        drag_from: Option<usize>,
        drag_to: Option<usize>,
    ) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..count).collect();
        if let (Some(from), Some(to)) = (drag_from, drag_to)
            && from < count
            && to < count
            && from != to
        {
            let moved = indices.remove(from);
            indices.insert(to, moved);
        }
        indices
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
#[allow(clippy::too_many_lines)]
pub fn compute_dock_layout(metrics: &DockMetrics, input: DockInput<'_>) -> DockLayout {
    let dpi = input.dpi;
    let bottom_margin = scale(metrics.bottom_margin, dpi) as f32;
    let right_items = input.right_count.max(usize::from(input.has_trash));
    let FittedDock {
        base_icon,
        gap,
        pad_x,
        pad_top,
        pad_bottom,
        corner_radius,
        divider_w,
    } = fit_dock(
        metrics,
        input.screen_width,
        dpi,
        input.app_count,
        right_items,
    );
    let dot_radius = scale(metrics.dot_radius, dpi).max(2) as f32;
    let tooltip_gap = scale(metrics.tooltip_gap, dpi) as f32;
    let tooltip_height = scale(metrics.tooltip_height, dpi) as f32;
    let max_bounce = scale(metrics.bounce_height, dpi) as f32;

    let total_items = input.app_count + right_items;

    let max_icon = base_icon * metrics.max_scale;
    let unmag_pill_h = pad_top + base_icon + pad_bottom;
    let headroom = (max_icon - base_icon) + max_bounce + tooltip_gap + tooltip_height + 16.0;
    let window_height = (unmag_pill_h + bottom_margin + headroom).ceil() as i32;
    let rest_height = (unmag_pill_h + bottom_margin).ceil() as i32;

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
            rest_height,
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
        if right_items > 0 && i == input.app_count && input.app_count > 0 {
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
        rest_height,
    }
}

#[must_use]
pub fn dock_rest_zone(layout: &DockLayout, screen_height: i32) -> Option<Rect> {
    (!layout.pill.is_empty()).then_some(Rect {
        x: layout.pill.x,
        y: screen_height - layout.rest_height,
        width: layout.pill.width,
        height: layout.rest_height,
    })
}

#[must_use]
pub const fn dock_should_hide(
    preferences: DockPreferences,
    fullscreen_covered: bool,
    window_overlaps: bool,
) -> bool {
    fullscreen_covered || (preferences.autohide && window_overlaps)
}

const SCREEN_EDGE_ALLOWANCE: f32 = 32.0;
const MIN_DOCK_WIDTH: f32 = 120.0;
const MAX_DOCK_SLOTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq)]
struct FittedDock {
    base_icon: f32,
    gap: f32,
    pad_x: f32,
    pad_top: f32,
    pad_bottom: f32,
    corner_radius: f32,
    divider_w: f32,
}

impl FittedDock {
    fn width(&self, total_items: usize) -> f32 {
        total_items as f32 * self.base_icon
            + total_items.saturating_sub(1) as f32 * self.gap
            + self.divider_w
            + self.pad_x * 2.0
    }
}

fn fit_dock(
    metrics: &DockMetrics,
    screen_width: i32,
    dpi: u32,
    app_count: usize,
    right_items: usize,
) -> FittedDock {
    let natural = FittedDock {
        base_icon: scale(metrics.base_icon_size, dpi) as f32,
        gap: scale(metrics.icon_gap, dpi) as f32,
        pad_x: scale(metrics.pad_x, dpi) as f32,
        pad_top: scale(metrics.pad_top, dpi) as f32,
        pad_bottom: scale(metrics.pad_bottom, dpi) as f32,
        corner_radius: scale(metrics.corner_radius, dpi) as f32,
        divider_w: if right_items > 0 && app_count > 0 {
            scale(metrics.divider_slot_width, dpi) as f32
        } else {
            0.0
        },
    };
    let total_items = app_count + right_items;
    let max_allowed_w = max_dock_width(screen_width);
    let needed_w =
        natural.width(total_items) + magnification_extra_width(metrics, natural.base_icon);
    if total_items == 0 || needed_w <= max_allowed_w {
        return natural;
    }
    let shrink = (max_allowed_w / needed_w).clamp(0.35, 1.0);
    FittedDock {
        base_icon: (natural.base_icon * shrink).max(16.0),
        gap: (natural.gap * shrink).max(2.0),
        pad_x: (natural.pad_x * shrink).max(4.0),
        pad_top: (natural.pad_top * shrink).max(4.0),
        pad_bottom: (natural.pad_bottom * shrink).max(4.0),
        corner_radius: (natural.corner_radius * shrink).max(6.0),
        divider_w: natural.divider_w * shrink,
    }
}

fn max_dock_width(screen_width: i32) -> f32 {
    (screen_width as f32 - SCREEN_EDGE_ALLOWANCE).max(MIN_DOCK_WIDTH)
}

fn magnification_extra_width(metrics: &DockMetrics, base_icon: f32) -> f32 {
    (metrics.max_scale - 1.0).max(0.0) * metrics.magnify_radius_slots * base_icon
}

#[must_use]
pub fn dock_capacity(
    metrics: &DockMetrics,
    screen_width: i32,
    dpi: u32,
    right_items: usize,
) -> usize {
    let fits = |apps: usize| {
        let total = apps + right_items;
        let fitted = fit_dock(metrics, screen_width, dpi, apps, right_items);
        fitted.width(total) + magnification_extra_width(metrics, fitted.base_icon)
            <= max_dock_width(screen_width) + 0.5
    };
    (1..=MAX_DOCK_SLOTS)
        .take_while(|&apps| fits(apps))
        .last()
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockOverflow {
    pub visible_apps: usize,
    pub hidden_apps: usize,
}

#[must_use]
pub fn split_dock_overflow(app_count: usize, capacity: usize) -> DockOverflow {
    if app_count <= capacity {
        return DockOverflow {
            visible_apps: app_count,
            hidden_apps: 0,
        };
    }
    let visible_apps = capacity.saturating_sub(1);
    DockOverflow {
        visible_apps,
        hidden_apps: app_count - visible_apps,
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
    let right_items = input.right_count.max(usize::from(input.has_trash));
    let total_items = input.app_count + right_items;
    let base_step = base_icon + gap;
    let base_content_w =
        total_items as f32 * base_icon + total_items.saturating_sub(1) as f32 * gap + divider_w;
    let base_left = (input.screen_width as f32 - base_content_w) * 0.5;

    let mut base_centers = Vec::with_capacity(total_items);
    let mut cx = base_left;
    for i in 0..total_items {
        if right_items > 0 && i == input.app_count && input.app_count > 0 {
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

#[must_use]
pub fn parse_title_badge_count(title: &str) -> Option<u32> {
    let trimmed = title.trim();
    for (open, close) in [('(', ')'), ('[', ']')] {
        if let Some(rest) = trimmed.strip_prefix(open)
            && let Some((inner, _)) = rest.split_once(close)
            && let Ok(count) = inner.trim().parse::<u32>()
            && (1..=999).contains(&count)
        {
            return Some(count);
        }
    }
    None
}

#[must_use]
pub fn parse_title_progress(title: &str) -> Option<f32> {
    let trimmed = title.trim();
    let candidate = trimmed
        .strip_prefix('(')
        .and_then(|s| s.split_once(')').map(|(inner, _)| inner))
        .or_else(|| {
            trimmed
                .strip_prefix('[')
                .and_then(|s| s.split_once(']').map(|(inner, _)| inner))
        })
        .or_else(|| trimmed.split_whitespace().next())?;
    let pct_str = candidate.trim().strip_suffix('%')?;
    let pct: f32 = pct_str.trim().parse().ok()?;
    if (0.0..=100.0).contains(&pct) {
        Some((pct / 100.0).clamp(0.0, 1.0))
    } else {
        None
    }
}

#[must_use]
pub fn dock_entry_badge(window_titles: &[String], notification_count: Option<u32>) -> Option<u32> {
    let title_count = window_titles
        .iter()
        .filter_map(|title| parse_title_badge_count(title))
        .max();
    let best = notification_count
        .unwrap_or(0)
        .max(title_count.unwrap_or(0));
    (best > 0).then_some(best)
}

#[must_use]
pub fn dock_entry_progress(
    window_titles: &[String],
    override_progress: Option<f32>,
) -> Option<f32> {
    if let Some(progress) = override_progress {
        return Some(progress.clamp(0.0, 1.0));
    }
    window_titles
        .iter()
        .find_map(|title| parse_title_progress(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: DockMetrics = DockMetrics::MACOS_SONOMA;

    fn footprint(fitted: &FittedDock, total_items: usize) -> f32 {
        fitted.width(total_items) + magnification_extra_width(&M, fitted.base_icon)
    }

    fn sample_input(cursor_x: Option<f32>, mag_strength: f32) -> DockInput<'static> {
        DockInput {
            screen_width: 1920,
            dpi: 96,
            app_count: 4,
            right_count: 1,
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
    fn sector_separation_places_divider_between_apps_and_right_sector() {
        let mut input = sample_input(None, 0.0);
        input.app_count = 3;
        input.right_count = 2;
        let layout = compute_dock_layout(&M, input);
        assert_eq!(layout.items.len(), 5);
        assert!(layout.divider_x.is_some());
        let div = layout.divider_x.unwrap();
        assert!(div > layout.items[2].x + layout.items[2].size);
        assert!(div < layout.items[3].x);
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

    #[test]
    fn capacity_is_the_most_apps_that_fit_at_the_smallest_icon_size() {
        let capacity = dock_capacity(&M, 800, 96, 2);
        assert!(capacity > 10);
        let fitted = fit_dock(&M, 800, 96, capacity, 2);
        assert!(footprint(&fitted, capacity + 2) <= 800.0 - 32.0 + 0.5);
        let over = fit_dock(&M, 800, 96, capacity + 1, 2);
        assert!(footprint(&over, capacity + 3) > 800.0 - 32.0 + 0.5);
        assert!(dock_capacity(&M, 2240, 144, 2) > dock_capacity(&M, 800, 96, 2) / 2);
    }

    #[test]
    fn overflow_keeps_every_app_reachable_through_one_slot() {
        assert_eq!(
            split_dock_overflow(10, 20),
            DockOverflow {
                visible_apps: 10,
                hidden_apps: 0
            }
        );
        assert_eq!(
            split_dock_overflow(20, 20),
            DockOverflow {
                visible_apps: 20,
                hidden_apps: 0
            }
        );
        let full = split_dock_overflow(25, 20);
        assert_eq!(full.visible_apps, 19);
        assert_eq!(full.hidden_apps, 6);
        assert_eq!(full.visible_apps + 1, 20);
        assert_eq!(full.visible_apps + full.hidden_apps, 25);
    }

    #[test]
    fn a_full_dock_with_its_overflow_slot_stays_on_screen() {
        let capacity = dock_capacity(&M, 800, 96, 2);
        let split = split_dock_overflow(capacity + 40, capacity);
        let mut input = sample_input(None, 0.0);
        input.screen_width = 800;
        input.app_count = split.visible_apps + 1;
        input.right_count = 2;
        let layout = compute_dock_layout(&M, input);
        assert!(layout.pill.x >= 0);
        assert!(layout.pill.x + layout.pill.width <= 800);
        let last = layout.items.last().unwrap();
        assert!(last.x + last.size <= 800.0);
    }

    #[test]
    fn preferences_round_trip_through_the_config_and_clamp_sizes() {
        let prefs = DockPreferences {
            autohide: true,
            magnification: false,
            icon_size: DOCK_ICON_LARGE,
            ..DockPreferences::default()
        };
        let text = format!("version=1\nenabled=topbar\n{}", prefs.config_lines());
        assert_eq!(DockPreferences::parse_config(&text), prefs);
        assert_eq!(
            DockPreferences::parse_config(""),
            DockPreferences::default()
        );
        let huge = DockPreferences::parse_config("dock_icon_size=500\n");
        assert_eq!(huge.icon_size, 80);
        let junk = DockPreferences::parse_config("dock_icon_size=abc\ndock_autohide=yes\n");
        assert_eq!(junk, DockPreferences::default());
    }

    #[test]
    fn auto_hide_and_reserved_space_are_mutually_exclusive() {
        let base = DockPreferences::default();
        let hiding = base.apply(DockSettingsChange::ToggleHiding);
        assert!(hiding.autohide && !hiding.reserve_space);
        let reserved = hiding.apply(DockSettingsChange::ToggleReserveSpace);
        assert!(reserved.reserve_space && !reserved.autohide);
        let back = reserved.apply(DockSettingsChange::ToggleHiding);
        assert!(back.autohide && !back.reserve_space);
        let off = back.apply(DockSettingsChange::ToggleHiding);
        assert!(!off.autohide && !off.reserve_space);
        let both = DockPreferences::parse_config(
            "dock_autohide=true
dock_reserve_space=true
",
        );
        assert!(both.reserve_space && !both.autohide);
        let legacy = DockPreferences::parse_config(
            "dock_autohide=true
",
        );
        assert!(legacy.autohide && !legacy.reserve_space);
        let text = reserved.config_lines();
        assert_eq!(DockPreferences::parse_config(&text), reserved);
    }

    #[test]
    fn smart_hide_follows_overlap_and_fullscreen_hides_in_every_mode() {
        let always = DockPreferences::default();
        let smart = always.apply(DockSettingsChange::ToggleHiding);
        let reserved = always.apply(DockSettingsChange::ToggleReserveSpace);
        assert!(!dock_should_hide(always, false, true));
        assert!(!dock_should_hide(reserved, false, true));
        assert!(dock_should_hide(smart, false, true));
        assert!(!dock_should_hide(smart, false, false));
        for prefs in [always, smart, reserved] {
            assert!(dock_should_hide(prefs, true, false));
        }
    }

    #[test]
    fn the_rest_zone_ignores_hide_progress_and_magnification() {
        let mut resting = sample_input(None, 0.0);
        let mut hidden = sample_input(Some(900.0), 1.0);
        hidden.hide_progress = 1.0;
        resting.hide_progress = 0.0;
        let a = compute_dock_layout(&M, resting);
        let b = compute_dock_layout(&M, hidden);
        assert_eq!(a.rest_height, b.rest_height);
        let zone = dock_rest_zone(&a, 1080).expect("zone");
        assert_eq!(zone.bottom(), 1080);
        assert_eq!(zone.y, 1080 - a.rest_height);
        assert_eq!(zone.x, a.pill.x);
        assert_eq!(zone.width, a.pill.width);
        assert!(a.rest_height >= a.pill.height + 6);
        let mut empty = sample_input(None, 0.0);
        empty.app_count = 0;
        empty.right_count = 0;
        empty.has_trash = false;
        assert!(dock_rest_zone(&compute_dock_layout(&M, empty), 1080).is_none());
    }

    #[test]
    fn config_tolerates_spacing_line_endings_and_first_value_wins() {
        let prefs = DockPreferences::parse_config(
            "  dock_icon_size = 64 \r\n\tdock_autohide=true\r\ndock_icon_size=40\r\n",
        );
        assert_eq!(prefs.icon_size, DOCK_ICON_LARGE);
        assert!(prefs.autohide);
    }

    #[test]
    fn malformed_flags_keep_their_defaults_instead_of_turning_off() {
        assert!(DockPreferences::parse_config("dock_magnification=yes\n").magnification);
        assert!(!DockPreferences::parse_config("dock_autohide=maybe\n").autohide);
        assert!(DockPreferences::parse_config("dock_autohide=TRUE\n").autohide);
        assert!(!DockPreferences::parse_config("dock_magnification= FALSE \n").magnification);
    }

    #[test]
    fn overflow_edges_for_no_single_and_exact_capacity_apps() {
        assert_eq!(
            split_dock_overflow(0, 0),
            DockOverflow {
                visible_apps: 0,
                hidden_apps: 0
            }
        );
        assert_eq!(
            split_dock_overflow(1, 0),
            DockOverflow {
                visible_apps: 0,
                hidden_apps: 1
            }
        );
        assert_eq!(
            split_dock_overflow(1, 1),
            DockOverflow {
                visible_apps: 1,
                hidden_apps: 0
            }
        );
        assert_eq!(
            split_dock_overflow(2, 1),
            DockOverflow {
                visible_apps: 0,
                hidden_apps: 2
            }
        );
        assert_eq!(
            split_dock_overflow(5, 5),
            DockOverflow {
                visible_apps: 5,
                hidden_apps: 0
            }
        );
    }

    #[test]
    fn capacity_fits_and_is_maximal_across_screen_widths_and_dpi() {
        for screen_width in (0..=3840).step_by(160) {
            for dpi in [96_u32, 144, 192, 288] {
                for right_items in 0..=3 {
                    let limit = max_dock_width(screen_width) + 0.5;
                    let capacity = dock_capacity(&M, screen_width, dpi, right_items);
                    if capacity > 0 {
                        let fitted = fit_dock(&M, screen_width, dpi, capacity, right_items);
                        assert!(footprint(&fitted, capacity + right_items) <= limit);
                    }
                    if capacity < MAX_DOCK_SLOTS {
                        let over = fit_dock(&M, screen_width, dpi, capacity + 1, right_items);
                        assert!(footprint(&over, capacity + 1 + right_items) > limit);
                    }
                }
            }
        }
    }

    #[test]
    fn without_magnification_capacity_ignores_the_extra_width() {
        let flat = DockMetrics {
            max_scale: 1.0,
            ..M
        };
        for screen_width in (0..=3840).step_by(160) {
            for dpi in [96_u32, 144, 192, 288] {
                for right_items in 0..=3 {
                    let limit = max_dock_width(screen_width) + 0.5;
                    let expected = (1..=MAX_DOCK_SLOTS)
                        .take_while(|&apps| {
                            fit_dock(&flat, screen_width, dpi, apps, right_items)
                                .width(apps + right_items)
                                <= limit
                        })
                        .last()
                        .unwrap_or(0);
                    assert_eq!(
                        dock_capacity(&flat, screen_width, dpi, right_items),
                        expected,
                        "width {screen_width} dpi {dpi} right {right_items}"
                    );
                }
            }
        }
    }

    #[test]
    fn magnification_never_increases_capacity() {
        let flat = DockMetrics {
            max_scale: 1.0,
            ..M
        };
        for screen_width in (0..=3840).step_by(160) {
            for dpi in [96_u32, 144, 192, 288] {
                for right_items in 0..=3 {
                    assert!(
                        dock_capacity(&M, screen_width, dpi, right_items)
                            <= dock_capacity(&flat, screen_width, dpi, right_items)
                    );
                }
            }
        }
    }

    #[test]
    fn a_full_magnified_dock_never_leaves_the_screen() {
        for screen_width in [800_i32, 1280, 1920, 2240, 3840] {
            for dpi in [96_u32, 144, 192, 288] {
                let right_items = 2;
                let capacity = dock_capacity(&M, screen_width, dpi, right_items);
                if capacity == 0 {
                    continue;
                }
                let mut cursor = 0.0_f32;
                while cursor <= screen_width as f32 {
                    let layout = compute_dock_layout(
                        &M,
                        DockInput {
                            screen_width,
                            dpi,
                            app_count: capacity,
                            right_count: right_items,
                            has_trash: true,
                            cursor_x: Some(cursor),
                            mag_strength: 1.0,
                            bounce_offsets: &[],
                            item_presence: &[],
                            hide_progress: 0.0,
                        },
                    );
                    assert!(
                        layout.pill.x >= 0 && layout.pill.right() <= screen_width,
                        "width {screen_width} dpi {dpi} cursor {cursor}: pill {}..{}",
                        layout.pill.x,
                        layout.pill.right()
                    );
                    cursor += 24.0;
                }
            }
        }
    }

    #[test]
    fn settings_changes_toggle_and_resize() {
        let prefs = DockPreferences::default();
        assert!(prefs.hide_taskbar);
        assert!(prefs.show_downloads);
        assert!(prefs.show_trash);
        assert!(prefs.apply(DockSettingsChange::ToggleHiding).autohide);
        assert!(
            !prefs
                .apply(DockSettingsChange::ToggleMagnification)
                .magnification
        );
        assert!(
            !prefs
                .apply(DockSettingsChange::ToggleHideTaskbar)
                .hide_taskbar
        );
        assert!(
            !prefs
                .apply(DockSettingsChange::ToggleShowDownloads)
                .show_downloads
        );
        assert!(!prefs.apply(DockSettingsChange::ToggleShowTrash).show_trash);
        assert_eq!(
            prefs
                .apply(DockSettingsChange::IconSize(DOCK_ICON_SMALL))
                .icon_size,
            DOCK_ICON_SMALL
        );
        assert_eq!(prefs.apply(DockSettingsChange::IconSize(4)).icon_size, 32);

        let custom = DockPreferences {
            autohide: true,
            magnification: false,
            icon_size: DOCK_ICON_LARGE,
            hide_taskbar: false,
            show_downloads: false,
            show_trash: false,
            ..DockPreferences::default()
        };
        assert_eq!(
            DockPreferences::parse_config(&custom.config_lines()),
            custom
        );
    }

    #[test]
    fn preferences_drive_icon_size_and_magnification() {
        let still = DockPreferences {
            magnification: false,
            icon_size: DOCK_ICON_LARGE,
            ..DockPreferences::default()
        }
        .metrics();
        assert_eq!(still.base_icon_size, DOCK_ICON_LARGE);
        let base = compute_dock_layout(&still, sample_input(None, 0.0));
        let target = base.items[2].x + base.items[2].size * 0.5;
        let hovered = compute_dock_layout(&still, sample_input(Some(target), 1.0));
        assert!((hovered.items[2].size - base.items[2].size).abs() < 0.01);
        let small = DockPreferences {
            icon_size: DOCK_ICON_SMALL,
            ..DockPreferences::default()
        }
        .metrics();
        let small_layout = compute_dock_layout(&small, sample_input(None, 0.0));
        assert!(small_layout.items[0].size < base.items[0].size);
    }

    #[test]
    fn dock_scales_adaptively_when_items_exceed_screen_width() {
        let mut input = sample_input(None, 0.0);
        input.screen_width = 800;
        input.app_count = 30;
        let layout = compute_dock_layout(&M, input);
        assert!(layout.pill.width <= 800);
        assert!(layout.pill.x >= 0);
    }

    #[test]
    fn title_badges_and_progress_parse_cleanly_and_badge_overhang_fits_headroom() {
        assert_eq!(parse_title_badge_count("(3) Slack - #general"), Some(3));
        assert_eq!(parse_title_badge_count("[12] Inbox"), Some(12));
        assert_eq!(parse_title_badge_count("(2026) Annual Report"), None);
        assert_eq!(parse_title_badge_count("(0) Notifications"), None);
        assert_eq!(
            dock_entry_badge(&[String::from("(4) Mail")], Some(2)),
            Some(4)
        );
        assert_eq!(dock_entry_badge(&[String::from("Mail")], Some(5)), Some(5));
        assert_eq!(dock_entry_badge(&[String::from("Mail")], Some(0)), None);

        let p = parse_title_progress("42% - Exporting build.zip").expect("pct");
        assert!((p - 0.42).abs() < 0.001);
        assert_eq!(parse_title_progress("140% overflow"), None);
        assert_eq!(
            dock_entry_progress(&[String::from("(75%) Copying")], None),
            Some(0.75)
        );
        assert_eq!(dock_entry_progress(&[], Some(1.4)), Some(1.0));

        for &dpi in &[96_u32, 144, 192] {
            let max_bounce = [1.0_f32; 5];
            let input = DockInput {
                screen_width: 1920,
                dpi,
                app_count: 4,
                right_count: 1,
                has_trash: true,
                cursor_x: Some(960.0),
                mag_strength: 1.0,
                bounce_offsets: &max_bounce,
                item_presence: &[],
                hide_progress: 0.0,
            };
            let layout = compute_dock_layout(&M, input);
            let badge_overhang = scale(8, dpi) as f32;
            assert!(
                layout
                    .items
                    .iter()
                    .all(|item| item.y - badge_overhang >= 0.0)
            );
        }
    }

    #[test]
    fn live_drag_preview_reorders_slots_and_resolves_target_x() {
        assert_eq!(
            DockLayout::preview_reordered_indices(6, Some(3), Some(1)),
            vec![0, 3, 1, 2, 4, 5]
        );
        assert_eq!(
            DockLayout::preview_reordered_indices(6, Some(1), Some(4)),
            vec![0, 2, 3, 4, 1, 5]
        );
        assert_eq!(
            DockLayout::preview_reordered_indices(6, Some(2), Some(2)),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert_eq!(
            DockLayout::preview_reordered_indices(6, None, Some(2)),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert_eq!(
            DockLayout::preview_reordered_indices(4, Some(4), Some(1)),
            vec![0, 1, 2, 3]
        );

        let layout = compute_dock_layout(&M, sample_input(None, 0.0));
        let slot2_x = (layout.items[2].x + layout.items[2].size * 0.5) as i32;
        assert_eq!(layout.reorder_slot_index(slot2_x, 4), Some(2));
        let trash_x = (layout.items[4].x + layout.items[4].size * 0.5) as i32;
        assert_eq!(layout.reorder_slot_index(trash_x, 4), None);
        assert_eq!(layout.reorder_slot_index(0, 4), None);
        assert_eq!(layout.reorder_slot_index(slot2_x, 0), None);
    }
}
