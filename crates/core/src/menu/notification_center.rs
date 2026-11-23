use std::collections::BTreeSet;
use std::sync::Arc;

use crate::layout::{Rect, scale};

use super::notifications::{NotificationAccess, NotificationSummary};

pub const NOTIFICATION_CENTER_WIDTH: i32 = 328;
pub const NOTIFICATION_CENTER_MAX_HEIGHT: i32 = 580;
pub const NOTIFICATION_CENTER_PAD: i32 = 8;
pub const NOTIFICATION_CENTER_SHELL_RADIUS: i32 = 20;
pub const NOTIFICATION_CENTER_CARD_RADIUS: i32 = 12;
pub const NOTIFICATION_CENTER_PILL_RADIUS: i32 = 8;
pub const NOTIFICATION_CENTER_HEADER_HEIGHT: i32 = 24;
pub const NOTIFICATION_CENTER_GROUP_HEADER_HEIGHT: i32 = 24;
pub const NOTIFICATION_CENTER_CARD_COMPACT_HEIGHT: i32 = 50;
pub const NOTIFICATION_CENTER_CARD_HEIGHT: i32 = 64;
pub const NOTIFICATION_CENTER_CARD_TALL_HEIGHT: i32 = 76;
pub const NOTIFICATION_CENTER_STACK_LIP_STEP: i32 = 4;
pub const NOTIFICATION_CENTER_EMPTY_HEIGHT: i32 = 108;
pub const NOTIFICATION_CENTER_FOOTER_HEIGHT: i32 = 24;
pub const NOTIFICATION_CENTER_CARD_GAP: i32 = 6;
pub const NOTIFICATION_CENTER_CLOSE_HIT_SIZE: i32 = 24;
pub const NOTIFICATION_CENTER_SCREEN_MARGIN: i32 = 6;
const SINGLE_LINE_BODY_CHAR_LIMIT: usize = 38;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationGroup {
    pub app_key: String,
    pub app_name: String,
    pub items: Vec<NotificationSummary>,
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationCenterHit {
    #[default]
    None,
    ClearAll,
    ExpandGroup(usize),
    CollapseGroup(usize),
    ClearGroup(usize),
    OpenCard(u32),
    CloseCard(u32),
    AccessAction,
    OpenWindowsNotifications,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationCardKind {
    Single,
    CollapsedStack { total_count: usize },
    ExpandedItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationCardLayout {
    pub id: u32,
    pub group_index: usize,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_label: String,
    pub icon_bytes: Option<Arc<[u8]>>,
    pub rect: Rect,
    pub close_rect: Rect,
    pub lips: Vec<Rect>,
    pub kind: NotificationCardKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationGroupHeaderLayout {
    pub group_index: usize,
    pub app_name: String,
    pub count: usize,
    pub rect: Rect,
    pub collapse_rect: Rect,
    pub clear_rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationCenterStateCardLayout {
    pub rect: Rect,
    pub action_rect: Option<Rect>,
    pub title: String,
    pub subtitle: String,
    pub action_label: Option<String>,
    pub action_hit: NotificationCenterHit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotificationCenterLayout {
    pub dpi: u32,
    pub window: Rect,
    pub viewport: Rect,
    pub header_rect: Rect,
    pub title_rect: Rect,
    pub clear_all_rect: Option<Rect>,
    pub footer_rect: Rect,
    pub footer_button_rect: Rect,
    pub content_height: i32,
    pub scroll_offset: i32,
    pub shell_radius: f32,
    pub card_radius: f32,
    pub pill_radius: f32,
    pub group_headers: Vec<NotificationGroupHeaderLayout>,
    pub cards: Vec<NotificationCardLayout>,
    pub state_card: Option<NotificationCenterStateCardLayout>,
}

fn normalized_app_key(item: &NotificationSummary) -> String {
    let id = item.app_id.trim();
    if !id.is_empty() {
        return id.to_ascii_lowercase();
    }
    let name = item.app_name.trim();
    if !name.is_empty() {
        return name.to_ascii_lowercase();
    }
    format!("item:{}", item.id)
}

#[must_use]
pub fn group_notifications(
    items: &[NotificationSummary],
    expanded_apps: &BTreeSet<String>,
) -> Vec<NotificationGroup> {
    let mut groups: Vec<NotificationGroup> = Vec::new();
    for item in items {
        let key = normalized_app_key(item);
        if let Some(group) = groups.iter_mut().find(|existing| existing.app_key == key) {
            group.items.push(item.clone());
        } else {
            groups.push(NotificationGroup {
                expanded: expanded_apps.contains(&key),
                app_key: key,
                app_name: item.app_name.clone(),
                items: vec![item.clone()],
            });
        }
    }
    groups
}

fn close_rect_for_card(card: Rect, dpi: u32) -> Rect {
    let size = scale(NOTIFICATION_CENTER_CLOSE_HIT_SIZE, dpi);
    let pad_right = scale(6, dpi);
    let pad_top = scale(4, dpi);
    Rect {
        x: (card.right() - pad_right - size).max(card.x),
        y: card.y + pad_top,
        width: size,
        height: size,
    }
}

fn card_height_points(item: &NotificationSummary) -> i32 {
    let body = item.body.trim();
    if item.title.trim().is_empty() || body.is_empty() {
        NOTIFICATION_CENTER_CARD_COMPACT_HEIGHT
    } else if body.chars().count() <= SINGLE_LINE_BODY_CHAR_LIMIT {
        NOTIFICATION_CENTER_CARD_HEIGHT
    } else {
        NOTIFICATION_CENTER_CARD_TALL_HEIGHT
    }
}

fn build_card(
    item: &NotificationSummary,
    group_index: usize,
    x: i32,
    y: i32,
    width: i32,
    dpi: u32,
    kind: NotificationCardKind,
) -> (NotificationCardLayout, i32) {
    let card_h = scale(card_height_points(item), dpi);
    let rect = Rect {
        x,
        y,
        width,
        height: card_h,
    };
    let lip_step = scale(NOTIFICATION_CENTER_STACK_LIP_STEP, dpi);
    let mut lips = Vec::new();
    let total_lips = match kind {
        NotificationCardKind::CollapsedStack { total_count, .. } => {
            usize::from(total_count >= 2) + usize::from(total_count >= 3)
        }
        NotificationCardKind::Single | NotificationCardKind::ExpandedItem => 0,
    };
    for idx in 1..=total_lips {
        let inset = scale(NOTIFICATION_CENTER_CARD_RADIUS * idx as i32, dpi);
        let lip_h = card_h.min(scale(18, dpi));
        lips.push(Rect {
            x: x + inset,
            y: rect.bottom() + lip_step * idx as i32 - lip_h,
            width: (width - inset * 2).max(1),
            height: lip_h,
        });
    }
    let total_height = card_h + lip_step * total_lips as i32;
    let card = NotificationCardLayout {
        id: item.id,
        group_index,
        app_name: item.app_name.clone(),
        title: item.title.clone(),
        body: item.body.clone(),
        timestamp_label: item.timestamp_label.clone(),
        icon_bytes: item.icon_bytes.clone(),
        rect,
        close_rect: close_rect_for_card(rect, dpi),
        lips,
        kind,
    };
    (card, total_height)
}

fn state_copy(
    access: NotificationAccess,
) -> (String, String, Option<String>, NotificationCenterHit) {
    match access {
        NotificationAccess::Allowed => (
            String::from("No Notifications"),
            String::from("Recent alerts from Windows apps will appear here."),
            None,
            NotificationCenterHit::None,
        ),
        NotificationAccess::Denied => (
            String::from("Notification Access Denied"),
            String::from("Allow Otto OS to read Windows notifications to see alerts here."),
            Some(String::from("Check Permission")),
            NotificationCenterHit::AccessAction,
        ),
        NotificationAccess::RequiresPackageIdentity => (
            String::from("Requires Packaged App"),
            String::from("Windows limits notification feed access to packaged MSIX builds."),
            None,
            NotificationCenterHit::None,
        ),
        NotificationAccess::Unavailable => (
            String::from("Notifications Unavailable"),
            String::from("Turn on Windows notification access or open the system feed."),
            Some(String::from("Enable Notification Feed")),
            NotificationCenterHit::AccessAction,
        ),
    }
}

#[must_use]
#[expect(clippy::too_many_lines)]
pub fn compute_notification_center_layout(
    access: NotificationAccess,
    groups: &[NotificationGroup],
    bounds: Rect,
    dpi: u32,
    anchor_right: i32,
    anchor_y: i32,
    scroll_offset: i32,
) -> NotificationCenterLayout {
    let margin = scale(NOTIFICATION_CENTER_SCREEN_MARGIN, dpi);
    let pad = scale(NOTIFICATION_CENTER_PAD, dpi);
    let header_h = scale(NOTIFICATION_CENTER_HEADER_HEIGHT, dpi);
    let footer_h = scale(NOTIFICATION_CENTER_FOOTER_HEIGHT, dpi);
    let gap = scale(NOTIFICATION_CENTER_CARD_GAP, dpi);
    let width = scale(NOTIFICATION_CENTER_WIDTH, dpi).min((bounds.width - margin * 2).max(120));
    let inner_w = (width - pad * 2).max(1);

    let has_items = access == NotificationAccess::Allowed && !groups.is_empty();
    let header_rect = Rect {
        x: pad,
        y: pad,
        width: inner_w,
        height: header_h,
    };
    let clear_w = scale(64, dpi);
    let clear_h = scale(22, dpi);
    let clear_all_rect = has_items.then_some(Rect {
        x: (header_rect.right() - clear_w).max(header_rect.x),
        y: header_rect.y + (header_h - clear_h) / 2,
        width: clear_w,
        height: clear_h,
    });
    let title_right = clear_all_rect.map_or(header_rect.right(), |r| r.x - gap);
    let title_rect = Rect {
        x: header_rect.x + scale(4, dpi),
        y: header_rect.y,
        width: (title_right - (header_rect.x + scale(4, dpi))).max(1),
        height: header_h,
    };

    let mut content_h = 0;
    let group_h = scale(NOTIFICATION_CENTER_GROUP_HEADER_HEIGHT, dpi);
    let empty_h = scale(NOTIFICATION_CENTER_EMPTY_HEIGHT, dpi);
    if has_items {
        for (idx, group) in groups.iter().enumerate() {
            if idx > 0 {
                content_h += gap;
            }
            if group.items.len() > 1 && group.expanded {
                content_h += group_h + scale(4, dpi);
                for (item_idx, item) in group.items.iter().enumerate() {
                    if item_idx > 0 {
                        content_h += scale(6, dpi);
                    }
                    let (_, h) = build_card(
                        item,
                        idx,
                        pad,
                        0,
                        inner_w,
                        dpi,
                        NotificationCardKind::ExpandedItem,
                    );
                    content_h += h;
                }
            } else if let Some(first) = group.items.first() {
                let kind = if group.items.len() > 1 {
                    NotificationCardKind::CollapsedStack {
                        total_count: group.items.len(),
                    }
                } else {
                    NotificationCardKind::Single
                };
                let (_, h) = build_card(first, idx, pad, 0, inner_w, dpi, kind);
                content_h += h;
            }
        }
    } else {
        content_h = empty_h;
    }

    let max_win_h = (bounds.height - margin * 2)
        .min(scale(NOTIFICATION_CENTER_MAX_HEIGHT, dpi))
        .max(pad * 2 + header_h + footer_h + gap * 2 + 1);
    let chrome_h = pad * 2 + header_h + footer_h + gap * 2;
    let available_viewport_h = (max_win_h - chrome_h).max(1);
    let viewport_h = content_h.min(available_viewport_h);
    let height = chrome_h + viewport_h;
    let clamped_scroll = scroll_offset.clamp(0, (content_h - viewport_h).max(0));

    let viewport = Rect {
        x: 0,
        y: pad + header_h + gap,
        width,
        height: viewport_h,
    };

    let mut y = viewport.y - clamped_scroll;
    let mut group_headers = Vec::new();
    let mut cards = Vec::new();
    let mut state_card = None;

    if has_items {
        for (idx, group) in groups.iter().enumerate() {
            if idx > 0 {
                y += gap;
            }
            if group.items.len() > 1 && group.expanded {
                let pill_h = scale(20, dpi);
                let clear_pill_w = scale(46, dpi);
                let collapse_pill_w = scale(72, dpi);
                let pill_gap = scale(4, dpi);
                let pill_y = y + (group_h - pill_h) / 2;
                let clear_rect = Rect {
                    x: (pad + inner_w - clear_pill_w).max(pad),
                    y: pill_y,
                    width: clear_pill_w,
                    height: pill_h,
                };
                let collapse_rect = Rect {
                    x: (clear_rect.x - pill_gap - collapse_pill_w).max(pad),
                    y: pill_y,
                    width: collapse_pill_w,
                    height: pill_h,
                };
                group_headers.push(NotificationGroupHeaderLayout {
                    group_index: idx,
                    app_name: group.app_name.clone(),
                    count: group.items.len(),
                    rect: Rect {
                        x: pad,
                        y,
                        width: inner_w,
                        height: group_h,
                    },
                    collapse_rect,
                    clear_rect,
                });
                y += group_h + scale(4, dpi);
                for (item_idx, item) in group.items.iter().enumerate() {
                    if item_idx > 0 {
                        y += scale(6, dpi);
                    }
                    let (card, h) = build_card(
                        item,
                        idx,
                        pad,
                        y,
                        inner_w,
                        dpi,
                        NotificationCardKind::ExpandedItem,
                    );
                    cards.push(card);
                    y += h;
                }
            } else if let Some(first) = group.items.first() {
                let kind = if group.items.len() > 1 {
                    NotificationCardKind::CollapsedStack {
                        total_count: group.items.len(),
                    }
                } else {
                    NotificationCardKind::Single
                };
                let (card, h) = build_card(first, idx, pad, y, inner_w, dpi, kind);
                cards.push(card);
                y += h;
            }
        }
    } else {
        let (title, subtitle, action_label, action_hit) = state_copy(access);
        let rect = Rect {
            x: pad,
            y,
            width: inner_w,
            height: empty_h,
        };
        let action_rect = action_label.as_ref().map(|_| {
            let btn_w = scale(180, dpi).min(inner_w - scale(24, dpi));
            let btn_h = scale(26, dpi);
            Rect {
                x: rect.x + (rect.width - btn_w) / 2,
                y: rect.bottom() - scale(14, dpi) - btn_h,
                width: btn_w,
                height: btn_h,
            }
        });
        state_card = Some(NotificationCenterStateCardLayout {
            rect,
            action_rect,
            title,
            subtitle,
            action_label,
            action_hit,
        });
    }

    let footer_rect = Rect {
        x: pad,
        y: viewport.bottom() + gap,
        width: inner_w,
        height: footer_h,
    };
    let footer_button_rect = footer_rect;

    let min_x = bounds.x + margin;
    let min_y = bounds.y + margin;
    let win_x = (anchor_right - width).clamp(min_x, (bounds.right() - width - margin).max(min_x));
    let win_y = anchor_y.clamp(min_y, (bounds.bottom() - height - margin).max(min_y));

    NotificationCenterLayout {
        dpi,
        window: Rect {
            x: win_x,
            y: win_y,
            width,
            height,
        },
        viewport,
        header_rect,
        title_rect,
        clear_all_rect,
        footer_rect,
        footer_button_rect,
        content_height: content_h,
        scroll_offset: clamped_scroll,
        shell_radius: scale(NOTIFICATION_CENTER_SHELL_RADIUS, dpi) as f32,
        card_radius: scale(NOTIFICATION_CENTER_CARD_RADIUS, dpi) as f32,
        pill_radius: scale(NOTIFICATION_CENTER_PILL_RADIUS, dpi) as f32,
        group_headers,
        cards,
        state_card,
    }
}

impl NotificationCenterLayout {
    #[must_use]
    pub const fn max_scroll(&self) -> i32 {
        let overflow = self.content_height - self.viewport.height;
        if overflow > 0 { overflow } else { 0 }
    }

    #[must_use]
    pub const fn is_scrollable(&self) -> bool {
        self.max_scroll() > 0
    }

    #[must_use]
    pub fn scrolled_by(&self, delta: i32) -> i32 {
        (self.scroll_offset + delta).clamp(0, self.max_scroll())
    }

    #[must_use]
    pub fn is_rect_visible(&self, rect: Rect) -> bool {
        rect.bottom() > self.viewport.y && rect.y < self.viewport.bottom()
    }

    #[must_use]
    pub fn hit_test(&self, local_x: i32, local_y: i32) -> NotificationCenterHit {
        if let Some(clear) = self.clear_all_rect
            && clear.contains(local_x, local_y)
        {
            return NotificationCenterHit::ClearAll;
        }
        if self.footer_button_rect.contains(local_x, local_y) {
            return NotificationCenterHit::OpenWindowsNotifications;
        }
        if !self.viewport.contains(local_x, local_y) {
            return NotificationCenterHit::None;
        }
        for header in &self.group_headers {
            if header.collapse_rect.contains(local_x, local_y) {
                return NotificationCenterHit::CollapseGroup(header.group_index);
            }
            if header.clear_rect.contains(local_x, local_y) {
                return NotificationCenterHit::ClearGroup(header.group_index);
            }
        }
        for card in &self.cards {
            if !self.is_rect_visible(card.rect) {
                continue;
            }
            if card.close_rect.contains(local_x, local_y) {
                return match card.kind {
                    NotificationCardKind::CollapsedStack { .. } => {
                        NotificationCenterHit::ClearGroup(card.group_index)
                    }
                    NotificationCardKind::Single | NotificationCardKind::ExpandedItem => {
                        NotificationCenterHit::CloseCard(card.id)
                    }
                };
            }
            if card.rect.contains(local_x, local_y) {
                return match card.kind {
                    NotificationCardKind::CollapsedStack { .. } => {
                        NotificationCenterHit::ExpandGroup(card.group_index)
                    }
                    NotificationCardKind::Single | NotificationCardKind::ExpandedItem => {
                        NotificationCenterHit::OpenCard(card.id)
                    }
                };
            }
        }
        if let Some(state) = &self.state_card
            && let Some(action_rect) = state.action_rect
            && action_rect.contains(local_x, local_y)
        {
            return state.action_hit;
        }
        NotificationCenterHit::None
    }

    #[must_use]
    pub fn interactive_hits(&self) -> Vec<NotificationCenterHit> {
        let mut hits = Vec::new();
        if self.clear_all_rect.is_some() {
            hits.push(NotificationCenterHit::ClearAll);
        }
        for header in &self.group_headers {
            hits.push(NotificationCenterHit::CollapseGroup(header.group_index));
            hits.push(NotificationCenterHit::ClearGroup(header.group_index));
        }
        for card in &self.cards {
            match card.kind {
                NotificationCardKind::CollapsedStack { .. } => {
                    hits.push(NotificationCenterHit::ExpandGroup(card.group_index));
                    hits.push(NotificationCenterHit::ClearGroup(card.group_index));
                }
                NotificationCardKind::Single | NotificationCardKind::ExpandedItem => {
                    hits.push(NotificationCenterHit::OpenCard(card.id));
                    hits.push(NotificationCenterHit::CloseCard(card.id));
                }
            }
        }
        if let Some(state) = &self.state_card
            && state.action_rect.is_some()
        {
            hits.push(state.action_hit);
        }
        hits.push(NotificationCenterHit::OpenWindowsNotifications);
        hits
    }

    #[must_use]
    pub fn next_hit(&self, current: NotificationCenterHit) -> NotificationCenterHit {
        let hits = self.interactive_hits();
        let Some(pos) = hits.iter().position(|&item| item == current) else {
            return hits.first().copied().unwrap_or(NotificationCenterHit::None);
        };
        hits[(pos + 1) % hits.len()]
    }

    #[must_use]
    pub fn prev_hit(&self, current: NotificationCenterHit) -> NotificationCenterHit {
        let hits = self.interactive_hits();
        let Some(pos) = hits.iter().position(|&item| item == current) else {
            return hits.last().copied().unwrap_or(NotificationCenterHit::None);
        };
        hits[(pos + hits.len() - 1) % hits.len()]
    }

    #[must_use]
    pub fn target_rect(&self, hit: NotificationCenterHit) -> Option<Rect> {
        match hit {
            NotificationCenterHit::None => None,
            NotificationCenterHit::ClearAll => self.clear_all_rect,
            NotificationCenterHit::OpenWindowsNotifications => Some(self.footer_button_rect),
            NotificationCenterHit::AccessAction => {
                self.state_card.as_ref().and_then(|s| s.action_rect)
            }
            NotificationCenterHit::CollapseGroup(group_index) => self
                .group_headers
                .iter()
                .find(|h| h.group_index == group_index)
                .map(|h| h.collapse_rect),
            NotificationCenterHit::ClearGroup(group_index) => self
                .group_headers
                .iter()
                .find(|h| h.group_index == group_index)
                .map(|h| h.clear_rect)
                .or_else(|| {
                    self.cards
                        .iter()
                        .find(|c| {
                            c.group_index == group_index
                                && matches!(c.kind, NotificationCardKind::CollapsedStack { .. })
                        })
                        .map(|c| c.close_rect)
                }),
            NotificationCenterHit::ExpandGroup(group_index) => self
                .cards
                .iter()
                .find(|c| {
                    c.group_index == group_index
                        && matches!(c.kind, NotificationCardKind::CollapsedStack { .. })
                })
                .map(|c| c.rect),
            NotificationCenterHit::OpenCard(id) => {
                self.cards.iter().find(|c| c.id == id).map(|c| c.rect)
            }
            NotificationCenterHit::CloseCard(id) => {
                self.cards.iter().find(|c| c.id == id).map(|c| c.close_rect)
            }
        }
    }

    #[must_use]
    pub fn scroll_revealing(&self, hit: NotificationCenterHit) -> i32 {
        let Some(rect) = self.target_rect(hit) else {
            return self.scroll_offset;
        };
        if rect.y < self.viewport.y && matches!(hit, NotificationCenterHit::ClearAll) {
            return 0;
        }
        if matches!(hit, NotificationCenterHit::OpenWindowsNotifications) {
            return self.scroll_offset;
        }
        let above = self.viewport.y - rect.y;
        let below = rect.bottom() - self.viewport.bottom();
        if above > 0 {
            self.scrolled_by(-above)
        } else if below > 0 {
            self.scrolled_by(below)
        } else {
            self.scroll_offset
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    fn summary(id: u32, app: &str, title: &str) -> NotificationSummary {
        NotificationSummary {
            id,
            app_id: format!("Contoso.{app}"),
            app_name: String::from(app),
            title: String::from(title),
            body: String::from("Preview message body"),
            timestamp_label: String::from("2m"),
            icon_bytes: None,
        }
    }

    #[test]
    fn groups_collapse_into_stacks_until_expanded_and_separate_close_from_open() {
        let items = vec![
            summary(1, "Mail", "First"),
            summary(2, "Mail", "Second"),
            summary(3, "Mail", "Third"),
            summary(4, "Chat", "Ping"),
        ];
        let mut expanded = BTreeSet::new();
        let collapsed_groups = group_notifications(&items, &expanded);
        assert_eq!(collapsed_groups.len(), 2);
        assert_eq!(collapsed_groups[0].items.len(), 3);
        assert!(!collapsed_groups[0].expanded);

        let collapsed = compute_notification_center_layout(
            NotificationAccess::Allowed,
            &collapsed_groups,
            SCREEN,
            96,
            1880,
            36,
            0,
        );
        assert_eq!(collapsed.cards.len(), 2);
        assert_eq!(collapsed.cards[0].lips.len(), 2);
        assert!(matches!(
            collapsed.cards[0].kind,
            NotificationCardKind::CollapsedStack { total_count: 3 }
        ));
        assert!(matches!(
            collapsed.cards[1].kind,
            NotificationCardKind::Single
        ));

        let stack_card = &collapsed.cards[0];
        assert_eq!(
            collapsed.hit_test(stack_card.rect.x + 20, stack_card.rect.y + 30),
            NotificationCenterHit::ExpandGroup(0)
        );
        assert_eq!(
            collapsed.hit_test(
                stack_card.close_rect.x + stack_card.close_rect.width / 2,
                stack_card.close_rect.y + stack_card.close_rect.height / 2,
            ),
            NotificationCenterHit::ClearGroup(0)
        );
        let single_card = &collapsed.cards[1];
        assert_eq!(
            collapsed.hit_test(single_card.rect.x + 20, single_card.rect.y + 30),
            NotificationCenterHit::OpenCard(4)
        );
        assert_eq!(
            collapsed.hit_test(
                single_card.close_rect.x + single_card.close_rect.width / 2,
                single_card.close_rect.y + single_card.close_rect.height / 2,
            ),
            NotificationCenterHit::CloseCard(4)
        );

        expanded.insert(String::from("contoso.mail"));
        let expanded_groups = group_notifications(&items, &expanded);
        let expanded_layout = compute_notification_center_layout(
            NotificationAccess::Allowed,
            &expanded_groups,
            SCREEN,
            96,
            1880,
            36,
            0,
        );
        assert_eq!(expanded_layout.group_headers.len(), 1);
        assert_eq!(expanded_layout.cards.len(), 4);
        let header = &expanded_layout.group_headers[0];
        assert_eq!(
            expanded_layout.hit_test(header.collapse_rect.x + 8, header.collapse_rect.y + 8),
            NotificationCenterHit::CollapseGroup(0)
        );
        assert_eq!(
            expanded_layout.hit_test(header.clear_rect.x + 8, header.clear_rect.y + 8),
            NotificationCenterHit::ClearGroup(0)
        );
    }

    #[test]
    fn concentric_radii_and_hit_targets_hold_at_all_dpis() {
        let items = vec![summary(1, "Mail", "One"), summary(2, "Mail", "Two")];
        let groups = group_notifications(&items, &BTreeSet::new());
        for dpi in [96, 120, 144, 192] {
            let layout = compute_notification_center_layout(
                NotificationAccess::Allowed,
                &groups,
                SCREEN,
                dpi,
                1880,
                36,
                0,
            );
            let pad = scale(NOTIFICATION_CENTER_PAD, dpi) as f32;
            assert!((layout.shell_radius - pad - layout.card_radius).abs() < 0.01);
            let card = &layout.cards[0];
            assert!(card.close_rect.width >= scale(24, dpi));
            assert!(card.close_rect.height >= scale(24, dpi));
            assert!(
                layout.window.right()
                    <= SCREEN.right() - scale(NOTIFICATION_CENTER_SCREEN_MARGIN, dpi)
            );
        }
    }

    #[test]
    fn empty_and_access_states_render_state_card_without_clear_all() {
        let empty = compute_notification_center_layout(
            NotificationAccess::Allowed,
            &[],
            SCREEN,
            96,
            1880,
            36,
            0,
        );
        assert!(empty.clear_all_rect.is_none());
        let state = empty.state_card.as_ref().expect("empty state card");
        assert_eq!(state.title, "No Notifications");
        assert!(state.action_rect.is_none());

        let denied = compute_notification_center_layout(
            NotificationAccess::Denied,
            &[],
            SCREEN,
            96,
            1880,
            36,
            0,
        );
        let denied_card = denied.state_card.as_ref().expect("denied state card");
        let action_rect = denied_card.action_rect.expect("action rect");
        assert_eq!(
            denied.hit_test(action_rect.x + 10, action_rect.y + 10),
            NotificationCenterHit::AccessAction
        );
    }

    #[test]
    fn adaptive_card_heights_stay_compact_without_body_and_stack_lips_align_cleanly() {
        let items = vec![
            NotificationSummary {
                id: 10,
                app_id: String::from("WhatsApp"),
                app_name: String::from("WhatsApp"),
                title: String::from("You may have new messages"),
                body: String::new(),
                timestamp_label: String::from("4m"),
                icon_bytes: None,
            },
            NotificationSummary {
                id: 11,
                app_id: String::from("Chrome"),
                app_name: String::from("Google Chrome"),
                title: String::from("Listiarso Wastuargo"),
                body: String::from(
                    "Kenapa War Tiket Konser Selalu Chaos? Sistem di Baliknya (Coldplay, Ye GBK)",
                ),
                timestamp_label: String::from("2d"),
                icon_bytes: None,
            },
            NotificationSummary {
                id: 12,
                app_id: String::from("Chrome"),
                app_name: String::from("Google Chrome"),
                title: String::from("Earlier alert"),
                body: String::from("Short body"),
                timestamp_label: String::from("3d"),
                icon_bytes: None,
            },
        ];
        let groups = group_notifications(&items, &BTreeSet::new());
        let layout = compute_notification_center_layout(
            NotificationAccess::Allowed,
            &groups,
            SCREEN,
            96,
            1880,
            36,
            0,
        );
        assert_eq!(layout.window.width, NOTIFICATION_CENTER_WIDTH);
        assert_eq!(
            layout.cards[0].rect.height,
            NOTIFICATION_CENTER_CARD_COMPACT_HEIGHT
        );
        assert_eq!(
            layout.cards[1].rect.height,
            NOTIFICATION_CENTER_CARD_TALL_HEIGHT
        );
        assert!(layout.cards[0].rect.height < layout.cards[1].rect.height);
        let lip = layout.cards[1].lips[0];
        assert!(lip.x >= layout.cards[1].rect.x + NOTIFICATION_CENTER_CARD_RADIUS);
        assert_eq!(
            lip.bottom(),
            layout.cards[1].rect.bottom() + NOTIFICATION_CENTER_STACK_LIP_STEP
        );
    }
}
