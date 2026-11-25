use otto_core::{
    CommandOutcome, FocusActivityState, FocusDisclosure, FocusRulePolicy, FocusTextField,
    OttoSettingKind, SettingsAction, SettingsHit, SettingsPage, UpdatesPort, normalize_domain,
};
use otto_platform::{
    WindowsUpdatesPort, current_process_id, kill_window_timer, pick_blocked_app_executable,
    post_window_message, read_launch_at_login, save_focus_activity, set_window_timer,
    signal_command_failure, terminate_process_group, toggle_process_group_throttle,
};
use windows::Win32::UI::WindowsAndMessaging::WM_CLOSE;

use crate::host::RuntimeContext;
use crate::win_msg::{AUDIT_TIMER_ID, UPDATE_TIMER_ID};

use super::lifecycle::repaint;
use super::{refresh_model, with_window};

const UPDATE_POLL_MS: u32 = 250;
const AUDIT_POLL_MS: u32 = 2_000;

pub(super) fn arm_audit_timer(hwnd: isize) {
    let _ = set_window_timer(hwnd, AUDIT_TIMER_ID, AUDIT_POLL_MS);
}

pub(super) fn disarm_audit_timer(hwnd: isize) {
    kill_window_timer(hwnd, AUDIT_TIMER_ID);
}

pub(super) fn sync_audit_timer(hwnd: isize, page: SettingsPage) {
    if page == SettingsPage::General {
        arm_audit_timer(hwnd);
    } else {
        disarm_audit_timer(hwnd);
    }
}

pub(super) fn arm_update_timer(hwnd: isize) {
    let _ = set_window_timer(hwnd, UPDATE_TIMER_ID, UPDATE_POLL_MS);
}

fn disarm_update_timer(hwnd: isize) {
    kill_window_timer(hwnd, UPDATE_TIMER_ID);
}

fn start_update_check(runtime: &RuntimeContext) {
    let port = WindowsUpdatesPort;
    let started = port.start_update_check();
    let state = port.poll_update_state();
    let poll = started || state.is_busy();
    let hwnd = with_window(runtime, |window| {
        window.update_state = state;
        window.hwnd
    });
    if let (Some(hwnd), true) = (hwnd, poll) {
        arm_update_timer(hwnd);
    }
}

fn start_update_install(runtime: &RuntimeContext) {
    let port = WindowsUpdatesPort;
    let started = port.start_update_install();
    let state = port.poll_update_state();
    if !started && !state.is_busy() {
        signal_command_failure();
    }
    let poll = started || state.is_busy();
    let hwnd = with_window(runtime, |window| {
        window.update_state = state;
        window.hwnd
    });
    if let (Some(hwnd), true) = (hwnd, poll) {
        arm_update_timer(hwnd);
    }
}

fn request_quit_for_update(runtime: &RuntimeContext) {
    let _ = post_window_message(runtime.bar_hwnd.get(), WM_CLOSE, 0, 0);
}

pub(super) fn poll_update_check(runtime: &RuntimeContext, hwnd: isize) {
    let port = WindowsUpdatesPort;
    let next = port.poll_update_state();
    let changed = with_window(runtime, |window| {
        if window.update_state == next {
            false
        } else {
            window.update_state = next.clone();
            true
        }
    })
    .unwrap_or(false);
    if let Some(installer_path) = port.take_ready_installer() {
        disarm_update_timer(hwnd);
        if port.launch_update_installer(&installer_path) {
            repaint(runtime);
            request_quit_for_update(runtime);
        } else {
            port.mark_update_install_failed("Could not start installer");
            let failed = port.poll_update_state();
            with_window(runtime, |window| window.update_state = failed);
            signal_command_failure();
            repaint(runtime);
        }
        return;
    }
    if !next.is_busy() {
        disarm_update_timer(hwnd);
    }
    if changed {
        repaint(runtime);
    }
}

fn update_focus_activity(
    runtime: &RuntimeContext,
    mutate: impl FnOnce(&mut FocusActivityState) -> bool,
) -> bool {
    let updated = runtime.focus_activity.try_borrow_mut().is_ok_and(|mut fa| {
        let changed = mutate(&mut fa);
        if changed {
            if save_focus_activity(&fa) {
                fa.mark_saved();
            } else {
                signal_command_failure();
            }
        }
        changed
    });
    if updated {
        crate::bar::sync_focus_blocklist(runtime);
    }
    updated
}

pub(super) fn submit_focus_category(runtime: &RuntimeContext) {
    let raw = with_window(runtime, |window| {
        window.focus_state.category_name_input.clone()
    })
    .unwrap_or_default();
    let trimmed = otto_core::sanitize_category_name(&raw);
    if trimmed.is_empty() {
        with_window(runtime, |window| {
            window.focus_state.category_error = Some(String::from("Enter a category name"));
        });
        return;
    }
    let catalog =
        with_window(runtime, |window| window.focus_state.catalog.clone()).unwrap_or_default();
    let audit = otto_platform::collect_process_audit(current_process_id());
    let created = update_focus_activity(runtime, |fa| {
        let suggestion =
            otto_core::suggest_for_category(&trimmed, &catalog, &fa.today.apps, &audit.groups);
        if fa
            .categories
            .create_category(&trimmed, Some(&suggestion))
            .is_some()
        {
            fa.mark_dirty();
            true
        } else {
            false
        }
    });
    if created {
        with_window(runtime, |window| {
            window.focus_state.category_name_input.clear();
            window.focus_state.category_error = None;
            window.focus_state.focused_field = FocusTextField::None;
            window
                .focus_state
                .set_disclosure(FocusDisclosure::CreateMode, false);
        });
    } else {
        with_window(runtime, |window| {
            window.focus_state.category_error = Some(String::from("Max 12 focus modes reached"));
        });
        signal_command_failure();
    }
}

pub(super) fn submit_rename_focus_category(runtime: &RuntimeContext) {
    let raw = with_window(runtime, |window| {
        window.focus_state.category_rename_input.clone()
    })
    .unwrap_or_default();
    let renamed = update_focus_activity(runtime, |fa| {
        let active_id = fa.categories.active_category_id.clone();
        if fa.categories.rename_category(&active_id, &raw) {
            fa.mark_dirty();
            true
        } else {
            false
        }
    });
    if renamed {
        with_window(runtime, |window| {
            window.focus_state.category_rename_input.clear();
            window.focus_state.category_error = None;
            window.focus_state.focused_field = FocusTextField::None;
            window
                .focus_state
                .set_disclosure(FocusDisclosure::RenameMode, false);
        });
    } else {
        with_window(runtime, |window| {
            window.focus_state.category_error = Some(String::from("Enter a unique mode name"));
        });
        signal_command_failure();
    }
}

pub(super) fn submit_blocked_website(runtime: &RuntimeContext) {
    let raw =
        with_window(runtime, |window| window.focus_state.website_input.clone()).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        with_window(runtime, |window| {
            window.focus_state.website_error =
                Some(String::from("Enter a domain like youtube.com"));
        });
        return;
    }
    let Some(domain) = normalize_domain(trimmed) else {
        with_window(runtime, |window| {
            window.focus_state.website_error =
                Some(String::from("Enter a valid domain like youtube.com"));
        });
        signal_command_failure();
        return;
    };
    let already_blocked = runtime
        .focus_activity
        .try_borrow()
        .is_ok_and(|fa| fa.blocklist.blocked_websites.contains(&domain));
    if already_blocked {
        with_window(runtime, |window| {
            window.focus_state.website_input.clear();
            window.focus_state.website_error = Some(format!("{domain} is already blocked"));
        });
        return;
    }
    let added = update_focus_activity(runtime, |fa| {
        let ok = fa.categories.active_mut().add_blocked_website(&domain)
            || fa.blocklist.add_website(&domain);
        if ok {
            fa.mark_dirty();
        }
        ok
    });
    with_window(runtime, |window| {
        if added {
            window.focus_state.website_input.clear();
            window.focus_state.website_error = None;
        } else {
            window.focus_state.website_error = Some(String::from("Website blocklist is full"));
        }
    });
}

#[allow(clippy::too_many_lines)]
fn apply_action(runtime: &RuntimeContext, action: SettingsAction) {
    if !matches!(
        action,
        SettingsAction::FocusInput(_)
            | SettingsAction::ClearFocusInput(_)
            | SettingsAction::SubmitBlockedWebsite
    ) {
        with_window(runtime, |window| {
            window.focus_state.focused_field = FocusTextField::None;
        });
    }
    match action {
        SettingsAction::Toggle(kind) => {
            super::state::toggle_setting(runtime, kind);
            if kind == OttoSettingKind::LaunchAtLogin {
                with_window(runtime, |window| {
                    window.launch_at_login = read_launch_at_login();
                });
            }
        }
        SettingsAction::CycleDockIconSize => super::state::cycle_dock_icon_size(runtime),
        SettingsAction::CycleMenuBarHide => super::state::cycle_menu_bar_hide(runtime),
        SettingsAction::CycleBrandIcon => super::state::cycle_brand_icon(runtime),
        SettingsAction::Open(page) => {
            let hwnd = with_window(runtime, |window| {
                window.select_page(page);
                window.hwnd
            });
            if let Some(hwnd) = hwnd {
                sync_audit_timer(hwnd, page);
            }
            refresh_model(runtime);
        }
        SettingsAction::OpenLogs => {
            let _ = otto_platform::open_logs_folder();
        }
        SettingsAction::CheckForUpdates => start_update_check(runtime),
        SettingsAction::InstallUpdate => start_update_install(runtime),
        SettingsAction::OpenReleases => WindowsUpdatesPort.open_releases_page(),
        SettingsAction::ToggleFocusBlockNotifications => {
            let _ = update_focus_activity(runtime, |fa| {
                fa.blocklist.block_notifications = !fa.blocklist.block_notifications;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleFocusAllowCalls => {
            let _ = update_focus_activity(runtime, |fa| {
                fa.blocklist.allow_calls_during_focus = !fa.blocklist.allow_calls_during_focus;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleBlockedApp(app_key) => {
            let _ = update_focus_activity(runtime, |fa| {
                let _ = fa.blocklist.toggle_app(&app_key);
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleBlockedWebsite(domain) => {
            let _ = update_focus_activity(runtime, |fa| {
                let _ = fa.blocklist.toggle_website(&domain);
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::PickBlockedAppFile => {
            let owner = with_window(runtime, |window| window.hwnd).unwrap_or(0);
            if let Some(app_key) = pick_blocked_app_executable(owner) {
                let _ = update_focus_activity(runtime, |fa| {
                    let active = fa.categories.active_mut();
                    let changed = match active.policy {
                        FocusRulePolicy::Allowlist => active.toggle_allowed_app(&app_key),
                        FocusRulePolicy::Blocklist => active.toggle_blocked_app(&app_key),
                    };
                    if changed {
                        fa.mark_dirty();
                    }
                    changed
                });
            }
        }
        SettingsAction::ResetFocusActivityStats => {
            let _ = update_focus_activity(runtime, |fa| {
                fa.reset_stats();
                true
            });
        }
        SettingsAction::SetFocusTab(tab) => {
            with_window(runtime, |window| {
                window.focus_state.select_tab(tab);
                window.scroll = 0;
                window.hovered = None;
            });
        }
        SettingsAction::SetFocusAppFilter(filter) => {
            with_window(runtime, |window| {
                window.focus_state.select_filter(filter);
                window.scroll = 0;
                window.hovered = None;
            });
        }
        SettingsAction::FocusInput(field) => {
            with_window(runtime, |window| window.focus_state.focus_field(field));
        }
        SettingsAction::ClearFocusInput(field) => {
            with_window(runtime, |window| {
                window.focus_state.clear_field(field);
                window.focus_state.focus_field(field);
            });
        }
        SettingsAction::SubmitBlockedWebsite => submit_blocked_website(runtime),
        SettingsAction::ExpandFocusAllApps => {
            with_window(runtime, |window| window.focus_state.expand_all_apps());
        }
        SettingsAction::SelectFocusChartDay(day) => {
            with_window(runtime, |window| {
                window.focus_state.selected_bar_day = Some(day);
            });
        }
        SettingsAction::ToggleFocusActivityDetails => {
            with_window(runtime, |window| {
                window.focus_state.details_expanded = !window.focus_state.details_expanded;
            });
        }
        SettingsAction::SelectFocusCategory(id) => {
            let changed = update_focus_activity(runtime, |fa| {
                if fa.categories.select(&id) {
                    fa.mark_dirty();
                    true
                } else {
                    false
                }
            });
            if changed {
                with_window(runtime, |window| {
                    window.focus_state.category_error = None;
                    window
                        .focus_state
                        .set_disclosure(FocusDisclosure::RenameMode, false);
                });
            }
        }
        SettingsAction::SubmitFocusCategory => submit_focus_category(runtime),
        SettingsAction::SubmitRenameFocusCategory => submit_rename_focus_category(runtime),
        SettingsAction::DeleteFocusCategory(id) => {
            let changed = update_focus_activity(runtime, |fa| {
                if fa.categories.delete_category(&id) {
                    fa.mark_dirty();
                    true
                } else {
                    false
                }
            });
            if changed {
                with_window(runtime, |window| {
                    window.focus_state.category_error = None;
                    window
                        .focus_state
                        .set_disclosure(FocusDisclosure::RenameMode, false);
                });
            }
        }
        SettingsAction::CycleFocusCategoryColor => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                active.color = active.color.next();
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::SetFocusCategoryColor(color) => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                if active.color == color {
                    return false;
                }
                active.color = color;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::AdjustFocusCategoryDuration(delta) => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                let next = active.focus_minutes as i32 + delta;
                active.focus_minutes = if next > otto_core::POMODORO_MAX_MINUTES as i32 {
                    15
                } else {
                    next.clamp(5, otto_core::POMODORO_MAX_MINUTES as i32) as u32
                };
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::CycleFocusCategoryBreaks => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                let (s, l, c) = match (
                    active.short_break_minutes,
                    active.long_break_minutes,
                    active.cycles_before_long_break,
                ) {
                    (5, 15, 4) => (10, 20, 4),
                    (10, 20, 4) => (5, 20, 3),
                    (5, 20, 3) => (10, 30, 4),
                    _ => (5, 15, 4),
                };
                active.short_break_minutes = s;
                active.long_break_minutes = l;
                active.cycles_before_long_break = c;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleFocusCategoryPolicy => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                active.policy = active.policy.toggled();
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::SetFocusCategoryPolicy(policy) => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                if active.policy == policy {
                    return false;
                }
                active.policy = policy;
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::CycleFocusCategoryBackgroundPolicy => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                active.background_policy = active.background_policy.next();
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ApplyFocusCategorySuggestions => {
            let catalog = with_window(runtime, |window| window.focus_state.catalog.clone())
                .unwrap_or_default();
            let audit = otto_platform::collect_process_audit(current_process_id());
            let _ = update_focus_activity(runtime, |fa| {
                let name = fa.categories.active().name.clone();
                let suggestion =
                    otto_core::suggest_for_category(&name, &catalog, &fa.today.apps, &audit.groups);
                otto_core::apply_suggestion_to_category(fa.categories.active_mut(), &suggestion);
                fa.mark_dirty();
                true
            });
        }
        SettingsAction::ToggleCategoryAllowedApp(app_key) => {
            let _ = update_focus_activity(runtime, |fa| {
                let changed = fa.categories.active_mut().toggle_allowed_app(&app_key);
                if changed {
                    fa.mark_dirty();
                }
                changed
            });
        }
        SettingsAction::ToggleCategoryBlockedApp(app_key) => {
            let _ = update_focus_activity(runtime, |fa| {
                let changed = fa.categories.active_mut().toggle_blocked_app(&app_key);
                if changed {
                    fa.mark_dirty();
                }
                changed
            });
        }
        SettingsAction::ToggleCategoryApp(app_key) => {
            let _ = update_focus_activity(runtime, |fa| {
                let active = fa.categories.active_mut();
                let changed = match active.policy {
                    FocusRulePolicy::Allowlist => active.toggle_allowed_app(&app_key),
                    FocusRulePolicy::Blocklist => active.toggle_blocked_app(&app_key),
                };
                if changed {
                    fa.mark_dirty();
                }
                changed
            });
        }
        SettingsAction::ToggleCategoryWebsite(domain) => {
            let _ = update_focus_activity(runtime, |fa| {
                let changed = fa.categories.active_mut().toggle_blocked_website(&domain);
                if changed {
                    fa.mark_dirty();
                }
                changed
            });
        }
        SettingsAction::ToggleCategoryCloseApp(app_key) => {
            let _ = update_focus_activity(runtime, |fa| {
                let changed = fa.categories.active_mut().toggle_close_app(&app_key);
                if changed {
                    fa.mark_dirty();
                }
                changed
            });
        }
        SettingsAction::ToggleFocusDisclosure(disc) => {
            let current_name = runtime
                .focus_activity
                .try_borrow()
                .ok()
                .map(|fa| fa.categories.active().name.clone())
                .unwrap_or_default();
            with_window(runtime, |window| {
                window.focus_state.toggle_disclosure(disc);
                let open = window.focus_state.is_open(disc);
                if disc == FocusDisclosure::RawActivity {
                    window.focus_state.details_expanded = open;
                } else if disc == FocusDisclosure::CreateMode && open {
                    window.focus_state.category_name_input.clear();
                    window.focus_state.category_error = None;
                    window.focus_state.focused_field = FocusTextField::CategoryName;
                } else if disc == FocusDisclosure::RenameMode && open {
                    window.focus_state.category_rename_input = current_name;
                    window.focus_state.category_error = None;
                    window.focus_state.focused_field = FocusTextField::CategoryRename;
                }
            });
        }
        SettingsAction::SetFocusHeatmapFilter(category_id) => {
            with_window(runtime, |window| {
                window.focus_state.heatmap_category_id = category_id;
                window.focus_state.selected_heatmap_cell = None;
            });
        }
        SettingsAction::SelectFocusHeatmapCell(cell) => {
            with_window(runtime, |window| {
                window.focus_state.selected_heatmap_cell =
                    if window.focus_state.selected_heatmap_cell == Some(cell) {
                        None
                    } else {
                        Some(cell)
                    };
            });
        }
        SettingsAction::SetAppearanceStyle(style) => {
            super::state::set_appearance_style(runtime, style);
        }
        SettingsAction::SetAppearanceTheme(theme) => {
            super::state::set_appearance_theme(runtime, theme);
        }
        SettingsAction::SetAppearanceAccent(accent) => {
            super::state::set_appearance_accent(runtime, accent);
        }
        SettingsAction::SetProcessAuditFilter(filter) => {
            with_window(runtime, |window| {
                window.general_state.select_filter(filter);
                window.general_state.status_note = None;
                window.scroll = 0;
                window.hovered = None;
            });
        }
        SettingsAction::RefreshProcessAudit => {
            with_window(runtime, |window| {
                window.general_state.status_note = None;
                window.refresh_process_audit();
            });
        }
        SettingsAction::ExpandProcessAuditList => {
            with_window(runtime, |window| {
                window.general_state.expand_list();
            });
        }
        SettingsAction::ToggleProcessThrottle(exe_name) => {
            let note = match toggle_process_group_throttle(&exe_name) {
                Some(true) => format!("EcoQoS efficiency mode enabled for {exe_name}"),
                Some(false) => format!("Restored normal scheduling for {exe_name}"),
                None => {
                    signal_command_failure();
                    format!("Could not change EcoQoS for {exe_name} (protected or exited)")
                }
            };
            with_window(runtime, |window| {
                window.general_state.status_note = Some(note);
                window.refresh_process_audit();
            });
        }
        SettingsAction::TerminateProcessGroup(exe_name) => {
            let outcome = terminate_process_group(&exe_name, current_process_id());
            let note = match outcome {
                CommandOutcome::Completed => format!("Ended background process {exe_name}"),
                CommandOutcome::Denied => {
                    signal_command_failure();
                    format!("Cannot end {exe_name}: protected or elevated")
                }
                CommandOutcome::TargetGone => {
                    format!("{exe_name} is no longer running")
                }
                _ => {
                    signal_command_failure();
                    format!("Could not end {exe_name}")
                }
            };
            with_window(runtime, |window| {
                window.general_state.status_note = Some(note);
                window.refresh_process_audit();
            });
        }
    }
}

pub(super) fn activate(runtime: &RuntimeContext, hit: SettingsHit) {
    match hit {
        SettingsHit::Page(page) => {
            let hwnd = with_window(runtime, |window| {
                window.select_page(page);
                window.hwnd
            });
            if let Some(hwnd) = hwnd {
                sync_audit_timer(hwnd, page);
            }
            refresh_model(runtime);
        }
        SettingsHit::Row { section, row } => {
            let action = with_window(runtime, |window| {
                window
                    .content
                    .row(section, row)
                    .and_then(|row| row.action.clone())
            })
            .flatten();
            if let Some(action) = action {
                apply_action(runtime, action);
            } else {
                with_window(runtime, |window| {
                    window.focus_state.focused_field = FocusTextField::None;
                });
            }
        }
        SettingsHit::Part { section, row, part } => {
            let action = with_window(runtime, |window| {
                window.content.part_action(section, row, part)
            })
            .flatten();
            if let Some(action) = action {
                apply_action(runtime, action);
            } else {
                with_window(runtime, |window| {
                    window.focus_state.focused_field = FocusTextField::None;
                });
            }
        }
        SettingsHit::Content => {
            with_window(runtime, |window| {
                window.focus_state.focused_field = FocusTextField::None;
            });
        }
        SettingsHit::WindowControl(_) => {}
    }
    repaint(runtime);
}
