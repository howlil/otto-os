use crate::appearance::surface::draw_on;
use crate::{CLEAR, FOREGROUND, LiquidGlassPalette, Renderer};
use otto_core::{
    Appearance, BrandIcon, CallScene, ClipboardState, GlassShape, HitTarget, Layout, LyricsView,
    MediaScene, NotchExpandedTab, NotchHudState, NotificationPresentation, PomodoroScene,
    PomodoroSurface, PrivacyState, Rect, ShelfState, SystemStatus, TasksState,
};
use windows::core::Result;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct BarNotchShellKey {
    rect: Rect,
    corner_radius_bits: u32,
    ear_radius: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarFocusKey {
    None,
    Slot(Rect),
    Notch(Rect),
    Privacy(Rect),
}

#[derive(Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent frame-cache fields"
)]
pub(crate) struct BarFrameStaticKey {
    surface_size: (i32, i32),
    bar: Rect,
    notch_visible: bool,
    notch_shell: Option<BarNotchShellKey>,
    panel_open_top_band: bool,
    status: SystemStatus,
    hovered_slot: Option<Rect>,
    focused_state: BarFocusKey,
    icon_size: i32,
    start_slot: Rect,
    menu_slots: [Rect; 6],
    overflow_slot: Rect,
    tray_slot: Rect,
    status_slot: Rect,
    search_slot: Rect,
    control_center_slot: Rect,
    notification_slot: Rect,
    clock_slot: Rect,
    appearance: Appearance,
    brand_icon: BrandIcon,
    bar_background: bool,
    bar_chrome_hidden: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct BarFrameKey {
    fields: BarFrameStaticKey,
    app_name: String,
}

impl BarFrameKey {
    pub(crate) fn new(fields: &BarFrameStaticKey, app_name: &str) -> Self {
        Self {
            fields: *fields,
            app_name: app_name.to_owned(),
        }
    }

    pub(crate) fn matches(&self, fields: &BarFrameStaticKey, app_name: &str) -> bool {
        self.fields == *fields && self.app_name == app_name
    }
}

impl Renderer {
    pub(crate) fn bar_frame_fields(
        &self,
        layout: &Layout,
        status: SystemStatus,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        hud: Option<NotchHudState>,
        appearance: Appearance,
    ) -> Option<BarFrameStaticKey> {
        let panel_open_top_band = layout.panel_expansion >= 0.15;
        let hud_inactive = hud.is_none_or(|h| h.content_alpha <= 0.0);
        if !panel_open_top_band || !hud_inactive {
            return None;
        }
        let notch_visible =
            !layout.notch.is_empty() && (layout.notch.y == 0 || layout.panel_expansion <= 0.005);
        let notch_shell = notch_visible.then(|| {
            let rect = if layout.notch.y > 0 {
                layout.notch
            } else {
                Rect {
                    x: layout.notch.x,
                    y: 0,
                    width: layout.notch.width,
                    height: layout.bar.height,
                }
            };
            let corner_radius = if layout.notch.y == 0 && layout.panel_expansion > 0.05 {
                0.0
            } else {
                layout.notch_corner_radius as f32
            };
            BarNotchShellKey {
                rect,
                corner_radius_bits: corner_radius.to_bits(),
                ear_radius: layout.notch_ear_radius,
            }
        });
        let hovered_slot = hovered.and_then(|t| layout.slot_rect(t));
        let focused_state = if let Some(rect) = focused.and_then(|t| layout.slot_rect(t)) {
            BarFocusKey::Slot(rect)
        } else if focused == Some(HitTarget::Notch)
            && !layout.notch.is_empty()
            && layout.panel_expansion <= 0.55
        {
            BarFocusKey::Notch(layout.notch)
        } else if focused == Some(HitTarget::PrivacyIndicator) && !layout.privacy.dots.is_empty() {
            BarFocusKey::Privacy(layout.privacy.dots)
        } else {
            BarFocusKey::None
        };
        Some(BarFrameStaticKey {
            surface_size: self.bar_surface.size(),
            bar: layout.bar,
            notch_visible,
            notch_shell,
            panel_open_top_band,
            status,
            hovered_slot,
            focused_state,
            icon_size: layout.icon_size,
            start_slot: layout.slots.start_slot,
            menu_slots: [
                layout.slots.app_slot,
                layout.slots.file_slot,
                layout.slots.edit_slot,
                layout.slots.view_slot,
                layout.slots.window_slot,
                layout.slots.help_slot,
            ],
            overflow_slot: layout.slots.overflow_slot,
            tray_slot: layout.slots.tray_slot,
            status_slot: layout.slots.status_slot,
            search_slot: layout.slots.search_slot,
            control_center_slot: layout.slots.control_center_slot,
            notification_slot: layout.slots.notification_slot,
            clock_slot: layout.slots.clock_slot,
            appearance,
            brand_icon: self.brand_icon.get(),
            bar_background: self.bar_background.get(),
            bar_chrome_hidden: self.bar_chrome_hidden.get(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_bar(
        &self,
        layout: &Layout,
        status: SystemStatus,
        app_name: &str,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        hud: Option<NotchHudState>,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
        appearance: Appearance,
        privacy: &PrivacyState,
        call: Option<&CallScene>,
    ) -> Result<()> {
        let frame_fields = self.bar_frame_fields(layout, status, hovered, focused, hud, appearance);
        let appearance = self.glass.adaptive_appearance(&appearance);
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        let surface = &self.bar_surface;
        draw_on(&self.target, surface, surface.size(), || unsafe {
            let transparent = CLEAR;
            self.target.Clear(Some(&raw const transparent));
            let (bar_width, bar_height) = surface.size();
            let bleed = crate::scale(22, self.dpi);
            let bar_shape = GlassShape::from_rect(
                Rect {
                    x: -bleed,
                    y: -bleed,
                    width: bar_width + bleed * 2,
                    height: bar_height + bleed,
                },
                0.0,
            );
            let glass_drawn = self.shows_bar_glass()
                && self
                    .glass
                    .draw(&self.target, &appearance, self.dpi, bar_shape, true)?;
            if self.shows_bar_glass() && !glass_drawn {
                let clear = palette.bar_background;
                self.target.Clear(Some(&raw const clear));
            }
            self.brushes.update_theme(&palette);

            if !layout.notch.is_empty() && (layout.notch.y == 0 || layout.panel_expansion <= 0.005)
            {
                self.draw_bar_notch_shell(layout, palette.is_high_contrast)?;
            }
            if !layout.notch.is_empty()
                && layout.panel_expansion < 0.15
                && layout.secondary_expansion > 0.05
                && !layout.notch_secondary.is_empty()
            {
                self.draw_secondary_notch_frame(layout, hovered, focused, palette.is_high_contrast);
            }

            let chrome = !self.bar_chrome_hidden.get();
            if let Some(target_slot) = hovered.and_then(|t| layout.slot_rect(t)).filter(|_| chrome)
            {
                self.draw_slot_pill(target_slot, &self.brushes.hover);
            }
            if let Some(focus_slot) = focused.and_then(|t| layout.slot_rect(t)).filter(|_| chrome) {
                self.draw_slot_pill(focus_slot, &self.brushes.hover);
                self.draw_slot_focus_ring(focus_slot, &self.brushes.text_primary);
            } else if focused == Some(HitTarget::Notch)
                && !layout.notch.is_empty()
                && layout.panel_expansion <= 0.55
            {
                self.draw_control_focus_ring(layout.notch, &self.brushes.text_primary);
            } else if focused == Some(HitTarget::PrivacyIndicator)
                && !layout.privacy.dots.is_empty()
            {
                self.draw_control_focus_ring(layout.privacy.dots, &self.brushes.text_primary);
            }

            if chrome {
                self.draw_bar_slots(layout, status, app_name)?;
            }

            if !layout.notch.is_empty() && layout.panel_expansion < 0.15 {
                if let Some(hud_state) = hud
                    && hud_state.content_alpha > 0.0
                {
                    self.draw_notch_hud(layout, hud_state)?;
                } else if layout.compact_expansion > 0.15 {
                    self.draw_compact_notch(layout, pomodoro, media, call, anim_phase)?;
                }
                if layout.secondary_expansion > 0.15 && !layout.notch_secondary.is_empty() {
                    self.draw_secondary_notch_content(layout, pomodoro, media, anim_phase)?;
                }
                self.draw_bar_privacy_dots(layout, hovered, privacy);
            }
            Ok(())
        })?;
        *self.last_bar_frame.borrow_mut() = frame_fields
            .as_ref()
            .map(|fields| BarFrameKey::new(fields, app_name));
        Ok(())
    }

    fn shows_bar_glass(&self) -> bool {
        self.bar_background.get() && !self.bar_chrome_hidden.get()
    }

    fn draw_bar_notch_shell(&self, layout: &Layout, is_high_contrast: bool) -> Result<()> {
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
        let corner_r = if layout.notch.y == 0 && layout.panel_expansion > 0.05 {
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
        unsafe {
            self.target
                .FillGeometry(&notch_geom, &self.brushes.notch_obsidian, None);
            if layout.notch.y > 0 && !is_high_contrast {
                self.draw_directional_rim(
                    &notch_geom,
                    bar_notch.y as f32,
                    bar_notch.bottom() as f32,
                    0.18,
                    0.06,
                    0.11,
                );
            } else {
                self.target.DrawGeometry(
                    &notch_geom,
                    &self.brushes.specular_rim,
                    1.0,
                    &self.stroke_style,
                );
            }
        }
        Ok(())
    }

    fn draw_bar_slots(&self, layout: &Layout, status: SystemStatus, app_name: &str) -> Result<()> {
        let fg = &self.brushes.text_primary;
        let muted = &self.brushes.text_muted;
        if layout.slots.start_slot.width > 0 {
            match self.brand_icon.get() {
                BrandIcon::Otto => {
                    let slot = layout.slots.start_slot;
                    let cx = slot.x as f32 + slot.width as f32 * 0.5;
                    let cy = slot.y as f32 + slot.height as f32 * 0.5;
                    let radius = layout.icon_size as f32 * 0.44;
                    let stroke = (layout.icon_size as f32 * 0.14).max(1.4);
                    self.draw_otto_mark(cx, cy, radius, stroke, fg);
                }
                BrandIcon::ClassicApple => self.draw_apple_logo(layout, fg)?,
            }
        }
        self.draw_left_menu_bar(layout, app_name, fg);
        if layout.slots.overflow_slot.width > 0 {
            self.draw_slot_label(layout.slots.overflow_slot, "»", &self.text_menu, fg);
        }
        if layout.slots.tray_slot.width > 0 {
            self.draw_tray_chevron(layout, fg)?;
        }
        if layout.slots.status_slot.width > 0 {
            self.draw_status_slot(layout, status, fg, muted)?;
        }
        if layout.slots.search_slot.width > 0 {
            self.draw_search_slot(layout, fg);
        }
        if layout.slots.control_center_slot.width > 0 {
            self.draw_control_center_slot(layout, fg, muted);
        }
        if layout.slots.notification_slot.width > 0 {
            self.draw_notification_slot(layout, fg)?;
        }
        if layout.slots.clock_slot.width > 0 {
            self.draw_clock_slot(layout, status, fg);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_panel(
        &self,
        layout: &Layout,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
        shelf_state: &ShelfState,
        clipboard_state: &ClipboardState,
        tasks_state: &TasksState,
        privacy: &PrivacyState,
        notification: Option<&NotificationPresentation>,
    ) -> Result<()> {
        let draw_size = (layout.panel_window.width, layout.panel_window.height);
        let offset_x = layout.panel_window.x;
        let local_notch = Rect {
            x: layout.notch.x - offset_x,
            y: layout.notch.y,
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

            let rim_color = self.brushes.specular_rim.GetColor();
            if rim_color.a >= 0.85 {
                self.target.DrawGeometry(
                    &geom,
                    &self.brushes.specular_rim,
                    1.0,
                    &self.stroke_style,
                );
            } else if layout.notch.y > 0 {
                self.draw_directional_rim(
                    &geom,
                    local_notch.y as f32,
                    local_notch.bottom() as f32,
                    0.18,
                    0.06,
                    0.11,
                );
            } else {
                let rim_alpha = (layout.panel_expansion * 0.12).clamp(0.0, 0.12);
                if rim_alpha > 0.01 {
                    let rim = self.brushes.alpha_brush(FOREGROUND, rim_alpha);
                    self.target
                        .DrawGeometry(&geom, &rim, 1.0, &self.stroke_style);
                }
            }

            self.set_content_offset(0.0, 0.0);
            if layout.panel_expansion > 0.22
                && let (Some(card), Some(content)) = (&layout.notification, notification)
            {
                self.draw_notification_card(layout, card, content, offset_x, hovered, focused)?;
            } else if layout.panel_expansion > 0.22 {
                self.draw_panel_open_content(
                    layout,
                    offset_x,
                    hovered,
                    focused,
                    pomodoro,
                    media,
                    anim_phase,
                    lyrics,
                    shelf_state,
                    clipboard_state,
                    tasks_state,
                    privacy,
                )?;
            }
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn draw_panel_open_content(
        &self,
        layout: &Layout,
        offset_x: i32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
        shelf_state: &ShelfState,
        clipboard_state: &ClipboardState,
        tasks_state: &TasksState,
        privacy: &PrivacyState,
    ) -> Result<()> {
        let alpha = ((layout.panel_expansion - 0.22) / 0.78).clamp(0.0, 1.0);
        let rise =
            ((1.0 - alpha) * crate::scale(crate::PANEL_CONTENT_RISE_PT, self.dpi) as f32).round();
        let slide = crate::scale(crate::NOTCH_TAB_SLIDE_PT, self.dpi) as f32;
        let blend = layout.tab_blend.clamp(0.0, 1.0);
        let active_tab = if layout.active_tab == NotchExpandedTab::Media && media.is_none() {
            NotchExpandedTab::Pomodoro
        } else {
            layout.active_tab
        };
        let previous_tab = if layout.previous_tab == NotchExpandedTab::Media && media.is_none() {
            NotchExpandedTab::Pomodoro
        } else {
            layout.previous_tab
        };
        let transitioning = previous_tab != active_tab && blend < 1.0;
        let pomodoro_weight = if transitioning {
            if active_tab == NotchExpandedTab::Pomodoro {
                blend
            } else if previous_tab == NotchExpandedTab::Pomodoro {
                1.0 - blend
            } else {
                0.0
            }
        } else if active_tab == NotchExpandedTab::Pomodoro {
            1.0
        } else {
            0.0
        };
        if pomodoro.view().surface == PomodoroSurface::Alert {
            self.set_content_offset(0.0, rise);
            self.draw_expanded_alert(layout, offset_x, alpha, hovered, pomodoro);
        } else {
            self.set_content_offset(0.0, rise);
            self.draw_notch_header(
                layout,
                offset_x,
                alpha,
                alpha * pomodoro_weight,
                hovered,
                media.is_some(),
                pomodoro,
            );
            self.draw_panel_privacy_pill(layout, offset_x, alpha, hovered, privacy)?;
            let dir = if active_tab.index() >= previous_tab.index() {
                1.0_f32
            } else {
                -1.0_f32
            };
            if transitioning && 1.0 - blend > 0.001 {
                self.set_content_offset(-blend * dir * slide, rise);
                self.draw_expanded_tab_body(
                    previous_tab,
                    layout,
                    offset_x,
                    alpha * (1.0 - blend),
                    hovered,
                    focused,
                    pomodoro,
                    media,
                    anim_phase,
                    lyrics,
                    shelf_state,
                    clipboard_state,
                    tasks_state,
                )?;
            }
            if !transitioning || blend > 0.001 {
                let in_alpha = if transitioning { alpha * blend } else { alpha };
                let in_dx = if transitioning {
                    (1.0 - blend) * dir * slide
                } else {
                    0.0
                };
                self.set_content_offset(in_dx, rise);
                self.draw_expanded_tab_body(
                    active_tab,
                    layout,
                    offset_x,
                    in_alpha,
                    hovered,
                    focused,
                    pomodoro,
                    media,
                    anim_phase,
                    lyrics,
                    shelf_state,
                    clipboard_state,
                    tasks_state,
                )?;
            }
        }
        self.set_content_offset(0.0, rise);
        if let Some(rect) = focused.and_then(|target| layout.notch_control_rect(target)) {
            let ring_brush = &self.brushes.alpha_brush(crate::FOREGROUND, 0.9 * alpha);
            self.draw_control_focus_ring(crate::translate_rect(rect, -offset_x), ring_brush);
        }
        self.set_content_offset(0.0, 0.0);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_expanded_tab_body(
        &self,
        tab: NotchExpandedTab,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
        shelf_state: &ShelfState,
        clipboard_state: &ClipboardState,
        tasks_state: &TasksState,
    ) -> Result<()> {
        match tab {
            NotchExpandedTab::Pomodoro => self.draw_expanded_pomodoro(
                layout,
                offset_x,
                alpha,
                hovered,
                pomodoro,
                tasks_state.active_task().map(|task| task.text.as_str()),
            ),
            NotchExpandedTab::Shelf => {
                self.draw_expanded_shelf(layout, offset_x, alpha, hovered, shelf_state)
            }
            NotchExpandedTab::Clipboard => {
                self.draw_expanded_clipboard(layout, offset_x, alpha, hovered, clipboard_state)
            }
            NotchExpandedTab::Tasks => {
                self.draw_expanded_tasks(layout, offset_x, alpha, hovered, tasks_state);
                Ok(())
            }
            NotchExpandedTab::Media => {
                if let Some(active_media) = media {
                    self.draw_expanded_media(
                        layout,
                        offset_x,
                        alpha,
                        hovered,
                        focused,
                        active_media,
                        anim_phase,
                        lyrics,
                    )
                } else {
                    self.draw_expanded_tab_placeholder(
                        layout,
                        offset_x,
                        alpha,
                        "Now Playing",
                        "Nothing playing right now",
                    );
                    Ok(())
                }
            }
        }
    }
}
