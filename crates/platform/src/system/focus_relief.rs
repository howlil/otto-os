use std::path::Path;

use otto_core::{
    BackgroundReliefPolicy, CommandOutcome, FocusCategory, ProcessSafety, WindowPresence,
    normalize_app_key,
};

use super::background_cleanup::{
    collect_process_audit, release_focus_throttles, terminate_process_group_matching,
    throttle_exe_for_focus,
};
use crate::window::model::enumerate_windows;
use crate::window::shield::post_window_message;

const WM_CLOSE_MSG: u32 = 0x0010;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusReliefOutcome {
    pub throttled_apps: usize,
    pub closed_apps: usize,
    pub saved_memory_bytes: u64,
}

fn post_graceful_close_for_exe(exe_name: &str, own_pid: u32) -> bool {
    let target = exe_name.to_ascii_lowercase();
    let mut sent = false;
    for win in enumerate_windows(own_pid) {
        if win.presence == WindowPresence::Hidden {
            continue;
        }
        let matches = Path::new(&win.executable)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(&target));
        if matches && post_window_message(win.identity.hwnd, WM_CLOSE_MSG, 0, 0) {
            sent = true;
        }
    }
    sent
}

#[must_use]
pub fn apply_focus_relief(category: &FocusCategory, own_pid: u32) -> FocusReliefOutcome {
    release_focus_throttles();
    if category.background_policy == BackgroundReliefPolicy::ShieldOnly {
        return FocusReliefOutcome::default();
    }
    let report = collect_process_audit(own_pid);
    let mut outcome = FocusReliefOutcome::default();
    for group in report.groups {
        if group.safety != ProcessSafety::SafeBackground {
            continue;
        }
        let key = normalize_app_key(&group.exe_name);
        if key.is_empty() {
            continue;
        }
        let should_act = category.should_block_app(&key, &group.display_name);
        let opt_in_close = category.background_policy == BackgroundReliefPolicy::EcoQosAndClose
            && category.is_app_close_on_focus(&key);
        if !should_act && !opt_in_close {
            continue;
        }
        if opt_in_close {
            if group.has_visible_window {
                if post_graceful_close_for_exe(&group.exe_name, own_pid) {
                    outcome.closed_apps += 1;
                    outcome.saved_memory_bytes = outcome
                        .saved_memory_bytes
                        .saturating_add(group.memory_bytes);
                    continue;
                }
            } else if group.can_terminate
                && terminate_process_group_matching(
                    &group.exe_name,
                    group.exe_path.as_deref(),
                    own_pid,
                ) == CommandOutcome::Completed
            {
                outcome.closed_apps += 1;
                outcome.saved_memory_bytes = outcome
                    .saved_memory_bytes
                    .saturating_add(group.memory_bytes);
                continue;
            }
        }
        if group.can_throttle && throttle_exe_for_focus(&group.exe_name) > 0 {
            outcome.throttled_apps += 1;
            outcome.saved_memory_bytes = outcome
                .saved_memory_bytes
                .saturating_add(group.memory_bytes);
        }
    }
    outcome
}

pub fn release_focus_relief() {
    release_focus_throttles();
}
