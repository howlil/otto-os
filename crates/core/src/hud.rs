use crate::control_center::BluetoothDeviceKind;
use crate::layout::{Metrics, Rect, scale};
use crate::motion::MotionMetrics;
use crate::status::{HudDeviceLabel, InputLangCode, NotchHudKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudTone {
    Neutral,
    Positive,
    Warning,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudGlyph {
    Speaker,
    SpeakerMuted,
    Sun,
    Bolt,
    Battery,
    CapsLock,
    NumLock,
    Keyboard,
    BatterySaver,
    Moon,
    Bluetooth,
    Headphones,
    Otto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudLabel {
    Static(&'static str),
    Custom(HudDeviceLabel),
}

impl HudLabel {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Static(text) => text,
            Self::Custom(text) => text.as_str(),
        }
    }
}

impl PartialEq<&str> for HudLabel {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for HudLabel {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudValue {
    Percent(u8),
    Muted,
    Pill { on: bool },
    Text(InputLangCode),
    Badge(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudPresentation {
    pub tone: HudTone,
    pub glyph: HudGlyph,
    pub label: HudLabel,
    pub value: HudValue,
    pub level: Option<f32>,
}

#[must_use]
pub fn present_hud(kind: NotchHudKind) -> HudPresentation {
    match kind {
        NotchHudKind::Volume { percent, muted } => HudPresentation {
            tone: if muted {
                HudTone::Muted
            } else {
                HudTone::Neutral
            },
            glyph: if muted || percent == 0 {
                HudGlyph::SpeakerMuted
            } else {
                HudGlyph::Speaker
            },
            label: HudLabel::Static("Volume"),
            value: if muted {
                HudValue::Muted
            } else {
                HudValue::Percent(percent.min(100))
            },
            level: Some(if muted { 0.0 } else { fraction(percent) }),
        },
        NotchHudKind::Brightness { percent } => HudPresentation {
            tone: HudTone::Neutral,
            glyph: HudGlyph::Sun,
            label: HudLabel::Static("Brightness"),
            value: HudValue::Percent(percent.min(100)),
            level: Some(fraction(percent)),
        },
        NotchHudKind::Charging {
            percent,
            plugged_in,
        } => charging_hud(percent, plugged_in),
        NotchHudKind::LowBattery { percent } => HudPresentation {
            tone: HudTone::Warning,
            glyph: HudGlyph::Battery,
            label: HudLabel::Static("Low battery"),
            value: HudValue::Percent(percent.min(100)),
            level: Some(fraction(percent)),
        },
        NotchHudKind::CapsLock { enabled } => toggle_hud(HudGlyph::CapsLock, "Caps Lock", enabled),
        NotchHudKind::NumLock { enabled } => toggle_hud(HudGlyph::NumLock, "Num Lock", enabled),
        NotchHudKind::BatterySaver { enabled } => {
            toggle_hud(HudGlyph::BatterySaver, "Battery Saver", enabled)
        }
        NotchHudKind::Dnd { enabled } => toggle_hud(HudGlyph::Moon, "Do Not Disturb", enabled),
        NotchHudKind::KeyboardLayout { code } => HudPresentation {
            tone: HudTone::Neutral,
            glyph: HudGlyph::Keyboard,
            label: HudLabel::Static("Input Source"),
            value: HudValue::Text(code),
            level: None,
        },
        NotchHudKind::BluetoothConnected {
            name,
            kind,
            battery_percent,
        } => bluetooth_hud(name, kind, battery_percent),
        NotchHudKind::Welcome => HudPresentation {
            tone: HudTone::Positive,
            glyph: HudGlyph::Otto,
            label: HudLabel::Static("Otto Ready"),
            value: HudValue::Badge("Ctrl+Alt+N"),
            level: None,
        },
    }
}
fn charging_hud(percent: u8, plugged_in: bool) -> HudPresentation {
    HudPresentation {
        tone: if plugged_in {
            HudTone::Positive
        } else {
            HudTone::Neutral
        },
        glyph: if plugged_in {
            HudGlyph::Bolt
        } else {
            HudGlyph::Battery
        },
        label: HudLabel::Static(if plugged_in { "Charging" } else { "On battery" }),
        value: HudValue::Percent(percent.min(100)),
        level: Some(fraction(percent)),
    }
}

fn bluetooth_hud(
    name: HudDeviceLabel,
    kind: BluetoothDeviceKind,
    battery_percent: Option<u8>,
) -> HudPresentation {
    HudPresentation {
        tone: if battery_percent.is_some_and(|pct| pct <= 20) {
            HudTone::Warning
        } else {
            HudTone::Positive
        },
        glyph: if matches!(
            kind,
            BluetoothDeviceKind::Headphones | BluetoothDeviceKind::Speaker
        ) {
            HudGlyph::Headphones
        } else {
            HudGlyph::Bluetooth
        },
        label: if name.is_empty() {
            HudLabel::Static("Bluetooth")
        } else {
            HudLabel::Custom(name)
        },
        value: battery_percent.map_or(HudValue::Badge("Connected"), |pct| {
            HudValue::Percent(pct.min(100))
        }),
        level: None,
    }
}

fn toggle_hud(glyph: HudGlyph, label: &'static str, enabled: bool) -> HudPresentation {
    HudPresentation {
        tone: if enabled {
            HudTone::Positive
        } else {
            HudTone::Muted
        },
        glyph,
        label: HudLabel::Static(label),
        value: HudValue::Pill { on: enabled },
        level: None,
    }
}

fn fraction(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudGeometry {
    pub glyph: Rect,
    pub label: Rect,
    pub meter: Rect,
    pub value: Rect,
}

#[must_use]
pub fn compute_hud_geometry(area: Rect, metrics: &Metrics, dpi: u32) -> HudGeometry {
    let pad = scale(metrics.hud_pad_points, dpi);
    let gap = scale(metrics.hud_gap_points, dpi);
    let glyph_w = scale(metrics.hud_glyph_points, dpi);
    let label_w = scale(metrics.hud_label_points, dpi);
    let value_w = scale(metrics.hud_value_points, dpi);
    let meter_h = scale(metrics.hud_meter_height_points, dpi);

    let glyph = Rect {
        x: area.x + pad,
        y: area.y,
        width: glyph_w,
        height: area.height,
    };
    let label = Rect {
        x: glyph.right() + gap,
        y: area.y,
        width: label_w,
        height: area.height,
    };
    let value = Rect {
        x: area.right() - pad - value_w,
        y: area.y,
        width: value_w,
        height: area.height,
    };
    let meter_x = label.right() + gap;
    let meter = Rect {
        x: meter_x,
        y: area.y + (area.height - meter_h) / 2,
        width: (value.x - gap - meter_x).max(0),
        height: meter_h,
    };
    HudGeometry {
        glyph,
        label,
        meter,
        value,
    }
}

#[must_use]
pub fn hud_content_alpha(expansion: f32, motion: &MotionMetrics) -> f32 {
    let span = (motion.hud_content_end - motion.hud_content_start).max(f32::EPSILON);
    let t = ((expansion - motion.hud_content_start) / span).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 356,
            height: 32,
        }
    }

    #[test]
    fn muted_volume_reads_as_a_word_with_an_empty_meter() {
        let p = present_hud(NotchHudKind::Volume {
            percent: 0,
            muted: true,
        });
        assert_eq!(p.tone, HudTone::Muted);
        assert_eq!(p.value, HudValue::Muted);
        assert_eq!(p.glyph, HudGlyph::SpeakerMuted);
        assert_eq!(p.level, Some(0.0));
    }

    #[test]
    fn silent_volume_keeps_its_percentage_and_speaker_glyph_muted() {
        let p = present_hud(NotchHudKind::Volume {
            percent: 0,
            muted: false,
        });
        assert_eq!(p.tone, HudTone::Neutral);
        assert_eq!(p.value, HudValue::Percent(0));
        assert_eq!(p.glyph, HudGlyph::SpeakerMuted);
    }

    #[test]
    fn caps_lock_is_a_pill_without_a_meter() {
        let on = present_hud(NotchHudKind::CapsLock { enabled: true });
        assert_eq!(on.value, HudValue::Pill { on: true });
        assert_eq!(on.tone, HudTone::Positive);
        assert_eq!(on.level, None);

        let off = present_hud(NotchHudKind::CapsLock { enabled: false });
        assert_eq!(off.value, HudValue::Pill { on: false });
        assert_eq!(off.tone, HudTone::Muted);

        let num = present_hud(NotchHudKind::NumLock { enabled: true });
        assert_eq!(num.glyph, HudGlyph::NumLock);
        assert_eq!(num.label, "Num Lock");
        assert_eq!(num.value, HudValue::Pill { on: true });
        assert_eq!(num.level, None);

        let saver = present_hud(NotchHudKind::BatterySaver { enabled: false });
        assert_eq!(saver.glyph, HudGlyph::BatterySaver);
        assert_eq!(saver.label, "Battery Saver");
        assert_eq!(saver.value, HudValue::Pill { on: false });
        assert_eq!(saver.tone, HudTone::Muted);

        let lang = present_hud(NotchHudKind::KeyboardLayout {
            code: InputLangCode::from_tag("id-ID"),
        });
        assert_eq!(lang.glyph, HudGlyph::Keyboard);
        assert_eq!(lang.label, "Input Source");
        assert_eq!(lang.value, HudValue::Text(InputLangCode::from_tag("ID")));
        assert_eq!(lang.level, None);

        let welcome = present_hud(NotchHudKind::Welcome);
        assert_eq!(welcome.glyph, HudGlyph::Otto);
        assert_eq!(welcome.tone, HudTone::Positive);
        assert_eq!(welcome.label, "Otto Ready");
        assert_eq!(welcome.value, HudValue::Badge("Ctrl+Alt+N"));
        assert_eq!(welcome.level, None);
    }

    #[test]
    fn battery_events_carry_their_state_in_tone_and_label() {
        let charging = present_hud(NotchHudKind::Charging {
            percent: 84,
            plugged_in: true,
        });
        assert_eq!(charging.label, "Charging");
        assert_eq!(charging.tone, HudTone::Positive);

        let unplugged = present_hud(NotchHudKind::Charging {
            percent: 84,
            plugged_in: false,
        });
        assert_eq!(unplugged.label, "On battery");
        assert_eq!(unplugged.glyph, HudGlyph::Battery);

        let low = present_hud(NotchHudKind::LowBattery { percent: 20 });
        assert_eq!(low.label, "Low battery");
        assert_eq!(low.tone, HudTone::Warning);
        assert_eq!(low.level, Some(0.2));
    }

    #[test]
    fn levels_clamp_to_the_meter_range() {
        let p = present_hud(NotchHudKind::Brightness { percent: 250 });
        assert_eq!(p.value, HudValue::Percent(100));
        assert_eq!(p.level, Some(1.0));
    }

    #[test]
    fn bluetooth_connected_uses_device_label_appropriate_glyph_and_battery_or_badge() {
        let headset = present_hud(NotchHudKind::BluetoothConnected {
            name: crate::status::HudDeviceLabel::from_name("AirPods Pro"),
            kind: crate::BluetoothDeviceKind::Headphones,
            battery_percent: Some(85),
        });
        assert_eq!(headset.glyph, HudGlyph::Headphones);
        assert_eq!(headset.tone, HudTone::Positive);
        assert_eq!(headset.label, "AirPods Pro");
        assert_eq!(headset.value, HudValue::Percent(85));
        assert_eq!(headset.level, None);

        let mouse = present_hud(NotchHudKind::BluetoothConnected {
            name: crate::status::HudDeviceLabel::from_name("MX Master 3S"),
            kind: crate::BluetoothDeviceKind::Mouse,
            battery_percent: None,
        });
        assert_eq!(mouse.glyph, HudGlyph::Bluetooth);
        assert_eq!(mouse.tone, HudTone::Positive);
        assert_eq!(mouse.label, "MX Master 3S");
        assert_eq!(mouse.value, HudValue::Badge("Connected"));
        assert_eq!(mouse.level, None);

        let low = present_hud(NotchHudKind::BluetoothConnected {
            name: crate::status::HudDeviceLabel::from_name(""),
            kind: crate::BluetoothDeviceKind::Speaker,
            battery_percent: Some(15),
        });
        assert_eq!(low.glyph, HudGlyph::Headphones);
        assert_eq!(low.tone, HudTone::Warning);
        assert_eq!(low.label, "Bluetooth");
        assert_eq!(low.value, HudValue::Percent(15));
    }

    #[test]
    fn geometry_fills_the_hud_width_without_overlap() {
        let g = compute_hud_geometry(area(), &Metrics::MACBOOK_PRO_14, 96);
        assert_eq!(g.glyph.x, 16);
        assert_eq!(g.glyph.width, 16);
        assert_eq!(g.label.width, 88);
        assert_eq!(g.value.width, 40);
        assert_eq!(g.value.right(), 356 - 16);
        assert_eq!(g.meter.width, 150);
        assert!(g.label.right() <= g.meter.x);
        assert!(g.meter.right() <= g.value.x);
        assert_eq!(g.meter.height, 4);
        assert_eq!(g.meter.y, (32 - 4) / 2);
    }

    #[test]
    fn geometry_never_produces_a_negative_meter() {
        let narrow = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 32,
        };
        let g = compute_hud_geometry(narrow, &Metrics::MACBOOK_PRO_14, 96);
        assert_eq!(g.meter.width, 0);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() <= 1e-6, "{actual} != {expected}");
    }

    #[test]
    fn content_alpha_waits_for_the_shape_then_settles() {
        let motion = MotionMetrics::default();
        assert_close(hud_content_alpha(0.0, &motion), 0.0);
        assert_close(hud_content_alpha(motion.hud_content_start, &motion), 0.0);
        let mid = hud_content_alpha(0.55, &motion);
        assert!(mid > 0.0 && mid < 1.0);
        assert_close(hud_content_alpha(motion.hud_content_end, &motion), 1.0);
        assert_close(hud_content_alpha(1.0, &motion), 1.0);
    }

    #[test]
    fn dnd_uses_moon_glyph_and_toggle_pill() {
        let on = present_hud(NotchHudKind::Dnd { enabled: true });
        assert_eq!(on.glyph, HudGlyph::Moon);
        assert_eq!(on.tone, HudTone::Positive);
        assert_eq!(on.label, "Do Not Disturb");
        assert_eq!(on.value, HudValue::Pill { on: true });
        assert_eq!(on.level, None);

        let off = present_hud(NotchHudKind::Dnd { enabled: false });
        assert_eq!(off.glyph, HudGlyph::Moon);
        assert_eq!(off.tone, HudTone::Muted);
        assert_eq!(off.value, HudValue::Pill { on: false });
    }
}
