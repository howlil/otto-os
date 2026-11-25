use super::compute::compute_layout_with_notch;
use super::geometry::Rect;
use super::hit::HitTarget;
use super::input::NotchLayoutInput;
use super::metrics::Metrics;
use super::model::Layout;

pub(super) const MENU_SLOT_HORIZONTAL_PAD_POINTS: i32 = 10;

#[must_use]
pub(super) fn estimate_menu_text_width_points(label: &str) -> i32 {
    label
        .chars()
        .take(16)
        .map(|ch| match ch {
            'i' | 'l' | 'I' | 't' | 'f' | 'j' | 'r' | ' ' | '.' | '-' => 5,
            'm' | 'w' | 'M' | 'W' => 10,
            _ => 8,
        })
        .sum()
}

pub(super) fn estimate_app_slot_points(metrics: &Metrics, app_name: &str) -> i32 {
    (estimate_menu_text_width_points(app_name) + MENU_SLOT_HORIZONTAL_PAD_POINTS * 2)
        .clamp(metrics.app_slot_min_width, metrics.app_slot_max_width)
}

pub(super) const LEFT_TARGETS: [HitTarget; 7] = [
    HitTarget::Start,
    HitTarget::AppMenu,
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
];

pub(super) const RIGHT_VISUAL_ORDER: [HitTarget; 6] = [
    HitTarget::Clock,
    HitTarget::Notifications,
    HitTarget::ControlCenter,
    HitTarget::Search,
    HitTarget::QuickSettings,
    HitTarget::Tray,
];

pub(super) const RIGHT_KEEP_PRIORITY: [HitTarget; 6] = [
    HitTarget::Clock,
    HitTarget::Notifications,
    HitTarget::ControlCenter,
    HitTarget::QuickSettings,
    HitTarget::Tray,
    HitTarget::Search,
];

pub(super) struct SlotRow {
    pub(super) slots: Vec<(HitTarget, Rect)>,
    pub(super) hidden: Vec<HitTarget>,
    pub(super) overflow: Rect,
}

impl SlotRow {
    pub(super) fn rect(&self, target: HitTarget) -> Rect {
        self.slots
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map_or_else(|| self.overflow_anchor(), |(_, rect)| *rect)
    }

    pub(super) const fn overflow_anchor(&self) -> Rect {
        Rect {
            x: self.overflow.x,
            y: 0,
            width: 0,
            height: self.overflow.height,
        }
    }
}

pub(super) fn resolve_right_slots(
    widths: &[(HitTarget, i32)],
    gap: i32,
    left_limit: i32,
    right_edge: i32,
    height: i32,
) -> (Vec<(HitTarget, Rect)>, Vec<HitTarget>) {
    let available = (right_edge - left_limit).max(0);
    let width_of = |target: HitTarget| {
        widths
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map_or(0, |(_, width)| *width)
    };
    let mut kept = Vec::new();
    let mut hidden = Vec::new();
    let mut used = 0;
    for target in RIGHT_KEEP_PRIORITY {
        let width = width_of(target);
        if width <= 0 {
            continue;
        }
        let needed = if kept.is_empty() { width } else { width + gap };
        if used + needed <= available {
            used += needed;
            kept.push(target);
        } else {
            hidden.push(target);
        }
    }
    let mut cursor = right_edge;
    let mut slots = Vec::new();
    for target in RIGHT_VISUAL_ORDER {
        if !kept.contains(&target) {
            continue;
        }
        let width = width_of(target);
        slots.push((
            target,
            Rect {
                x: cursor - width,
                y: 0,
                width,
                height,
            },
        ));
        cursor -= width + gap;
    }
    (slots, hidden)
}

pub(super) struct LeftSlotInput<'a> {
    pub(super) widths: &'a [i32; 7],
    pub(super) overflow_width: i32,
    pub(super) gap: i32,
    pub(super) start: i32,
    pub(super) limit: i32,
    pub(super) height: i32,
    pub(super) force_overflow: bool,
}

pub(super) fn resolve_left_slots(input: &LeftSlotInput<'_>) -> SlotRow {
    let total: i32 = input.widths.iter().sum::<i32>() + input.gap * 6;
    let needs_overflow = input.force_overflow || input.start + total > input.limit;
    let reserved = if needs_overflow {
        input.overflow_width + input.gap
    } else {
        0
    };
    let mut cursor = input.start;
    let mut slots = Vec::new();
    let mut hidden = Vec::new();
    for (target, width) in LEFT_TARGETS.into_iter().zip(*input.widths) {
        if hidden.is_empty() && cursor + width + reserved <= input.limit {
            slots.push((
                target,
                Rect {
                    x: cursor,
                    y: 0,
                    width,
                    height: input.height,
                },
            ));
            cursor += width + input.gap;
        } else {
            hidden.push(target);
        }
    }
    let overflow = Rect {
        x: cursor.min(input.limit),
        y: 0,
        width: if needs_overflow {
            input.overflow_width.min((input.limit - cursor).max(0))
        } else {
            0
        },
        height: input.height,
    };
    SlotRow {
        slots,
        hidden,
        overflow,
    }
}

#[must_use]
pub fn compute_layout(metrics: &Metrics, screen_width: i32, dpi: u32) -> Layout {
    compute_layout_for_app(metrics, screen_width, dpi, "Finder")
}

#[must_use]
pub fn compute_layout_for_app(
    metrics: &Metrics,
    screen_width: i32,
    dpi: u32,
    app_name: &str,
) -> Layout {
    compute_layout_with_hud(metrics, screen_width, dpi, app_name, 0.0)
}

#[must_use]
pub fn compute_layout_with_hud(
    metrics: &Metrics,
    screen_width: i32,
    dpi: u32,
    app_name: &str,
    hud_expansion: f32,
) -> Layout {
    compute_layout_with_notch(
        metrics,
        screen_width,
        dpi,
        app_name,
        NotchLayoutInput {
            hud_expansion,
            ..NotchLayoutInput::default()
        },
    )
}
