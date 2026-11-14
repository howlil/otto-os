use super::surface::draw_on;
use super::{CLEAR, LiquidGlassPalette, Renderer};
use otto_core::{
    Appearance, GlassShape, HitTarget, Layout, LyricsView, NotchExpandedTab, NotchHudState,
    NowPlaying, PomodoroState, PomodoroStatus, Rect, SystemStatus,
};
use windows::core::Result;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_bar(
        &self,
        layout: &Layout,
        status: SystemStatus,
        app_name: &str,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        hud: Option<NotchHudState>,
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
        appearance: Appearance,
    ) -> Result<()> {
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        let surface = &self.bar_surface;
        draw_on(&self.target, surface, surface.size(), || unsafe {
            let transparent = CLEAR;
            self.target.Clear(Some(&raw const transparent));
            let (bar_width, bar_height) = surface.size();
            let bar_shape = GlassShape::from_rect(
                Rect {
                    x: 0,
                    y: 0,
                    width: bar_width,
                    height: bar_height,
                },
                0.0,
            );
            let glass_drawn =
                self.glass
                    .draw(&self.target, &appearance, self.dpi, bar_shape, false)?;
            if !glass_drawn {
                let clear = palette.bar_background;
                self.target.Clear(Some(&raw const clear));
            }
            self.brushes.update_theme(&palette);

            let bar_notch = Rect {
                x: layout.notch.x,
                y: 0,
                width: layout.notch.width,
                height: layout.bar.height,
            };
            let corner_r = if layout.panel_expansion > 0.05 {
                0.0
            } else {
                layout.notch_corner_radius as f32
            };
            let notch_geom = self.notch_geometry_rect(
                bar_notch,
                corner_r,
                layout.notch_ear_radius as f32,
                &self.cached_bar_notch,
            )?;
            self.target
                .FillGeometry(&notch_geom, &self.brushes.notch_obsidian, None);
            self.target.DrawGeometry(
                &notch_geom,
                &self.brushes.specular_rim,
                1.0,
                &self.stroke_style,
            );

            if let Some(target_slot) = hovered.and_then(|t| layout.slot_rect(t)) {
                self.draw_slot_pill(target_slot, &self.brushes.hover);
            }
            if let Some(focus_slot) = focused.and_then(|t| layout.slot_rect(t)) {
                self.draw_slot_pill(focus_slot, &self.brushes.hover);
                self.draw_slot_focus_ring(focus_slot, &self.brushes.text_primary);
            }

            let fg = &self.brushes.text_primary;
            let muted = &self.brushes.text_muted;

            if layout.start_slot.width > 0 {
                self.draw_apple_logo(layout, fg)?;
            }
            self.draw_left_menu_bar(layout, app_name, fg);
            if layout.overflow_slot.width > 0 {
                self.draw_slot_label(layout.overflow_slot, "»", &self.text_menu, fg);
            }
            if layout.tray_slot.width > 0 {
                self.draw_tray_chevron(layout, fg)?;
            }
            if layout.status_slot.width > 0 {
                self.draw_status_slot(layout, status, fg, muted)?;
            }
            if layout.search_slot.width > 0 {
                self.draw_search_slot(layout, fg);
            }
            if layout.control_center_slot.width > 0 {
                self.draw_control_center_slot(layout, fg, muted);
            }
            if layout.clock_slot.width > 0 {
                self.draw_clock_slot(layout, status, fg)?;
            }

            if layout.panel_expansion < 0.15 {
                if let Some(hud_state) = hud
                    && hud_state.expansion > 0.12
                {
                    self.draw_notch_hud(layout, hud_state)?;
                } else if layout.compact_expansion > 0.15 {
                    self.draw_compact_notch(layout, pomodoro, media, anim_phase)?;
                }
            }
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_panel(
        &self,
        layout: &Layout,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
    ) -> Result<()> {
        let draw_size = (layout.panel_window.width, layout.panel_window.height);
        let offset_x = layout.panel_window.x;
        let local_notch = Rect {
            x: layout.notch.x - offset_x,
            y: 0,
            width: layout.notch.width,
            height: layout.notch.height,
        };

        draw_on(&self.target, &self.panel_surface, draw_size, || unsafe {
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));

            let geom = self.notch_geometry_rect(
                local_notch,
                layout.notch_corner_radius as f32,
                layout.notch_ear_radius as f32,
                &self.cached_panel_notch,
            )?;
            self.target
                .FillGeometry(&geom, &self.brushes.notch_obsidian, None);

            let rim_alpha = (layout.panel_expansion * 0.12).clamp(0.0, 0.12);
            if rim_alpha > 0.01 {
                self.target.DrawGeometry(
                    &geom,
                    &self.brushes.specular_rim,
                    1.0,
                    &self.stroke_style,
                );
            }

            if layout.panel_expansion > 0.22 {
                let alpha = ((layout.panel_expansion - 0.22) / 0.78).clamp(0.0, 1.0);
                if pomodoro.status == PomodoroStatus::Alert
                    && let Some(alert) = pomodoro.alert
                {
                    self.draw_expanded_alert(layout, offset_x, alpha, hovered, pomodoro, alert)?;
                } else if layout.active_tab == NotchExpandedTab::Media
                    && let Some(active_media) = media
                {
                    self.draw_expanded_tabs(
                        layout,
                        offset_x,
                        alpha,
                        hovered,
                        layout.active_tab,
                        true,
                        pomodoro,
                    );
                    self.draw_expanded_media(
                        layout,
                        offset_x,
                        alpha,
                        hovered,
                        active_media,
                        anim_phase,
                        lyrics,
                    )?;
                } else {
                    let has_media = media.is_some();
                    self.draw_expanded_tabs(
                        layout,
                        offset_x,
                        alpha,
                        hovered,
                        layout.active_tab,
                        has_media,
                        pomodoro,
                    );
                    self.draw_expanded_pomodoro(layout, offset_x, alpha, hovered, pomodoro)?;
                }

                if let Some(rect) = focused.and_then(|target| layout.notch_control_rect(target)) {
                    let ring_brush = &self.brushes.alpha_brush(super::FOREGROUND, 0.9 * alpha);
                    self.draw_control_focus_ring(
                        super::translate_rect(rect, -offset_x),
                        ring_brush,
                    );
                }
            }
            Ok(())
        })
    }
}
