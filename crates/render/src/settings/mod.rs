mod palette;
mod rows;
#[cfg(test)]
mod tests;
mod tiles;

use otto_core::{
    ContentLayout, Rect, SettingsContent, SettingsHit, SettingsLayout, SettingsPage, scale,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LINE_JOIN_ROUND, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_STROKE_STYLE_PROPERTIES, ID2D1HwndRenderTarget,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_WORD_WRAPPING_WRAP, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;

use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

use self::palette::{
    HERO_TILE_POINTS, Palette, ROW_PAD_POINTS, SIDEBAR_RADIUS_POINTS, SIDEBAR_TILE_POINTS, WHITE,
    d2d, rgba,
};

pub struct SettingsPaintState<'a> {
    pub layout: &'a SettingsLayout,
    pub content: &'a SettingsContent,
    pub blocks: &'a ContentLayout,
    pub page: SettingsPage,
    pub scroll: i32,
    pub hovered: Option<SettingsHit>,
    pub dark: bool,
    pub glass: bool,
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
    pub(super) hero_title: IDWriteTextFormat,
    pub(super) hero_subtitle: IDWriteTextFormat,
}

impl Formats {
    fn new(dpi: u32) -> Result<Self> {
        unsafe {
            let dwrite = shared_dwrite_factory()?;
            let make = |points: i32, weight, align| {
                create_text_format(&dwrite, scale(points, dpi) as f32, weight, align)
            };
            let footer = make(
                11,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            footer.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            footer.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
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
                hero_title: make(
                    22,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                )?,
                hero_subtitle: make(13, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_TEXT_ALIGNMENT_CENTER)?,
            })
        }
    }
}

pub struct SettingsRenderer {
    pub(super) target: ID2D1HwndRenderTarget,
    pub(super) brush: ID2D1SolidColorBrush,
    pub(super) stroke: ID2D1StrokeStyle,
    pub(super) formats: Formats,
    pub(super) dpi: u32,
}

impl SettingsRenderer {
    pub fn new(hwnd: isize, width: i32, height: i32, dpi: u32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = factory.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    ..D2D1_RENDER_TARGET_PROPERTIES::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: HWND(hwnd as *mut _),
                    pixelSize: D2D_SIZE_U {
                        width: width.max(1) as u32,
                        height: height.max(1) as u32,
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
            )?;
            target.SetDpi(96.0, 96.0);
            let brush = target.CreateSolidColorBrush(&rgba(0, 1.0), None)?;
            let stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 2.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )?;
            Ok(Self {
                target,
                brush,
                stroke,
                formats: Formats::new(dpi)?,
                dpi,
            })
        }
    }

    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        unsafe {
            self.target.Resize(&D2D_SIZE_U {
                width: width.max(1) as u32,
                height: height.max(1) as u32,
            })
        }
    }

    pub fn set_dpi(&mut self, dpi: u32) -> Result<()> {
        if dpi != self.dpi {
            self.formats = Formats::new(dpi)?;
            self.dpi = dpi;
        }
        Ok(())
    }

    pub fn draw(&mut self, state: &SettingsPaintState<'_>) -> Result<()> {
        let palette = Palette::pick(state.dark, state.glass);
        let clear = if palette.glass {
            rgba(0, 0.0)
        } else {
            palette.background
        };
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&clear));
        }
        if palette.glass {
            self.fill(
                Rect {
                    x: 0,
                    y: 0,
                    width: state.layout.sidebar.width + state.layout.content.width,
                    height: state.layout.sidebar.height,
                },
                palette.background,
            );
        }
        self.draw_body(state, &palette);
        self.draw_toolbar(state, &palette);
        self.draw_sidebar(state, &palette);
        unsafe { self.target.EndDraw(None, None) }
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
                    if selected { WHITE } else { palette.text },
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
        unsafe {
            self.target
                .PushAxisAlignedClip(&d2d(area), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
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
        unsafe {
            self.target.PopAxisAlignedClip();
        }
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
