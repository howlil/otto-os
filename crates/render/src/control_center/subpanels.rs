use crate::{LiquidGlassPalette, rect_to_d2d, scale};
use otto_core::{
    BluetoothDevice, BluetoothDeviceKind, ControlCenterHitTarget, ControlCenterLayout,
    ControlCenterPage, ControlCenterSnapshot, Rect, WifiNetwork,
};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ELLIPSE, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS, D2D1_ROUNDED_RECT, ID2D1Geometry,
};
use windows::core::Result;
use windows_numerics::Vector2;

use super::ControlCenterPaintState;
use super::ControlCenterRenderer;
use super::ControlVisualState;
use super::PointerState;

impl ControlCenterRenderer {
    pub(super) fn draw_subpanel_page(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_subpanel_header(layout, snapshot, state, palette);
        match layout.page {
            ControlCenterPage::Sound => {
                self.draw_sound_subpanel(layout, snapshot, state, palette)?;
            }
            ControlCenterPage::Display => {
                self.draw_display_subpanel(layout, snapshot, state, palette);
            }
            ControlCenterPage::Wifi => {
                self.draw_scrollable_list(layout, snapshot, state, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Wi-Fi Settings...",
                    state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::Bluetooth => {
                self.draw_scrollable_list(layout, snapshot, state, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Bluetooth Settings...",
                    state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::Main => {}
        }
        Ok(())
    }

    pub(super) fn draw_sound_subpanel(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(
            layout.sound_card,
            layout.card_radius as f32,
            false,
            false,
            palette,
        );
        let hdr = layout.sound_header;
        let pad = scale(10, self.dpi) as f32;
        let value_text = if snapshot.status.muted || snapshot.status.volume == 0 {
            String::from("Muted")
        } else {
            format!("{}%", snapshot.status.volume)
        };
        self.draw_text(
            "Volume",
            &self.title_format,
            D2D_RECT_F {
                left: hdr.x as f32 + pad,
                top: hdr.y as f32 + scale(4, self.dpi) as f32,
                right: hdr.right() as f32 - scale(92, self.dpi) as f32,
                bottom: hdr.bottom() as f32,
            },
            &self.brushes.text_primary,
        );
        self.draw_text(
            &value_text,
            &self.value_format,
            D2D_RECT_F {
                left: hdr.right() as f32 - scale(90, self.dpi) as f32,
                top: hdr.y as f32 + scale(4, self.dpi) as f32,
                right: hdr.right() as f32 - pad,
                bottom: hdr.bottom() as f32,
            },
            &self.brushes.text_muted,
        );
        self.draw_sound_controls(layout, snapshot, state, palette)?;
        self.draw_scrollable_list(layout, snapshot, state, palette);
        self.draw_footer_button(
            layout.footer_primary,
            if snapshot.audio_switch_failed {
                "Open Sound Settings..."
            } else {
                "Sound Settings..."
            },
            state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
            state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
            palette,
        );
        Ok(())
    }

    pub(super) fn draw_display_subpanel(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let card = layout.display_card;
        self.draw_card(card, layout.card_radius as f32, false, false, palette);
        let pad = scale(10, self.dpi) as f32;
        let val_text = snapshot
            .status
            .brightness
            .map_or_else(|| String::from("Unavailable"), |pct| format!("{pct}%"));
        self.draw_text(
            "Brightness",
            &self.title_format,
            D2D_RECT_F {
                left: card.x as f32 + pad,
                top: card.y as f32 + scale(4, self.dpi) as f32,
                right: card.right() as f32 - scale(90, self.dpi) as f32,
                bottom: card.y as f32 + scale(26, self.dpi) as f32,
            },
            &self.brushes.text_primary,
        );
        self.draw_text(
            &val_text,
            &self.value_format,
            D2D_RECT_F {
                left: card.right() as f32 - scale(90, self.dpi) as f32,
                top: card.y as f32 + scale(4, self.dpi) as f32,
                right: card.right() as f32 - pad,
                bottom: card.y as f32 + scale(26, self.dpi) as f32,
            },
            &self.brushes.text_muted,
        );
        self.draw_capsule_slider(
            layout.display_slider,
            snapshot.status.brightness,
            state.hovered == Some(ControlCenterHitTarget::DisplaySlider),
            state.pressed == Some(ControlCenterHitTarget::DisplaySlider),
            true,
            palette,
        );
        self.draw_footer_button(
            layout.footer_primary,
            "Display Settings...",
            state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
            state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
            palette,
        );
        self.draw_footer_button(
            layout.footer_secondary,
            "Night Light...",
            state.hovered == Some(ControlCenterHitTarget::FooterSecondary),
            state.pressed == Some(ControlCenterHitTarget::FooterSecondary),
            palette,
        );
    }

    pub(super) fn draw_subpanel_header(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let back = layout.back_button;
        let back_r = back.height as f32 * 0.5;
        let back_bg = self.brushes.color_brush(palette.toggle_inactive);
        unsafe {
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(back),
                    radiusX: back_r,
                    radiusY: back_r,
                },
                &back_bg,
            );
        }
        self.draw_interaction_overlay(
            back,
            back_r,
            state.hovered == Some(ControlCenterHitTarget::BackButton),
            state.pressed == Some(ControlCenterHitTarget::BackButton),
            palette,
        );
        self.draw_chevron(
            back.x as f32 + back.width as f32 * 0.5,
            back.y as f32 + back.height as f32 * 0.5,
            scale(4, self.dpi) as f32,
            false,
            &self.brushes.text_primary,
        );

        let title = match layout.page {
            ControlCenterPage::Wifi => "Wi-Fi",
            ControlCenterPage::Bluetooth => "Bluetooth",
            ControlCenterPage::Sound => "Sound",
            ControlCenterPage::Display => "Display",
            ControlCenterPage::Main => "",
        };
        let title_r = D2D_RECT_F {
            left: back.right() as f32 + scale(10, self.dpi) as f32,
            top: layout.header_rect.y as f32,
            right: if layout.header_toggle.is_empty() {
                layout.header_rect.right() as f32
            } else {
                layout.header_toggle.x as f32 - scale(8, self.dpi) as f32
            },
            bottom: layout.header_rect.bottom() as f32,
        };
        self.draw_text(
            title,
            &self.title_format,
            title_r,
            &self.brushes.text_primary,
        );

        if !layout.header_toggle.is_empty() {
            let (enabled, supported) = match layout.page {
                ControlCenterPage::Wifi => (snapshot.wifi.enabled, snapshot.wifi.supported),
                ControlCenterPage::Bluetooth => {
                    (snapshot.bluetooth.enabled, snapshot.bluetooth.supported)
                }
                _ => (false, false),
            };
            self.draw_toggle_pill(
                layout.header_toggle,
                ControlVisualState {
                    active: enabled,
                    enabled: supported,
                    pointer: PointerState::from_flags(
                        state.hovered == Some(ControlCenterHitTarget::HeaderToggle),
                        state.pressed == Some(ControlCenterHitTarget::HeaderToggle),
                    ),
                },
                palette,
            );
        }
    }

    pub(super) fn draw_scrollable_list(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        if vp.is_empty() {
            return;
        }
        unsafe {
            self.target
                .PushAxisAlignedClip(&rect_to_d2d(vp), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
        self.draw_list_contents(layout, snapshot, state, palette);
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    pub(super) fn draw_list_contents(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        if layout.items.is_empty() {
            self.draw_empty_list_state(layout.page, vp, snapshot);
            return;
        }
        for group in &layout.groups {
            self.draw_list_group(layout, group, snapshot, state, palette);
        }
        if layout.max_scroll > 0 {
            self.draw_list_scrollbar(vp, layout.scroll_offset, layout.max_scroll, palette);
        }
    }

    pub(super) fn draw_empty_list_state(
        &self,
        page: ControlCenterPage,
        vp: Rect,
        snapshot: &ControlCenterSnapshot,
    ) {
        let empty = match page {
            ControlCenterPage::Wifi => {
                if !snapshot.wifi.supported {
                    "Wi-Fi adapter unavailable"
                } else if !snapshot.wifi.enabled {
                    "Turn on Wi-Fi to view networks"
                } else if snapshot.wifi.busy {
                    "Scanning networks..."
                } else {
                    "No networks found"
                }
            }
            ControlCenterPage::Bluetooth => {
                if !snapshot.bluetooth.supported {
                    "Bluetooth adapter unavailable"
                } else if !snapshot.bluetooth.enabled {
                    "Turn on Bluetooth to view devices"
                } else if snapshot.bluetooth.busy {
                    "Scanning devices..."
                } else {
                    "No paired Bluetooth devices"
                }
            }
            ControlCenterPage::Sound => "No audio output devices found",
            _ => "",
        };
        if !empty.is_empty() {
            self.draw_text(
                empty,
                &self.center_format,
                rect_to_d2d(vp),
                &self.brushes.text_muted,
            );
        }
    }

    pub(super) fn list_group_clip_geometry(
        &self,
        group_d2d: D2D_RECT_F,
        group_radius: f32,
    ) -> Option<ID2D1Geometry> {
        self.squircle_path(group_d2d, group_radius)
            .map(Into::into)
            .or_else(|| {
                unsafe {
                    self.factory
                        .CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                            rect: group_d2d,
                            radiusX: group_radius,
                            radiusY: group_radius,
                        })
                }
                .ok()
                .map(Into::into)
            })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn draw_list_group(
        &self,
        layout: &ControlCenterLayout,
        group: &otto_core::SubPanelGroupLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        let group_radius = layout.card_radius as f32;
        let sep_inset = scale(12, self.dpi) as f32;

        if !group.header.is_empty()
            && group.header.bottom() >= vp.y
            && group.header.y <= vp.bottom()
        {
            let hdr_r = D2D_RECT_F {
                left: group.header.x as f32 + scale(6, self.dpi) as f32,
                top: group.header.y as f32 + scale(5, self.dpi) as f32,
                right: group.header.right() as f32,
                bottom: group.header.bottom() as f32 - scale(3, self.dpi) as f32,
            };
            self.draw_text(
                group.title,
                &self.subtitle_format,
                hdr_r,
                &self.brushes.text_muted,
            );
        }
        if group.container.bottom() < vp.y || group.container.y > vp.bottom() {
            return;
        }
        self.draw_card(group.container, group_radius, false, false, palette);

        let group_d2d = rect_to_d2d(group.container);
        let clip_geo = self.list_group_clip_geometry(group_d2d, group_radius);
        let clip_layer = clip_geo
            .as_ref()
            .and_then(|_| unsafe { self.target.CreateLayer(None) }.ok());
        let mut active_mask = None;
        if let (Some(geo), Some(layer)) = (clip_geo, clip_layer.as_ref()) {
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: group_d2d,
                geometricMask: std::mem::ManuallyDrop::new(Some(geo)),
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

        for offset in 0..group.count {
            let index = group.start_index + offset;
            let Some(rect) = layout.items.get(index).copied() else {
                break;
            };
            if rect.bottom() < vp.y || rect.y > vp.bottom() {
                continue;
            }
            if offset > 0 {
                let sep = self.brushes.color_brush(palette.menu_separator);
                unsafe {
                    self.target.DrawLine(
                        Vector2::new(rect.x as f32 + sep_inset, rect.y as f32),
                        Vector2::new(rect.right() as f32 - sep_inset, rect.y as f32),
                        &sep,
                        1.0,
                        None,
                    );
                }
            }
            let target = ControlCenterHitTarget::SubPanelItem(index);
            let is_hover = state.hovered == Some(target);
            let is_pressed = state.pressed == Some(target);
            self.draw_interaction_overlay(rect, 0.0, is_hover, is_pressed, palette);
            self.draw_subpanel_row(layout.page, rect, index, snapshot, palette);
        }

        if let Some(mask) = active_mask {
            unsafe {
                self.target.PopLayer();
            }
            std::mem::ManuallyDrop::into_inner(mask);
        }
    }

    pub(super) fn draw_subpanel_row(
        &self,
        page: ControlCenterPage,
        rect: Rect,
        index: usize,
        snapshot: &ControlCenterSnapshot,
        palette: &LiquidGlassPalette,
    ) {
        match page {
            ControlCenterPage::Wifi => {
                if let Some(net) = snapshot.wifi.networks.get(index) {
                    self.draw_wifi_network_row(rect, net, palette);
                }
            }
            ControlCenterPage::Bluetooth => {
                if let Some(dev) = snapshot.bluetooth.devices.get(index) {
                    self.draw_bluetooth_device_row(rect, dev, palette);
                }
            }
            ControlCenterPage::Sound => {
                if let Some(dev) = snapshot.audio_outputs.get(index) {
                    self.draw_audio_output_row(rect, &dev.name, dev.is_default, palette);
                }
            }
            ControlCenterPage::Main | ControlCenterPage::Display => {}
        }
    }

    pub(super) fn draw_list_scrollbar(
        &self,
        vp: Rect,
        scroll_offset: i32,
        max_scroll: i32,
        palette: &LiquidGlassPalette,
    ) {
        let bar_w = scale(3, self.dpi) as f32;
        let pad_v = scale(2, self.dpi) as f32;
        let track_h = (vp.height as f32 - pad_v * 2.0).max(8.0);
        let total_h = (vp.height + max_scroll) as f32;
        let thumb_h = (track_h * (track_h / total_h)).max(scale(20, self.dpi) as f32);
        let frac = scroll_offset as f32 / max_scroll as f32;
        let thumb_y = vp.y as f32 + pad_v + frac * (track_h - thumb_h);
        let bar_right = vp.right() as f32 - scale(1, self.dpi) as f32;
        let thumb_r = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bar_right - bar_w,
                top: thumb_y,
                right: bar_right,
                bottom: thumb_y + thumb_h,
            },
            radiusX: bar_w * 0.5,
            radiusY: bar_w * 0.5,
        };
        let brush = self.brushes.color_brush(palette.text_disabled);
        unsafe {
            self.target.FillRoundedRectangle(&thumb_r, &brush);
        }
    }

    pub(super) fn draw_wifi_network_row(
        &self,
        rect: Rect,
        net: &WifiNetwork,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(10, self.dpi) as f32;
        let label_r = D2D_RECT_F {
            left: rect.x as f32 + pad,
            top: rect.y as f32,
            right: rect.right() as f32 - scale(76, self.dpi) as f32,
            bottom: rect.bottom() as f32,
        };
        let status_r = D2D_RECT_F {
            left: rect.right() as f32 - scale(84, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - pad,
            bottom: rect.bottom() as f32,
        };
        self.draw_text(
            &net.ssid,
            &self.body_format,
            label_r,
            &self.brushes.text_primary,
        );
        let right_label = if net.connected {
            format!("Connected · {}%", net.signal_percent)
        } else if net.secured {
            format!("{}% · Lock", net.signal_percent)
        } else {
            format!("{}%", net.signal_percent)
        };
        let status_brush = if net.connected {
            self.brushes.color_brush(palette.toggle_active)
        } else {
            self.brushes.text_muted.clone()
        };
        self.draw_text(&right_label, &self.value_format, status_r, &status_brush);
    }

    pub(super) fn draw_bluetooth_device_row(
        &self,
        rect: Rect,
        device: &BluetoothDevice,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(10, self.dpi) as f32;
        let kind = match device.kind {
            BluetoothDeviceKind::Headphones => "Audio",
            BluetoothDeviceKind::Speaker => "Speaker",
            BluetoothDeviceKind::Mouse => "Mouse",
            BluetoothDeviceKind::Keyboard => "Keyboard",
            BluetoothDeviceKind::Phone => "Phone",
            BluetoothDeviceKind::Other => "Device",
        };
        let label_r = D2D_RECT_F {
            left: rect.x as f32 + pad,
            top: rect.y as f32,
            right: rect.right() as f32 - scale(96, self.dpi) as f32,
            bottom: rect.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: rect.right() as f32 - scale(96, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - pad,
            bottom: rect.bottom() as f32,
        };
        self.draw_text(
            &device.name,
            &self.body_format,
            label_r,
            &self.brushes.text_primary,
        );
        let right_text = if device.connected {
            device.battery_percent.map_or_else(
                || String::from("Connected"),
                |pct| format!("Connected · {pct}%"),
            )
        } else {
            String::from(kind)
        };
        let brush = if device.connected {
            self.brushes.color_brush(palette.toggle_active)
        } else {
            self.brushes.text_muted.clone()
        };
        self.draw_text(&right_text, &self.value_format, right_r, &brush);
    }

    pub(super) fn draw_audio_output_row(
        &self,
        rect: Rect,
        name: &str,
        is_default: bool,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(10, self.dpi) as f32;
        let dot_r = scale(4, self.dpi) as f32;
        let cx = rect.x as f32 + pad + dot_r;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let accent = self.brushes.color_brush(palette.toggle_active);
        unsafe {
            let ellipse = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: dot_r,
                radiusY: dot_r,
            };
            if is_default {
                self.target.FillEllipse(&ellipse, &accent);
            } else {
                self.target
                    .DrawEllipse(&ellipse, &self.brushes.text_muted, 1.0, None);
            }
        }
        let label_r = D2D_RECT_F {
            left: cx + dot_r + scale(8, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - scale(56, self.dpi) as f32,
            bottom: rect.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: rect.right() as f32 - scale(56, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - pad,
            bottom: rect.bottom() as f32,
        };
        self.draw_text(name, &self.body_format, label_r, &self.brushes.text_primary);
        if is_default {
            self.draw_text("Active", &self.value_format, right_r, &accent);
        }
    }
}
