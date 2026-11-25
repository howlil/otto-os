mod appearance;
mod focus;
mod focus_heatmap;
mod palette;
mod rows;
#[cfg(test)]
mod tests;
mod tiles;

use std::collections::{HashMap, HashSet};

use otto_core::{
    BarWindowControlsView, ContentLayout, ContrastColors, PixelLimits, Rect, Rgba, SettingsContent,
    SettingsHit, SettingsLayout, SettingsPage, scale,
};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    IDWriteTextFormat,
};
use windows::core::Result;

use crate::kit::canvas::{HwndCanvas, make_text_format};
use crate::window::controls::{HighContrastTrafficLights, TrafficLightsStyle};

use self::palette::{
    HERO_TILE_POINTS, Palette, ROW_PAD_POINTS, SIDEBAR_RADIUS_POINTS, SIDEBAR_TILE_POINTS, rgba,
};

const MAX_SETTINGS_APP_ICONS: usize = 64;

pub struct SettingsPaintState<'a> {
    pub layout: &'a SettingsLayout,
    pub content: &'a SettingsContent,
    pub blocks: &'a ContentLayout,
    pub page: SettingsPage,
    pub scroll: i32,
    pub hovered: Option<SettingsHit>,
    pub window_controls: BarWindowControlsView,
    pub dark: bool,
    pub glass: bool,
    pub high_contrast: bool,
    pub contrast: Option<ContrastColors>,
    pub accent: Rgba,
    pub on_accent: Rgba,
}

pub(super) struct Formats {
    pub(super) label: IDWriteTextFormat,
    pub(super) subtitle: IDWriteTextFormat,
    pub(super) section: IDWriteTextFormat,
    pub(super) footer: IDWriteTextFormat,
    pub(super) title: IDWriteTextFormat,
    pub(super) sidebar: IDWriteTextFormat,
    pub(super) value: IDWriteTextFormat,
    pub(super) keycap: IDWriteTextFormat,
    pub(super) chip: IDWriteTextFormat,
    pub(super) kpi_value: IDWriteTextFormat,
    pub(super) caption_center: IDWriteTextFormat,
    pub(super) caption_leading: IDWriteTextFormat,
    pub(super) hero_title: IDWriteTextFormat,
    pub(super) hero_subtitle: IDWriteTextFormat,
}

impl Formats {
    fn new(dpi: u32) -> Result<Self> {
        let make = |points: i32, weight, align| make_text_format(points, dpi, weight, align, false);
        let footer = make_text_format(
            11,
            dpi,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            true,
        )?;
        Ok(Self {
            label: make(13, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_LEADING)?,
            subtitle: make(
                11,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            section: make(12, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_LEADING)?,
            footer,
            title: make(
                15,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            sidebar: make(
                13,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            value: make(
                13,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?,
            keycap: make(11, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_CENTER)?,
            chip: make(12, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_CENTER)?,
            kpi_value: make(
                20,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            caption_center: make(10, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_CENTER)?,
            caption_leading: make(
                10,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            hero_title: make(
                22,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?,
            hero_subtitle: make(13, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_TEXT_ALIGNMENT_CENTER)?,
        })
    }
}

pub struct SettingsRenderer {
    pub(super) canvas: HwndCanvas,
    pub(super) formats: Formats,
    pub(super) dpi: u32,
    pub(super) app_icons: HashMap<String, ID2D1Bitmap>,
}

impl SettingsRenderer {
    pub fn new(hwnd: isize, width: i32, height: i32, dpi: u32) -> Result<Self> {
        Ok(Self {
            canvas: HwndCanvas::new(hwnd, width, height)?,
            formats: Formats::new(dpi)?,
            dpi,
            app_icons: HashMap::new(),
        })
    }

    #[must_use]
    pub fn has_app_icon(&self, key: &str) -> bool {
        self.app_icons.contains_key(key)
    }

    pub fn set_app_icon(
        &mut self,
        key: String,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<()> {
        if self.app_icons.len() >= MAX_SETTINGS_APP_ICONS && !self.app_icons.contains_key(&key) {
            self.app_icons.clear();
        }
        let bitmap = self
            .canvas
            .upload_bitmap(width, height, pixels, PixelLimits::ICON)?;
        self.app_icons.insert(key, bitmap);
        Ok(())
    }

    pub fn retain_app_icons(&mut self, keys: &HashSet<&str>) {
        self.app_icons.retain(|key, _| keys.contains(key.as_str()));
    }

    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        self.canvas.resize(width, height)
    }

    pub fn set_dpi(&mut self, dpi: u32) -> Result<()> {
        if dpi != self.dpi {
            self.formats = Formats::new(dpi)?;
            self.dpi = dpi;
        }
        Ok(())
    }

    pub fn draw(&mut self, state: &SettingsPaintState<'_>) -> Result<()> {
        let palette = Palette::pick(
            state.dark,
            state.glass,
            state.high_contrast,
            state.contrast,
            state.accent,
            state.on_accent,
        );
        let clear = if palette.glass {
            rgba(0, 0.0)
        } else {
            palette.background
        };
        self.canvas.begin_frame(clear);
        self.draw_body(state, &palette);
        self.draw_hover_detail_bar(state, &palette);
        self.draw_toolbar(state, &palette);
        self.draw_sidebar(state, &palette);
        self.canvas.end_frame()
    }

    pub(super) fn px(&self, points: i32) -> i32 {
        scale(points, self.dpi)
    }

    fn draw_toolbar(&self, state: &SettingsPaintState<'_>, palette: &Palette) {
        let header = state.layout.header;
        if !palette.glass {
            self.fill(header, palette.background);
        }
        if state.scroll > 0 {
            self.fill(
                Rect {
                    y: header.bottom() - 1,
                    height: 1,
                    ..header
                },
                palette.hairline,
            );
        }
        self.text(
            state.page.label(),
            &self.formats.title,
            Rect {
                x: state.layout.column.x + self.px(ROW_PAD_POINTS),
                width: state.layout.column.width,
                ..header
            },
            palette.text,
        );
    }

    fn draw_sidebar(&self, state: &SettingsPaintState<'_>, palette: &Palette) {
        let layout = state.layout;
        self.fill(layout.sidebar, palette.sidebar);
        self.fill(
            Rect {
                x: layout.sidebar.right() - 1,
                width: 1,
                ..layout.sidebar
            },
            palette.hairline,
        );
        if !layout.window_controls_group.is_empty() {
            let slots = layout.window_controls.map(|slot| (slot.action, slot.rect));
            let radius = self.px(if layout.rail { 10 } else { 12 }) as f32 * 0.5;
            let high_contrast = (state.high_contrast || state.contrast.is_some()).then_some(
                HighContrastTrafficLights {
                    window: palette.background,
                    text: palette.text,
                    disabled: palette.secondary,
                },
            );
            self.canvas.draw_traffic_lights(
                &slots,
                radius,
                TrafficLightsStyle {
                    state: state.window_controls.state,
                    maximized: state.window_controls.maximized,
                    hovered: state.window_controls.hovered,
                    pressed: state.window_controls.pressed,
                    dark_surface: state.dark,
                    high_contrast,
                },
            );
        }
        let radius = self.px(SIDEBAR_RADIUS_POINTS) as f32;
        let tile = self.px(SIDEBAR_TILE_POINTS);
        let pad = self.px(8);
        for heading in &layout.group_headings {
            self.text(
                heading.group.title(),
                &self.formats.subtitle,
                Rect {
                    x: heading.rect.x + pad,
                    width: heading.rect.width - pad,
                    ..heading.rect
                },
                palette.secondary,
            );
        }
        for slot in &layout.pages {
            let selected = slot.page == state.page;
            if selected {
                self.fill_round(slot.rect, radius, palette.accent);
            } else if state.hovered == Some(SettingsHit::Page(slot.page)) {
                self.fill_round(slot.rect, radius, palette.hover);
            }
            let tile_x = if layout.rail {
                slot.rect.x + (slot.rect.width - tile) / 2
            } else {
                slot.rect.x + pad
            };
            self.draw_tile(
                Rect {
                    x: tile_x,
                    y: slot.rect.y + (slot.rect.height - tile) / 2,
                    width: tile,
                    height: tile,
                },
                slot.page,
            );
            if !layout.rail {
                let text_x = tile_x + tile + self.px(8);
                self.text(
                    slot.page.label(),
                    &self.formats.sidebar,
                    Rect {
                        x: text_x,
                        width: slot.rect.right() - text_x,
                        ..slot.rect
                    },
                    if selected {
                        palette.on_accent
                    } else {
                        palette.text
                    },
                );
            }
        }
    }

    fn draw_body(&self, state: &SettingsPaintState<'_>, palette: &Palette) {
        let layout = state.layout;
        let area = Rect {
            x: layout.content.x,
            y: layout.header.bottom(),
            width: layout.content.width,
            height: layout.content.height - layout.header.height,
        };
        self.canvas.push_clip(area);
        if let Some(hero) = &state.content.hero {
            self.draw_hero(
                state.blocks.hero,
                hero.title.as_str(),
                hero.subtitle.as_str(),
                palette,
            );
        }
        for (section_index, (section, card)) in state
            .content
            .sections
            .iter()
            .zip(&state.blocks.cards)
            .enumerate()
        {
            if card.card.bottom() < area.y || card.header.y > area.bottom() {
                continue;
            }
            self.draw_card(
                state,
                section_index,
                section.title,
                &section.rows,
                card,
                palette,
            );
            if let Some(footer) = section.footer {
                self.text(footer, &self.formats.footer, card.footer, palette.secondary);
            }
        }
        self.canvas.pop_clip();
    }

    fn draw_hero(&self, rect: Rect, title: &str, subtitle: &str, palette: &Palette) {
        let edge = self.px(HERO_TILE_POINTS);
        let icon = Rect {
            x: rect.x + (rect.width - edge) / 2,
            y: rect.y + self.px(8),
            width: edge,
            height: edge,
        };
        self.draw_app_icon(icon);
        self.text(
            title,
            &self.formats.hero_title,
            Rect {
                y: icon.bottom() + self.px(8),
                height: self.px(30),
                ..rect
            },
            palette.text,
        );
        self.text(
            subtitle,
            &self.formats.hero_subtitle,
            Rect {
                y: icon.bottom() + self.px(38),
                height: self.px(20),
                ..rect
            },
            palette.secondary,
        );
    }
}
