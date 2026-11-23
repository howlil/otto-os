use super::focus_settings::{FilterChip, KpiItem};
use super::model::{OttoSettingItem, OttoSettingKind, OttoSettingsSnapshot};
use super::settings_content::{
    BadgeTone, SettingsAction, SettingsContent, SettingsRow, SettingsRowValue, SettingsSection,
    module_badge, setting_row,
};
use crate::runtime::process_hog::{
    AuditedAppGroup, ProcessAuditFilter, ProcessAuditReport, ProcessSafety, format_cpu_tenths,
    format_memory_bytes,
};
use crate::settings_window::{ModuleId, ModuleRow, SettingsPage};

pub const PROCESS_AUDIT_INITIAL_LIMIT: usize = 10;
pub const PROCESS_AUDIT_PAGE_STEP: usize = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralPageState {
    pub filter: ProcessAuditFilter,
    pub visible_limit: usize,
    pub audit: ProcessAuditReport,
    pub status_note: Option<String>,
}

impl Default for GeneralPageState {
    fn default() -> Self {
        Self {
            filter: ProcessAuditFilter::Background,
            visible_limit: PROCESS_AUDIT_INITIAL_LIMIT,
            audit: ProcessAuditReport::default(),
            status_note: None,
        }
    }
}

impl GeneralPageState {
    pub fn select_filter(&mut self, filter: ProcessAuditFilter) {
        if self.filter != filter {
            self.filter = filter;
            self.visible_limit = PROCESS_AUDIT_INITIAL_LIMIT;
        }
    }

    pub fn expand_list(&mut self) {
        self.visible_limit = self.visible_limit.saturating_add(PROCESS_AUDIT_PAGE_STEP);
    }
}

fn matches_filter(group: &AuditedAppGroup, filter: ProcessAuditFilter) -> bool {
    match filter {
        ProcessAuditFilter::Background => group.safety == ProcessSafety::SafeBackground,
        ProcessAuditFilter::Throttled => group.throttle_kind.is_throttled(),
        ProcessAuditFilter::Protected => group.safety == ProcessSafety::SystemProtected,
        ProcessAuditFilter::All => true,
    }
}

fn build_cleanup_section(
    snapshot: &OttoSettingsSnapshot,
    modules: &[ModuleRow],
    audit: &ProcessAuditReport,
) -> SettingsSection {
    let mut feature_row = setting_row(
        snapshot,
        OttoSettingItem::Toggle(OttoSettingKind::BackgroundCleanup),
        SettingsPage::General,
    );
    let module_row = modules
        .iter()
        .find(|row| row.id == ModuleId::BackgroundCleanup)
        .copied();
    let cleanup_on = snapshot.is_enabled(OttoSettingKind::BackgroundCleanup);
    feature_row.label = if cleanup_on {
        String::from("Background Cleanup: On")
    } else {
        String::from("Background Cleanup: Off")
    };
    feature_row.subtitle = if cleanup_on {
        String::from("Auto-throttling background CPU hogs with EcoQoS · Never closes apps")
    } else {
        String::from("Turn on to auto-throttle sustained background CPU hogs")
    };
    feature_row.badge = Some(if cleanup_on {
        ("Active", BadgeTone::Good)
    } else {
        module_row.map_or(("Off", BadgeTone::Neutral), module_badge)
    });

    let background_detail = if audit.safe_to_end_count > 0 {
        format!("{} safe to end (>=150 MB)", audit.safe_to_end_count)
    } else if audit.memory_buffer_count > 0 {
        format!("{} hidden buffers (>=100 MB)", audit.memory_buffer_count)
    } else {
        format!("{} background apps", audit.background_group_count)
    };
    let throttled_total = audit.auto_throttled_count + audit.manual_throttled_count;
    let kpi_row = SettingsRow::new(
        "",
        "",
        SettingsRowValue::KpiStrip([
            KpiItem {
                label: "Background RAM",
                value: format_memory_bytes(audit.background_memory_bytes),
                detail: background_detail,
            },
            KpiItem {
                label: "EcoQoS Throttled",
                value: throttled_total.to_string(),
                detail: format!(
                    "{} auto · {} manual",
                    audit.auto_throttled_count, audit.manual_throttled_count
                ),
            },
            KpiItem {
                label: "System Protected",
                value: audit.protected_group_count.to_string(),
                detail: String::from("OS & services locked"),
            },
        ]),
    );

    SettingsSection {
        title: "Background Cleanup",
        footer: Some(
            "EcoQoS lowers power & CPU priority without terminating apps. Protected OS services are locked.",
        ),
        rows: vec![feature_row, kpi_row],
    }
}

fn build_audit_row(group: &AuditedAppGroup) -> SettingsRow {
    let label = if group.process_count > 1 {
        format!("{} ({})", group.display_name, group.process_count)
    } else {
        group.display_name.clone()
    };
    let recommended_end = group.is_safe_to_end_recommended();
    let subtitle = if recommended_end {
        format!(
            "{} · {} · Safe to End · {}",
            format_memory_bytes(group.memory_bytes),
            format_cpu_tenths(group.cpu_tenths_percent),
            group.diagnosis_label()
        )
    } else {
        format!(
            "{} · {} · {}",
            format_memory_bytes(group.memory_bytes),
            format_cpu_tenths(group.cpu_tenths_percent),
            group.diagnosis_label()
        )
    };
    let value = if group.safety == ProcessSafety::SystemProtected {
        SettingsRowValue::ProcessAudit {
            status_label: "Protected",
            status_tone: BadgeTone::Warning,
            throttle_label: None,
            throttle_action: None,
            throttled: false,
            terminate_label: None,
            terminate_action: None,
            recommended_end: false,
        }
    } else {
        let throttled = group.throttle_kind.is_throttled();
        let status_tone = if recommended_end || group.is_cpu_hog() || group.is_memory_buffer() {
            BadgeTone::Warning
        } else if throttled {
            BadgeTone::Good
        } else {
            BadgeTone::Neutral
        };
        SettingsRowValue::ProcessAudit {
            status_label: group.diagnosis_label(),
            status_tone,
            throttle_label: group.can_throttle.then_some(if throttled {
                "Restore"
            } else {
                "EcoQoS"
            }),
            throttle_action: group
                .can_throttle
                .then(|| SettingsAction::ToggleProcessThrottle(group.exe_name.clone())),
            throttled,
            terminate_label: group.can_terminate.then_some("End Task"),
            terminate_action: group
                .can_terminate
                .then(|| SettingsAction::TerminateProcessGroup(group.exe_name.clone())),
            recommended_end,
        }
    };
    let mut row = SettingsRow::new(label, subtitle, value);
    row.app_icon = Some(
        group
            .exe_path
            .clone()
            .unwrap_or_else(|| group.exe_name.clone()),
    );
    row.detail_hint.clone_from(&group.exe_path);
    row
}

fn empty_audit_row(filter: ProcessAuditFilter) -> SettingsRow {
    match filter {
        ProcessAuditFilter::Background => SettingsRow::new(
            "No background apps detected",
            "Click Rescan to sample running processes",
            SettingsRowValue::Detail("Rescan"),
        )
        .with_action(SettingsAction::RefreshProcessAudit),
        ProcessAuditFilter::Throttled => SettingsRow::new(
            "No throttled apps yet",
            "Turn on Background Cleanup or click EcoQoS on any background app",
            SettingsRowValue::Detail("Background"),
        )
        .with_action(SettingsAction::SetProcessAuditFilter(
            ProcessAuditFilter::Background,
        )),
        ProcessAuditFilter::Protected => SettingsRow::new(
            "No protected processes scanned yet",
            "Click Rescan to inspect Windows services and shell processes",
            SettingsRowValue::Detail("Rescan"),
        )
        .with_action(SettingsAction::RefreshProcessAudit),
        ProcessAuditFilter::All => SettingsRow::new(
            "No processes scanned yet",
            "Click Rescan above to inspect running apps and services",
            SettingsRowValue::Detail("Rescan"),
        )
        .with_action(SettingsAction::RefreshProcessAudit),
    }
}

fn build_audit_section(state: &GeneralPageState) -> SettingsSection {
    let audit = &state.audit;
    let throttled_count = audit
        .groups
        .iter()
        .filter(|group| group.throttle_kind.is_throttled())
        .count();
    let chips_row = SettingsRow::new(
        "",
        "",
        SettingsRowValue::FilterChips {
            chips: vec![
                FilterChip {
                    label: format!("Background ({})", audit.background_group_count),
                    action: SettingsAction::SetProcessAuditFilter(ProcessAuditFilter::Background),
                },
                FilterChip {
                    label: format!("Throttled ({throttled_count})"),
                    action: SettingsAction::SetProcessAuditFilter(ProcessAuditFilter::Throttled),
                },
                FilterChip {
                    label: format!("System Protected ({})", audit.protected_group_count),
                    action: SettingsAction::SetProcessAuditFilter(ProcessAuditFilter::Protected),
                },
                FilterChip {
                    label: format!("All ({})", audit.groups.len()),
                    action: SettingsAction::SetProcessAuditFilter(ProcessAuditFilter::All),
                },
            ],
            selected: match state.filter {
                ProcessAuditFilter::Background => 0,
                ProcessAuditFilter::Throttled => 1,
                ProcessAuditFilter::Protected => 2,
                ProcessAuditFilter::All => 3,
            },
        },
    );

    let scan_subtitle = state.status_note.clone().unwrap_or_else(|| {
        format!(
            "Audited {} processes across {} apps · Click to rescan",
            audit.total_processes,
            audit.groups.len()
        )
    });
    let scan_row = SettingsRow::new(
        "Scan running apps & services",
        scan_subtitle,
        SettingsRowValue::Detail("Rescan"),
    )
    .with_action(SettingsAction::RefreshProcessAudit);

    let mut rows = vec![chips_row, scan_row];
    let matching: Vec<&AuditedAppGroup> = audit
        .groups
        .iter()
        .filter(|group| matches_filter(group, state.filter))
        .collect();
    let total_matching = matching.len();
    if matching.is_empty() {
        rows.push(empty_audit_row(state.filter));
    } else {
        for group in matching.iter().take(state.visible_limit) {
            rows.push(build_audit_row(group));
        }
        if total_matching > state.visible_limit {
            let shown = state.visible_limit.min(total_matching);
            rows.push(
                SettingsRow::new(
                    format!("Show more processes ({shown} of {total_matching})"),
                    "Expand to audit remaining entries",
                    SettingsRowValue::Detail("More"),
                )
                .with_action(SettingsAction::ExpandProcessAuditList),
            );
        }
    }

    SettingsSection {
        title: "Process & Memory Audit",
        footer: Some(
            "Safe to End highlights headless background apps >=150 MB (excluding sync, VPN, and driver tools). Hover a row for its executable path.",
        ),
        rows,
    }
}

#[must_use]
pub(crate) fn build_general_page(
    snapshot: &OttoSettingsSnapshot,
    modules: &[ModuleRow],
) -> SettingsContent {
    let startup_row = setting_row(
        snapshot,
        OttoSettingItem::Toggle(OttoSettingKind::LaunchAtLogin),
        SettingsPage::General,
    );
    SettingsContent {
        hero: None,
        sections: vec![
            SettingsSection {
                title: "Startup",
                footer: None,
                rows: vec![startup_row],
            },
            build_cleanup_section(snapshot, modules, &snapshot.process_audit.audit),
            build_audit_section(&snapshot.process_audit),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{GeneralPageState, PROCESS_AUDIT_INITIAL_LIMIT, build_general_page};
    use crate::runtime::process_hog::{
        ProcessAuditFilter, ProcessThrottleKind, RawProcessAuditSample, summarize_process_audit,
    };
    use crate::settings::model::{OttoSettingsSnapshot, SystemSettingsState};
    use crate::settings::settings_content::{SettingsAction, SettingsRowValue};
    use crate::settings_window::resolve_modules;

    fn sample_snapshot() -> OttoSettingsSnapshot {
        let audit = summarize_process_audit(&[
            RawProcessAuditSample {
                pid: 100,
                exe_name: String::from("svchost.exe"),
                exe_path: Some(String::from(r"C:\Windows\System32\svchost.exe")),
                memory_bytes: 180 * 1024 * 1024,
                cpu_tenths_percent: 4,
                session_id: 0,
                is_critical: false,
                controllable: false,
                terminatable: false,
                is_foreground: false,
                has_visible_window: false,
                throttle_kind: ProcessThrottleKind::None,
            },
            RawProcessAuditSample {
                pid: 200,
                exe_name: String::from("discord.exe"),
                exe_path: Some(String::from(r"C:\Apps\Discord.exe")),
                memory_bytes: 260 * 1024 * 1024,
                cpu_tenths_percent: 15,
                session_id: 1,
                is_critical: false,
                controllable: true,
                terminatable: true,
                is_foreground: false,
                has_visible_window: false,
                throttle_kind: ProcessThrottleKind::AutoEcoQos,
            },
            RawProcessAuditSample {
                pid: 250,
                exe_name: String::from("OneDrive.exe"),
                exe_path: Some(String::from(r"C:\Apps\OneDrive.exe")),
                memory_bytes: 210 * 1024 * 1024,
                cpu_tenths_percent: 1,
                session_id: 1,
                is_critical: false,
                controllable: true,
                terminatable: true,
                is_foreground: false,
                has_visible_window: false,
                throttle_kind: ProcessThrottleKind::None,
            },
            RawProcessAuditSample {
                pid: 300,
                exe_name: String::from("code.exe"),
                exe_path: Some(String::from(r"C:\Apps\Code.exe")),
                memory_bytes: 400 * 1024 * 1024,
                cpu_tenths_percent: 10,
                session_id: 1,
                is_critical: false,
                controllable: true,
                terminatable: true,
                is_foreground: true,
                has_visible_window: true,
                throttle_kind: ProcessThrottleKind::None,
            },
        ]);
        OttoSettingsSnapshot {
            system: SystemSettingsState {
                background_cleanup: true,
                ..SystemSettingsState::default()
            },
            process_audit: GeneralPageState {
                audit,
                ..GeneralPageState::default()
            },
            ..OttoSettingsSnapshot::default()
        }
    }

    #[test]
    fn general_page_renders_kpis_protected_locks_and_safe_actions() {
        let mut snap = sample_snapshot();
        let modules = resolve_modules(&snap, |_| None);
        let bg_page = build_general_page(&snap, &modules);
        assert_eq!(bg_page.sections.len(), 3);
        let kpi = &bg_page.sections[1].rows[1].value;
        assert!(matches!(kpi, SettingsRowValue::KpiStrip(_)));
        let discord = bg_page.sections[2]
            .rows
            .iter()
            .find(|row| row.label == "Discord")
            .expect("discord row");
        assert!(matches!(
            discord.value,
            SettingsRowValue::ProcessAudit {
                throttled: true,
                throttle_label: Some("Restore"),
                terminate_label: Some("End Task"),
                recommended_end: true,
                ..
            }
        ));
        assert!(discord.subtitle.contains("Safe to End"));
        let onedrive = bg_page.sections[2]
            .rows
            .iter()
            .find(|row| row.label == "Onedrive")
            .expect("onedrive row");
        assert!(matches!(
            onedrive.value,
            SettingsRowValue::ProcessAudit {
                recommended_end: false,
                ..
            }
        ));
        assert!(!onedrive.subtitle.contains("Safe to End"));
        assert!(
            !bg_page.sections[2]
                .rows
                .iter()
                .any(|row| row.label == "Windows Service Host")
        );

        snap.process_audit
            .select_filter(ProcessAuditFilter::Protected);
        let protected_page = build_general_page(&snap, &modules);
        let svchost = protected_page.sections[2]
            .rows
            .iter()
            .find(|row| row.label == "Windows Service Host")
            .expect("svchost row");
        assert!(matches!(
            svchost.value,
            SettingsRowValue::ProcessAudit {
                status_label: "Protected",
                throttle_action: None,
                terminate_action: None,
                ..
            }
        ));
    }

    #[test]
    fn general_page_caps_long_lists_and_expands_on_request() {
        let samples: Vec<RawProcessAuditSample> = (0..15)
            .map(|idx| RawProcessAuditSample {
                pid: 1000 + idx,
                exe_name: format!("helper{idx}.exe"),
                exe_path: Some(format!(r"C:\Apps\helper{idx}.exe")),
                memory_bytes: u64::from(idx + 1) * 10 * 1024 * 1024,
                cpu_tenths_percent: 0,
                session_id: 1,
                is_critical: false,
                controllable: true,
                terminatable: true,
                is_foreground: false,
                has_visible_window: false,
                throttle_kind: ProcessThrottleKind::None,
            })
            .collect();
        let mut snap = OttoSettingsSnapshot {
            process_audit: GeneralPageState {
                audit: summarize_process_audit(&samples),
                ..GeneralPageState::default()
            },
            ..OttoSettingsSnapshot::default()
        };
        let modules = resolve_modules(&snap, |_| None);
        let initial = build_general_page(&snap, &modules);
        let audit_count = initial.sections[2]
            .rows
            .iter()
            .filter(|row| matches!(row.value, SettingsRowValue::ProcessAudit { .. }))
            .count();
        assert_eq!(audit_count, PROCESS_AUDIT_INITIAL_LIMIT);
        assert!(
            initial.sections[2]
                .rows
                .iter()
                .any(|row| row.action == Some(SettingsAction::ExpandProcessAuditList))
        );

        snap.process_audit.expand_list();
        let expanded = build_general_page(&snap, &modules);
        let expanded_count = expanded.sections[2]
            .rows
            .iter()
            .filter(|row| matches!(row.value, SettingsRowValue::ProcessAudit { .. }))
            .count();
        assert_eq!(expanded_count, 15);
    }
}
