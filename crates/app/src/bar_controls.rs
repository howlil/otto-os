use otto_core::{HitTarget, SystemStatus, is_top_bar_menu_target};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarControlAction {
    TopBarMenu(HitTarget),
    Tray,
    ControlCenter,
    QuickSettings,
    Search,
    Clock,
    AdjustVolume(i16),
    StepBrightness(i16),
}

pub(crate) struct BarControlsController {
    status: SystemStatus,
    active_app: String,
    active_hwnd: isize,
}

impl BarControlsController {
    pub(crate) fn new(status: SystemStatus, active_app: String, active_hwnd: isize) -> Self {
        Self {
            status,
            active_app,
            active_hwnd,
        }
    }

    pub(crate) fn status(&self) -> SystemStatus {
        self.status
    }

    pub(crate) fn active_app(&self) -> &str {
        &self.active_app
    }

    pub(crate) fn active_hwnd(&self) -> isize {
        self.active_hwnd
    }

    pub(crate) fn apply_status(&mut self, status: SystemStatus) -> bool {
        if self.status == status {
            return false;
        }
        self.status = status;
        true
    }

    pub(crate) fn apply_foreground(&mut self, active_app: String, active_hwnd: isize) -> bool {
        let app_changed = self.active_app != active_app;
        self.active_app = active_app;
        self.active_hwnd = active_hwnd;
        app_changed
    }

    pub(crate) fn click_action(target: HitTarget) -> Option<BarControlAction> {
        if is_top_bar_menu_target(target) {
            return Some(BarControlAction::TopBarMenu(target));
        }
        match target {
            HitTarget::Tray => Some(BarControlAction::Tray),
            HitTarget::ControlCenter => Some(BarControlAction::ControlCenter),
            HitTarget::QuickSettings => Some(BarControlAction::QuickSettings),
            HitTarget::Search => Some(BarControlAction::Search),
            HitTarget::Clock => Some(BarControlAction::Clock),
            _ => None,
        }
    }

    pub(crate) fn wheel_action(
        target: HitTarget,
        delta: i16,
        notch_expanded: bool,
    ) -> Option<BarControlAction> {
        match target {
            HitTarget::QuickSettings => Some(BarControlAction::AdjustVolume(delta)),
            HitTarget::ControlCenter => Some(BarControlAction::StepBrightness(delta)),
            HitTarget::Notch if !notch_expanded => Some(BarControlAction::StepBrightness(delta)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BarControlAction, BarControlsController};
    use otto_core::{HitTarget, SystemStatus};

    #[test]
    fn snapshots_update_only_when_values_change() {
        let status = SystemStatus::default();
        let mut controls = BarControlsController::new(status, String::from("Finder"), 7);

        assert!(!controls.apply_status(status));
        assert!(!controls.apply_foreground(String::from("Finder"), 7));
        assert!(controls.apply_foreground(String::from("Code"), 8));
        assert_eq!(controls.active_app(), "Code");
        assert_eq!(controls.active_hwnd(), 8);
        assert!(!controls.apply_foreground(String::from("Code"), 9));
        assert_eq!(controls.active_hwnd(), 9);
    }

    #[test]
    fn click_actions_cover_top_bar_and_status_targets() {
        assert_eq!(
            BarControlsController::click_action(HitTarget::FileMenu),
            Some(BarControlAction::TopBarMenu(HitTarget::FileMenu))
        );
        assert_eq!(
            BarControlsController::click_action(HitTarget::Clock),
            Some(BarControlAction::Clock)
        );
        assert_eq!(BarControlsController::click_action(HitTarget::Notch), None);
    }

    #[test]
    fn wheel_actions_keep_notch_brightness_separate_from_expanded_pomodoro() {
        assert_eq!(
            BarControlsController::wheel_action(HitTarget::QuickSettings, 120, false),
            Some(BarControlAction::AdjustVolume(120))
        );
        assert_eq!(
            BarControlsController::wheel_action(HitTarget::Notch, -120, false),
            Some(BarControlAction::StepBrightness(-120))
        );
        assert_eq!(
            BarControlsController::wheel_action(HitTarget::Notch, -120, true),
            None
        );
    }
}
