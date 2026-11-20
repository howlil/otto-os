use crate::layout::Rect;

pub const NOTCH_RECT_PROPERTY: &str = "OttoOS.NotchRect";

#[must_use]
pub fn pack_notch_rect(rect: Rect) -> Option<u64> {
    if rect.is_empty() {
        return None;
    }
    let x = i16::try_from(rect.x).ok()?;
    let y = i16::try_from(rect.y).ok()?;
    let width = u16::try_from(rect.width).ok()?;
    let height = u16::try_from(rect.height).ok()?;
    Some(
        u64::from(x as u16)
            | (u64::from(y as u16) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48),
    )
}

#[must_use]
pub fn unpack_notch_rect(packed: u64) -> Option<Rect> {
    let rect = Rect {
        x: i32::from((packed & 0xFFFF) as u16 as i16),
        y: i32::from(((packed >> 16) & 0xFFFF) as u16 as i16),
        width: i32::from(((packed >> 32) & 0xFFFF) as u16),
        height: i32::from(((packed >> 48) & 0xFFFF) as u16),
    };
    (!rect.is_empty()).then_some(rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_round_trips_through_the_published_encoding() {
        let rect = Rect {
            x: 1020,
            y: 6,
            width: 300,
            height: 48,
        };
        assert_eq!(
            unpack_notch_rect(pack_notch_rect(rect).unwrap()),
            Some(rect)
        );
    }

    #[test]
    fn negative_origin_round_trips() {
        let rect = Rect {
            x: -40,
            y: -2,
            width: 10,
            height: 10,
        };
        assert_eq!(
            unpack_notch_rect(pack_notch_rect(rect).unwrap()),
            Some(rect)
        );
    }

    #[test]
    fn empty_or_out_of_range_rects_are_not_published() {
        assert_eq!(pack_notch_rect(Rect::default()), None);
        let too_wide = Rect {
            x: 0,
            y: 0,
            width: 70_000,
            height: 10,
        };
        assert_eq!(pack_notch_rect(too_wide), None);
        assert_eq!(unpack_notch_rect(0), None);
    }
}
