use crate::notch::focus_activity::{FocusActivityState, PRESET_BLOCKED_WEBSITES, normalize_domain};
use crate::notch::focus_mode::{
    CategoryColor, FocusCategory, FocusRulePolicy, MAX_FOCUS_CATEGORIES, sanitize_category_name,
};
use crate::notch::focus_suggester::suggest_for_category;
use crate::runtime::process_hog::format_memory_bytes;
use crate::settings::focus_settings::{
    FOCUS_SEARCH_MAX_RESULTS, FilterChip, FocusAppFilter, FocusDisclosure, FocusPageState,
    FocusSettingsTab, FocusTextField, ResolvedApp, SegmentedOption, app_usage_subtitle,
    build_during_focus_section, matches_app_query, merge_focus_apps, sort_alphabetically,
    sort_by_usage_then_name,
};
use crate::settings::model::OttoSettingsSnapshot;
use crate::settings_content::{
    BadgeTone, SettingsAction, SettingsRow, SettingsRowValue, SettingsSection,
};

#[must_use]
pub(crate) fn focus_tabs_row(state: &FocusPageState) -> SettingsRow {
    SettingsRow::new(
        "",
        "",
        SettingsRowValue::Segmented {
            options: vec![
                SegmentedOption {
                    label: "Focus Modes",
                    action: SettingsAction::SetFocusTab(FocusSettingsTab::Modes),
                },
                SegmentedOption {
                    label: "Insights",
                    action: SettingsAction::SetFocusTab(FocusSettingsTab::Insights),
                },
            ],
            selected: match state.tab {
                FocusSettingsTab::Modes => 0,
                FocusSettingsTab::Insights => 1,
            },
        },
    )
}

fn build_mode_switcher_section(
    snapshot: &OttoSettingsSnapshot,
    state: &FocusPageState,
) -> SettingsSection {
    let activity = &snapshot.focus_activity;
    let active = activity.categories.active();
    let mut chips: Vec<FilterChip> = activity
        .categories
        .categories
        .iter()
        .map(|cat| FilterChip {
            label: cat.name.clone(),
            action: SettingsAction::SelectFocusCategory(cat.id.clone()),
        })
        .collect();
    let selected_idx = activity
        .categories
        .categories
        .iter()
        .position(|cat| cat.id == active.id)
        .unwrap_or(0);
    if activity.categories.categories.len() < MAX_FOCUS_CATEGORIES {
        chips.push(FilterChip {
            label: if state.is_open(FocusDisclosure::CreateMode) {
                String::from("Cancel + New")
            } else {
                String::from("+ New")
            },
            action: SettingsAction::ToggleFocusDisclosure(FocusDisclosure::CreateMode),
        });
    }

    let mut rows = vec![SettingsRow::new(
        "",
        "",
        SettingsRowValue::FilterChips {
            chips,
            selected: selected_idx,
        },
    )];

    if state.is_open(FocusDisclosure::CreateMode)
        && activity.categories.categories.len() < MAX_FOCUS_CATEGORIES
    {
        rows.push(SettingsRow::new(
            "",
            "",
            SettingsRowValue::TextInput {
                field: FocusTextField::CategoryName,
                text: state.category_name_input.clone(),
                placeholder: "New mode name (e.g. Skripsi, Desain, Reading)...",
                button_label: Some("Create"),
                focused: state.focused_field == FocusTextField::CategoryName,
                error: state.category_error.clone(),
            },
        ));
        let typed = sanitize_category_name(&state.category_name_input);
        if !typed.is_empty() {
            let preview = suggest_for_category(
                &typed,
                &state.catalog,
                &activity.today.apps,
                &snapshot.process_audit.audit.groups,
            );
            rows.push(SettingsRow::new(
                format!("Detected: {}", preview.archetype.label()),
                format!(
                    "Will suggest {} tools to allow & {} distractions",
                    preview.allow_apps.len(),
                    preview.block_apps.len()
                ),
                SettingsRowValue::Text(String::from("Ready")),
            ));
        }
    }

    SettingsSection {
        title: "Focus modes",
        footer: Some("Each Focus session belongs to a mode. Switch modes here or in the Notch."),
        rows,
    }
}

fn build_mode_hero_and_manage_section(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> SettingsSection {
    let active = activity.categories.active();
    let mut hero = SettingsRow::new(
        format!("When {} starts", active.name),
        active.hero_summary(),
        SettingsRowValue::Text(String::from(active.color.label())),
    );
    hero.badge = Some((active.policy.label(), BadgeTone::Good));

    let rename_open = state.is_open(FocusDisclosure::RenameMode);
    let mut rows = vec![
        hero,
        SettingsRow::new(
            format!("Rename \"{}\"", active.name),
            "Keep all heatmap history while updating the mode name",
            SettingsRowValue::Disclosure {
                expanded: rename_open,
            },
        )
        .with_action(SettingsAction::ToggleFocusDisclosure(
            FocusDisclosure::RenameMode,
        )),
    ];
    if rename_open {
        rows.push(SettingsRow::new(
            "",
            "",
            SettingsRowValue::TextInput {
                field: FocusTextField::CategoryRename,
                text: state.category_rename_input.clone(),
                placeholder: "Enter new mode name...",
                button_label: Some("Save"),
                focused: state.focused_field == FocusTextField::CategoryRename,
                error: state.category_error.clone(),
            },
        ));
    }
    if activity.categories.categories.len() > 1 {
        let mut del = SettingsRow::new(
            format!("Delete \"{}\"", active.name),
            "Removes this mode (past heatmap history stays intact)",
            SettingsRowValue::Detail("Delete"),
        )
        .with_action(SettingsAction::DeleteFocusCategory(active.id.clone()));
        del.badge = Some(("Mode", BadgeTone::Warning));
        rows.push(del);
    }

    SettingsSection {
        title: "Mode overview",
        footer: None,
        rows,
    }
}

fn build_timer_and_appearance_section(active: &FocusCategory) -> SettingsSection {
    let mut duration_chips = vec![FilterChip {
        label: String::from("-5m"),
        action: SettingsAction::AdjustFocusCategoryDuration(-5),
    }];
    let mut selected_dur = 1usize;
    let presets = [25u32, 45, 60, 90];
    let mut chip_values = vec![active.focus_minutes];
    for p in presets {
        if p != active.focus_minutes {
            chip_values.push(p);
        }
    }
    for (idx, mins) in chip_values.iter().enumerate() {
        if *mins == active.focus_minutes {
            selected_dur = idx + 1;
        }
        duration_chips.push(FilterChip {
            label: if idx == 0 {
                format!("{mins} min")
            } else {
                format!("{mins}m")
            },
            action: SettingsAction::AdjustFocusCategoryDuration(
                *mins as i32 - active.focus_minutes as i32,
            ),
        });
    }
    duration_chips.push(FilterChip {
        label: String::from("+5m"),
        action: SettingsAction::AdjustFocusCategoryDuration(5),
    });

    let color_chips: Vec<FilterChip> = CategoryColor::ALL
        .iter()
        .map(|col| FilterChip {
            label: String::from(col.label()),
            action: SettingsAction::SetFocusCategoryColor(*col),
        })
        .collect();
    let color_idx = CategoryColor::ALL
        .iter()
        .position(|col| *col == active.color)
        .unwrap_or(0);

    SettingsSection {
        title: "Timer & appearance",
        footer: Some("Synced live with the Notch timer, ring accent, and Insights heatmap."),
        rows: vec![
            SettingsRow::new(
                "Focus length",
                "Step ±5m or pick a preset; click digits in Notch for exact 1-180m",
                SettingsRowValue::FilterChips {
                    chips: duration_chips,
                    selected: selected_dur,
                },
            ),
            SettingsRow::new(
                "Break rhythm",
                "Click to cycle short / long break cadence",
                SettingsRowValue::Text(format!(
                    "{}m / {}m ({}x)",
                    active.short_break_minutes,
                    active.long_break_minutes,
                    active.cycles_before_long_break
                )),
            )
            .with_action(SettingsAction::CycleFocusCategoryBreaks),
            SettingsRow::new(
                "Mode color",
                "Pick the accent color for this mode",
                SettingsRowValue::FilterChips {
                    chips: color_chips,
                    selected: color_idx,
                },
            ),
        ],
    }
}

fn mode_app_row(app: &ResolvedApp, active: &FocusCategory) -> SettingsRow {
    let selected = match active.policy {
        FocusRulePolicy::Allowlist => active.is_app_allowed(&app.block_key),
        FocusRulePolicy::Blocklist => active.is_app_blocked_explicit(&app.block_key),
    };
    let subtitle = if selected {
        match active.policy {
            FocusRulePolicy::Allowlist => String::from("Allowed during Focus"),
            FocusRulePolicy::Blocklist => String::from("Blocked during Focus"),
        }
    } else {
        app_usage_subtitle(app)
    };
    let mut row = SettingsRow::new(
        app.display_name.clone(),
        subtitle,
        SettingsRowValue::Toggle(selected),
    )
    .with_action(SettingsAction::ToggleCategoryApp(app.block_key.clone()));
    row.app_icon = Some(
        app.icon_source
            .clone()
            .unwrap_or_else(|| app.block_key.clone()),
    );
    row.detail_hint.clone_from(&app.detail);
    row
}

fn build_smart_setup_rows(
    snapshot: &OttoSettingsSnapshot,
    state: &FocusPageState,
) -> Vec<SettingsRow> {
    let activity = &snapshot.focus_activity;
    let active = activity.categories.active();
    let suggestion = suggest_for_category(
        &active.name,
        &state.catalog,
        &activity.today.apps,
        &snapshot.process_audit.audit.groups,
    );
    let preview_open = state.is_open(FocusDisclosure::SmartPreview);
    let mut rows = vec![
        SettingsRow::new(
            format!("Smart Auto-Setup for {}", active.name),
            format!(
                "{} · {} tools · {} distractions · {} heavy background",
                suggestion.archetype.label(),
                suggestion.allow_apps.len(),
                suggestion.block_apps.len(),
                suggestion.background_apps.len()
            ),
            SettingsRowValue::Detail("Apply"),
        )
        .with_action(SettingsAction::ApplyFocusCategorySuggestions),
        SettingsRow::new(
            "Preview smart recommendations",
            "Review or toggle individual suggested apps",
            SettingsRowValue::Disclosure {
                expanded: preview_open,
            },
        )
        .with_action(SettingsAction::ToggleFocusDisclosure(
            FocusDisclosure::SmartPreview,
        )),
    ];
    if preview_open {
        let items = match active.policy {
            FocusRulePolicy::Allowlist => &suggestion.allow_apps,
            FocusRulePolicy::Blocklist => &suggestion.block_apps,
        };
        for item in items.iter().take(6) {
            let is_on = match active.policy {
                FocusRulePolicy::Allowlist => active.is_app_allowed(&item.app_key),
                FocusRulePolicy::Blocklist => active.is_app_blocked_explicit(&item.app_key),
            };
            let mut row = SettingsRow::new(
                item.display_name.clone(),
                format!("Suggested · {}", item.reason),
                SettingsRowValue::Toggle(is_on),
            )
            .with_action(SettingsAction::ToggleCategoryApp(item.app_key.clone()));
            row.badge = Some((
                if active.policy == FocusRulePolicy::Allowlist {
                    "Allow"
                } else {
                    "Block"
                },
                BadgeTone::Good,
            ));
            rows.push(row);
        }
    }
    rows
}

fn build_mode_app_picker_rows(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsRow> {
    let active = activity.categories.active();
    let all = merge_focus_apps(activity, &state.catalog);
    let query_lower = state.app_search.trim().to_lowercase();
    let is_selected = |app: &ResolvedApp| match active.policy {
        FocusRulePolicy::Allowlist => active.is_app_allowed(&app.block_key),
        FocusRulePolicy::Blocklist => active.is_app_blocked_explicit(&app.block_key),
    };
    let selected_count = all.iter().filter(|app| is_selected(app)).count();
    let recent_count = all.iter().filter(|app| app.in_recent).count();

    let mut rows = vec![
        SettingsRow::new(
            "",
            "",
            SettingsRowValue::TextInput {
                field: FocusTextField::AppSearch,
                text: state.app_search.clone(),
                placeholder: "Search apps for this mode...",
                button_label: None,
                focused: state.focused_field == FocusTextField::AppSearch,
                error: None,
            },
        ),
        SettingsRow::new(
            "",
            "",
            SettingsRowValue::FilterChips {
                chips: vec![
                    FilterChip {
                        label: format!("Selected ({selected_count})"),
                        action: SettingsAction::SetFocusAppFilter(FocusAppFilter::Blocked),
                    },
                    FilterChip {
                        label: format!("Recent ({recent_count})"),
                        action: SettingsAction::SetFocusAppFilter(FocusAppFilter::Recent),
                    },
                    FilterChip {
                        label: format!("All installed ({})", all.len()),
                        action: SettingsAction::SetFocusAppFilter(FocusAppFilter::AllApps),
                    },
                ],
                selected: match state.app_filter {
                    FocusAppFilter::Blocked => 0,
                    FocusAppFilter::Recent => 1,
                    FocusAppFilter::AllApps => 2,
                },
            },
        ),
    ];

    let mut filtered: Vec<&ResolvedApp> = all
        .iter()
        .filter(|app| match state.app_filter {
            FocusAppFilter::Blocked => is_selected(app),
            FocusAppFilter::Recent => app.in_recent,
            FocusAppFilter::AllApps => app.in_catalog || app.in_recent || is_selected(app),
        })
        .filter(|app| matches_app_query(app, &query_lower))
        .collect();
    if state.app_filter == FocusAppFilter::AllApps && query_lower.is_empty() {
        sort_alphabetically(&mut filtered);
    } else {
        sort_by_usage_then_name(&mut filtered);
    }
    let limit = match state.app_filter {
        FocusAppFilter::AllApps if query_lower.is_empty() => state.all_apps_limit,
        FocusAppFilter::AllApps => FOCUS_SEARCH_MAX_RESULTS,
        FocusAppFilter::Blocked | FocusAppFilter::Recent => filtered.len(),
    };
    if filtered.is_empty() {
        rows.push(
            SettingsRow::new(
                "No apps in this view",
                "Browse All installed apps or click Apply on Smart Auto-Setup",
                SettingsRowValue::Detail("All apps"),
            )
            .with_action(SettingsAction::SetFocusAppFilter(FocusAppFilter::AllApps)),
        );
    } else {
        for app in filtered.iter().take(limit) {
            rows.push(mode_app_row(app, active));
        }
        if filtered.len() > limit && state.app_filter == FocusAppFilter::AllApps {
            rows.push(
                SettingsRow::new(
                    format!("Show more apps ({limit} of {})", filtered.len()),
                    "Or type in the search box above to filter",
                    SettingsRowValue::Detail("More"),
                )
                .with_action(SettingsAction::ExpandFocusAllApps),
            );
        }
    }
    rows.push(
        SettingsRow::new(
            "+ Choose an app manually",
            "Pick an .exe for portable apps not listed above",
            SettingsRowValue::Detail(""),
        )
        .with_action(SettingsAction::PickBlockedAppFile),
    );
    rows
}

fn build_mode_apps_section(
    snapshot: &OttoSettingsSnapshot,
    state: &FocusPageState,
) -> SettingsSection {
    let active = snapshot.focus_activity.categories.active();
    let mut rows = vec![SettingsRow::new(
        "",
        "",
        SettingsRowValue::Segmented {
            options: vec![
                SegmentedOption {
                    label: "Only allow chosen apps",
                    action: SettingsAction::SetFocusCategoryPolicy(FocusRulePolicy::Allowlist),
                },
                SegmentedOption {
                    label: "Block distractions only",
                    action: SettingsAction::SetFocusCategoryPolicy(FocusRulePolicy::Blocklist),
                },
            ],
            selected: usize::from(active.policy == FocusRulePolicy::Blocklist),
        },
    )];
    rows.extend(build_smart_setup_rows(snapshot, state));
    rows.extend(build_mode_app_picker_rows(&snapshot.focus_activity, state));

    SettingsSection {
        title: "Apps for this mode",
        footer: Some("Everything here belongs to the selected mode only."),
        rows,
    }
}

fn build_bg_power_rows(
    snapshot: &OttoSettingsSnapshot,
    state: &FocusPageState,
) -> Vec<SettingsRow> {
    let activity = &snapshot.focus_activity;
    let active = activity.categories.active();
    let bg_open = state.is_open(FocusDisclosure::BackgroundPower);
    let mut rows = vec![
        SettingsRow::new(
            "Background Power & Auto-Close",
            format!(
                "{} · Auto-restores when Focus pauses or ends",
                active.background_policy.label()
            ),
            SettingsRowValue::Disclosure { expanded: bg_open },
        )
        .with_action(SettingsAction::ToggleFocusDisclosure(
            FocusDisclosure::BackgroundPower,
        )),
    ];
    if bg_open {
        rows.push(
            SettingsRow::new(
                "Background handling",
                "Windows system & essential utilities are always protected",
                SettingsRowValue::Choice(active.background_policy.label()),
            )
            .with_action(SettingsAction::CycleFocusCategoryBackgroundPolicy),
        );
        let suggestion = suggest_for_category(
            &active.name,
            &state.catalog,
            &activity.today.apps,
            &snapshot.process_audit.audit.groups,
        );
        for item in suggestion.background_apps.iter().take(5) {
            let mem = item
                .running_memory_bytes
                .map_or_else(String::new, |b| format!(" · {}", format_memory_bytes(b)));
            let mut r = SettingsRow::new(
                item.display_name.clone(),
                format!("Close gracefully (WM_CLOSE) on start{mem}"),
                SettingsRowValue::Toggle(active.is_app_close_on_focus(&item.app_key)),
            )
            .with_action(SettingsAction::ToggleCategoryCloseApp(item.app_key.clone()));
            r.badge = Some(("Close", BadgeTone::Warning));
            rows.push(r);
        }
    }
    rows
}

fn build_mode_websites_rows(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsRow> {
    let active = activity.categories.active();
    let web_open = state.is_open(FocusDisclosure::BlockedWebsites);
    let mut rows = vec![
        SettingsRow::new(
            "Blocked Websites",
            format!(
                "{} distracting domains blocked",
                active.blocked_websites.len()
            ),
            SettingsRowValue::Disclosure { expanded: web_open },
        )
        .with_action(SettingsAction::ToggleFocusDisclosure(
            FocusDisclosure::BlockedWebsites,
        )),
    ];
    if web_open {
        rows.push(SettingsRow::new(
            "",
            "",
            SettingsRowValue::TextInput {
                field: FocusTextField::WebsiteDomain,
                text: state.website_input.clone(),
                placeholder: "Add domain (e.g. youtube.com)...",
                button_label: Some("Add"),
                focused: state.focused_field == FocusTextField::WebsiteDomain,
                error: state.website_error.clone(),
            },
        ));
        let filter = state.website_input.trim().to_lowercase();
        let norm_filter = normalize_domain(&filter);
        for domain in &active.blocked_websites {
            let matches = filter.is_empty()
                || domain.contains(&filter)
                || norm_filter.as_deref().is_some_and(|n| domain.contains(n));
            if matches {
                rows.push(
                    SettingsRow::new(
                        domain.clone(),
                        "Blocked during this Focus mode",
                        SettingsRowValue::Toggle(true),
                    )
                    .with_action(SettingsAction::ToggleCategoryWebsite(domain.clone())),
                );
            }
        }
        for (preset, label) in PRESET_BLOCKED_WEBSITES.iter().take(6) {
            if !active.should_block_website(preset) {
                rows.push(
                    SettingsRow::new(
                        *preset,
                        format!("{label} · Suggested"),
                        SettingsRowValue::Toggle(false),
                    )
                    .with_action(SettingsAction::ToggleCategoryWebsite(String::from(*preset))),
                );
            }
        }
    }
    rows
}

fn build_notifications_rows(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsRow> {
    let notif_open = state.is_open(FocusDisclosure::Notifications);
    let mut rows = vec![
        SettingsRow::new(
            "Notifications & Calls",
            format!(
                "{} · {}",
                if activity.blocklist.block_notifications {
                    "Mute banners"
                } else {
                    "Keep banners"
                },
                if activity.blocklist.allow_calls_during_focus {
                    "Allow incoming calls"
                } else {
                    "Silence calls"
                }
            ),
            SettingsRowValue::Disclosure {
                expanded: notif_open,
            },
        )
        .with_action(SettingsAction::ToggleFocusDisclosure(
            FocusDisclosure::Notifications,
        )),
    ];
    if notif_open {
        rows.extend(build_during_focus_section(activity).rows);
    }
    rows
}

fn build_advanced_protection_section(
    snapshot: &OttoSettingsSnapshot,
    state: &FocusPageState,
) -> SettingsSection {
    let mut rows = build_bg_power_rows(snapshot, state);
    rows.extend(build_mode_websites_rows(&snapshot.focus_activity, state));
    rows.extend(build_notifications_rows(&snapshot.focus_activity, state));
    SettingsSection {
        title: "Advanced protection",
        footer: Some("Expand any row above for background power, websites, or notification rules."),
        rows,
    }
}

#[must_use]
pub(crate) fn build_focus_modes_sections(
    snapshot: &OttoSettingsSnapshot,
    state: &FocusPageState,
) -> Vec<SettingsSection> {
    let activity = &snapshot.focus_activity;
    let active = activity.categories.active();
    vec![
        build_mode_switcher_section(snapshot, state),
        build_mode_hero_and_manage_section(activity, state),
        build_timer_and_appearance_section(active),
        build_mode_apps_section(snapshot, state),
        build_advanced_protection_section(snapshot, state),
    ]
}
