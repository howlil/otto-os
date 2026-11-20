use crate::app_model::{AppInfo, WindowCommand, WindowIdentity, WindowInfo, WindowPresence};
use crate::layout::{Rect, scale};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlAction {
    ToggleMaximize,
    Minimize,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Top,
    TopRight,
    Right,
}

pub type TrafficLightAction = WindowControlAction;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowControlsMetrics {
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub button_radius: i32,
    pub button_spacing: i32,
    pub left_margin: i32,
    pub hit_slop: i32,
}

pub type TrafficLightMetrics = WindowControlsMetrics;

impl WindowControlsMetrics {
    pub const MACOS_SONOMA: Self = Self {
        width: 72,
        height: 28,
        offset_x: 10,
        offset_y: 6,
        button_radius: 6,
        button_spacing: 20,
        left_margin: 12,
        hit_slop: 4,
    };
}

impl Default for WindowControlsMetrics {
    fn default() -> Self {
        Self::MACOS_SONOMA
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowControlsLayout {
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub radius: f32,
    pub hit_radius: f32,
    pub center_y: f32,
    pub centers_x: [f32; 3],
    pub actions: [WindowControlAction; 3],
    pub slots: usize,
    pub cell_width: f32,
}

pub type TrafficLightLayout = WindowControlsLayout;

impl WindowControlsLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        if self.cell_width > 0.0 {
            return self.cell_hit_test(x, y);
        }
        let (fx, fy) = (x as f32, y as f32);
        let hit_r = self.hit_radius;
        let dy = fy - self.center_y;
        if dy.abs() > hit_r {
            return None;
        }
        for (cx, action) in self
            .centers_x
            .into_iter()
            .zip(self.actions)
            .take(self.slots)
        {
            let dx = fx - cx;
            if dx * dx + dy * dy <= hit_r * hit_r {
                return Some(action);
            }
        }
        None
    }

    fn cell_hit_test(&self, x: i32, y: i32) -> Option<WindowControlAction> {
        if x < 0 || y < 0 || y >= self.height {
            return None;
        }
        let slot = (x as f32 / self.cell_width) as usize;
        (slot < self.slots).then(|| self.actions[slot])
    }

    #[must_use]
    pub fn hits_group(&self, x: i32, y: i32) -> bool {
        if self.slots == 0 || x < 0 || y < 0 || x >= self.width || y >= self.height {
            return false;
        }
        let (fx, fy) = (x as f32, y as f32);
        let hit_r = self.hit_radius;
        if (fy - self.center_y).abs() > hit_r {
            return false;
        }
        let first = self.centers_x[0];
        let last = self.centers_x[self.slots - 1];
        let min_x = first.min(last) - hit_r;
        let max_x = first.max(last) + hit_r;
        fx >= min_x && fx <= max_x
    }

    #[must_use]
    pub fn center_of(&self, action: WindowControlAction) -> Option<f32> {
        self.actions
            .into_iter()
            .zip(self.centers_x)
            .take(self.slots)
            .find_map(|(candidate, center)| (candidate == action).then_some(center))
    }

    #[must_use]
    pub fn resize_edge(
        &self,
        x: i32,
        y: i32,
        maximized: bool,
        resizable: bool,
        dpi: u32,
    ) -> Option<ResizeEdge> {
        if maximized || !resizable || x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        if self.hit_test(x, y).is_some() {
            return None;
        }
        let edge = scale(RESIZE_BORDER_DIP, dpi).max(2);
        let on_top = y < edge;
        let on_right = x >= self.width - edge;
        match (on_top, on_right) {
            (true, true) => Some(ResizeEdge::TopRight),
            (true, false) => Some(ResizeEdge::Top),
            (false, true) => Some(ResizeEdge::Right),
            (false, false) => None,
        }
    }
}

const LEFT_ACTIONS: [WindowControlAction; 3] = [
    WindowControlAction::Close,
    WindowControlAction::Minimize,
    WindowControlAction::ToggleMaximize,
];

const COVER_ACTIONS: [WindowControlAction; 3] = [
    WindowControlAction::Minimize,
    WindowControlAction::ToggleMaximize,
    WindowControlAction::Close,
];

#[must_use]
pub fn compute_window_controls_layout(
    metrics: &WindowControlsMetrics,
    dpi: u32,
) -> WindowControlsLayout {
    let width = scale(metrics.width, dpi);
    let height = scale(metrics.height, dpi);
    let offset_x = scale(metrics.offset_x, dpi);
    let offset_y = scale(metrics.offset_y, dpi);
    let radius = scale(metrics.button_radius, dpi) as f32;
    let spacing = scale(metrics.button_spacing, dpi) as f32;
    let hit_radius = radius + scale(metrics.hit_slop, dpi) as f32;
    let left_margin = scale(metrics.left_margin, dpi) as f32;
    let close_x = left_margin;
    let min_x = close_x + spacing;
    let max_x = min_x + spacing;
    let center_y = height as f32 * 0.5;

    WindowControlsLayout {
        width,
        height,
        offset_x,
        offset_y,
        radius,
        hit_radius,
        center_y,
        centers_x: [close_x, min_x, max_x],
        actions: LEFT_ACTIONS,
        slots: 3,
        cell_width: 0.0,
    }
}

#[must_use]
pub fn compute_traffic_light_layout(metrics: &TrafficLightMetrics, dpi: u32) -> TrafficLightLayout {
    compute_window_controls_layout(metrics, dpi)
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowControlsState {
    pub active: bool,
    pub close: bool,
    pub minimize: bool,
    pub zoom: bool,
    pub alt_zoom_glyph: bool,
}

impl WindowControlsState {
    #[must_use]
    pub const fn of(window: &WindowInfo) -> Self {
        Self {
            active: window.active,
            close: window.capability(WindowCommand::Close).is_available(),
            minimize: window.capability(WindowCommand::Minimize).is_available(),
            zoom: window.capability(WindowCommand::Zoom).is_available()
                || window
                    .capability(WindowCommand::ToggleFullScreen)
                    .is_available(),
            alt_zoom_glyph: false,
        }
    }

    #[must_use]
    pub const fn with_alt_zoom_glyph(mut self, alt: bool) -> Self {
        self.alt_zoom_glyph = alt;
        self
    }

    #[must_use]
    pub const fn allows(self, action: WindowControlAction) -> bool {
        match action {
            WindowControlAction::Close => self.close,
            WindowControlAction::Minimize => self.minimize,
            WindowControlAction::ToggleMaximize => self.zoom,
        }
    }

    #[must_use]
    pub const fn shows_color(self, action: WindowControlAction, hovered: bool) -> bool {
        self.allows(action) && (self.active || hovered)
    }

    #[must_use]
    pub const fn any(self) -> bool {
        self.close || self.minimize || self.zoom
    }
}

#[must_use]
pub const fn accepts_window_controls(window: &WindowInfo) -> bool {
    window.traits.title_bar
        && !window.traits.elevated
        && !window.full_screen
        && matches!(window.presence, WindowPresence::Visible)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandScope {
    Window,
    Application,
}

#[must_use]
pub const fn window_control_scope(action: WindowControlAction, alt: bool) -> CommandScope {
    match action {
        WindowControlAction::Close | WindowControlAction::Minimize if alt => {
            CommandScope::Application
        }
        _ => CommandScope::Window,
    }
}

#[must_use]
pub fn application_command_targets(
    app: &AppInfo,
    clicked: WindowIdentity,
    command: WindowCommand,
) -> Vec<WindowIdentity> {
    let clicked_window = app
        .windows
        .iter()
        .find(|window| window.identity.is_same_window(&clicked));
    let siblings = app
        .primary_windows()
        .filter(|window| !window.identity.is_same_window(&clicked));
    clicked_window
        .into_iter()
        .chain(siblings)
        .filter(|window| window.presence != WindowPresence::Hidden)
        .filter(|window| window.capability(command).is_available())
        .map(|window| window.identity)
        .collect()
}

#[must_use]
pub const fn window_controls_visible_under_bar(
    caption_top: i32,
    caption_height: i32,
    bar_height: Option<i32>,
) -> bool {
    match bar_height {
        Some(bar) => caption_top + caption_height > bar,
        None => true,
    }
}

#[must_use]
pub const fn window_controls_top(
    window_top: i32,
    offset_y: i32,
    bar_height: Option<i32>,
    maximized: bool,
) -> i32 {
    let natural = window_top + offset_y;
    match bar_height {
        Some(bar) if maximized && natural < bar + offset_y => bar + offset_y,
        Some(bar) if natural < bar => bar,
        _ => natural,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverStyle {
    pub rows: Vec<[u8; 3]>,
    pub top_right_radius: f32,
    pub top_border_inset: bool,
    pub right_border_inset: bool,
    pub feather_width: f32,
    pub is_dark: bool,
}

impl CoverStyle {
    #[must_use]
    pub fn solid(rgb: [u8; 3], top_right_radius: f32, border_inset: bool) -> Self {
        Self {
            rows: vec![rgb],
            top_right_radius,
            top_border_inset: border_inset,
            right_border_inset: border_inset,
            feather_width: 0.0,
            is_dark: rgb_is_dark(rgb),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverBand {
    pub top: i32,
    pub bottom: i32,
    pub rgb: [u8; 3],
}

const fn rgb_luma(rgb: [u8; 3]) -> u32 {
    299 * rgb[0] as u32 + 587 * rgb[1] as u32 + 114 * rgb[2] as u32
}

const fn rgb_is_dark(rgb: [u8; 3]) -> bool {
    rgb_luma(rgb) < 128_000
}

#[must_use]
pub fn cover_is_dark(rows: &[[u8; 3]]) -> bool {
    if rows.is_empty() {
        return true;
    }
    let slice = if rows.len() > 2 {
        &rows[1..rows.len() - 1]
    } else {
        rows
    };
    let mut lumas: Vec<u32> = slice.iter().copied().map(rgb_luma).collect();
    lumas.sort_unstable();
    lumas[lumas.len() / 2] < 128_000
}

fn channel_median(values: &mut [u8]) -> u8 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn max_channel_diff(a: [u8; 3], b: [u8; 3]) -> u8 {
    a[0].abs_diff(b[0])
        .max(a[1].abs_diff(b[1]))
        .max(a[2].abs_diff(b[2]))
}

fn row_pixel_rgb(row: &[u8], x: usize) -> [u8; 3] {
    let pixel = &row[x * 4..x * 4 + 4];
    [pixel[2], pixel[1], pixel[0]]
}

fn is_horizontally_smooth(row: &[u8], x: usize, width: usize) -> bool {
    let cur = row_pixel_rgb(row, x);
    let left_smooth = x > 0 && max_channel_diff(cur, row_pixel_rgb(row, x - 1)) <= 12;
    let right_smooth = x + 1 < width && max_channel_diff(cur, row_pixel_rgb(row, x + 1)) <= 12;
    left_smooth || right_smooth
}

fn dominant_background_rgb(bgra: &[u8], width: usize, height: usize, row_bytes: usize) -> [u8; 3] {
    let row_range = if height > 2 { 1..height - 1 } else { 0..height };
    let mut sum_r = vec![0_u64; 4096];
    let mut sum_g = vec![0_u64; 4096];
    let mut sum_b = vec![0_u64; 4096];
    let mut counts = vec![0_u32; 4096];
    let mut saw_smooth = false;
    for y in row_range.clone() {
        let row = &bgra[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..width {
            if is_horizontally_smooth(row, x, width) {
                saw_smooth = true;
                let rgb = row_pixel_rgb(row, x);
                let idx = (usize::from(rgb[0] >> 4) << 8)
                    | (usize::from(rgb[1] >> 4) << 4)
                    | usize::from(rgb[2] >> 4);
                sum_r[idx] += u64::from(rgb[0]);
                sum_g[idx] += u64::from(rgb[1]);
                sum_b[idx] += u64::from(rgb[2]);
                counts[idx] += 1;
            }
        }
    }
    if !saw_smooth {
        for y in row_range {
            let row = &bgra[y * row_bytes..(y + 1) * row_bytes];
            for x in 0..width {
                let rgb = row_pixel_rgb(row, x);
                let idx = (usize::from(rgb[0] >> 4) << 8)
                    | (usize::from(rgb[1] >> 4) << 4)
                    | usize::from(rgb[2] >> 4);
                sum_r[idx] += u64::from(rgb[0]);
                sum_g[idx] += u64::from(rgb[1]);
                sum_b[idx] += u64::from(rgb[2]);
                counts[idx] += 1;
            }
        }
    }
    let mut bins: Vec<([u8; 3], u32)> = Vec::new();
    for idx in 0..4096 {
        let count = counts[idx];
        if count > 0 {
            let denom = u64::from(count);
            bins.push((
                [
                    (sum_r[idx] / denom) as u8,
                    (sum_g[idx] / denom) as u8,
                    (sum_b[idx] / denom) as u8,
                ],
                count,
            ));
        }
    }
    bins.sort_unstable_by_key(|bin| std::cmp::Reverse(bin.1));
    bins.truncate(64);
    let Some(&(best_seed, _)) = bins.iter().max_by_key(|&&(seed_rgb, seed_count)| {
        let cluster_count: u64 = bins
            .iter()
            .filter(|&&(other_rgb, _)| max_channel_diff(seed_rgb, other_rgb) <= 48)
            .map(|&(_, other_count)| u64::from(other_count))
            .sum();
        (cluster_count, seed_count)
    }) else {
        return [0, 0, 0];
    };
    let mut total_r = 0_u64;
    let mut total_g = 0_u64;
    let mut total_b = 0_u64;
    let mut total_count = 0_u64;
    for &(bin_rgb, bin_count) in &bins {
        if max_channel_diff(best_seed, bin_rgb) <= 48 {
            let w = u64::from(bin_count);
            total_r += u64::from(bin_rgb[0]) * w;
            total_g += u64::from(bin_rgb[1]) * w;
            total_b += u64::from(bin_rgb[2]) * w;
            total_count += w;
        }
    }
    if total_count == 0 {
        return best_seed;
    }
    [
        (total_r / total_count) as u8,
        (total_g / total_count) as u8,
        (total_b / total_count) as u8,
    ]
}

fn median_rgb_from_indices(
    row: &[u8],
    indices: &[usize],
    reds: &mut Vec<u8>,
    greens: &mut Vec<u8>,
    blues: &mut Vec<u8>,
) -> [u8; 3] {
    reds.clear();
    greens.clear();
    blues.clear();
    for &x in indices {
        let rgb = row_pixel_rgb(row, x);
        reds.push(rgb[0]);
        greens.push(rgb[1]);
        blues.push(rgb[2]);
    }
    [
        channel_median(reds),
        channel_median(greens),
        channel_median(blues),
    ]
}

#[must_use]
pub fn extract_cover_rows(bgra: &[u8], width: usize, height: usize) -> Option<Vec<[u8; 3]>> {
    if width == 0 || height == 0 {
        return None;
    }
    let row_bytes = width.checked_mul(4)?;
    let total_bytes = row_bytes.checked_mul(height)?;
    if bgra.len() < total_bytes {
        return None;
    }
    let base_rgb = dominant_background_rgb(bgra, width, height, row_bytes);
    let min_cluster_matches = (width / 4).max(usize::from(width >= 2));
    let mut rows = Vec::with_capacity(height);
    let mut cluster_smooth = Vec::with_capacity(width);
    let mut smooth_only = Vec::with_capacity(width);
    let mut cluster_any = Vec::with_capacity(width);
    let mut all_indices = Vec::with_capacity(width);
    let mut reds = Vec::with_capacity(width);
    let mut greens = Vec::with_capacity(width);
    let mut blues = Vec::with_capacity(width);
    for y in 0..height {
        let row = &bgra[y * row_bytes..(y + 1) * row_bytes];
        cluster_smooth.clear();
        smooth_only.clear();
        cluster_any.clear();
        all_indices.clear();
        for x in 0..width {
            all_indices.push(x);
            let rgb = row_pixel_rgb(row, x);
            let in_cluster = max_channel_diff(rgb, base_rgb) <= 48;
            if in_cluster {
                cluster_any.push(x);
            }
            if is_horizontally_smooth(row, x, width) {
                smooth_only.push(x);
                if in_cluster {
                    cluster_smooth.push(x);
                }
            }
        }
        let chosen = if cluster_smooth.len() >= min_cluster_matches && !cluster_smooth.is_empty() {
            &cluster_smooth
        } else if smooth_only.len() >= (width / 2).max(2) {
            &smooth_only
        } else if !cluster_any.is_empty() {
            &cluster_any
        } else {
            &all_indices
        };
        rows.push(median_rgb_from_indices(
            row,
            chosen,
            &mut reds,
            &mut greens,
            &mut blues,
        ));
    }
    Some(rows)
}

#[must_use]
pub fn cover_row_bands(rows: &[[u8; 3]], height: usize, border_inset: bool) -> Vec<CoverBand> {
    let start_row = usize::from(border_inset);
    if height <= start_row || rows.is_empty() {
        return Vec::new();
    }
    let fallback = *rows.last().unwrap_or(&[0, 0, 0]);
    let mut bands: Vec<CoverBand> = Vec::new();
    for y in start_row..height {
        let rgb = rows.get(y).copied().unwrap_or(fallback);
        let row_top = y as i32;
        let row_bottom = (y + 1) as i32;
        if let Some(last) = bands.last_mut()
            && last.rgb == rgb
            && last.bottom == row_top
        {
            last.bottom = row_bottom;
        } else {
            bands.push(CoverBand {
                top: row_top,
                bottom: row_bottom,
                rgb,
            });
        }
    }
    bands
}

pub const NATIVE_CAPTION_BUTTON_DIP: i32 = 46;
pub const NATIVE_CAPTION_HEIGHT_DIP: i32 = 30;
pub const NATIVE_CORNER_RADIUS_DIP: i32 = 8;
pub const COVER_RIGHT_MARGIN_DIP: i32 = 14;
pub const COVER_FEATHER_DIP: i32 = 6;
pub const COVER_SAMPLE_GAP_DIP: i32 = 2;
pub const COVER_SAMPLE_WIDTH_DIP: i32 = 24;
pub const RESIZE_BORDER_DIP: i32 = 4;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCaptionStyle {
    pub has_caption: bool,
    pub has_system_menu: bool,
    pub can_minimize: bool,
    pub can_maximize: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCaption {
    pub frame_right: i32,
    pub top: i32,
    pub height: i32,
    pub slots: u8,
    pub can_minimize: bool,
    pub rounded_top_right: bool,
    pub top_border_inset: bool,
    pub dpi: u32,
    pub measured_width: i32,
}

impl NativeCaption {
    #[must_use]
    pub const fn new(
        style: NativeCaptionStyle,
        frame_right: i32,
        top: i32,
        height: i32,
        rounded_top_right: bool,
        dpi: u32,
    ) -> Option<Self> {
        if !style.has_caption || !style.has_system_menu || height <= 0 {
            return None;
        }
        let slots = if style.can_minimize || style.can_maximize {
            3
        } else {
            1
        };
        Some(Self {
            frame_right,
            top,
            height,
            slots,
            can_minimize: style.can_minimize,
            rounded_top_right,
            top_border_inset: rounded_top_right,
            dpi,
            measured_width: 0,
        })
    }

    #[must_use]
    pub const fn with_top_border_inset(self, top_border_inset: bool) -> Self {
        Self {
            top_border_inset,
            ..self
        }
    }

    #[must_use]
    pub fn standard(
        style: NativeCaptionStyle,
        frame_right: i32,
        top: i32,
        rounded_top_right: bool,
        dpi: u32,
    ) -> Option<Self> {
        Self::new(
            style,
            frame_right,
            top,
            scale(NATIVE_CAPTION_HEIGHT_DIP, dpi),
            rounded_top_right,
            dpi,
        )
    }

    #[must_use]
    pub fn measured(self, width: i32) -> Self {
        let standard = self.standard_width();
        let plausible = width * 4 >= standard * 3 && width * 4 <= standard * 5;
        Self {
            measured_width: if plausible { width } else { 0 },
            ..self
        }
    }

    fn standard_width(&self) -> i32 {
        scale(NATIVE_CAPTION_BUTTON_DIP, self.dpi) * i32::from(self.slots)
    }

    fn button_span(&self) -> i32 {
        if self.measured_width > 0 {
            self.measured_width
        } else {
            self.standard_width()
        }
    }

    #[must_use]
    pub fn cover_rect(&self) -> Rect {
        let width = self.button_span();
        Rect {
            x: self.frame_right - width,
            y: self.top,
            width,
            height: self.height,
        }
    }

    #[must_use]
    pub fn minimize_button(&self) -> Option<Rect> {
        if self.slots != 3 || !self.can_minimize {
            return None;
        }
        let cover = self.cover_rect();
        Some(Rect {
            width: cover.width / i32::from(self.slots),
            ..cover
        })
    }

    #[must_use]
    pub fn sample_rect(&self) -> Rect {
        let cover = self.cover_rect();
        let gap = scale(COVER_SAMPLE_GAP_DIP, self.dpi).max(1);
        let width = scale(COVER_SAMPLE_WIDTH_DIP, self.dpi).max(1);
        Rect {
            x: cover.x - gap - width,
            y: cover.y,
            width,
            height: cover.height,
        }
    }

    #[must_use]
    pub fn clipped_sample_rect(&self, frame_left: i32, min_top: i32) -> Rect {
        let base = self.sample_rect();
        let min_left = frame_left + scale(COVER_SAMPLE_GAP_DIP, self.dpi).max(1);
        let left = base.x.max(min_left);
        let right = base.right();
        let top = base.y.max(min_top);
        let bottom = base.bottom();
        Rect {
            x: left,
            y: top,
            width: (right - left).max(0),
            height: (bottom - top).max(0),
        }
    }

    #[must_use]
    pub fn corner_radius(&self) -> f32 {
        if self.rounded_top_right {
            scale(NATIVE_CORNER_RADIUS_DIP, self.dpi) as f32
        } else {
            0.0
        }
    }
}

#[must_use]
pub fn compute_cover_layout(
    caption: NativeCaption,
    metrics: &WindowControlsMetrics,
) -> Option<WindowControlsLayout> {
    let dpi = caption.dpi;
    let cover = caption.cover_rect();
    if cover.is_empty() {
        return None;
    }
    let slots = usize::from(caption.slots);
    let radius = scale(metrics.button_radius, dpi) as f32;
    let spacing = scale(metrics.button_spacing, dpi) as f32;
    let hit_radius = radius + scale(metrics.hit_slop, dpi) as f32;
    let right_margin = scale(COVER_RIGHT_MARGIN_DIP, dpi) as f32;
    let close_x = (cover.width as f32 - right_margin).max(radius);
    let (centers_x, actions) = if slots == 1 {
        ([close_x, 0.0, 0.0], [WindowControlAction::Close; 3])
    } else {
        let zoom_x = close_x - spacing;
        let min_x = zoom_x - spacing;
        ([min_x, zoom_x, close_x], COVER_ACTIONS)
    };
    Some(WindowControlsLayout {
        width: cover.width,
        height: cover.height,
        offset_x: 0,
        offset_y: 0,
        radius,
        hit_radius,
        center_y: cover.height as f32 * 0.5,
        centers_x,
        actions,
        slots,
        cell_width: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_model::{AppIdentity, WindowIdentity, WindowRole, WindowTraits};

    fn window(traits: WindowTraits) -> WindowInfo {
        WindowInfo {
            identity: WindowIdentity {
                hwnd: 1,
                process_id: 2,
                thread_id: 3,
                generation: 4,
            },
            app: AppIdentity::executable("app.exe"),
            app_name: String::from("App"),
            executable: String::from("app.exe"),
            title: String::from("App"),
            role: WindowRole::Primary,
            presence: WindowPresence::Visible,
            zoomed: false,
            full_screen: false,
            active: true,
            traits,
        }
    }

    fn sibling(hwnd: isize) -> WindowInfo {
        let mut sibling = window(WindowTraits::STANDARD);
        sibling.identity.hwnd = hwnd;
        sibling
    }

    fn application(windows: Vec<WindowInfo>) -> AppInfo {
        crate::app_model::group_windows(windows).remove(0)
    }

    #[test]
    fn alt_widens_only_close_and_minimize_to_the_whole_application() {
        use WindowControlAction::{Close, Minimize, ToggleMaximize};
        assert_eq!(window_control_scope(Close, true), CommandScope::Application);
        assert_eq!(
            window_control_scope(Minimize, true),
            CommandScope::Application
        );
        assert_eq!(window_control_scope(Close, false), CommandScope::Window);
        assert_eq!(window_control_scope(Minimize, false), CommandScope::Window);
        assert_eq!(
            window_control_scope(ToggleMaximize, true),
            CommandScope::Window
        );
    }

    #[test]
    fn application_targets_start_with_the_clicked_window_and_skip_unavailable_ones() {
        let clicked = sibling(1);
        let mut minimized = sibling(2);
        minimized.presence = WindowPresence::Minimized;
        let mut hidden = sibling(3);
        hidden.presence = WindowPresence::Hidden;
        let mut dialog = sibling(4);
        dialog.role = WindowRole::OwnedDialog;
        let mut fixed = sibling(5);
        fixed.traits.closable = false;
        let plain = sibling(6);
        let app = application(vec![
            plain.clone(),
            minimized.clone(),
            hidden,
            dialog,
            fixed.clone(),
            clicked.clone(),
        ]);

        let minimize = application_command_targets(&app, clicked.identity, WindowCommand::Minimize);
        assert_eq!(
            minimize,
            vec![clicked.identity, plain.identity, fixed.identity]
        );

        let close = application_command_targets(&app, clicked.identity, WindowCommand::Close);
        assert_eq!(
            close,
            vec![clicked.identity, plain.identity, minimized.identity]
        );
    }

    #[test]
    fn application_targets_keep_a_clicked_dialog_but_not_other_dialogs() {
        let mut clicked = sibling(1);
        clicked.role = WindowRole::OwnedDialog;
        let mut other_dialog = sibling(2);
        other_dialog.role = WindowRole::OwnedDialog;
        let main = sibling(3);
        let app = application(vec![other_dialog, main.clone(), clicked.clone()]);
        let targets = application_command_targets(&app, clicked.identity, WindowCommand::Close);
        assert_eq!(targets, vec![clicked.identity, main.identity]);
    }

    #[test]
    fn controls_cover_every_visible_captioned_window_even_when_its_actions_are_unavailable() {
        assert!(accepts_window_controls(&window(WindowTraits::STANDARD)));
        let mut borderless = WindowTraits::STANDARD;
        borderless.title_bar = false;
        assert!(!accepts_window_controls(&window(borderless)));
        let mut full = window(WindowTraits::STANDARD);
        full.full_screen = true;
        assert!(!accepts_window_controls(&full));
        let mut inert = WindowTraits::STANDARD;
        inert.closable = false;
        inert.minimizable = false;
        inert.maximizable = false;
        assert!(accepts_window_controls(&window(inert)));
        let mut blocked = WindowTraits::STANDARD;
        blocked.blocked_by_modal = true;
        assert!(accepts_window_controls(&window(blocked)));
        let mut hung = WindowTraits::STANDARD;
        hung.responding = false;
        assert!(accepts_window_controls(&window(hung)));
        let mut elevated = WindowTraits::STANDARD;
        elevated.elevated = true;
        assert!(!accepts_window_controls(&window(elevated)));
    }

    #[test]
    fn disabled_caption_buttons_disable_the_matching_control() {
        let mut dialog = WindowTraits::STANDARD;
        dialog.minimizable = false;
        dialog.maximizable = false;
        let state = WindowControlsState::of(&window(dialog));
        assert!(state.allows(WindowControlAction::Close));
        assert!(!state.allows(WindowControlAction::Minimize));
        assert!(!state.allows(WindowControlAction::ToggleMaximize));
        let mut blocked = WindowTraits::STANDARD;
        blocked.blocked_by_modal = true;
        assert!(!WindowControlsState::of(&window(blocked)).any());
    }

    #[test]
    fn full_screen_capable_window_keeps_the_green_control_without_zoom() {
        let mut browser = WindowTraits::STANDARD;
        browser.maximizable = false;
        browser.full_screen_shortcut = true;
        assert!(WindowControlsState::of(&window(browser)).zoom);
    }

    #[test]
    fn top_is_clamped_below_the_bar_only_on_the_bar_monitor() {
        assert_eq!(window_controls_top(0, 9, Some(48), false), 48);
        assert_eq!(window_controls_top(48, 9, Some(48), true), 57);
        assert_eq!(window_controls_top(300, 9, Some(48), false), 309);
        assert_eq!(window_controls_top(-400, 9, None, false), -391);
        assert_eq!(window_controls_top(0, 9, None, true), 9);
    }

    #[test]
    fn window_controls_hit_test_identifies_red_yellow_green_on_left() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 96);
        let cy = layout.center_y as i32;
        assert_eq!(
            layout.hit_test(layout.centers_x[0] as i32, cy),
            Some(WindowControlAction::Close)
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[1] as i32, cy),
            Some(WindowControlAction::Minimize)
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[2] as i32, cy),
            Some(WindowControlAction::ToggleMaximize)
        );
        assert!(layout.centers_x[0] < layout.centers_x[1]);
        assert!(layout.centers_x[1] < layout.centers_x[2]);
        assert_eq!(layout.hit_test(2, 2), None);
    }

    #[test]
    fn hit_slop_scales_with_the_monitor_dpi() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 192);
        let cy = layout.center_y as i32;
        let just_inside_close = layout.centers_x[0] as i32 + 18;
        assert_eq!(
            layout.hit_test(just_inside_close, cy),
            Some(WindowControlAction::Close)
        );
    }

    #[test]
    fn layout_follows_each_monitor_dpi_so_mixed_dpi_controls_stay_aligned() {
        let metrics = WindowControlsMetrics::MACOS_SONOMA;
        for dpi in [96, 120, 144, 192] {
            let layout = compute_window_controls_layout(&metrics, dpi);
            assert_eq!(layout.width, scale(metrics.width, dpi));
            assert_eq!(layout.height, scale(metrics.height, dpi));
            assert_eq!(layout.offset_y, scale(metrics.offset_y, dpi));
            let last_edge = layout.centers_x[2] + layout.radius;
            assert!(last_edge <= layout.width as f32, "dpi {dpi}");
            assert!(layout.center_y + layout.radius <= layout.height as f32);
            let cy = layout.center_y as i32;
            assert_eq!(
                layout.hit_test(layout.centers_x[2] as i32, cy),
                Some(WindowControlAction::ToggleMaximize),
                "dpi {dpi}"
            );
        }
        let standard = compute_window_controls_layout(&metrics, 96);
        let high = compute_window_controls_layout(&metrics, 192);
        assert_eq!(high.width, standard.width * 2);
        assert!((high.radius - standard.radius * 2.0).abs() <= 1.0);
    }

    const STANDARD_STYLE: NativeCaptionStyle = NativeCaptionStyle {
        has_caption: true,
        has_system_menu: true,
        can_minimize: true,
        can_maximize: true,
    };

    fn caption(style: NativeCaptionStyle) -> NativeCaption {
        NativeCaption::new(style, 1191, 300, 44, true, 144).unwrap()
    }

    #[test]
    fn three_native_buttons_are_right_aligned_to_the_visible_frame() {
        let caption = caption(STANDARD_STYLE);
        let cover = caption.cover_rect();
        assert_eq!(cover.width, 69 * 3);
        assert_eq!(cover.right(), 1191);
        assert_eq!(cover.y, 300);
        assert_eq!(cover.height, 44);
    }

    #[test]
    fn a_close_only_window_covers_a_single_button() {
        let style = NativeCaptionStyle {
            can_minimize: false,
            can_maximize: false,
            ..STANDARD_STYLE
        };
        let caption = caption(style);
        assert_eq!(caption.slots, 1);
        assert_eq!(caption.cover_rect().width, 69);
        assert_eq!(caption.minimize_button(), None);
    }

    #[test]
    fn a_window_with_only_a_maximize_box_still_shows_three_slots_but_no_minimize() {
        let style = NativeCaptionStyle {
            can_minimize: false,
            ..STANDARD_STYLE
        };
        let caption = caption(style);
        assert_eq!(caption.slots, 3);
        assert_eq!(caption.minimize_button(), None);
    }

    #[test]
    fn the_native_minimize_button_is_the_leftmost_native_slot() {
        let caption = caption(STANDARD_STYLE);
        let button = caption.minimize_button().unwrap();
        assert_eq!(button.x, 1191 - 69 * 3);
        assert_eq!(button.width, 69);
        assert!(button.contains(button.x, 300));
        assert!(!button.contains(button.right(), 300));
    }

    #[test]
    fn windows_without_a_caption_or_system_menu_have_no_native_caption() {
        for style in [
            NativeCaptionStyle {
                has_caption: false,
                ..STANDARD_STYLE
            },
            NativeCaptionStyle {
                has_system_menu: false,
                ..STANDARD_STYLE
            },
        ] {
            assert_eq!(NativeCaption::new(style, 1191, 300, 44, true, 144), None);
        }
        assert_eq!(
            NativeCaption::new(STANDARD_STYLE, 1191, 300, 0, true, 144),
            None
        );
    }

    #[test]
    fn a_plausible_measured_width_replaces_the_standard_cover_width() {
        let measured = caption(STANDARD_STYLE).measured(220);
        let cover = measured.cover_rect();
        assert_eq!(cover.width, 220);
        assert_eq!(cover.right(), 1191);
        assert_eq!(measured.minimize_button().unwrap().width, 73);
    }

    #[test]
    fn an_implausible_measured_width_keeps_the_standard_cover_width() {
        for width in [0, 40, 400] {
            let measured = caption(STANDARD_STYLE).measured(width);
            assert_eq!(measured.cover_rect().width, 69 * 3, "width {width}");
        }
    }

    #[test]
    fn a_standard_caption_derives_its_height_from_the_dpi() {
        let caption = NativeCaption::standard(STANDARD_STYLE, 1191, 300, true, 144).unwrap();
        assert_eq!(caption.height, 45);
        assert_eq!(caption.cover_rect().width, 69 * 3);
        assert_eq!(caption.cover_rect().right(), 1191);
        let none = NativeCaptionStyle {
            has_system_menu: false,
            ..STANDARD_STYLE
        };
        assert_eq!(NativeCaption::standard(none, 1191, 300, true, 144), None);
    }

    #[test]
    fn the_corner_radius_follows_rounding_and_dpi() {
        let rounded = caption(STANDARD_STYLE);
        assert!((rounded.corner_radius() - 12.0).abs() < 0.001);
        let square = NativeCaption::new(STANDARD_STYLE, 1191, 300, 44, false, 144).unwrap();
        assert!(square.corner_radius().abs() < 0.001);
    }

    #[test]
    fn the_cover_layout_orders_buttons_like_windows_with_mac_spacing_on_right() {
        let layout =
            compute_cover_layout(caption(STANDARD_STYLE), &WindowControlsMetrics::default())
                .unwrap();
        assert_eq!(layout.actions[0], WindowControlAction::Minimize);
        assert_eq!(layout.actions[1], WindowControlAction::ToggleMaximize);
        assert_eq!(layout.actions[2], WindowControlAction::Close);
        assert_eq!(layout.width, 207);
        assert!((layout.centers_x[0] - 126.0).abs() < 0.01);
        assert!((layout.centers_x[1] - 156.0).abs() < 0.01);
        assert!((layout.centers_x[2] - 186.0).abs() < 0.01);
    }

    #[test]
    fn the_cover_hit_test_matches_the_right_aligned_circle_group_and_leaves_left_cover_empty() {
        let layout =
            compute_cover_layout(caption(STANDARD_STYLE), &WindowControlsMetrics::default())
                .unwrap();
        let cy = layout.center_y as i32;
        assert_eq!(layout.hit_test(10, cy), None);
        assert_eq!(layout.hit_test(90, cy), None);
        assert!(!layout.hits_group(90, cy));
        assert!(layout.hits_group(126, cy));
        assert_eq!(
            layout.hit_test(126, cy),
            Some(WindowControlAction::Minimize)
        );
        assert_eq!(
            layout.hit_test(156, cy),
            Some(WindowControlAction::ToggleMaximize)
        );
        assert_eq!(layout.hit_test(186, cy), Some(WindowControlAction::Close));
        assert_eq!(layout.hit_test(207, cy), None);
        assert_eq!(layout.hit_test(156, 44), None);
        assert_eq!(layout.hit_test(-1, cy), None);
    }

    #[test]
    fn a_single_slot_layout_only_answers_for_close() {
        let style = NativeCaptionStyle {
            can_minimize: false,
            can_maximize: false,
            ..STANDARD_STYLE
        };
        let layout =
            compute_cover_layout(caption(style), &WindowControlsMetrics::default()).unwrap();
        let cy = layout.center_y as i32;
        assert_eq!(layout.slots, 1);
        assert_eq!(layout.hit_test(48, cy), Some(WindowControlAction::Close));
        assert_eq!(layout.hit_test(10, cy), None);
        assert_eq!(layout.hit_test(70, cy), None);
        assert_eq!(layout.center_of(WindowControlAction::Close), Some(48.0));
        assert_eq!(layout.center_of(WindowControlAction::Minimize), None);
    }

    #[test]
    fn inactive_windows_keep_buttons_uncolored_until_hovered() {
        let mut bg = window(WindowTraits::STANDARD);
        bg.active = false;
        let state = WindowControlsState::of(&bg);
        assert!(!state.shows_color(WindowControlAction::Close, false));
        assert!(!state.shows_color(WindowControlAction::Minimize, false));
        assert!(state.shows_color(WindowControlAction::Close, true));
        assert!(state.shows_color(WindowControlAction::Minimize, true));
    }

    #[test]
    fn cover_row_extraction_filters_stray_columns_and_coalesces_bands_with_border_inset() {
        let bgra = [
            10, 20, 30, 255, 10, 20, 30, 255, 250, 250, 250, 255, 40, 50, 60, 255, 40, 50, 60, 255,
            40, 50, 60, 255, 40, 50, 60, 255, 40, 50, 60, 255, 40, 50, 60, 255,
        ];
        let rows = extract_cover_rows(&bgra, 3, 3).unwrap();
        assert_eq!(rows, vec![[30, 20, 10], [60, 50, 40], [60, 50, 40]]);
        let inset_bands = cover_row_bands(&rows, 3, true);
        assert_eq!(
            inset_bands,
            vec![CoverBand {
                top: 1,
                bottom: 3,
                rgb: [60, 50, 40],
            }]
        );
        let full_bands = cover_row_bands(&rows, 3, false);
        assert_eq!(full_bands.len(), 2);
        assert_eq!(full_bands[0].top, 0);
        assert_eq!(full_bands[0].bottom, 1);
        assert_eq!(full_bands[1].top, 1);
        assert_eq!(full_bands[1].bottom, 3);
    }

    #[test]
    fn the_left_layout_keeps_its_circle_hit_test_and_order() {
        let layout = compute_window_controls_layout(&WindowControlsMetrics::default(), 96);
        assert_eq!(layout.actions[0], WindowControlAction::Close);
        assert_eq!(
            layout.center_of(WindowControlAction::ToggleMaximize),
            Some(layout.centers_x[2])
        );
        assert_eq!(
            layout.hit_test(layout.centers_x[1] as i32, layout.center_y as i32),
            Some(WindowControlAction::Minimize)
        );
    }

    #[test]
    fn extract_cover_rows_ignores_embedded_icon_spikes_and_preserves_gradients() {
        let width = 12;
        let height = 5;
        let mut bgra = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            let base = (220 - y * 5) as u8;
            for x in 0..width {
                let (r, g, b) = if (1..=3).contains(&y) && (2..=5).contains(&x) {
                    (220, 40, 40)
                } else {
                    (base, base + 5, base + 10)
                };
                bgra.extend_from_slice(&[b, g, r, 255]);
            }
        }
        let rows = extract_cover_rows(&bgra, width, height).unwrap();
        assert_eq!(rows.len(), height);
        for (y, row) in rows.iter().enumerate() {
            let base = (220 - y * 5) as u8;
            assert_eq!(*row, [base, base + 5, base + 10], "row {y}");
        }
    }

    #[test]
    fn cover_is_dark_distinguishes_dark_and_light_titlebars() {
        assert!(cover_is_dark(&[[30, 30, 34], [32, 32, 36], [34, 34, 38]]));
        assert!(!cover_is_dark(&[
            [240, 240, 244],
            [242, 242, 246],
            [244, 244, 248]
        ]));
        assert!(cover_is_dark(&[]));
    }

    #[test]
    fn clipped_sample_rect_clamps_to_frame_left_and_bar_bottom() {
        let wide = NativeCaption::new(STANDARD_STYLE, 500, 10, 30, true, 96).unwrap();
        let clipped = wide.clipped_sample_rect(0, 36);
        assert_eq!(clipped.x, wide.cover_rect().x - 2 - 24);
        assert_eq!(clipped.width, 24);
        assert_eq!(clipped.y, 36);
        assert_eq!(clipped.height, 4);

        let narrow = NativeCaption::new(STANDARD_STYLE, 160, 10, 30, true, 96).unwrap();
        let clipped_left = narrow.clipped_sample_rect(5, 0);
        assert_eq!(clipped_left.x, 7);
        assert_eq!(clipped_left.right(), narrow.cover_rect().x - 2);

        let hidden = wide.clipped_sample_rect(0, 45);
        assert!(hidden.is_empty());
    }

    #[test]
    fn resize_edge_matches_outer_four_dip_rim_when_unmaximized_and_resizable() {
        let caption = NativeCaption::new(STANDARD_STYLE, 500, 100, 30, true, 96).unwrap();
        let layout = compute_cover_layout(caption, &WindowControlsMetrics::default()).unwrap();
        assert_eq!(
            layout.resize_edge(10, 0, false, true, 96),
            Some(ResizeEdge::Top)
        );
        assert_eq!(
            layout.resize_edge(layout.width - 1, 0, false, true, 96),
            Some(ResizeEdge::TopRight)
        );
        assert_eq!(
            layout.resize_edge(layout.width - 1, 15, false, true, 96),
            Some(ResizeEdge::Right)
        );
        assert_eq!(layout.resize_edge(10, 15, false, true, 96), None);
        assert_eq!(
            layout.resize_edge(layout.centers_x[2] as i32, 0, false, true, 96),
            Some(ResizeEdge::Top)
        );
        assert_eq!(
            layout.resize_edge(
                layout.centers_x[2] as i32,
                layout.center_y as i32,
                false,
                true,
                96
            ),
            None
        );
        assert_eq!(layout.resize_edge(10, 0, true, true, 96), None);
        assert_eq!(layout.resize_edge(10, 0, false, false, 96), None);
    }

    #[test]
    fn window_controls_remain_visible_under_bar_until_bottom_edge_is_covered() {
        assert!(window_controls_visible_under_bar(10, 30, Some(38)));
        assert!(!window_controls_visible_under_bar(8, 30, Some(38)));
        assert!(!window_controls_visible_under_bar(0, 30, Some(38)));
        assert!(window_controls_visible_under_bar(0, 30, None));
    }

    #[test]
    fn alt_zoom_glyph_defaults_to_false_and_toggles_via_builder() {
        let state = WindowControlsState::of(&window(WindowTraits::STANDARD));
        assert!(!state.alt_zoom_glyph);
        assert!(state.with_alt_zoom_glyph(true).alt_zoom_glyph);
        assert!(
            !state
                .with_alt_zoom_glyph(true)
                .with_alt_zoom_glyph(false)
                .alt_zoom_glyph
        );
    }
}
