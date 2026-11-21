mod caption;
mod cover;
mod layout;
mod state;

#[cfg(test)]
mod tests;

pub use self::caption::{NativeCaption, NativeCaptionStyle, compute_cover_layout};
pub use self::cover::{
    COVER_FEATHER_DIP, COVER_RIGHT_MARGIN_DIP, COVER_SAMPLE_GAP_DIP, COVER_SAMPLE_WIDTH_DIP,
    CoverBand, CoverStyle, NATIVE_CAPTION_BUTTON_DIP, NATIVE_CORNER_RADIUS_DIP, RESIZE_BORDER_DIP,
    cover_is_dark, cover_row_bands, extract_cover_rows,
};
pub use self::layout::{
    ResizeEdge, WindowControlAction, WindowControlsLayout, WindowControlsMetrics,
};
pub use self::state::{
    CommandScope, WindowControlsState, accepts_window_controls, application_command_targets,
    window_control_scope, window_controls_visible_under_bar,
};
