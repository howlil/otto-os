use otto_core::{HitTarget, SystemStatus, WindowInfo, is_top_bar_menu_target};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ForegroundChange {
    pub app: bool,
    pub window: bool,
}

pub(crate) struct BarControlsController {
    status: SystemStatus,
    active_app: String,
    active_window: Option<WindowInfo>,
}

impl BarControlsController {
    pub(crate) fn new(
        status: SystemStatus,
        active_app: String,
        active_window: Option<WindowInfo>,
    ) -> Self {
        Self {
            status,
            active_app,
            active_window,
        }
    }

    pub(crate) fn status(&self) -> SystemStatus {
        self.status
    }

    pub(crate) fn active_app(&self) -> &str {
        &self.active_app
    }

    pub(crate) fn active_window(&self) -> Option<&WindowInfo> {
        self.active_window.as_ref()
    }

    pub(crate) fn apply_status(&mut self, status: SystemStatus) -> bool {
        if self.status == status {
            return false;
        }
        self.status = status;
        true
    }

    pub(crate) fn apply_foreground(
        &mut self,
        active_app: String,
        active_window: Option<WindowInfo>,
    ) -> ForegroundChange {
        let change = ForegroundChange {
            app: self.active_app != active_app,
            window: self.active_window.as_ref().map(|window| window.identity)
                != active_window.as_ref().map(|window| window.identity),
        };
        self.active_app = active_app;
        self.active_window = active_window;
        change
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
    use otto_core::{
        AppIdentity, HitTarget, SystemStatus, WindowIdentity, WindowInfo, WindowPresence,
        WindowRole, WindowTraits,
    };

    fn window(hwnd: isize) -> WindowInfo {
        WindowInfo {
            identity: WindowIdentity {
                hwnd,
                process_id: 1,
                thread_id: 2,
                generation: 3,
            },
            app: AppIdentity::executable("code.exe"),
            app_name: String::from("Code"),
            executable: String::from("code.exe"),
            title: String::from("window"),
            role: WindowRole::Primary,
            presence: WindowPresence::Visible,
            zoomed: false,
            full_screen: false,
            active: true,
            traits: WindowTraits::STANDARD,
        }
    }

    #[test]
    fn snapshots_update_only_when_values_change() {
        let status = SystemStatus::default();
        let mut controls =
            BarControlsController::new(status, String::from("Finder"), Some(window(7)));

        assert!(!controls.apply_status(status));
        let same = controls.apply_foreground(String::from("Finder"), Some(window(7)));
        assert!(!same.app && !same.window);
        let other_app = controls.apply_foreground(String::from("Code"), Some(window(8)));
        assert!(other_app.app && other_app.window);
        assert_eq!(controls.active_app(), "Code");
        let other_window = controls.apply_foreground(String::from("Code"), Some(window(9)));
        assert!(!other_window.app && other_window.window);
        assert_eq!(
            controls.active_window().map(|window| window.identity.hwnd),
            Some(9)
        );
        let desktop = controls.apply_foreground(String::from("Finder"), None);
        assert!(desktop.app && desktop.window);
        assert!(controls.active_window().is_none());
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
        assert_eq!(
            BarControlsController::click_action(HitTarget::MenuOverflow),
            Some(BarControlAction::TopBarMenu(HitTarget::MenuOverflow))
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
