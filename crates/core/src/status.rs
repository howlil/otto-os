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
pub struct SystemStatus {
    pub battery: BatteryStatus,
    pub clock: ClockTime,
    pub volume: u8,
    pub muted: bool,
    pub brightness: u8,
    pub caps_lock: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchHudKind {
    Volume { percent: u8, muted: bool },
    Brightness { percent: u8 },
    Charging { percent: u8, plugged_in: bool },
    LowBattery { percent: u8 },
    CapsLock { enabled: bool },
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
            | Self::LowBattery { percent } => f32::from(percent.min(100)) / 100.0,
            Self::CapsLock { enabled } => {
                if enabled {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchHudState {
    pub kind: NotchHudKind,
    pub expansion: f32,
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
    if next.brightness != prev.brightness {
        return Some(NotchHudKind::Brightness {
            percent: next.brightness.min(100),
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

    #[test]
    fn detect_hud_event_identifies_volume_brightness_power_and_capslock() {
        let base = SystemStatus {
            battery: BatteryStatus {
                percent: Some(80),
                charging: false,
            },
            clock: ClockTime::default(),
            volume: 50,
            muted: false,
            brightness: 70,
            caps_lock: false,
        };
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
                    brightness: 85,
                    ..base
                }
            ),
            Some(NotchHudKind::Brightness { percent: 85 })
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
}
