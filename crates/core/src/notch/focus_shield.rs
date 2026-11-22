use crate::layout::{Rect, scale};

const CARD_WIDTH_DIP: i32 = 360;
const CARD_HEIGHT_DIP: i32 = 196;
const BACK_WIDTH_DIP: i32 = 128;
const BACK_HEIGHT_DIP: i32 = 40;
const BACK_BOTTOM_DIP: i32 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusShieldLayout {
    pub card: Rect,
    pub back: Rect,
}

#[must_use]
pub fn focus_shield_layout(width: i32, height: i32, dpi: u32) -> FocusShieldLayout {
    let card_width = scale(CARD_WIDTH_DIP, dpi).min(width);
    let card_height = scale(CARD_HEIGHT_DIP, dpi).min(height);
    let card = Rect {
        x: (width - card_width) / 2,
        y: (height - card_height) / 2,
        width: card_width,
        height: card_height,
    };
    let back_width = scale(BACK_WIDTH_DIP, dpi);
    let back_height = scale(BACK_HEIGHT_DIP, dpi);
    let back = Rect {
        x: card.x + (card.width - back_width) / 2,
        y: card.bottom() - back_height - scale(BACK_BOTTOM_DIP, dpi),
        width: back_width,
        height: back_height,
    };
    FocusShieldLayout { card, back }
}

#[must_use]
pub fn is_back_hit(layout: &FocusShieldLayout, x: i32, y: i32) -> bool {
    layout.back.contains(x, y)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FocusShieldDismissal {
    held: Option<(isize, String)>,
}

impl FocusShieldDismissal {
    pub fn dismiss(&mut self, window: isize, page: &str) {
        self.held = Some((window, page.to_owned()));
    }

    #[must_use]
    pub fn suppresses(&mut self, window: isize, page: &str) -> bool {
        let holds = self
            .held
            .as_ref()
            .is_some_and(|(held_window, held_page)| *held_window == window && held_page == page);
        if !holds {
            self.held = None;
        }
        holds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_button_sits_inside_the_card_and_only_its_area_is_a_hit() {
        let layout = focus_shield_layout(1280, 800, 96);
        assert!(layout.card.contains_rect(&layout.back));
        assert!(is_back_hit(&layout, layout.back.x, layout.back.y));
        assert!(!is_back_hit(&layout, layout.back.x - 1, layout.back.y));
        assert!(!is_back_hit(&layout, 4, 4));
    }

    #[test]
    fn dismissal_holds_for_one_window_and_page_until_either_changes() {
        let mut dismissal = FocusShieldDismissal::default();
        dismissal.dismiss(42, "youtube.com");
        assert!(dismissal.suppresses(42, "youtube.com"));
        assert!(!dismissal.suppresses(42, "reddit.com"));
        assert!(!dismissal.suppresses(42, "youtube.com"));
        dismissal.dismiss(42, "youtube.com");
        assert!(!dismissal.suppresses(7, "youtube.com"));
        assert!(!dismissal.suppresses(42, "youtube.com"));
    }
}
