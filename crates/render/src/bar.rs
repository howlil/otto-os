use super::{CLEAR, FOREGROUND, LiquidGlassPalette, Renderer};
use otto_core::{
    HitTarget, Layout, LyricsSheet, NotchExpandedTab, NotchHudState, NowPlaying, PomodoroState,
    PomodoroStatus, Rect, ResolvedTheme, SystemStatus,
};
use windows::Win32::Foundation::RECT;
use windows::core::Result;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_bar(
        &self,
        layout: &Layout,
        status: SystemStatus,
        app_name: &str,
        hovered: Option<HitTarget>,
        hud: Option<NotchHudState>,
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
        theme: ResolvedTheme,
    ) -> Result<()> {
        let palette = LiquidGlassPalette::for_theme(theme);
        let (width, height) = self.bar_surface.size;
        unsafe {
            self.target.BindDC(
                self.bar_surface.dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )?;
            self.target.BeginDraw();
            let clear = palette.bar_background;
            self.target.Clear(Some(&raw const clear));
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
            self.target.FillGeometry(
                &self.notch_geometry_rect(
                    bar_notch,
                    corner_r,
                    layout.notch_ear_radius as f32,
                    &self.cached_bar_notch,
                )?,
                &self.brushes.black,
                None,
            );

            if let Some(target_slot) = hovered.and_then(|t| layout.slot_rect(t)) {
                self.draw_slot_pill(target_slot, &self.brushes.hover);
            }

            let fg = &self.brushes.text_primary;
            let muted = &self.brushes.text_muted;

            self.draw_apple_logo(layout, fg)?;
            self.draw_left_menu_bar(layout, app_name, fg);
            self.draw_tray_chevron(layout, fg)?;
            self.draw_status_slot(layout, status, fg, muted)?;
            self.draw_search_slot(layout, fg);
            if layout.control_center_slot.width > 0 {
                self.draw_control_center_slot(layout, fg, muted);
            }
            self.draw_clock_slot(layout, status, fg)?;

            if layout.panel_expansion < 0.15 {
                if let Some(hud_state) = hud
                    && hud_state.expansion > 0.12
                {
                    self.draw_notch_hud(layout, hud_state)?;
                } else if layout.compact_expansion > 0.15 {
                    self.draw_compact_notch(layout, pomodoro, media, anim_phase)?;
                }
            }

            self.target.EndDraw(None, None)
        }
    }

    pub(super) fn draw_panel(
        &self,
        layout: &Layout,
        hovered: Option<HitTarget>,
        pomodoro: PomodoroState,
        media: Option<&NowPlaying>,
        anim_phase: f32,
        lyrics: Option<&LyricsSheet>,
    ) -> Result<()> {
        let (max_width, max_height) = self.panel_surface.size;
        let draw_width = layout.panel_window.width.min(max_width);
        let draw_height = layout.panel_window.height.min(max_height);
        let offset_x = layout.panel_window.x;
        let local_notch = Rect {
            x: layout.notch.x - offset_x,
            y: 0,
            width: layout.notch.width,
            height: layout.notch.height,
        };

        unsafe {
            self.target.BindDC(
                self.panel_surface.dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: draw_width,
                    bottom: draw_height,
                },
            )?;
            self.target.BeginDraw();
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));

            let geom = self.notch_geometry_rect(
                local_notch,
                layout.notch_corner_radius as f32,
                layout.notch_ear_radius as f32,
                &self.cached_panel_notch,
            )?;
            self.target.FillGeometry(&geom, &self.brushes.black, None);

            let rim_alpha = (layout.panel_expansion * 0.12).clamp(0.0, 0.12);
            if rim_alpha > 0.01 {
                let rim = self.brushes.alpha_brush(0, FOREGROUND, rim_alpha);
                self.target
                    .DrawGeometry(&geom, rim, 1.0, &self.stroke_style);
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
            }

            self.target.EndDraw(None, None)
        }
    }
}
