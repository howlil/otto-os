use crate::layout::{Rect, scale};
use crate::window::controls::WindowControlAction;

use super::layout::{
    FinderBreadcrumbSlot, FinderLayout, FinderLayoutInput, FinderSidebarHeadingSlot,
    FinderSidebarSlot, FinderTabSlot, FinderToolbarButton, FinderToolbarSlot,
    FinderWindowControlSlot,
};
use super::model::{
    FinderFlags, FinderLocation, FinderSidebarSection, FinderTab, FinderViewMode,
    breadcrumbs_for_location,
};

pub(super) fn build_toolbar_layout(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
    sidebar_w: i32,
) {
    let dpi = layout.dpi;
    let th = layout.toolbar.height;
    let btn_h = scale(28, dpi);
    let btn_w = scale(30, dpi);
    let btn_y = (th - btn_h) / 2;
    let nav_x = build_toolbar_left(layout, input, tab, sidebar_w, btn_y, btn_w, btn_h);
    let seg_x = build_toolbar_right(layout, input, tab, btn_y, btn_w, btn_h);
    let title_x = nav_x + btn_w * 2 + scale(14, dpi);
    let title_w = (seg_x - scale(12, dpi) - title_x).max(0);
    layout.title_rect = Rect::new(title_x, btn_y, title_w, btn_h);
}

fn build_toolbar_left(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
    sidebar_w: i32,
    btn_y: i32,
    btn_w: i32,
    btn_h: i32,
) -> i32 {
    let dpi = layout.dpi;
    let th = layout.toolbar.height;
    let dot = scale(12, dpi);
    let dot_gap = scale(8, dpi);
    let start_x = scale(18, dpi);
    let dot_y = (th - dot) / 2;
    let actions = [
        WindowControlAction::Close,
        WindowControlAction::Minimize,
        WindowControlAction::ToggleMaximize,
    ];
    for (idx, action) in actions.into_iter().enumerate() {
        let x = start_x + (idx as i32) * (dot + dot_gap);
        let pad = scale(2, dpi);
        layout.window_controls.push(FinderWindowControlSlot {
            action,
            rect: Rect::new(x - pad, dot_y - pad, dot + pad * 2, dot + pad * 2),
        });
    }
    let sidebar_btn_x = if sidebar_w > 0 {
        (sidebar_w - btn_w - scale(12, dpi)).max(start_x + scale(68, dpi))
    } else {
        start_x + scale(68, dpi)
    };
    layout.toolbar_buttons.push(FinderToolbarSlot {
        button: FinderToolbarButton::ToggleSidebar,
        rect: Rect::new(sidebar_btn_x, btn_y, btn_w, btn_h),
        active: input.flags.has(FinderFlags::SIDEBAR),
        enabled: true,
    });
    let nav_x = sidebar_w.max(sidebar_btn_x + btn_w + scale(8, dpi)) + scale(12, dpi);
    layout.toolbar_buttons.push(FinderToolbarSlot {
        button: FinderToolbarButton::Back,
        rect: Rect::new(nav_x, btn_y, btn_w, btn_h),
        active: false,
        enabled: !tab.history_back.is_empty(),
    });
    layout.toolbar_buttons.push(FinderToolbarSlot {
        button: FinderToolbarButton::Forward,
        rect: Rect::new(nav_x + btn_w + scale(2, dpi), btn_y, btn_w, btn_h),
        active: false,
        enabled: !tab.history_forward.is_empty(),
    });
    nav_x
}

fn build_toolbar_right(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
    btn_y: i32,
    btn_w: i32,
    btn_h: i32,
) -> i32 {
    let dpi = layout.dpi;
    let right_pad = scale(14, dpi);
    let search_w = scale(196, dpi).min((layout.width / 4).max(scale(128, dpi)));
    let search_x = layout.width - right_pad - search_w;
    layout.search_box = Rect::new(search_x, btn_y, search_w, btn_h);
    if !tab.search_query.is_empty() {
        let cw = scale(18, dpi);
        layout.search_clear = Some(Rect::new(
            layout.search_box.right() - cw - scale(5, dpi),
            btn_y + (btn_h - cw) / 2,
            cw,
            cw,
        ));
        let pw = scale(52, dpi);
        layout.search_scope_pill = Some(Rect::new(
            layout.search_box.right() - cw - pw - scale(9, dpi),
            btn_y + scale(4, dpi),
            pw,
            btn_h - scale(8, dpi),
        ));
    }
    let gap = scale(6, dpi);
    let insp_x = search_x - gap - btn_w;
    layout.toolbar_buttons.push(FinderToolbarSlot {
        button: FinderToolbarButton::ToggleInspector,
        rect: Rect::new(insp_x, btn_y, btn_w, btn_h),
        active: input.flags.has(FinderFlags::INSPECTOR),
        enabled: true,
    });
    let action_x = insp_x - gap - btn_w;
    layout.toolbar_buttons.push(FinderToolbarSlot {
        button: FinderToolbarButton::ActionMenu,
        rect: Rect::new(action_x, btn_y, btn_w, btn_h),
        active: false,
        enabled: true,
    });
    let sort_x = action_x - gap - btn_w;
    layout.toolbar_buttons.push(FinderToolbarSlot {
        button: FinderToolbarButton::SortMenu,
        rect: Rect::new(sort_x, btn_y, btn_w, btn_h),
        active: false,
        enabled: true,
    });
    let seg_w = scale(30, dpi);
    let seg_total = seg_w * 4;
    let seg_x = sort_x - scale(14, dpi) - seg_total;
    for (idx, mode) in FinderViewMode::ALL.into_iter().enumerate() {
        layout.toolbar_buttons.push(FinderToolbarSlot {
            button: FinderToolbarButton::ViewMode(mode),
            rect: Rect::new(seg_x + (idx as i32) * seg_w, btn_y, seg_w, btn_h),
            active: tab.view_mode == mode,
            enabled: true,
        });
    }
    seg_x
}

pub(super) fn build_sidebar_layout(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
) {
    if layout.sidebar.width <= 0 {
        return;
    }
    let dpi = layout.dpi;
    let pad = scale(10, dpi);
    let row_h = scale(28, dpi);
    let hdr_h = scale(22, dpi);
    let gap = scale(2, dpi);
    let section_gap = scale(10, dpi);
    let row_w = (layout.sidebar.width - pad * 2).max(1);
    let mut y = layout.sidebar.y + scale(8, dpi);
    let mut current_section: Option<FinderSidebarSection> = None;

    for (index, item) in input.sidebar_items.iter().enumerate() {
        if current_section != Some(item.section) {
            if current_section.is_some() {
                y += section_gap;
            }
            current_section = Some(item.section);
            layout.sidebar_headings.push(FinderSidebarHeadingSlot {
                section: item.section,
                rect: Rect::new(pad + scale(6, dpi), y, row_w - scale(6, dpi), hdr_h),
            });
            y += hdr_h;
        }
        let rect = Rect::new(pad, y, row_w, row_h);
        if rect.bottom() <= layout.sidebar.bottom() + row_h {
            layout.sidebar_slots.push(FinderSidebarSlot {
                index,
                rect,
                active: tab.location == item.location,
            });
        }
        y += row_h + gap;
    }
}

pub(super) fn build_tabs_layout(
    layout: &mut FinderLayout,
    tabs: &[FinderTab],
    active_tab: usize,
    dpi: u32,
) {
    let add_w = scale(28, dpi);
    let avail_w = (layout.tab_bar.width - add_w).max(1);
    let count = tabs.len().max(1) as i32;
    let each_w = (avail_w / count).max(scale(88, dpi));
    let close_sz = scale(16, dpi);
    for (index, _) in tabs.iter().enumerate() {
        let x = layout.tab_bar.x + (index as i32) * each_w;
        let w = if index + 1 == tabs.len() {
            (layout.tab_bar.x + avail_w - x).max(1)
        } else {
            each_w
        };
        let rect = Rect::new(x, layout.tab_bar.y, w, layout.tab_bar.height);
        let close_rect = Rect::new(
            rect.x + scale(8, dpi),
            rect.y + (rect.height - close_sz) / 2,
            close_sz,
            close_sz,
        );
        layout.tab_slots.push(FinderTabSlot {
            index,
            rect,
            close_rect,
            active: index == active_tab,
        });
    }
    layout.tab_add_button = Rect::new(
        layout.tab_bar.right() - add_w,
        layout.tab_bar.y,
        add_w,
        layout.tab_bar.height,
    );
}

pub(super) fn build_breadcrumbs_layout(
    layout: &mut FinderLayout,
    location: &FinderLocation,
    dpi: u32,
) {
    let crumbs = breadcrumbs_for_location(location);
    let mut x = layout.path_bar.x + scale(12, dpi);
    let char_w = scale(7, dpi);
    let sep_w = scale(16, dpi);
    for (index, breadcrumb) in crumbs.into_iter().enumerate() {
        let text_w = (breadcrumb.label.chars().count() as i32 * char_w + scale(14, dpi))
            .clamp(scale(28, dpi), scale(180, dpi));
        let rect = Rect::new(x, layout.path_bar.y, text_w, layout.path_bar.height);
        if rect.x < layout.path_bar.right() {
            layout.breadcrumbs.push(FinderBreadcrumbSlot {
                index,
                rect,
                breadcrumb,
            });
        }
        x += text_w + sep_w;
    }
}
