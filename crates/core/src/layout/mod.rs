mod clipboard;
mod compact;
mod compute;
mod geometry;
mod hit;
mod input;
mod metrics;
mod model;
mod notification;
mod panel;
mod panel_types;
mod shelf;
mod slots;
mod tasks;

#[cfg(test)]
mod tests;

pub use self::compute::compute_layout_with_notch;
pub use self::geometry::{Rect, scale};
pub use self::hit::{HitTarget, TargetScope};
pub use self::input::{NotchLayoutInput, PrivacyLayoutState, ShelfLayoutInput};
pub use self::metrics::Metrics;
pub use self::model::{Layout, NOTCH_CATEGORY_OPTIONS, NOTCH_HEATMAP_CELLS, NOTCH_HEATMAP_FILTERS};
pub use self::notification::{
    NOTIFICATION_BANNER_HEIGHT_POINTS, NOTIFICATION_RING_HEIGHT_POINTS,
    NOTIFICATION_STACK_EXTRA_PANEL_POINTS, NOTIFICATION_STACK_LIP_HEIGHT_POINTS,
    NotificationLayout, NotificationLayoutInput,
};
pub use self::slots::{compute_layout, compute_layout_for_app, compute_layout_with_hud};
