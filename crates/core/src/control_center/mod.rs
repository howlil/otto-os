mod actions;
mod commands;
mod layout;
mod layout_types;
mod model;
mod snapshot;
mod update;

#[cfg(test)]
mod tests;

pub use self::actions::{
    control_center_action, control_center_focus_targets, focus_row_subtitle,
    is_update_available_status, scroll_offset_for_item, step_slider_percent,
    subpanel_opener_target,
};
pub use self::commands::{
    ControlCenterClickOutcome, ControlCenterCommand, ControlCenterFocus, ControlCenterFocusOutcome,
};
pub use self::layout::{
    compute_control_center_layout_for_snapshot, next_dock_icon_size, power_mode_label,
};
pub use self::layout_types::{
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterMetrics, ControlCenterPlacement,
};
pub use self::model::{
    AudioOutputDevice, BluetoothDevice, BluetoothDeviceKind, BluetoothState, ControlCenterPage,
    SLIDER_KEY_STEP_PERCENT, UPDATE_CHECKING_LABEL, WifiNetwork, WifiState,
};
pub use self::snapshot::{ControlCenterSnapshot, SubPanelGroupLayout, SubPanelGroupSpec};
pub use self::update::{format_update_status, parse_latest_release_tag};
