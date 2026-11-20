use super::ControlCenterRenderer;
use otto_core::{
    BarHideMode, BarItem, DOCK_ICON_LARGE, DOCK_ICON_SMALL, OttoSettingsArea, OttoSettingsSnapshot,
    Rect,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ELLIPSE, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS, D2D1_ROUNDED_RECT,
};
use windows_numerics::Vector2;

const ICON_COLORS: [[f32; 3]; 5] = [
    [0.29, 0.66, 1.0],
    [1.0, 0.42, 0.36],
    [1.0, 0.81, 0.29],
    [0.49, 0.88, 0.63],
    [0.75, 0.55, 1.0],
];
const RED: [f32; 3] = [0.94, 0.27, 0.22];
const GREEN: [f32; 3] = [0.30, 0.85, 0.50];
const AMBER: [f32; 3] = [1.0, 0.62, 0.04];
const BLUE: [f32; 3] = [0.54, 0.70, 1.0];

const fn bounds(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

const fn color(rgb: [f32; 3], a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: rgb[0],
        g: rgb[1],
        b: rgb[2],
        a,
    }
}

const fn white(a: f32) -> D2D1_COLOR_F {
    color([1.0, 1.0, 1.0], a)
}

const fn black(a: f32) -> D2D1_COLOR_F {
    color([0.0, 0.0, 0.0], a)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn pulse(p: f32, rise_from: f32, rise_to: f32, fall_from: f32, fall_to: f32) -> f32 {
    smooth((p - rise_from) / (rise_to - rise_from))
        * (1.0 - smooth((p - fall_from) / (fall_to - fall_from)))
}

fn phase(t: f32, period: f32) -> f32 {
    (t / period).fract()
}

pub(super) fn static_time(area: OttoSettingsArea) -> f32 {
    match area {
        OttoSettingsArea::Dock => 1.2,
        OttoSettingsArea::MenuBar | OttoSettingsArea::Appearance => 0.5,
        OttoSettingsArea::Notch => 1.7,
        OttoSettingsArea::NotchTools => 1.0,
        OttoSettingsArea::System => 2.0,
    }
}

struct Stage<'a> {
    renderer: &'a ControlCenterRenderer,
    rect: D2D_RECT_F,
    u: f32,
    t: f32,
}

impl Stage<'_> {
    fn x(&self, units: f32) -> f32 {
        self.rect.left + units * self.u
    }

    fn top(&self, units: f32) -> f32 {
        self.rect.top + units * self.u
    }

    fn bottom(&self, units: f32) -> f32 {
        self.rect.bottom - units * self.u
    }

    fn mid(&self, units: f32) -> f32 {
        (self.rect.top + self.rect.bottom) * 0.5 + units * self.u
    }

    fn fill_rr(&self, rect: D2D_RECT_F, radius: f32, c: D2D1_COLOR_F) {
        if c.a <= 0.003 || rect.right <= rect.left || rect.bottom <= rect.top {
            return;
        }
        let brush = self.renderer.brushes.color_brush(c);
        unsafe {
            self.renderer.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
            );
        }
    }

    fn stroke_rr(&self, rect: D2D_RECT_F, radius: f32, width: f32, c: D2D1_COLOR_F) {
        if c.a <= 0.003 {
            return;
        }
        let brush = self.renderer.brushes.color_brush(c);
        unsafe {
            self.renderer.target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
                width,
                None,
            );
        }
    }

    fn disc(&self, cx: f32, cy: f32, radius: f32, c: D2D1_COLOR_F) {
        if c.a <= 0.003 {
            return;
        }
        let brush = self.renderer.brushes.color_brush(c);
        unsafe {
            self.renderer.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
            );
        }
    }

    fn ring(&self, cx: f32, cy: f32, radius: f32, width: f32, c: D2D1_COLOR_F) {
        if c.a <= 0.003 {
            return;
        }
        let brush = self.renderer.brushes.color_brush(c);
        unsafe {
            self.renderer.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
                width,
                None,
            );
        }
    }

    fn line(&self, from: (f32, f32), to: (f32, f32), width: f32, c: D2D1_COLOR_F) {
        if c.a <= 0.003 {
            return;
        }
        let brush = self.renderer.brushes.color_brush(c);
        unsafe {
            self.renderer.target.DrawLine(
                Vector2::new(from.0, from.1),
                Vector2::new(to.0, to.1),
                &brush,
                width,
                &self.renderer.stroke_style,
            );
        }
    }

    fn arc(&self, center: (f32, f32), radius: f32, width: f32, sweep_deg: f32, c: D2D1_COLOR_F) {
        let steps = ((sweep_deg.abs() / 12.0).ceil() as usize).max(1);
        let point = |step: usize| {
            let angle = (-90.0 + sweep_deg * step as f32 / steps as f32).to_radians();
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        };
        for step in 0..steps {
            self.line(point(step), point(step + 1), width, c);
        }
    }

    fn glow(&self, cx: f32, cy: f32, radius: f32, rgb: [f32; 3], alpha: f32) {
        const LAYERS: usize = 24;
        for layer in 0..LAYERS {
            let share = 1.0 - layer as f32 / LAYERS as f32;
            self.disc(cx, cy, radius * share, color(rgb, alpha / LAYERS as f32));
        }
    }

    fn label(&self, text: &str, rect: D2D_RECT_F, c: D2D1_COLOR_F) {
        let brush = self.renderer.brushes.color_brush(c);
        self.renderer
            .draw_text(text, &self.renderer.badge_format, rect, &brush);
    }

    fn wallpaper(&self, drift: f32) {
        self.fill_rr(self.rect_all(), 0.0, color([0.11, 0.13, 0.22], 1.0));
        self.glow(
            self.x(18.0 + drift),
            self.mid(-4.0),
            self.u * 34.0,
            [0.30, 0.45, 0.95],
            0.55,
        );
        self.glow(
            self.x(84.0 - drift),
            self.mid(6.0),
            self.u * 30.0,
            [0.85, 0.30, 0.50],
            0.45,
        );
    }

    fn rect_all(&self) -> D2D_RECT_F {
        self.rect
    }

    #[allow(clippy::too_many_lines)]
    fn draw_dock(&self, settings: &OttoSettingsSnapshot) {
        let dock = settings.dock;
        let on = settings.plugins.dock;
        let scale = match dock.icon_size {
            size if size <= DOCK_ICON_SMALL => 0.78,
            size if size >= DOCK_ICON_LARGE => 1.2,
            _ => 1.0,
        };
        let icon = 7.2 * scale;
        let pitch = icon + 1.8;
        let bar_bottom = self.bottom(4.5);
        let taskbar_hidden = on && dock.hide_taskbar;
        if !taskbar_hidden {
            self.fill_rr(
                bounds(
                    self.rect.left,
                    self.bottom(3.0),
                    self.rect.right,
                    self.rect.bottom,
                ),
                0.0,
                black(0.5),
            );
        }
        let extras = usize::from(dock.show_downloads) + usize::from(dock.show_trash);
        let count = ICON_COLORS.len() + extras;
        let separator = if extras > 0 { 2.6 } else { 0.0 };
        let bar_w = count as f32 * pitch + 3.6 * 2.0 - 1.8 + separator;
        let bar_left = 50.0 - bar_w / 2.0;
        let bar_top_units = icon + 3.6;
        let p = phase(self.t, 4.0);
        let hide = if on && dock.autohide {
            pulse(p, 0.40, 0.52, 0.78, 0.90)
        } else {
            0.0
        };
        let drop = hide * (bar_top_units + 7.0) * self.u;
        let reserve = on && dock.hide_taskbar && dock.reserve_space;
        let window_bottom = if reserve {
            bar_bottom - bar_top_units * self.u - 1.5 * self.u
        } else {
            self.rect.bottom + 2.0
        };
        self.fill_rr(
            bounds(self.x(10.0), self.top(5.0), self.x(90.0), window_bottom),
            self.u * 1.6,
            white(0.10),
        );
        self.fill_rr(
            bounds(self.x(12.5), self.top(7.0), self.x(22.0), self.top(8.2)),
            self.u * 0.6,
            white(0.4),
        );
        let alpha = if on { 1.0 } else { 0.16 };
        self.fill_rr(
            bounds(
                self.x(bar_left),
                bar_bottom - bar_top_units * self.u + drop,
                self.x(bar_left + bar_w),
                bar_bottom + drop,
            ),
            self.u * 3.2,
            white(0.18 * alpha),
        );
        let sweeping = on && p < 0.58;
        let sweep_x = self.x(lerp(bar_left, bar_left + bar_w, (p / 0.56).clamp(0.0, 1.0)));
        let mut cursor = bar_left + 3.6;
        for index in 0..count {
            if index == ICON_COLORS.len() && extras > 0 {
                self.fill_rr(
                    bounds(
                        self.x(cursor - 0.2),
                        bar_bottom - (bar_top_units - 1.4) * self.u + drop,
                        self.x(cursor + 0.3),
                        bar_bottom - 1.4 * self.u + drop,
                    ),
                    self.u * 0.25,
                    white(0.3 * alpha),
                );
                cursor += separator;
            }
            let rgb = match index {
                i if i < ICON_COLORS.len() => ICON_COLORS[i],
                i if i == ICON_COLORS.len() && dock.show_downloads => [0.36, 0.42, 0.55],
                _ => [0.55, 0.57, 0.60],
            };
            let center = cursor + icon / 2.0;
            let grow = if dock.magnification && sweeping {
                let d = ((self.x(center) - sweep_x).abs() / (pitch * self.u * 2.4)).min(1.0);
                1.0 + 0.6 * (1.0 - d) * (1.0 - d)
            } else {
                1.0
            };
            let size = icon * grow;
            let base = bar_bottom - 1.8 * self.u + drop;
            self.fill_rr(
                bounds(
                    self.x(center - size / 2.0),
                    base - size * self.u,
                    self.x(center + size / 2.0),
                    base,
                ),
                self.u * 1.9 * grow,
                color(rgb, alpha),
            );
            cursor += pitch;
        }
        if sweeping {
            self.disc(
                sweep_x,
                bar_bottom - bar_top_units * self.u * 0.35,
                self.u * 1.2,
                white(0.95),
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn draw_menu_bar(&self, settings: &OttoSettingsSnapshot) {
        let prefs = settings.menu_bar;
        let on = settings.plugins.top_bar;
        let master = if on { 1.0 } else { 0.25 };
        let bar_h = 6.4 * self.u;
        let p = phase(self.t, 4.5);
        let slide = if on && prefs.hide_mode != BarHideMode::Never {
            pulse(p, 0.28, 0.40, 0.76, 0.90)
        } else {
            0.0
        };
        let bar_dy = -slide * (bar_h + self.u);
        let notch_dy = if !on || prefs.keep_notch { 0.0 } else { bar_dy };
        if prefs.hide_mode == BarHideMode::FullScreenOnly {
            self.fill_rr(self.rect_all(), 0.0, white(0.12 * slide));
            self.stroke_rr(self.rect_all(), 0.0, 1.0, white(0.2 * slide));
        }
        if prefs.show_background {
            self.fill_rr(
                bounds(
                    self.rect.left,
                    self.rect.top + bar_dy,
                    self.rect.right,
                    self.rect.top + bar_h + bar_dy,
                ),
                0.0,
                white(0.18 * master),
            );
        }
        let cy = self.rect.top + bar_h / 2.0 + bar_dy;
        self.disc(self.x(4.2), cy, self.u * 1.4, color(RED, master));
        let mut text_x = 8.0;
        for width in [6.0, 4.5, 5.0] {
            self.fill_rr(
                bounds(
                    self.x(text_x),
                    cy - self.u * 0.55,
                    self.x(text_x + width),
                    cy + self.u * 0.55,
                ),
                self.u * 0.5,
                white(0.55 * master),
            );
            text_x += width + 1.6;
        }
        let mut right = 96.0;
        let gap = 1.8;
        let item = |kind: BarItem| !prefs.hidden_items.contains(kind);
        let ink = white(0.8 * master);
        if item(BarItem::Clock) {
            self.fill_rr(
                bounds(
                    self.x(right - 5.4),
                    cy - self.u * 0.7,
                    self.x(right),
                    cy + self.u * 0.7,
                ),
                self.u * 0.5,
                ink,
            );
            right -= 5.4 + gap;
        }
        if item(BarItem::Notifications) {
            self.fill_rr(
                bounds(
                    self.x(right - 2.2),
                    cy - self.u * 1.1,
                    self.x(right),
                    cy + self.u * 1.1,
                ),
                self.u * 0.9,
                ink,
            );
            right -= 2.2 + gap;
        }
        if item(BarItem::Search) {
            self.ring(self.x(right - 1.1), cy, self.u * 0.95, 1.2, ink);
            right -= 2.2 + gap;
        }
        if item(BarItem::Status) {
            self.fill_rr(
                bounds(
                    self.x(right - 3.4),
                    cy - self.u * 0.8,
                    self.x(right),
                    cy + self.u * 0.8,
                ),
                self.u * 0.4,
                ink,
            );
            self.disc(self.x(right - 5.2), cy, self.u * 0.85, ink);
            self.disc(self.x(right - 7.6), cy, self.u * 0.85, ink);
            right -= 8.8 + gap;
        }
        if item(BarItem::Tray) {
            self.disc(self.x(right - 0.9), cy, self.u * 0.85, ink);
            self.disc(self.x(right - 3.2), cy, self.u * 0.85, ink);
        }
        if settings.plugins.notch {
            self.fill_rr(
                bounds(
                    self.x(40.0),
                    self.rect.top - 6.0 * self.u + notch_dy,
                    self.x(60.0),
                    self.rect.top + 5.8 * self.u + notch_dy,
                ),
                self.u * 3.0,
                black(1.0),
            );
        }
        if on
            && matches!(
                prefs.hide_mode,
                BarHideMode::Always | BarHideMode::DesktopOnly
            )
        {
            let reveal = pulse(p, 0.66, 0.72, 0.88, 0.94);
            self.disc(
                self.x(66.0),
                self.top(lerp(16.0, 2.4, reveal)),
                self.u * 1.1,
                white(0.9 * reveal),
            );
        }
    }

    fn draw_notch(&self, settings: &OttoSettingsSnapshot) {
        let on = settings.plugins.notch;
        let master = if on { 1.0 } else { 0.18 };
        let on_hud = on && settings.system.notch_hud;
        let island = settings.system.dynamic_island;
        if settings.plugins.top_bar {
            self.fill_rr(
                bounds(
                    self.rect.left,
                    self.rect.top,
                    self.rect.right,
                    self.top(6.4),
                ),
                0.0,
                white(0.08),
            );
        }
        let p = phase(self.t, 5.0);
        let expand = if on_hud {
            pulse(p, 0.14, 0.26, 0.66, 0.80)
        } else {
            0.0
        };
        let width = lerp(22.0, 50.0, expand);
        let height = lerp(5.8, 13.0, expand);
        let (top, radius) = if island {
            (self.top(2.4), self.u * height / 2.0)
        } else {
            (self.rect.top - 6.0 * self.u, self.u * 3.0)
        };
        let bottom = self.top(if island { 2.4 + height } else { height });
        let left = self.x(50.0 - width / 2.0);
        let right = self.x(50.0 + width / 2.0);
        self.fill_rr(bounds(left, top, right, bottom), radius, black(master));
        if expand > 0.6 {
            let fade = (expand - 0.6) / 0.4;
            let cy = bottom - (height / 2.0) * self.u;
            let icon_x = left + self.u * 4.5;
            self.fill_rr(
                bounds(
                    icon_x,
                    cy - self.u * 0.9,
                    icon_x + self.u * 1.4,
                    cy + self.u * 0.9,
                ),
                self.u * 0.3,
                white(fade),
            );
            self.line(
                (icon_x + self.u * 1.4, cy - self.u * 0.9),
                (icon_x + self.u * 3.0, cy - self.u * 1.8),
                1.4,
                white(fade),
            );
            self.line(
                (icon_x + self.u * 1.4, cy + self.u * 0.9),
                (icon_x + self.u * 3.0, cy + self.u * 1.8),
                1.4,
                white(fade),
            );
            self.line(
                (icon_x + self.u * 3.0, cy - self.u * 1.8),
                (icon_x + self.u * 3.0, cy + self.u * 1.8),
                1.4,
                white(fade),
            );
            let track_l = icon_x + self.u * 5.0;
            let track_r = right - self.u * 4.5;
            let fill = 0.3 + 0.5 * pulse(p, 0.30, 0.50, 0.58, 0.68);
            self.fill_rr(
                bounds(track_l, cy - self.u * 0.8, track_r, cy + self.u * 0.8),
                self.u * 0.8,
                white(0.25 * fade),
            );
            self.fill_rr(
                bounds(
                    track_l,
                    cy - self.u * 0.8,
                    lerp(track_l, track_r, fill),
                    cy + self.u * 0.8,
                ),
                self.u * 0.8,
                white(0.9 * fade),
            );
        }
        if on && settings.system.privacy_indicators {
            let beat = 0.65 + 0.35 * (self.t * 4.0).sin();
            self.disc(
                self.x(94.0),
                self.top(3.2),
                self.u * 1.1,
                color(AMBER, beat),
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn draw_notch_tools(&self, settings: &OttoSettingsSnapshot) {
        let on = settings.plugins.notch;
        let master = if on { 1.0 } else { 0.22 };
        self.fill_rr(
            bounds(
                self.x(6.0),
                self.rect.top - 6.0 * self.u,
                self.x(94.0),
                self.top(27.0),
            ),
            self.u * 5.0,
            black(master),
        );
        if !on {
            return;
        }
        let media = settings.system.media_module;
        let tiles = [
            media,
            settings.system.focus_module,
            settings.system.clipboard_history,
            settings.system.drop_shelf,
        ];
        let enabled = tiles.iter().filter(|on| **on).count();
        if enabled == 0 {
            return;
        }
        let gap = 2.0;
        let span = 78.0;
        let tile_w = (span - gap * (enabled as f32 - 1.0)) / enabled as f32;
        let mut left = 11.0;
        for (index, on) in tiles.into_iter().enumerate() {
            if !on {
                continue;
            }
            let tile = bounds(
                self.x(left),
                self.top(3.2),
                self.x(left + tile_w),
                self.top(24.0),
            );
            self.fill_rr(tile, self.u * 2.4, white(0.09));
            let (l, t) = (tile.left, tile.top);
            match index {
                0 => self.media_tile(tile, settings.online_lyrics),
                1 => {
                    let center = (f32::midpoint(tile.left, tile.right), t + self.u * 9.5);
                    self.ring(center.0, center.1, self.u * 4.6, 2.0, white(0.16));
                    let progress = 0.25 + 0.5 * phase(self.t, 6.0);
                    self.arc(center, self.u * 4.6, 2.0, 360.0 * progress, color(RED, 1.0));
                    self.label(
                        "24:10",
                        bounds(
                            l,
                            tile.bottom - self.u * 5.2,
                            tile.right,
                            tile.bottom - self.u,
                        ),
                        white(0.8),
                    );
                }
                2 => {
                    let slide = 1.0 - smooth((self.t % 3.0) / 1.2);
                    for row in 0..4 {
                        let y = t + self.u * (3.0 + 4.2 * row as f32);
                        let (shift, alpha, w) = if row == 0 {
                            (-slide * 4.0, 0.55, 1.0)
                        } else {
                            (0.0, 0.2, [0.0, 0.6, 0.75, 0.4][row])
                        };
                        self.fill_rr(
                            bounds(
                                l + self.u * (1.8 + shift),
                                y,
                                l + self.u * (1.8 + shift) + (tile.right - l - self.u * 3.6) * w,
                                y + self.u * 2.2,
                            ),
                            self.u * 0.8,
                            white(alpha * (1.0 - slide * 0.6)),
                        );
                    }
                }
                _ => {
                    self.stroke_rr(
                        bounds(
                            l + self.u * 1.8,
                            t + self.u * 1.8,
                            tile.right - self.u * 1.8,
                            tile.bottom - self.u * 1.8,
                        ),
                        self.u * 1.6,
                        1.0,
                        white(0.3),
                    );
                    let drop = smooth((self.t % 3.0) / 1.5);
                    let cx = f32::midpoint(tile.left, tile.right);
                    let cy = lerp(t - self.u * 4.0, t + self.u * 10.4, drop);
                    self.fill_rr(
                        bounds(
                            cx - self.u * 3.0,
                            cy - self.u * 3.4,
                            cx + self.u * 3.0,
                            cy + self.u * 3.4,
                        ),
                        self.u,
                        color(BLUE, 0.5 + 0.5 * drop),
                    );
                }
            }
            left += tile_w + gap;
        }
    }

    fn media_tile(&self, tile: D2D_RECT_F, lyrics: bool) {
        let (l, t) = (tile.left, tile.top);
        self.fill_rr(
            bounds(
                l + self.u * 1.8,
                t + self.u * 1.8,
                l + self.u * 8.0,
                t + self.u * 8.0,
            ),
            self.u * 1.2,
            color([1.0, 0.42, 0.36], 1.0),
        );
        for bar in 0..4 {
            let height = 1.6 + 3.4 * (0.5 + 0.5 * (self.t * 6.0 + bar as f32 * 1.7).sin());
            let x = tile.right - self.u * (2.4 + 1.8 * bar as f32);
            self.fill_rr(
                bounds(
                    x - self.u * 0.9,
                    t + self.u * (8.0 - height),
                    x,
                    t + self.u * 8.0,
                ),
                self.u * 0.3,
                color(GREEN, 0.9),
            );
        }
        let track = t + self.u * 11.0;
        self.fill_rr(
            bounds(
                l + self.u * 1.8,
                track,
                tile.right - self.u * 1.8,
                track + self.u * 0.9,
            ),
            self.u * 0.4,
            white(0.2),
        );
        self.fill_rr(
            bounds(
                l + self.u * 1.8,
                track,
                lerp(l + self.u * 1.8, tile.right - self.u * 1.8, 0.45),
                track + self.u * 0.9,
            ),
            self.u * 0.4,
            white(0.9),
        );
        if lyrics {
            self.fill_rr(
                bounds(
                    l + self.u * 1.8,
                    tile.bottom - self.u * 4.6,
                    tile.right - self.u * 3.0,
                    tile.bottom - self.u * 3.2,
                ),
                self.u * 0.6,
                color([1.0, 0.82, 0.80], 0.85),
            );
        }
    }

    fn draw_appearance(&self, settings: &OttoSettingsSnapshot) {
        let wave = (self.t * 0.6).sin();
        let p = phase(self.t, 6.0);
        let genie = settings.plugins.genie;
        let minimize = pulse(p, 0.30, 0.50, 0.72, 0.88);
        let dock_center = 50.0;
        let dock_top = self.bottom(7.4);
        self.fill_rr(
            bounds(
                self.x(dock_center - 12.0),
                dock_top,
                self.x(dock_center + 12.0),
                self.bottom(2.8),
            ),
            self.u * 2.4,
            white(0.2),
        );
        let (mut left, mut right) = (self.x(22.0), self.x(78.0));
        let (mut top, mut bottom) = (self.mid(-11.0), self.mid(8.0));
        let mut body_alpha = 1.0;
        if genie {
            let lower = smooth(minimize * 1.3);
            let upper = smooth(minimize * 1.3 - 0.3);
            let tip_l = self.x(dock_center - 2.0);
            let tip_r = self.x(dock_center + 2.0);
            left = lerp(left, tip_l, lower);
            right = lerp(right, tip_r, lower);
            bottom = lerp(bottom, dock_top, lower);
            top = lerp(top, dock_top - self.u, upper);
        } else {
            body_alpha = 1.0 - minimize;
        }
        self.glow(
            self.x(20.0 + 6.0 * wave),
            self.mid(-6.0),
            self.u * 24.0,
            [1.0, 0.42, 0.36],
            0.45,
        );
        self.glow(
            self.x(82.0 - 6.0 * wave),
            self.mid(8.0),
            self.u * 22.0,
            [0.29, 0.66, 1.0],
            0.45,
        );
        let body = bounds(left, top, right, bottom);
        self.fill_rr(body, self.u * 2.4, white(0.15 * body_alpha));
        self.stroke_rr(body, self.u * 2.4, 1.0, white(0.3 * body_alpha));
        let deco = body_alpha * (1.0 - (minimize / 0.2).min(1.0));
        if deco <= 0.0 {
            return;
        }
        if settings.plugins.window_controls {
            for (index, rgb) in [RED, [1.0, 0.74, 0.18], [0.16, 0.78, 0.25]]
                .into_iter()
                .enumerate()
            {
                self.disc(
                    left + self.u * (3.0 + 2.6 * index as f32),
                    top + self.u * 3.0,
                    self.u * 1.0,
                    color(rgb, deco),
                );
            }
        }
        if settings.system.classic_logo {
            self.disc(
                right - self.u * 4.0,
                top + self.u * 3.0,
                self.u * 1.7,
                color(RED, deco),
            );
        } else {
            self.ring(
                right - self.u * 4.0,
                top + self.u * 3.0,
                self.u * 1.5,
                self.u * 0.6,
                color(RED, deco),
            );
        }
        for (row, width) in [1.0, 0.7, 0.45].into_iter().enumerate() {
            let y = top + self.u * (8.0 + 3.0 * row as f32);
            self.fill_rr(
                bounds(
                    left + self.u * 3.0,
                    y,
                    left + self.u * 3.0 + (right - left - self.u * 6.0) * width,
                    y + self.u * 1.3,
                ),
                self.u * 0.6,
                white(0.38 * deco),
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn draw_system(&self, settings: &OttoSettingsSnapshot) {
        let tile_w = 20.75;
        let tile_h = 22.0;
        let top = self.mid(-tile_h / 2.0 + 0.0);
        let bottom = top + tile_h * self.u;
        let flags = [
            settings.system.launch_at_login,
            settings.system.hotkeys_enabled,
            settings.system.capture_visible,
            settings.system.background_cleanup,
        ];
        let names = ["Login", "Keys", "Shot", "CPU"];
        for (index, on) in flags.into_iter().enumerate() {
            let left = 4.0 + index as f32 * (tile_w + 3.0);
            let tile = bounds(self.x(left), top, self.x(left + tile_w), bottom);
            self.fill_rr(tile, self.u * 2.6, black(0.38));
            self.stroke_rr(tile, self.u * 2.6, 1.0, white(0.14));
            let cx = self.x(left + tile_w / 2.0);
            let cy = top + self.u * 9.0;
            match index {
                0 => {
                    let sweep = if on {
                        300.0 * smooth((self.t % 3.0) / 2.0)
                    } else {
                        300.0
                    };
                    let ink = if on { color(GREEN, 1.0) } else { white(0.3) };
                    self.arc((cx, cy + self.u * 0.6), self.u * 3.8, 1.8, sweep, ink);
                    self.line((cx, cy - self.u * 4.6), (cx, cy - self.u * 1.0), 1.8, ink);
                }
                1 => {
                    let key = bounds(
                        cx - self.u * 6.4,
                        cy - self.u * 3.0,
                        cx + self.u * 6.4,
                        cy + self.u * 3.0,
                    );
                    let glow = if on {
                        0.12 + 0.1 * (self.t * 2.6).sin().abs()
                    } else {
                        0.0
                    };
                    self.fill_rr(key, self.u * 1.4, color(BLUE, glow));
                    self.stroke_rr(
                        key,
                        self.u * 1.4,
                        1.2,
                        if on { color(BLUE, 1.0) } else { white(0.3) },
                    );
                    self.label("Alt", key, white(if on { 1.0 } else { 0.4 }));
                }
                2 => {
                    let ink = white(0.7);
                    let f = (
                        cx - self.u * 6.0,
                        cy - self.u * 4.4,
                        cx + self.u * 6.0,
                        cy + self.u * 4.4,
                    );
                    let arm = self.u * 2.4;
                    for (px, py, dx, dy) in [
                        (f.0, f.1, 1.0, 1.0),
                        (f.2, f.1, -1.0, 1.0),
                        (f.0, f.3, 1.0, -1.0),
                        (f.2, f.3, -1.0, -1.0),
                    ] {
                        self.line((px, py), (px + dx * arm, py), 1.4, ink);
                        self.line((px, py), (px, py + dy * arm), 1.4, ink);
                    }
                    if on {
                        self.fill_rr(
                            bounds(
                                cx - self.u * 3.6,
                                cy - self.u * 1.5,
                                cx + self.u * 3.6,
                                cy + self.u * 1.5,
                            ),
                            self.u * 1.5,
                            color(RED, 1.0),
                        );
                    }
                }
                _ => {
                    let throttle = if on {
                        smooth((self.t % 3.5) / 1.5)
                    } else {
                        0.0
                    };
                    let base = cy + self.u * 4.4;
                    for (bar, height) in
                        [4.0, 6.5, lerp(9.0, 3.5, throttle)].into_iter().enumerate()
                    {
                        let x = cx + self.u * (-4.2 + 3.2 * bar as f32);
                        let rgb = if bar == 2 {
                            [
                                lerp(RED[0], GREEN[0], throttle),
                                lerp(RED[1], GREEN[1], throttle),
                                lerp(RED[2], GREEN[2], throttle),
                            ]
                        } else {
                            [1.0, 1.0, 1.0]
                        };
                        self.fill_rr(
                            bounds(
                                x - self.u * 1.0,
                                base - self.u * height,
                                x + self.u * 1.0,
                                base,
                            ),
                            self.u * 0.4,
                            color(rgb, if bar == 2 { 1.0 } else { 0.6 }),
                        );
                    }
                }
            }
            self.label(
                names[index],
                bounds(
                    tile.left,
                    tile.bottom - self.u * 5.0,
                    tile.right,
                    tile.bottom - self.u,
                ),
                white(0.75),
            );
        }
    }
}

impl ControlCenterRenderer {
    pub(super) fn draw_area_scene(
        &self,
        area: OttoSettingsArea,
        scene: Rect,
        card: Rect,
        radius: f32,
        time: f32,
        settings: &OttoSettingsSnapshot,
    ) {
        let rect = D2D_RECT_F {
            left: scene.x as f32,
            top: scene.y as f32,
            right: scene.right() as f32,
            bottom: scene.bottom() as f32,
        };
        let card_rect = D2D_RECT_F {
            left: card.x as f32,
            top: card.y as f32,
            right: card.right() as f32,
            bottom: card.bottom() as f32,
        };
        let stage = Stage {
            renderer: self,
            rect,
            u: scene.width as f32 / 100.0,
            t: time,
        };
        self.with_card_clip(rect, card_rect, radius, || {
            stage.wallpaper(if area == OttoSettingsArea::Appearance {
                0.0
            } else {
                (time * 0.4).sin() * 2.0
            });
            match area {
                OttoSettingsArea::Dock => stage.draw_dock(settings),
                OttoSettingsArea::MenuBar => stage.draw_menu_bar(settings),
                OttoSettingsArea::Notch => stage.draw_notch(settings),
                OttoSettingsArea::NotchTools => stage.draw_notch_tools(settings),
                OttoSettingsArea::Appearance => stage.draw_appearance(settings),
                OttoSettingsArea::System => stage.draw_system(settings),
            }
        });
    }

    fn with_card_clip(
        &self,
        bounds: D2D_RECT_F,
        card: D2D_RECT_F,
        radius: f32,
        draw: impl FnOnce(),
    ) {
        unsafe {
            self.target
                .PushAxisAlignedClip(&bounds, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
        let geometry = self.list_group_clip_geometry(card, radius);
        let layer = geometry
            .as_ref()
            .and_then(|_| unsafe { self.target.CreateLayer(None) }.ok());
        let mut active_mask = None;
        if let (Some(geometry), Some(layer)) = (geometry, layer.as_ref()) {
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: card,
                geometricMask: std::mem::ManuallyDrop::new(Some(geometry)),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: windows_numerics::Matrix3x2::identity(),
                opacity: 1.0,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            unsafe {
                self.target.PushLayer(&params, layer);
            }
            active_mask = Some(params.geometricMask);
        }
        draw();
        if let Some(mask) = active_mask {
            unsafe {
                self.target.PopLayer();
            }
            std::mem::ManuallyDrop::into_inner(mask);
        }
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }
}
