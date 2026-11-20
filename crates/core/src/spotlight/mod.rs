mod layout;
mod math;
mod mode;
mod ranking;
mod text;

#[cfg(test)]
mod tests;

pub use self::layout::{
    SpotlightHit, SpotlightLayout, SpotlightMove, SpotlightResultLayout, SpotlightView,
    compute_spotlight_layout,
};
pub use self::mode::{
    SPOTLIGHT_DEFAULT_HEIGHT_POINTS, SPOTLIGHT_DEFAULT_WIDTH_POINTS, SpotlightMode,
};
pub use self::ranking::{move_spotlight_selection, rank_spotlight_results};
