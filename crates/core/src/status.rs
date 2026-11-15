#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatteryStatus {
    pub percent: Option<u8>,
    pub charging: bool,
}

impl BatteryStatus {
    #[must_use]
    pub fn label(self) -> String {
        match self.percent {
            Some(pct) => format!("{}%", pct.min(100)),
            None => String::from("AC"),
        }
    }

    #[must_use]
    pub fn fill_fraction(self) -> f32 {
        match self.percent {
            Some(pct) => f32::from(pct.min(100)) / 100.0,
            None => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClockTime {
    pub year: u16,
    pub weekday: u8,
    pub day: u8,
    pub month: u8,
    pub hour: u8,
    pub minute: u8,
}

impl ClockTime {
    #[must_use]
    pub const fn minute_of_day(self) -> u16 {
        self.hour as u16 * 60 + self.minute as u16
    }

    #[must_use]
    pub fn format(self) -> String {
        const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let day_name = WEEKDAYS
            .get(usize::from(self.weekday))
            .copied()
            .unwrap_or("Day");
        let month_name = MONTHS
            .get(usize::from(self.month.saturating_sub(1)))
            .copied()
            .unwrap_or("Jan");
        format!(
            "{day_name} {:02} {month_name}  {:02}:{:02}",
            self.day, self.hour, self.minute
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerMode {
    #[default]
    Standard,
    BatterySaver,
}

impl PowerMode {
    #[must_use]
    pub const fn from_battery_saver(enabled: bool) -> Self {
        if enabled {
            Self::BatterySaver
        } else {
            Self::Standard
        }
    }

    #[must_use]
    pub const fn is_battery_saver(self) -> bool {
        matches!(self, Self::BatterySaver)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct InputLangCode {
    bytes: [u8; 4],
    len: u8,
}

impl InputLangCode {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bytes: [0; 4],
            len: 0,
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        let mut bytes = [0u8; 4];
        let mut len = 0usize;
        for byte in tag.bytes() {
            if byte.is_ascii_alphanumeric() {
                bytes[len] = byte.to_ascii_uppercase();
                len += 1;
                if len == bytes.len() {
                    break;
                }
            } else if len > 0 {
                break;
            }
        }
        Self {
            bytes,
            len: len as u8,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        let len = usize::from(self.len).min(self.bytes.len());
        std::str::from_utf8(&self.bytes[..len]).unwrap_or("")
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl std::fmt::Debug for InputLangCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("InputLangCode")
            .field(&self.as_str())
            .finish()
    }
}

impl std::fmt::Display for InputLangCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SystemStatus {
    pub battery: BatteryStatus,
    pub clock: ClockTime,
    pub volume: u8,
    pub muted: bool,
    pub brightness: Option<u8>,
    pub caps_lock: bool,
    pub num_lock: bool,
    pub power_mode: PowerMode,
    pub input_lang: InputLangCode,
    pub dnd_active: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HudDeviceLabel {
    bytes: [u8; 48],
    len: u8,
}

impl Default for HudDeviceLabel {
    fn default() -> Self {
        Self {
            bytes: [0u8; 48],
            len: 0,
        }
    }
}

impl HudDeviceLabel {
    #[must_use]
    pub fn from_name(raw: &str) -> Self {
        let trimmed = raw.trim();
        let mut bytes = [0u8; 48];
        let mut end = 0usize;
        for (idx, ch) in trimmed.char_indices() {
            let next = idx + ch.len_utf8();
            if next > bytes.len() {
                break;
            }
            end = next;
        }
        bytes[..end].copy_from_slice(&trimmed.as_bytes()[..end]);
        Self {
            bytes,
            len: end as u8,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or("")
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl std::fmt::Debug for HudDeviceLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct BluetoothConnectTracker {
    connected_ids: Option<std::collections::BTreeSet<String>>,
}

impl BluetoothConnectTracker {
    pub fn observe(&mut self, state: &crate::BluetoothState) -> Option<NotchHudKind> {
        if !state.supported {
            return None;
        }
        let next_ids: std::collections::BTreeSet<String> = if state.enabled {
            state
                .devices
                .iter()
                .filter(|dev| dev.connected)
                .map(|dev| dev.id.clone())
                .collect()
        } else {
            std::collections::BTreeSet::new()
        };
        let Some(prev_ids) = self.connected_ids.as_mut() else {
            self.connected_ids = Some(next_ids);
            return None;
        };
        let picked = state
            .enabled
            .then(|| {
                let newly_connected = || {
                    state
                        .devices
                        .iter()
                        .filter(|dev| dev.connected && !prev_ids.contains(&dev.id))
                };
                newly_connected()
                    .find(|dev| {
                        matches!(
                            dev.kind,
                            crate::BluetoothDeviceKind::Headphones
                                | crate::BluetoothDeviceKind::Speaker
                        )
                    })
                    .or_else(|| newly_connected().next())
            })
            .flatten()
            .map(|dev| NotchHudKind::BluetoothConnected {
                name: HudDeviceLabel::from_name(&dev.name),
                kind: dev.kind,
                battery_percent: dev.battery_percent.map(|pct| pct.min(100)),
            });
        *prev_ids = next_ids;
        picked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchHudKind {
    Volume {
        percent: u8,
        muted: bool,
    },
    Brightness {
        percent: u8,
    },
    Charging {
        percent: u8,
        plugged_in: bool,
    },
    LowBattery {
        percent: u8,
    },
    CapsLock {
        enabled: bool,
    },
    NumLock {
        enabled: bool,
    },
    BatterySaver {
        enabled: bool,
    },
    Dnd {
        enabled: bool,
    },
    KeyboardLayout {
        code: InputLangCode,
    },
    BluetoothConnected {
        name: HudDeviceLabel,
        kind: crate::BluetoothDeviceKind,
        battery_percent: Option<u8>,
    },
    Welcome,
}

impl NotchHudKind {
    #[must_use]
    pub fn target_level(self) -> f32 {
        match self {
            Self::Volume { percent, muted } => {
                if muted {
                    0.0
                } else {
                    f32::from(percent.min(100)) / 100.0
                }
            }
            Self::Brightness { percent }
            | Self::Charging { percent, .. }
            | Self::LowBattery { percent }
            | Self::BluetoothConnected {
                battery_percent: Some(percent),
                ..
            } => f32::from(percent.min(100)) / 100.0,
            Self::CapsLock { enabled }
            | Self::NumLock { enabled }
            | Self::BatterySaver { enabled }
            | Self::Dnd { enabled } => {
                if enabled {
                    1.0
                } else {
                    0.0
                }
            }
            Self::KeyboardLayout { .. }
            | Self::BluetoothConnected {
                battery_percent: None,
                ..
            }
            | Self::Welcome => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchHudState {
    pub kind: NotchHudKind,
    pub expansion: f32,
    pub content_alpha: f32,
    pub level: f32,
}

#[must_use]
pub fn detect_hud_event(prev: SystemStatus, next: SystemStatus) -> Option<NotchHudKind> {
    if next.muted != prev.muted || next.volume != prev.volume {
        return Some(NotchHudKind::Volume {
            percent: if next.muted { 0 } else { next.volume.min(100) },
            muted: next.muted,
        });
    }
    if next.brightness != prev.brightness
        && let Some(pct) = next.brightness
    {
        return Some(NotchHudKind::Brightness {
            percent: pct.min(100),
        });
    }
    if next.battery.charging != prev.battery.charging {
        return Some(NotchHudKind::Charging {
            percent: next.battery.percent.unwrap_or(100).min(100),
            plugged_in: next.battery.charging,
        });
    }
    if !next.battery.charging
        && let Some(pct) = next.battery.percent
        && pct <= 20
        && prev.battery.percent.unwrap_or(100) > 20
    {
        return Some(NotchHudKind::LowBattery { percent: pct });
    }
    if next.caps_lock != prev.caps_lock {
        return Some(NotchHudKind::CapsLock {
            enabled: next.caps_lock,
        });
    }
    if next.num_lock != prev.num_lock {
        return Some(NotchHudKind::NumLock {
            enabled: next.num_lock,
        });
    }
    if next.power_mode != prev.power_mode {
        return Some(NotchHudKind::BatterySaver {
            enabled: next.power_mode.is_battery_saver(),
        });
    }
    if let (Some(was), Some(is_now)) = (prev.dnd_active, next.dnd_active)
        && was != is_now
    {
        return Some(NotchHudKind::Dnd { enabled: is_now });
    }
    if !prev.input_lang.is_empty()
        && !next.input_lang.is_empty()
        && next.input_lang != prev.input_lang
    {
        return Some(NotchHudKind::KeyboardLayout {
            code: next.input_lang,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_label_handles_percentage_and_desktop_ac() {
        assert_eq!(
            BatteryStatus {
                percent: Some(84),
                charging: false
            }
            .label(),
            "84%"
        );
        assert_eq!(
            BatteryStatus {
                percent: Some(120),
                charging: true
            }
            .label(),
            "100%"
        );
        assert_eq!(
            BatteryStatus {
                percent: None,
                charging: true
            }
            .label(),
            "AC"
        );
    }

    #[test]
    fn clock_formats_macos_style_date_and_time() {
        let clock = ClockTime {
            year: 2026,
            weekday: 3,
            day: 11,
            month: 11,
            hour: 1,
            minute: 5,
        };
        assert_eq!(clock.format(), "Wed 11 Nov  01:05");
    }

    fn sample_status() -> SystemStatus {
        SystemStatus {
            battery: BatteryStatus {
                percent: Some(80),
                charging: false,
            },
            clock: ClockTime::default(),
            volume: 50,
            muted: false,
            brightness: Some(70),
            caps_lock: false,
            num_lock: false,
            power_mode: PowerMode::Standard,
            input_lang: InputLangCode::from_tag("en-US"),
            dnd_active: None,
        }
    }

    #[test]
    fn detect_hud_event_identifies_volume_brightness_power_and_capslock() {
        let base = sample_status();
        assert_eq!(
            detect_hud_event(base, SystemStatus { volume: 65, ..base }),
            Some(NotchHudKind::Volume {
                percent: 65,
                muted: false
            })
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    brightness: Some(85),
                    ..base
                }
            ),
            Some(NotchHudKind::Brightness { percent: 85 })
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    brightness: None,
                    ..base
                }
            ),
            None
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    battery: BatteryStatus {
                        percent: Some(80),
                        charging: true
                    },
                    ..base
                }
            ),
            Some(NotchHudKind::Charging {
                percent: 80,
                plugged_in: true
            })
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    caps_lock: true,
                    ..base
                }
            ),
            Some(NotchHudKind::CapsLock { enabled: true })
        );
    }

    #[test]
    fn detect_hud_event_identifies_locks_power_mode_and_input_lang() {
        let base = sample_status();
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    num_lock: true,
                    ..base
                }
            ),
            Some(NotchHudKind::NumLock { enabled: true })
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    power_mode: PowerMode::BatterySaver,
                    ..base
                }
            ),
            Some(NotchHudKind::BatterySaver { enabled: true })
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    input_lang: InputLangCode::from_tag("id-ID"),
                    ..base
                }
            ),
            Some(NotchHudKind::KeyboardLayout {
                code: InputLangCode::from_tag("ID"),
            })
        );
        assert_eq!(
            detect_hud_event(
                base,
                SystemStatus {
                    input_lang: InputLangCode::empty(),
                    ..base
                }
            ),
            None
        );
        assert_eq!(
            detect_hud_event(
                SystemStatus {
                    input_lang: InputLangCode::empty(),
                    ..base
                },
                base
            ),
            None
        );
    }

    #[test]
    fn input_lang_code_normalizes_bcp47_and_iso_tags() {
        assert_eq!(InputLangCode::from_tag("en-US").as_str(), "EN");
        assert_eq!(InputLangCode::from_tag("id").as_str(), "ID");
        assert_eq!(InputLangCode::from_tag("fil-PH").as_str(), "FIL");
        assert_eq!(InputLangCode::from_tag("abcd-EF").as_str(), "ABCD");
        assert!(InputLangCode::from_tag("   ").is_empty());
    }

    #[test]
    fn hud_device_label_trims_and_truncates_utf8_on_char_boundary() {
        let short = HudDeviceLabel::from_name("  Sony WH-1000XM5  ");
        assert_eq!(short.as_str(), "Sony WH-1000XM5");
        assert!(!short.is_empty());

        let wide = format!("{}é", "a".repeat(47));
        let truncated = HudDeviceLabel::from_name(&wide);
        assert_eq!(truncated.as_str(), "a".repeat(47));
        assert!(HudDeviceLabel::from_name("   ").is_empty());
    }

    #[test]
    fn bluetooth_connect_tracker_ignores_baseline_and_reports_new_connections() {
        let mut tracker = BluetoothConnectTracker::default();
        let initial = crate::BluetoothState {
            supported: true,
            enabled: true,
            busy: false,
            devices: vec![crate::BluetoothDevice {
                id: String::from("AABBCCDDEEFF"),
                name: String::from("BT5.1 Mouse"),
                kind: crate::BluetoothDeviceKind::Mouse,
                connected: true,
                battery_percent: Some(75),
            }],
        };
        assert_eq!(tracker.observe(&initial), None);
        assert_eq!(tracker.observe(&initial), None);

        let with_headset = crate::BluetoothState {
            supported: true,
            enabled: true,
            busy: false,
            devices: vec![
                initial.devices[0].clone(),
                crate::BluetoothDevice {
                    id: String::from("112233445566"),
                    name: String::from("AirPods Pro"),
                    kind: crate::BluetoothDeviceKind::Headphones,
                    connected: true,
                    battery_percent: Some(85),
                },
            ],
        };
        assert_eq!(
            tracker.observe(&with_headset),
            Some(NotchHudKind::BluetoothConnected {
                name: HudDeviceLabel::from_name("AirPods Pro"),
                kind: crate::BluetoothDeviceKind::Headphones,
                battery_percent: Some(85),
            })
        );
        assert_eq!(tracker.observe(&with_headset), None);

        assert_eq!(tracker.observe(&initial), None);
        assert_eq!(
            tracker.observe(&with_headset),
            Some(NotchHudKind::BluetoothConnected {
                name: HudDeviceLabel::from_name("AirPods Pro"),
                kind: crate::BluetoothDeviceKind::Headphones,
                battery_percent: Some(85),
            })
        );
    }

    #[test]
    fn bluetooth_connect_tracker_prefers_audio_device_when_multiple_connect_together() {
        let mut tracker = BluetoothConnectTracker::default();
        let empty = crate::BluetoothState {
            supported: true,
            enabled: true,
            busy: false,
            devices: Vec::new(),
        };
        assert_eq!(tracker.observe(&empty), None);

        let both = crate::BluetoothState {
            supported: true,
            enabled: true,
            busy: false,
            devices: vec![
                crate::BluetoothDevice {
                    id: String::from("000000000001"),
                    name: String::from("Keychron K3"),
                    kind: crate::BluetoothDeviceKind::Keyboard,
                    connected: true,
                    battery_percent: Some(90),
                },
                crate::BluetoothDevice {
                    id: String::from("000000000002"),
                    name: String::from("WH-1000XM5"),
                    kind: crate::BluetoothDeviceKind::Headphones,
                    connected: true,
                    battery_percent: None,
                },
            ],
        };
        assert_eq!(
            tracker.observe(&both),
            Some(NotchHudKind::BluetoothConnected {
                name: HudDeviceLabel::from_name("WH-1000XM5"),
                kind: crate::BluetoothDeviceKind::Headphones,
                battery_percent: None,
            })
        );
    }

    #[test]
    fn dnd_transition_emits_hud_only_when_both_states_are_known() {
        let base = sample_status();
        let became_known = SystemStatus {
            dnd_active: Some(true),
            ..base
        };
        assert_eq!(detect_hud_event(base, became_known), None);

        let turned_off = SystemStatus {
            dnd_active: Some(false),
            ..base
        };
        assert_eq!(
            detect_hud_event(became_known, turned_off),
            Some(NotchHudKind::Dnd { enabled: false })
        );
        assert_eq!(
            detect_hud_event(turned_off, became_known),
            Some(NotchHudKind::Dnd { enabled: true })
        );
    }
}
