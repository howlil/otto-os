use otto_core::{HitTarget, Layout, PRIVACY_DOT_POINTS, PrivacyState, Rect, scale};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
    D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE, ID2D1SolidColorBrush,
};
use windows::core::Result;
use windows_numerics::Vector2;

use super::{EQUALIZER_GREEN, FOREGROUND, Renderer, rect_to_d2d, rgb, translate_rect};

const PRIVACY_ORANGE: D2D1_COLOR_F = rgb(255, 159, 10, 1.0);
const PRIVACY_BLUE: D2D1_COLOR_F = rgb(10, 132, 255, 1.0);
const MUTED_FILL: D2D1_COLOR_F = rgb(255, 69, 58, 0.24);
const MUTED_BORDER: D2D1_COLOR_F = rgb(255, 69, 58, 0.48);
const MIC_ACTIVE_FILL: D2D1_COLOR_F = rgb(255, 159, 10, 0.20);
const MIC_ACTIVE_BORDER: D2D1_COLOR_F = rgb(255, 159, 10, 0.48);
const CAM_ACTIVE_FILL: D2D1_COLOR_F = rgb(48, 209, 88, 0.18);
const CAM_ACTIVE_BORDER: D2D1_COLOR_F = rgb(48, 209, 88, 0.44);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivacyDotKind {
    Camera,
    Microphone,
    Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PillTone {
    Muted,
    MicActive,
    CameraActive,
    Neutral,
}

fn visible_bar_dot(privacy: &PrivacyState) -> Option<PrivacyDotKind> {
    if privacy.camera_active {
        Some(PrivacyDotKind::Camera)
    } else if privacy.mic.is_active() {
        Some(PrivacyDotKind::Microphone)
    } else if privacy.location_active {
        Some(PrivacyDotKind::Location)
    } else {
        None
    }
}

fn panel_pill_label(privacy: &PrivacyState, compact: bool) -> String {
    if compact {
        if privacy.mic.is_muted() {
            String::from("Off")
        } else {
            String::from("Mic")
        }
    } else {
        privacy.pill_label()
    }
}

fn pill_tone(privacy: &PrivacyState) -> PillTone {
    if privacy.mic.is_muted() {
        PillTone::Muted
    } else if privacy.mic.is_active() {
        PillTone::MicActive
    } else if privacy.camera_active {
        PillTone::CameraActive
    } else {
        PillTone::Neutral
    }
}

impl Renderer {
    pub(crate) fn draw_bar_privacy_dots(
        &self,
        layout: &Layout,
        hovered: Option<HitTarget>,
        privacy: &PrivacyState,
    ) {
        if layout.privacy_dots.is_empty() || !privacy.show_in_bar() {
            return;
        }
        let Some(kind) = visible_bar_dot(privacy) else {
            return;
        };
        if hovered == Some(HitTarget::PrivacyIndicator) {
            let hover_rect = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(Rect {
                    x: layout.privacy_dots.x - scale(2, self.dpi),
                    y: layout.privacy_dots.y + scale(6, self.dpi),
                    width: layout.privacy_dots.width + scale(4, self.dpi),
                    height: (layout.privacy_dots.height - scale(12, self.dpi)).max(8),
                }),
                radiusX: scale(9, self.dpi) as f32,
                radiusY: scale(9, self.dpi) as f32,
            };
            let hover_brush = self.brushes.alpha_brush(FOREGROUND, 0.14);
            unsafe {
                self.target.FillRoundedRectangle(&hover_rect, &hover_brush);
            }
        }
        let dot_d = (scale(PRIVACY_DOT_POINTS, self.dpi) as f32).max(5.0);
        let dot_r = dot_d * 0.5;
        let cx = layout.privacy_dots.x as f32 + layout.privacy_dots.width as f32 * 0.5;
        let cy = layout.privacy_dots.y as f32 + layout.privacy_dots.height as f32 * 0.5;
        if kind == PrivacyDotKind::Location {
            let _ = self.draw_location_arrow(cx, cy, dot_r * 1.5);
            return;
        }
        let color = match kind {
            PrivacyDotKind::Camera => EQUALIZER_GREEN,
            _ => PRIVACY_ORANGE,
        };
        let halo = self.brushes.alpha_brush(color, 0.24);
        let core = self.brushes.alpha_brush(color, 0.96);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: dot_r * 1.45,
                    radiusY: dot_r * 1.45,
                },
                &halo,
            );
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: dot_r,
                    radiusY: dot_r,
                },
                &core,
            );
        }
    }

    fn draw_location_arrow(&self, cx: f32, cy: f32, half: f32) -> Result<()> {
        let brush = self.brushes.alpha_brush(PRIVACY_BLUE, 0.96);
        unsafe {
            let arrow = self.factory.CreatePathGeometry()?;
            let sink = arrow.Open()?;
            sink.BeginFigure(
                Vector2::new(cx + half * 0.9, cy - half * 0.9),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx - half * 0.85, cy - half * 0.05));
            sink.AddLine(Vector2::new(cx - half * 0.05, cy + half * 0.05));
            sink.AddLine(Vector2::new(cx + half * 0.05, cy + half * 0.85));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&arrow, &brush, None);
        }
        Ok(())
    }

    pub(crate) fn draw_panel_privacy_pill(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        privacy: &PrivacyState,
    ) -> Result<()> {
        let pill = translate_rect(layout.privacy_mic_button, -offset_x);
        if pill.is_empty() {
            return Ok(());
        }
        let is_hovered = hovered == Some(HitTarget::PrivacyMicMute);
        let (fill, border, accent) = match pill_tone(privacy) {
            PillTone::Muted => (
                self.brushes.alpha_brush(MUTED_FILL, alpha),
                self.brushes.alpha_brush(MUTED_BORDER, alpha),
                self.brushes.alpha_brush(FOREGROUND, 0.94 * alpha),
            ),
            PillTone::MicActive => (
                self.brushes
                    .alpha_brush(MIC_ACTIVE_FILL, if is_hovered { 1.35 } else { 1.0 } * alpha),
                self.brushes.alpha_brush(MIC_ACTIVE_BORDER, alpha),
                self.brushes.alpha_brush(PRIVACY_ORANGE, 0.96 * alpha),
            ),
            PillTone::CameraActive => (
                self.brushes
                    .alpha_brush(CAM_ACTIVE_FILL, if is_hovered { 1.35 } else { 1.0 } * alpha),
                self.brushes.alpha_brush(CAM_ACTIVE_BORDER, alpha),
                self.brushes.alpha_brush(EQUALIZER_GREEN, 0.96 * alpha),
            ),
            PillTone::Neutral => (
                self.brushes
                    .alpha_brush(FOREGROUND, (if is_hovered { 0.16 } else { 0.08 }) * alpha),
                self.brushes.alpha_brush(FOREGROUND, 0.14 * alpha),
                self.brushes.alpha_brush(FOREGROUND, 0.88 * alpha),
            ),
        };
        let radius = pill.height as f32 * 0.5;
        let rounded = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(pill),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&rounded, &fill);
            self.target
                .DrawRoundedRectangle(&rounded, &border, 1.0, None);
        }

        let fg = self.brushes.alpha_brush(FOREGROUND, 0.94 * alpha);
        let compact = pill.width <= scale(54, self.dpi);
        let pad_x = scale(6, self.dpi) as f32;
        let half_h = scale(5, self.dpi) as f32;
        let icon_cx = pill.x as f32 + pad_x + half_h * 0.65;
        let cy = pill.y as f32 + pill.height as f32 * 0.5;
        let stroke = (scale(1, self.dpi) as f32).max(1.2);
        self.draw_mic_glyph(
            icon_cx,
            cy,
            half_h,
            stroke,
            privacy.mic.is_muted(),
            &accent,
            &fg,
        )?;

        let label = panel_pill_label(privacy, compact);
        let text_left = pill.x as f32 + scale(18, self.dpi) as f32;
        let text_rect = D2D_RECT_F {
            left: text_left,
            top: pill.y as f32,
            right: pill.right() as f32 - pad_x,
            bottom: pill.bottom() as f32,
        };
        if text_rect.right > text_rect.left + 2.0 {
            self.push_clip(text_rect);
            self.draw_text(&label, &self.text_pomodoro_sub_trailing, text_rect, &fg);
            self.pop_clip();
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_mic_glyph(
        &self,
        cx: f32,
        cy: f32,
        half_h: f32,
        stroke: f32,
        muted: bool,
        accent: &ID2D1SolidColorBrush,
        slash_brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let cap_half_w = half_h * 0.34;
        let cap_top = cy - half_h * 0.92;
        let cap_bottom = cy + half_h * 0.16;
        let capsule = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - cap_half_w,
                top: cap_top,
                right: cx + cap_half_w,
                bottom: cap_bottom,
            },
            radiusX: cap_half_w,
            radiusY: cap_half_w,
        };
        unsafe {
            self.target.FillRoundedRectangle(&capsule, accent);
            let cradle_r = half_h * 0.64;
            let cradle_y = cy - half_h * 0.05;
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - cradle_r, cradle_y),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(cx + cradle_r, cradle_y),
                size: D2D_SIZE_F {
                    width: cradle_r,
                    height: cradle_r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, accent, stroke, &self.stroke_style);
            let stem_top = cradle_y + cradle_r;
            let base_y = cy + half_h * 0.92;
            self.target.DrawLine(
                Vector2::new(cx, stem_top),
                Vector2::new(cx, base_y),
                accent,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - cradle_r * 0.55, base_y),
                Vector2::new(cx + cradle_r * 0.55, base_y),
                accent,
                stroke,
                &self.stroke_style,
            );
            if muted {
                self.target.DrawLine(
                    Vector2::new(cx - half_h * 0.72, cy - half_h * 0.72),
                    Vector2::new(cx + half_h * 0.72, cy + half_h * 0.72),
                    slash_brush,
                    stroke * 1.1,
                    &self.stroke_style,
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use otto_core::{MicState, PrivacyState};

    use super::{PillTone, PrivacyDotKind, panel_pill_label, pill_tone, visible_bar_dot};

    #[test]
    fn bar_indicator_shows_one_dot_and_prioritizes_camera() {
        let mut state = PrivacyState {
            mic: MicState::Idle { muted: true },
            camera_active: false,
            location_active: false,
            app_name: None,
        };
        assert_eq!(visible_bar_dot(&state), None);

        state.mic = MicState::Active { muted: false };
        assert_eq!(visible_bar_dot(&state), Some(PrivacyDotKind::Microphone));

        state.camera_active = true;
        assert_eq!(visible_bar_dot(&state), Some(PrivacyDotKind::Camera));

        state.camera_active = false;
        state.mic = MicState::Idle { muted: false };
        state.location_active = true;
        assert_eq!(visible_bar_dot(&state), Some(PrivacyDotKind::Location));

        state.mic = MicState::Active { muted: false };
        assert_eq!(visible_bar_dot(&state), Some(PrivacyDotKind::Microphone));
    }

    #[test]
    fn panel_pill_uses_compact_short_labels_and_full_app_labels() {
        let live = PrivacyState {
            mic: MicState::Active { muted: false },
            camera_active: false,
            location_active: false,
            app_name: Some(String::from("Zoom")),
        };
        assert_eq!(panel_pill_label(&live, true), "Mic");
        assert_eq!(panel_pill_label(&live, false), "Mic On · Zoom");
        assert_eq!(pill_tone(&live), PillTone::MicActive);

        let muted = PrivacyState {
            mic: MicState::Active { muted: true },
            camera_active: true,
            location_active: false,
            app_name: Some(String::from("OBS")),
        };
        assert_eq!(panel_pill_label(&muted, true), "Off");
        assert_eq!(panel_pill_label(&muted, false), "Muted · OBS");
        assert_eq!(pill_tone(&muted), PillTone::Muted);
    }
}
