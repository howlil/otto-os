use otto_core::{
    HitTarget, Layout, NotchExpandedTab, POMODORO_EXTEND_LABEL, PomodoroProgress, PomodoroScene,
    PomodoroStatus, PomodoroSurface, PomodoroTone, PomodoroView, Rect, scale,
};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::palette::TonePalette;
use crate::appearance::tokens::{
    LONG_BREAK_BLUE as LONG_BREAK_ACCENT, SYSTEM_GREEN as SHORT_BREAK_ACCENT,
    URGENT_CORAL as CORAL, category_d2d_color,
};
use crate::kit::draw::center_of;
use crate::{
    BLACK, FOREGROUND, GLYPH_STROKE_SCALE, Renderer, brighten, rect_to_d2d, translate_rect,
};

fn pomodoro_colors_with_focus(
    tone: PomodoroTone,
    focus_accent: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    dim: f32,
    urgent: bool,
) -> TonePalette {
    let accent = match tone {
        PomodoroTone::Focus => focus_accent,
        PomodoroTone::ShortBreak => SHORT_BREAK_ACCENT,
        PomodoroTone::LongBreak => LONG_BREAK_ACCENT,
    };
    let mut colors = TonePalette {
        from: accent,
        to: accent,
        accent,
        text: FOREGROUND,
    };
    if urgent {
        colors.from = CORAL;
        colors.to = CORAL;
        colors.accent = CORAL;
        colors.text = CORAL;
    }
    colors.dimmed(dim)
}

impl Renderer {
    fn resolve_pomodoro_colors(
        &self,
        tone: PomodoroTone,
        scene: &PomodoroScene,
        dim: f32,
        urgent: bool,
    ) -> TonePalette {
        let focus_accent = category_d2d_color(scene.category_color);
        let mut colors = pomodoro_colors_with_focus(tone, focus_accent, dim, urgent);
        if self.high_contrast.get() && !urgent {
            colors.text = self.notch_ink();
        }
        colors
    }

    fn contrast_ink_on(
        bg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    ) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
        if bg.r * 0.2126 + bg.g * 0.7152 + bg.b * 0.0722 < 0.4 {
            FOREGROUND
        } else {
            BLACK
        }
    }

    fn icon_stroke(size: f32) -> f32 {
        (size * GLYPH_STROKE_SCALE).max(1.5)
    }

    fn draw_clock_glyph(&self, center: Vector2, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = Self::icon_stroke(r);
        unsafe {
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: r,
                    radiusY: r,
                },
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                center,
                Vector2::new(center.X, center.Y - r * 0.6),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                center,
                Vector2::new(center.X + r * 0.45, center.Y + r * 0.2),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    fn draw_tomato_glyph(&self, center: Vector2, g: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = Self::icon_stroke(g);
        let body = D2D1_ELLIPSE {
            point: Vector2::new(center.X, center.Y + g * 0.14),
            radiusX: g * 0.95,
            radiusY: g * 0.82,
        };
        let stem_base = Vector2::new(center.X, center.Y - g * 0.62);
        unsafe {
            self.target
                .DrawEllipse(&body, brush, stroke, &self.stroke_style);
            self.target.DrawLine(
                stem_base,
                Vector2::new(center.X, center.Y - g * 1.05),
                brush,
                stroke,
                &self.stroke_style,
            );
            for dx in [-0.5, 0.5] {
                self.target.DrawLine(
                    stem_base,
                    Vector2::new(center.X + g * dx, center.Y - g * 0.92),
                    brush,
                    stroke,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_cup_glyph(&self, center: Vector2, g: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = Self::icon_stroke(g);
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: center.X - g * 0.85,
                top: center.Y - g * 0.15,
                right: center.X + g * 0.55,
                bottom: center.Y + g * 0.85,
            },
            radiusX: g * 0.28,
            radiusY: g * 0.28,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&body, brush, stroke, &self.stroke_style);
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(center.X + g * 0.62, center.Y + g * 0.36),
                    radiusX: g * 0.32,
                    radiusY: g * 0.28,
                },
                brush,
                stroke,
                &self.stroke_style,
            );
            for dx in [-0.4, 0.05] {
                self.target.DrawLine(
                    Vector2::new(center.X + g * dx, center.Y - g * 0.4),
                    Vector2::new(center.X + g * dx, center.Y - g * 0.85),
                    brush,
                    stroke,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_moon_glyph(&self, center: Vector2, g: f32, brush: &ID2D1SolidColorBrush) -> Result<()> {
        crate::draw_crescent_moon_outline(
            &self.factory,
            &self.target,
            center,
            g,
            Self::icon_stroke(g),
            &self.stroke_style,
            brush,
        )
    }

    fn draw_check_glyph(&self, center: Vector2, g: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = Self::icon_stroke(g);
        let left = Vector2::new(center.X - g * 0.55, center.Y + g * 0.05);
        let valley = Vector2::new(center.X - g * 0.12, center.Y + g * 0.5);
        let right = Vector2::new(center.X + g * 0.6, center.Y - g * 0.45);
        unsafe {
            self.target
                .DrawLine(left, valley, brush, stroke, &self.stroke_style);
            self.target
                .DrawLine(valley, right, brush, stroke, &self.stroke_style);
        }
    }

    fn draw_tone_glyph(
        &self,
        center: Vector2,
        g: f32,
        tone: PomodoroTone,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        match tone {
            PomodoroTone::Focus => self.draw_tomato_glyph(center, g, brush),
            PomodoroTone::ShortBreak => self.draw_cup_glyph(center, g, brush),
            PomodoroTone::LongBreak => self.draw_moon_glyph(center, g, brush)?,
        }
        Ok(())
    }

    pub(crate) fn draw_compact_pomodoro(
        &self,
        layout: &Layout,
        scene: &PomodoroScene,
        alpha_scale: f32,
    ) -> Result<()> {
        let alpha = ((layout.compact_expansion - 0.15) / 0.85).clamp(0.0, 1.0)
            * alpha_scale.clamp(0.0, 1.0);
        if alpha <= 0.005 {
            return Ok(());
        }
        let view = scene.view();
        let colors = self.resolve_pomodoro_colors(view.tone, scene, scene.dim, view.urgent);
        let (ring_progress, ring_status, time_text) = scene.compact_ring();
        let center = center_of(layout.compact_ring);
        let radius = scale(7, self.dpi) as f32;
        let stroke = (scale(5, self.dpi) as f32 * 0.5).max(2.0);
        let ink = self.notch_ink();

        let accent = self.brushes.alpha_brush(colors.accent, alpha);
        let track = self.brushes.alpha_brush(ink, 0.18 * alpha);
        self.draw_progress_ring(
            center.X,
            center.Y,
            radius,
            stroke,
            ring_progress,
            &accent,
            &track,
        )?;
        if ring_status == PomodoroStatus::Paused {
            let glyph = self.brushes.alpha_brush(colors.text, 0.92 * alpha);
            self.draw_pause_glyph(center.X, center.Y, radius * 0.5, &glyph);
        } else {
            let dot = D2D1_ELLIPSE {
                point: center,
                radiusX: stroke * 0.75,
                radiusY: stroke * 0.75,
            };
            unsafe {
                self.target.FillEllipse(&dot, &accent);
            }
        }

        let time_brush = self.brushes.alpha_brush(colors.text, alpha);
        self.draw_tabular_text(
            &time_text,
            &self.text_compact_time,
            rect_to_d2d(layout.compact_time),
            &time_brush,
        );

        if let Some(frame) = &scene.peek {
            let peek_brush = self
                .brushes
                .alpha_brush(colors.text, frame.alpha.clamp(0.0, 1.0) * alpha);
            self.draw_text(
                &frame.text,
                &self.text_tab,
                rect_to_d2d(layout.compact_peek),
                &peek_brush,
            );
        } else {
            let fills = scene.compact_cycle_progress();
            let track = self.brushes.alpha_brush(ink, 0.16 * alpha);
            for (rect, fill) in layout.compact_cycle_steps.iter().zip(fills) {
                let radius = rect.height as f32 * 0.5;
                self.draw_rounded_box(*rect, radius, &track);
                if fill <= 0.0 {
                    continue;
                }
                let width = (rect.width as f32 * fill.clamp(0.0, 1.0))
                    .round()
                    .max(rect.height as f32)
                    .min(rect.width as f32);
                let strength = if fill >= 0.999 { 0.55 } else { 1.0 };
                self.fill_gradient_rounded(
                    D2D_RECT_F {
                        left: rect.x as f32,
                        top: rect.y as f32,
                        right: rect.x as f32 + width,
                        bottom: rect.bottom() as f32,
                    },
                    radius,
                    &colors,
                    strength * alpha,
                );
            }
        }

        if view.urgent {
            self.draw_urgent_rim(layout, scene.pulse * alpha)?;
        }
        Ok(())
    }

    pub(crate) fn draw_secondary_pomodoro_circle(
        &self,
        layout: &Layout,
        scene: &PomodoroScene,
        alpha_scale: f32,
    ) -> Result<()> {
        let alpha = ((layout.secondary_expansion - 0.15) / 0.85).clamp(0.0, 1.0)
            * alpha_scale.clamp(0.0, 1.0);
        if alpha <= 0.005 || layout.notch_secondary.is_empty() {
            return Ok(());
        }
        let view = scene.view();
        let colors = self.resolve_pomodoro_colors(view.tone, scene, scene.dim, view.urgent);
        let (ring_progress, ring_status, _) = scene.compact_ring();
        let center = center_of(layout.notch_secondary);
        let radius = (layout.notch_secondary.width as f32 * 0.28).max(scale(6, self.dpi) as f32);
        let stroke = (scale(5, self.dpi) as f32 * 0.5).max(2.0);
        let accent = self.brushes.alpha_brush(colors.accent, alpha);
        let track = self.brushes.alpha_brush(self.notch_ink(), 0.18 * alpha);
        self.draw_progress_ring(
            center.X,
            center.Y,
            radius,
            stroke,
            ring_progress,
            &accent,
            &track,
        )?;
        if ring_status == PomodoroStatus::Paused {
            let glyph = self.brushes.alpha_brush(colors.text, 0.92 * alpha);
            self.draw_pause_glyph(center.X, center.Y, radius * 0.5, &glyph);
        } else {
            let dot = D2D1_ELLIPSE {
                point: center,
                radiusX: stroke * 0.75,
                radiusY: stroke * 0.75,
            };
            unsafe {
                self.target.FillEllipse(&dot, &accent);
            }
        }
        Ok(())
    }

    fn draw_urgent_rim(&self, layout: &Layout, strength: f32) -> Result<()> {
        let bar_notch = if layout.notch.y > 0 {
            layout.notch
        } else {
            Rect {
                x: layout.notch.x,
                y: 0,
                width: layout.notch.width,
                height: layout.bar.height,
            }
        };
        let corner = if layout.notch.y == 0 && layout.panel_expansion > 0.05 {
            0.0
        } else {
            layout.notch_corner_radius as f32
        };
        let geometry = self.notch_geometry_rect(
            bar_notch,
            corner,
            layout.notch_ear_radius as f32,
            &self.cached_bar_notch,
        )?;
        let brush = self
            .brushes
            .alpha_brush(CORAL, 0.75 * strength.clamp(0.0, 1.0));
        unsafe {
            self.target
                .DrawGeometry(&geometry, &brush, 1.2, &self.stroke_style);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn draw_notch_header(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        session_alpha: f32,
        hovered: Option<HitTarget>,
        has_media: bool,
        scene: &PomodoroScene,
    ) {
        let ink = self.notch_ink();
        let strip = translate_rect(layout.notch_tab_strip, -offset_x);
        if !strip.is_empty() {
            let strip_brush = self.brushes.alpha_brush(ink, 0.07 * alpha);
            self.draw_rounded_box(strip, strip.height as f32 * 0.5, &strip_brush);
        }

        let focus_label = scene.state.phase.short_label();
        let tabs = [
            (
                translate_rect(layout.notch_tab_pomodoro, -offset_x),
                NotchExpandedTab::Pomodoro,
                HitTarget::NotchTabPomodoro,
                focus_label,
                !layout.notch_tab_pomodoro.is_empty(),
            ),
            (
                translate_rect(layout.notch_tab_stats, -offset_x),
                NotchExpandedTab::Stats,
                HitTarget::NotchTabStats,
                NotchExpandedTab::Stats.short_label(),
                !layout.notch_tab_stats.is_empty(),
            ),
            (
                translate_rect(layout.notch_tab_shelf, -offset_x),
                NotchExpandedTab::Shelf,
                HitTarget::NotchTabShelf,
                NotchExpandedTab::Shelf.short_label(),
                !layout.notch_tab_shelf.is_empty(),
            ),
            (
                translate_rect(layout.notch_tab_clipboard, -offset_x),
                NotchExpandedTab::Clipboard,
                HitTarget::NotchTabClipboard,
                NotchExpandedTab::Clipboard.short_label(),
                !layout.notch_tab_clipboard.is_empty(),
            ),
            (
                translate_rect(layout.notch_tab_tasks, -offset_x),
                NotchExpandedTab::Tasks,
                HitTarget::NotchTabTasks,
                NotchExpandedTab::Tasks.short_label(),
                !layout.notch_tab_tasks.is_empty(),
            ),
            (
                translate_rect(layout.notch_tab_media, -offset_x),
                NotchExpandedTab::Media,
                HitTarget::NotchTabMedia,
                NotchExpandedTab::Media.short_label(),
                has_media && !layout.notch_tab_media.is_empty(),
            ),
        ];
        for (rect, tab, target, label, shown) in tabs {
            if !shown || rect.is_empty() {
                continue;
            }
            let active = layout.active_tab == tab;
            self.draw_notch_tab_pill(rect, tab, label, active, hovered == Some(target), alpha);
        }

        if session_alpha > 0.0 {
            self.draw_focus_category_pill(layout, offset_x, session_alpha, hovered, scene);
            self.draw_focus_settings_button(layout, offset_x, session_alpha, hovered);
            let view = scene.view();
            let muted = self.brushes.alpha_brush(ink, 0.55 * session_alpha);
            self.draw_text(
                &view.session_label,
                &self.text_pomodoro_sub_trailing,
                rect_to_d2d(translate_rect(layout.pomodoro.session_label, -offset_x)),
                &muted,
            );
        }
    }

    fn draw_notch_tab_pill(
        &self,
        rect: Rect,
        tab: NotchExpandedTab,
        label: &str,
        active: bool,
        hovered: bool,
        alpha: f32,
    ) {
        let fg = self.notch_ink();
        let fill = if active {
            0.18
        } else if hovered {
            0.10
        } else {
            0.0
        };
        if fill > 0.0 {
            let pill = self.brushes.alpha_brush(fg, fill * alpha);
            self.draw_rounded_box(rect, rect.height as f32 * 0.5, &pill);
        }
        let ink = self
            .brushes
            .alpha_brush(fg, if active { 0.96 } else { 0.58 } * alpha);
        let glyph_x = if active {
            rect.x as f32 + scale(15, self.dpi) as f32
        } else {
            rect.x as f32 + rect.width as f32 * 0.5
        };
        let glyph_center = Vector2::new(glyph_x, rect.y as f32 + rect.height as f32 * 0.5);
        let glyph_r = scale(5, self.dpi) as f32;
        self.draw_notch_tab_glyph(glyph_center, glyph_r, tab, &ink);
        if active {
            self.draw_text(
                label,
                &self.text_tab,
                D2D_RECT_F {
                    left: glyph_center.X + scale(7, self.dpi) as f32,
                    top: rect.y as f32,
                    right: rect.right() as f32 - scale(6, self.dpi) as f32,
                    bottom: rect.bottom() as f32,
                },
                &ink,
            );
        }
    }

    fn draw_notch_tab_glyph(
        &self,
        center: Vector2,
        r: f32,
        tab: NotchExpandedTab,
        ink: &ID2D1SolidColorBrush,
    ) {
        let stroke = (r * 0.24).max(1.25);
        match tab {
            NotchExpandedTab::Pomodoro => self.draw_clock_glyph(center, r, ink),
            NotchExpandedTab::Stats => self.draw_stats_bars_glyph(center.X, center.Y, r, ink),
            NotchExpandedTab::Shelf => self.draw_tray_glyph(center, r, ink),
            NotchExpandedTab::Clipboard => self.draw_clipboard_glyph(center, r, stroke, ink),
            NotchExpandedTab::Tasks => self.draw_tasks_glyph(center, r, ink),
            NotchExpandedTab::Media => self.draw_music_note_glyph(center.X, center.Y, r, ink),
        }
    }

    pub(crate) fn draw_tray_glyph(&self, center: Vector2, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (r * 0.24).max(1.25);
        let left = center.X - r * 0.92;
        let right = center.X + r * 0.92;
        let top = center.Y - r * 0.15;
        let bottom = center.Y + r * 0.78;
        let segs = [
            (Vector2::new(left, top), Vector2::new(left, bottom)),
            (Vector2::new(left, bottom), Vector2::new(right, bottom)),
            (Vector2::new(right, bottom), Vector2::new(right, top)),
            (
                Vector2::new(center.X, center.Y - r * 0.82),
                Vector2::new(center.X, center.Y + r * 0.22),
            ),
            (
                Vector2::new(center.X - r * 0.36, center.Y - r * 0.14),
                Vector2::new(center.X, center.Y + r * 0.24),
            ),
            (
                Vector2::new(center.X + r * 0.36, center.Y - r * 0.14),
                Vector2::new(center.X, center.Y + r * 0.24),
            ),
        ];
        unsafe {
            for (a, b) in segs {
                self.target
                    .DrawLine(a, b, brush, stroke, &self.stroke_style);
            }
        }
    }

    pub(crate) fn draw_clipboard_glyph(
        &self,
        center: Vector2,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let board = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: center.X - r * 0.72,
                top: center.Y - r * 0.68,
                right: center.X + r * 0.72,
                bottom: center.Y + r * 0.94,
            },
            radiusX: r * 0.24,
            radiusY: r * 0.24,
        };
        let clip = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: center.X - r * 0.34,
                top: center.Y - r * 0.96,
                right: center.X + r * 0.34,
                bottom: center.Y - r * 0.48,
            },
            radiusX: r * 0.16,
            radiusY: r * 0.16,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&board, brush, stroke, &self.stroke_style);
            self.target.FillRoundedRectangle(&clip, brush);
        }
    }

    pub(crate) fn draw_expanded_tab_placeholder(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        title: &str,
        subtitle: &str,
    ) {
        let local_notch_x = layout.notch.x - offset_x;
        let oy = layout.notch.y;
        let pad = scale(16, self.dpi);
        let title_rect = D2D_RECT_F {
            left: (local_notch_x + pad) as f32,
            top: (oy + scale(48, self.dpi)) as f32,
            right: (local_notch_x + layout.notch.width - pad) as f32,
            bottom: (oy + scale(74, self.dpi)) as f32,
        };
        let sub_rect = D2D_RECT_F {
            left: title_rect.left,
            top: title_rect.bottom + scale(4, self.dpi) as f32,
            right: title_rect.right,
            bottom: (oy + scale(96, self.dpi)) as f32,
        };
        let ink = self.notch_ink();
        let fg = self.brushes.alpha_brush(ink, 0.92 * alpha);
        let muted = self.brushes.alpha_brush(ink, 0.56 * alpha);
        self.draw_text(title, &self.text_app_bold, title_rect, &fg);
        self.draw_text(subtitle, &self.text_pomodoro_sub, sub_rect, &muted);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_expanded_pomodoro(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
        active_task: Option<&str>,
    ) -> Result<()> {
        let view = scene.view();
        let colors = self.resolve_pomodoro_colors(view.tone, scene, scene.dim, view.urgent);
        let fg_ink = self.notch_ink();
        let icon = translate_rect(layout.pomodoro.icon, -offset_x);
        let icon_center = center_of(icon);
        let icon_radius = icon.width as f32 * 0.5;

        self.fill_solid_circle(icon_center, icon_radius, colors.accent, alpha);
        if view.urgent {
            let ring = self
                .brushes
                .alpha_brush(CORAL, 0.9 * scene.pulse.clamp(0.0, 1.0) * alpha);
            unsafe {
                self.target.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: icon_center,
                        radiusX: icon_radius + scale(2, self.dpi) as f32,
                        radiusY: icon_radius + scale(2, self.dpi) as f32,
                    },
                    &ring,
                    2.0,
                    &self.stroke_style,
                );
            }
        }
        let glyph_ink = self.brushes.alpha_brush(
            Self::contrast_ink_on(colors.accent),
            (0.78 - scene.dim * 0.2) * alpha,
        );
        self.draw_tone_glyph(icon_center, icon_radius * 0.46, view.tone, &glyph_ink)?;

        self.draw_focus_duration_steppers(layout, offset_x, alpha, hovered, scene);

        let time_alpha = 0.96 - scene.dim * 0.34;
        let time_brush = if view.urgent {
            self.brushes.alpha_brush(CORAL, alpha)
        } else {
            self.brushes.alpha_brush(fg_ink, time_alpha * alpha)
        };
        self.draw_tabular_text(
            &view.headline,
            &self.text_timer_display,
            rect_to_d2d(translate_rect(layout.pomodoro.time, -offset_x)),
            &time_brush,
        );
        let muted = self.brushes.alpha_brush(fg_ink, 0.60 * alpha);
        self.draw_text(
            active_task.unwrap_or(&view.subtitle),
            &self.text_pomodoro_sub,
            rect_to_d2d(translate_rect(layout.pomodoro.subtitle, -offset_x)),
            &muted,
        );

        self.draw_pomodoro_buttons(layout, offset_x, alpha, hovered, scene, &colors)?;
        if scene.category_picker_open {
            self.draw_focus_category_picker_strip(layout, offset_x, alpha, hovered, scene);
        } else {
            match view.surface {
                PomodoroSurface::Setup => {
                    self.draw_pomodoro_presets(layout, offset_x, alpha, hovered, scene);
                }
                _ => self.draw_pomodoro_progress(layout, offset_x, alpha, scene, &view, &colors),
            }
        }
        Ok(())
    }

    fn draw_pomodoro_buttons(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
        colors: &TonePalette,
    ) -> Result<()> {
        let notch_fg = self.notch_ink();
        let fg = self.brushes.alpha_brush(notch_fg, 0.96 * alpha);
        let secondary = [
            (layout.pomodoro.reset_button, HitTarget::PomodoroReset, true),
            (layout.pomodoro.skip_button, HitTarget::PomodoroSkip, false),
        ];
        for (rect, target, is_reset) in secondary {
            if rect.is_empty() {
                continue;
            }
            let rect = translate_rect(rect, -offset_x);
            let fill = if hovered == Some(target) { 0.26 } else { 0.14 };
            self.draw_circle_button(rect, &self.brushes.alpha_brush(notch_fg, fill * alpha));
            let center = center_of(rect);
            let size = scale(6, self.dpi) as f32;
            if is_reset {
                self.draw_reset_glyph(center.X, center.Y, size, &fg)?;
            } else {
                self.draw_skip_glyph(center.X, center.Y, size, &fg)?;
            }
        }

        let play = translate_rect(layout.pomodoro.play_button, -offset_x);
        if play.is_empty() {
            return Ok(());
        }
        let center = center_of(play);
        let radius = play.width as f32 * 0.5;
        let lit = if hovered == Some(HitTarget::PomodoroPlayPause) {
            TonePalette {
                from: brighten(colors.from, 0.12),
                to: brighten(colors.to, 0.12),
                ..*colors
            }
        } else {
            *colors
        };
        self.fill_gradient_circle(center, radius, &lit, alpha);
        let ink = self.brushes.alpha_brush(
            Self::contrast_ink_on(lit.accent),
            (0.92 - scene.dim * 0.2) * alpha,
        );
        let glyph = scale(8, self.dpi) as f32;
        if scene.state.status == PomodoroStatus::Running {
            self.draw_pause_glyph(center.X, center.Y, glyph, &ink);
        } else {
            self.draw_play_glyph(center.X, center.Y, glyph, &ink)?;
        }
        Ok(())
    }

    fn draw_pomodoro_presets(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
    ) {
        let presets = scene.presets_minutes();
        let fg = self.notch_ink();
        let selected = scene.state.total_seconds;
        for (index, minutes) in presets.iter().enumerate() {
            let rect = translate_rect(layout.pomodoro.presets[index], -offset_x);
            if rect.is_empty() {
                continue;
            }
            let active = selected == minutes * 60;
            let fill = if active {
                0.20
            } else if hovered == Some(HitTarget::PomodoroPreset(index as u8)) {
                0.14
            } else {
                0.08
            };
            let chip = self.brushes.alpha_brush(fg, fill * alpha);
            self.draw_rounded_box(rect, rect.height as f32 * 0.5, &chip);
            let ink = self
                .brushes
                .alpha_brush(fg, if active { 0.96 } else { 0.68 } * alpha);
            self.draw_text(
                &format!("{minutes} min"),
                &self.text_tab,
                rect_to_d2d(rect),
                &ink,
            );
        }
    }

    fn draw_pomodoro_progress(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        scene: &PomodoroScene,
        view: &PomodoroView,
        colors: &TonePalette,
    ) {
        let fg = self.notch_ink();
        let track = self.brushes.alpha_brush(fg, 0.12 * alpha);
        let segments = layout
            .pomodoro
            .segments
            .map(|rect| translate_rect(rect, -offset_x));
        match view.progress {
            PomodoroProgress::Segments(fills) => {
                for (rect, fill) in segments.iter().zip(fills) {
                    let radius = rect.height as f32 * 0.5;
                    self.draw_rounded_box(*rect, radius, &track);
                    let width = (rect.width as f32 * fill.clamp(0.0, 1.0)).round();
                    if width < rect.height as f32 {
                        continue;
                    }
                    let strength = if fill >= 0.999 { 0.55 } else { 1.0 };
                    self.fill_gradient_rounded(
                        D2D_RECT_F {
                            left: rect.x as f32,
                            top: rect.y as f32,
                            right: rect.x as f32 + width,
                            bottom: rect.bottom() as f32,
                        },
                        radius,
                        colors,
                        strength * alpha,
                    );
                }
            }
            PomodoroProgress::Bar(fill) => {
                let bar = segments[0].union(&segments[3]);
                let radius = bar.height as f32 * 0.5;
                self.draw_rounded_box(bar, radius, &track);
                let width = (bar.width as f32 * fill.clamp(0.0, 1.0)).round();
                if width >= bar.height as f32 {
                    self.fill_gradient_rounded(
                        D2D_RECT_F {
                            left: bar.x as f32,
                            top: bar.y as f32,
                            right: bar.x as f32 + width,
                            bottom: bar.bottom() as f32,
                        },
                        radius,
                        colors,
                        alpha,
                    );
                }
            }
        }
        let _ = scene;
        let footer = self.brushes.alpha_brush(fg, 0.52 * alpha);
        self.draw_text(
            &view.footer_left,
            &self.text_pomodoro_sub,
            rect_to_d2d(translate_rect(layout.pomodoro.footer_left, -offset_x)),
            &footer,
        );
        self.draw_text(
            &view.footer_right,
            &self.text_pomodoro_sub_trailing,
            rect_to_d2d(translate_rect(layout.pomodoro.footer_right, -offset_x)),
            &footer,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn draw_expanded_alert(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
    ) {
        let view = scene.view();
        let colors = self.resolve_pomodoro_colors(view.tone, scene, 0.0, false);
        let notch_fg = self.notch_ink();
        let icon = translate_rect(layout.pomodoro.icon, -offset_x);
        let icon_center = center_of(icon);
        let icon_radius = icon.width as f32 * 0.5;
        let muted = self.brushes.alpha_brush(notch_fg, 0.55 * alpha);
        let header = translate_rect(layout.notch_tab_pomodoro, -offset_x);
        self.draw_text(
            "Pomodoro",
            &self.text_pomodoro_sub,
            D2D_RECT_F {
                left: header.x as f32,
                top: header.y as f32,
                right: header.x as f32 + scale(120, self.dpi) as f32,
                bottom: header.bottom() as f32,
            },
            &muted,
        );
        self.draw_text(
            &view.session_label,
            &self.text_pomodoro_sub_trailing,
            rect_to_d2d(translate_rect(layout.pomodoro.session_label, -offset_x)),
            &muted,
        );

        self.fill_solid_circle(icon_center, icon_radius, colors.accent, alpha);
        let ink = self
            .brushes
            .alpha_brush(Self::contrast_ink_on(colors.accent), 0.78 * alpha);
        self.draw_check_glyph(icon_center, icon_radius * 0.46, &ink);

        let fg = self.brushes.alpha_brush(notch_fg, 0.96 * alpha);
        self.draw_text(
            &view.headline,
            &self.text_alert_title,
            rect_to_d2d(translate_rect(layout.pomodoro.time, -offset_x)),
            &fg,
        );
        self.draw_text(
            &view.subtitle,
            &self.text_pomodoro_sub,
            rect_to_d2d(translate_rect(layout.pomodoro.subtitle, -offset_x)),
            &self.brushes.alpha_brush(notch_fg, 0.60 * alpha),
        );

        let extend = translate_rect(layout.pomodoro.alert_extend_button, -offset_x);
        let extend_fill = if hovered == Some(HitTarget::PomodoroExtendFive) {
            0.26
        } else {
            0.15
        };
        self.draw_rounded_box(
            extend,
            extend.height as f32 * 0.5,
            &self.brushes.alpha_brush(notch_fg, extend_fill * alpha),
        );
        self.draw_text(
            POMODORO_EXTEND_LABEL,
            &self.text_app_bold,
            rect_to_d2d(extend),
            &fg,
        );

        let start = translate_rect(layout.pomodoro.alert_start_button, -offset_x);
        let lit = if hovered == Some(HitTarget::PomodoroStartNext) {
            TonePalette {
                from: brighten(colors.from, 0.12),
                to: brighten(colors.to, 0.12),
                ..colors
            }
        } else {
            colors
        };
        self.fill_gradient_rounded(rect_to_d2d(start), start.height as f32 * 0.5, &lit, alpha);
        let label = scene
            .state
            .alert
            .map_or("Start", |alert| alert.action_label());
        self.draw_text(
            label,
            &self.text_app_bold,
            rect_to_d2d(start),
            &self
                .brushes
                .alpha_brush(Self::contrast_ink_on(lit.accent), 0.92 * alpha),
        );
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

    use super::*;
    use crate::appearance::tokens::FOCUS_ORANGE;

    fn pomodoro_colors(tone: PomodoroTone, dim: f32, urgent: bool) -> TonePalette {
        pomodoro_colors_with_focus(tone, FOCUS_ORANGE, dim, urgent)
    }

    fn channel_gap(a: D2D1_COLOR_F, b: D2D1_COLOR_F) -> f32 {
        (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs()
    }

    #[test]
    fn phase_accents_are_solid() {
        let focus = pomodoro_colors(PomodoroTone::Focus, 0.0, false);
        let short = pomodoro_colors(PomodoroTone::ShortBreak, 0.0, false);
        let long = pomodoro_colors(PomodoroTone::LongBreak, 0.0, false);
        assert!(channel_gap(focus.accent, short.accent) > 0.3);
        assert!(channel_gap(short.accent, long.accent) > 0.3);
        assert!(channel_gap(focus.from, focus.to) < 0.001);
        assert!(channel_gap(short.from, short.to) < 0.001);
        assert!(channel_gap(long.from, long.to) < 0.001);
    }

    #[test]
    fn pause_steps_colour_back_by_losing_saturation() {
        let saturation = |color: D2D1_COLOR_F| {
            color.r.max(color.g).max(color.b) - color.r.min(color.g).min(color.b)
        };
        let live = pomodoro_colors(PomodoroTone::Focus, 0.0, false);
        let paused = pomodoro_colors(PomodoroTone::Focus, 1.0, false);
        assert!(saturation(paused.accent) < saturation(live.accent) * 0.6);
        assert!(paused.accent.r < live.accent.r);
    }

    #[test]
    fn last_minute_turns_the_time_colour_coral() {
        let urgent = pomodoro_colors(PomodoroTone::Focus, 0.0, true);
        assert!(channel_gap(urgent.text, CORAL) < 0.001);
        let calm = pomodoro_colors(PomodoroTone::ShortBreak, 0.0, false);
        assert!(channel_gap(calm.text, CORAL) > 0.3);
    }

    #[test]
    fn dimming_is_clamped() {
        let over = pomodoro_colors(PomodoroTone::Focus, 5.0, false);
        let full = pomodoro_colors(PomodoroTone::Focus, 1.0, false);
        assert!(channel_gap(over.accent, full.accent) < 0.001);
    }
}
