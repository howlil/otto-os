use super::focus_settings::SegmentedOption;
use super::model::{OttoSettingItem, OttoSettingKind, OttoSettingsSnapshot};
use super::settings_content::{
    SettingsAction, SettingsContent, SettingsRow, SettingsRowValue, SettingsSection, setting_row,
};
use crate::appearance::theme::{AccentColor, AppearanceStyle, ThemeMode};
use crate::settings_window::SettingsPage;

fn style_footer(snapshot: &OttoSettingsSnapshot) -> &'static str {
    if snapshot.system_appearance.contrast.is_some() {
        "Windows High Contrast is active, so surfaces use system contrast colors."
    } else if !snapshot.system_appearance.transparency && snapshot.appearance.style.uses_glass() {
        "Windows transparency is off or Battery Saver is active, so glass styles temporarily use Solid."
    } else if snapshot.appearance.style == AppearanceStyle::Solid {
        "Solid disables live backdrop capture for lower battery use and full screenshot visibility."
    } else {
        "Notch stays Obsidian Black. Otto Settings uses clean Windows Mica or Solid."
    }
}

fn theme_footer(snapshot: &OttoSettingsSnapshot) -> &'static str {
    match snapshot.appearance.theme {
        ThemeMode::Auto => {
            if snapshot.system_appearance.dark {
                "Following Windows (Dark)."
            } else {
                "Following Windows (Light)."
            }
        }
        ThemeMode::Light => "Always uses Light surfaces across overlays and Otto Settings.",
        ThemeMode::Dark => "Always uses Dark surfaces across overlays and Otto Settings.",
    }
}

fn capture_footer(snapshot: &OttoSettingsSnapshot) -> &'static str {
    if snapshot.appearance.style == AppearanceStyle::Solid
        || snapshot.system_appearance.contrast.is_some()
        || !snapshot.system_appearance.transparency
    {
        "Surfaces are already visible in captures while Solid or High Contrast is active."
    } else {
        "Keep Otto overlays visible in screenshots and screen recordings when glass is active."
    }
}

#[must_use]
pub(crate) fn build_appearance_page(snapshot: &OttoSettingsSnapshot) -> SettingsContent {
    let resolved = snapshot
        .system_appearance
        .resolve_preferences(snapshot.appearance);
    let accent_rgb = snapshot
        .appearance
        .accent
        .swatch(snapshot.system_appearance.accent)
        .to_rgb_u32();
    let system_accent_rgb = snapshot
        .system_appearance
        .accent
        .map(crate::appearance::theme::Rgba::to_rgb_u32);
    let theme_selected = match snapshot.appearance.theme {
        ThemeMode::Auto => 0,
        ThemeMode::Light => 1,
        ThemeMode::Dark => 2,
    };
    let capture_row = setting_row(
        snapshot,
        OttoSettingItem::Toggle(OttoSettingKind::CaptureVisible),
        SettingsPage::Appearance,
    );
    SettingsContent {
        hero: None,
        sections: vec![
            SettingsSection {
                title: "Style",
                footer: Some(style_footer(snapshot)),
                rows: vec![SettingsRow::new(
                    "Surface Style",
                    "",
                    SettingsRowValue::StylePicker {
                        selected: snapshot.appearance.style,
                        dark: resolved.content_theme().is_dark(),
                        accent_rgb,
                    },
                )],
            },
            SettingsSection {
                title: "Theme",
                footer: Some(theme_footer(snapshot)),
                rows: vec![SettingsRow::new(
                    "Theme Mode",
                    "",
                    SettingsRowValue::Segmented {
                        options: vec![
                            SegmentedOption {
                                label: ThemeMode::Auto.label(),
                                action: SettingsAction::SetAppearanceTheme(ThemeMode::Auto),
                            },
                            SegmentedOption {
                                label: ThemeMode::Light.label(),
                                action: SettingsAction::SetAppearanceTheme(ThemeMode::Light),
                            },
                            SegmentedOption {
                                label: ThemeMode::Dark.label(),
                                action: SettingsAction::SetAppearanceTheme(ThemeMode::Dark),
                            },
                        ],
                        selected: theme_selected,
                    },
                )],
            },
            SettingsSection {
                title: "Accent Color",
                footer: Some("Used for active highlights, controls, and the Tinted glass style."),
                rows: vec![SettingsRow::new(
                    snapshot.appearance.accent.label(),
                    "",
                    SettingsRowValue::AccentSwatches {
                        selected: snapshot.appearance.accent,
                        system_accent_rgb,
                    },
                )],
            },
            SettingsSection {
                title: "Screen Capture",
                footer: Some(capture_footer(snapshot)),
                rows: vec![capture_row],
            },
        ],
    }
}

#[must_use]
pub(crate) const fn style_for_part(part: usize) -> Option<AppearanceStyle> {
    if part < AppearanceStyle::ALL.len() {
        Some(AppearanceStyle::ALL[part])
    } else {
        None
    }
}

#[must_use]
pub(crate) const fn accent_for_part(part: usize) -> Option<AccentColor> {
    if part < AccentColor::ALL.len() {
        Some(AccentColor::ALL[part])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::build_appearance_page;
    use crate::appearance::material::{ContrastColors, SystemAppearance};
    use crate::appearance::theme::{
        AccentColor, AppearancePreferences, AppearanceStyle, ThemeMode,
    };
    use crate::layout::Rect;
    use crate::settings::model::OttoSettingsSnapshot;
    use crate::settings::settings_content::{
        SettingsAction, SettingsRowValue, compute_content_layout,
    };

    #[test]
    fn appearance_page_exposes_style_cards_theme_segments_accent_swatches_and_capture() {
        let snapshot = OttoSettingsSnapshot {
            appearance: AppearancePreferences {
                style: AppearanceStyle::Tinted,
                theme: ThemeMode::Auto,
                accent: AccentColor::Rose,
            },
            system_appearance: SystemAppearance {
                dark: false,
                transparency: true,
                contrast: None,
                accent: None,
            },
            ..OttoSettingsSnapshot::default()
        };
        let content = build_appearance_page(&snapshot);
        assert_eq!(content.sections.len(), 4);
        assert!(matches!(
            content.sections[0].rows[0].value,
            SettingsRowValue::StylePicker {
                selected: AppearanceStyle::Tinted,
                dark: false,
                ..
            }
        ));
        assert_eq!(
            content.part_action(0, 0, 1),
            Some(SettingsAction::SetAppearanceStyle(AppearanceStyle::Frosted))
        );
        assert_eq!(
            content.part_action(0, 0, 3),
            Some(SettingsAction::SetAppearanceStyle(AppearanceStyle::Solid))
        );
        assert_eq!(
            content.part_action(1, 0, 1),
            Some(SettingsAction::SetAppearanceTheme(ThemeMode::Light))
        );
        assert_eq!(
            content.part_action(2, 0, 0),
            Some(SettingsAction::SetAppearanceAccent(AccentColor::System))
        );
        assert_eq!(
            content.part_action(2, 0, 5),
            Some(SettingsAction::SetAppearanceAccent(AccentColor::Emerald))
        );
        assert!(content.sections[1].footer.unwrap_or("").contains("Light"));

        let viewport = Rect {
            x: 0,
            y: 0,
            width: 560,
            height: 640,
        };
        let blocks = compute_content_layout(viewport, &content, 96, 0);
        assert_eq!(blocks.cards[0].row_parts[0].len(), 4);
        assert_eq!(blocks.cards[1].row_parts[0].len(), 3);
        assert_eq!(blocks.cards[2].row_parts[0].len(), 7);
    }

    #[test]
    fn appearance_page_reports_high_contrast_and_transparency_fallbacks() {
        let mut snapshot = OttoSettingsSnapshot::default();
        snapshot.system_appearance.transparency = false;
        let off = build_appearance_page(&snapshot);
        assert!(
            off.sections[0]
                .footer
                .unwrap_or("")
                .contains("temporarily use Solid")
        );

        snapshot.system_appearance.contrast = Some(ContrastColors::BLACK_THEME);
        let hc = build_appearance_page(&snapshot);
        assert!(
            hc.sections[0]
                .footer
                .unwrap_or("")
                .contains("High Contrast")
        );
    }
}
