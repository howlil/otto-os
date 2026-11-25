use crate::layout::{Rect, scale};
use crate::window::controls::WindowControlAction;

use super::layout_chrome::{
    build_breadcrumbs_layout, build_sidebar_layout, build_tabs_layout, build_toolbar_layout,
};
use super::layout_views::{
    layout_column_view, layout_gallery_view, layout_go_to_overlay, layout_icon_view,
    layout_inspector, layout_list_view, layout_quick_look,
};
use super::model::{
    FINDER_LIST_HEADER_HEIGHT_POINTS, FINDER_SIDEBAR_MAX_POINTS, FINDER_SIDEBAR_MIN_POINTS,
    FinderBreadcrumb, FinderFlags, FinderLocation, FinderMetrics, FinderMillerColumn,
    FinderSidebarItem, FinderSidebarSection, FinderSortKey, FinderTab, FinderTag, FinderTreeRow,
    FinderViewMode,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderWindowControlSlot {
    pub action: WindowControlAction,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderToolbarButton {
    ToggleSidebar,
    Back,
    Forward,
    ViewMode(FinderViewMode),
    SortMenu,
    ActionMenu,
    ToggleInspector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderToolbarSlot {
    pub button: FinderToolbarButton,
    pub rect: Rect,
    pub active: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderSidebarHeadingSlot {
    pub section: FinderSidebarSection,
    pub rect: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderSidebarSlot {
    pub index: usize,
    pub rect: Rect,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderTabSlot {
    pub index: usize,
    pub rect: Rect,
    pub close_rect: Rect,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderBreadcrumbSlot {
    pub index: usize,
    pub rect: Rect,
    pub breadcrumb: FinderBreadcrumb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderColumnHeaderSlot {
    pub key: FinderSortKey,
    pub rect: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderListRowSlot {
    pub index: usize,
    pub path: String,
    pub rect: Rect,
    pub chevron_rect: Option<Rect>,
    pub name_rect: Rect,
    pub date_rect: Rect,
    pub size_rect: Rect,
    pub kind_rect: Rect,
    pub depth: u16,
    pub expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderIconTileSlot {
    pub index: usize,
    pub path: String,
    pub rect: Rect,
    pub icon_rect: Rect,
    pub label_rect: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderColumnRowSlot {
    pub col_index: usize,
    pub row_index: usize,
    pub path: String,
    pub rect: Rect,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderColumnPaneSlot {
    pub col_index: usize,
    pub rect: Rect,
    pub rows: Vec<FinderColumnRowSlot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderGalleryThumbSlot {
    pub index: usize,
    pub path: String,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderQuickLookLayout {
    pub backdrop: Rect,
    pub card: Rect,
    pub header: Rect,
    pub close_button: Rect,
    pub open_button: Rect,
    pub content: Rect,
    pub prev_button: Rect,
    pub next_button: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderInspectorLayout {
    pub panel: Rect,
    pub preview_rect: Rect,
    pub tag_rects: [(FinderTag, Rect); 7],
    pub open_button: Rect,
    pub copy_path_button: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderGoToLayout {
    pub card: Rect,
    pub input_rect: Rect,
    pub go_button: Rect,
    pub cancel_button: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderBannerLayout {
    pub rect: Rect,
    pub primary_button: Rect,
    pub secondary_button: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinderHit {
    WindowControl(WindowControlAction),
    Toolbar(FinderToolbarButton),
    SearchBox,
    SearchClear,
    SearchScopeToggle,
    SidebarItem(usize),
    SidebarResizeHandle,
    Tab(usize),
    TabClose(usize),
    TabAdd,
    ListColumnHeader(FinderSortKey),
    ListChevron(String),
    Item {
        path: String,
        col_index: Option<usize>,
    },
    EmptyContent,
    Breadcrumb(usize),
    IconSizeSlider,
    QuickLookClose,
    QuickLookOpen,
    QuickLookPrev,
    QuickLookNext,
    QuickLookBackdrop,
    InspectorTag(FinderTag),
    InspectorOpen,
    InspectorCopyPath,
    GoToGo,
    GoToCancel,
    GoToInput,
    BannerPrimary,
    BannerSecondary,
}

#[derive(Clone, Debug)]
pub struct FinderLayoutInput<'a> {
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub metrics: FinderMetrics,
    pub flags: FinderFlags,
    pub tabs: &'a [FinderTab],
    pub active_tab: usize,
    pub sidebar_items: &'a [FinderSidebarItem],
    pub list_rows: &'a [FinderTreeRow],
    pub visible_entries_count: usize,
    pub active_columns: &'a [FinderMillerColumn],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinderLayout {
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub toolbar: Rect,
    pub drag_strip: Rect,
    pub window_controls: Vec<FinderWindowControlSlot>,
    pub toolbar_buttons: Vec<FinderToolbarSlot>,
    pub title_rect: Rect,
    pub search_box: Rect,
    pub search_clear: Option<Rect>,
    pub search_scope_pill: Option<Rect>,
    pub sidebar: Rect,
    pub sidebar_resize_handle: Rect,
    pub sidebar_headings: Vec<FinderSidebarHeadingSlot>,
    pub sidebar_slots: Vec<FinderSidebarSlot>,
    pub tab_bar: Rect,
    pub tab_slots: Vec<FinderTabSlot>,
    pub tab_add_button: Rect,
    pub banner: Option<FinderBannerLayout>,
    pub content_stage: Rect,
    pub list_header: Rect,
    pub list_column_headers: Vec<FinderColumnHeaderSlot>,
    pub list_rows: Vec<FinderListRowSlot>,
    pub icon_tiles: Vec<FinderIconTileSlot>,
    pub column_panes: Vec<FinderColumnPaneSlot>,
    pub column_preview_rect: Option<Rect>,
    pub gallery_stage_rect: Option<Rect>,
    pub gallery_strip_rect: Option<Rect>,
    pub gallery_thumbs: Vec<FinderGalleryThumbSlot>,
    pub inspector: Option<FinderInspectorLayout>,
    pub quick_look: Option<FinderQuickLookLayout>,
    pub go_to: Option<FinderGoToLayout>,
    pub path_bar: Rect,
    pub breadcrumbs: Vec<FinderBreadcrumbSlot>,
    pub status_bar: Rect,
    pub icon_size_slider: Option<Rect>,
    pub content_scroll_height: i32,
    pub content_scroll_width: i32,
}

impl FinderLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<FinderHit> {
        if let Some(go_to) = self.go_to {
            if go_to.go_button.contains(x, y) {
                return Some(FinderHit::GoToGo);
            }
            if go_to.cancel_button.contains(x, y) {
                return Some(FinderHit::GoToCancel);
            }
            if go_to.input_rect.contains(x, y) {
                return Some(FinderHit::GoToInput);
            }
            return Some(FinderHit::GoToCancel);
        }
        if let Some(ql) = self.quick_look {
            if ql.close_button.contains(x, y) {
                return Some(FinderHit::QuickLookClose);
            }
            if ql.open_button.contains(x, y) {
                return Some(FinderHit::QuickLookOpen);
            }
            if ql.prev_button.contains(x, y) {
                return Some(FinderHit::QuickLookPrev);
            }
            if ql.next_button.contains(x, y) {
                return Some(FinderHit::QuickLookNext);
            }
            if !ql.card.contains(x, y) {
                return Some(FinderHit::QuickLookBackdrop);
            }
            return None;
        }
        self.hit_test_chrome(x, y)
            .or_else(|| self.hit_test_inspector(x, y))
            .or_else(|| self.hit_test_content(x, y))
    }

    fn hit_test_chrome(&self, x: i32, y: i32) -> Option<FinderHit> {
        for slot in &self.window_controls {
            if slot.rect.contains(x, y) {
                return Some(FinderHit::WindowControl(slot.action));
            }
        }
        if let Some(clear) = self.search_clear
            && clear.contains(x, y)
        {
            return Some(FinderHit::SearchClear);
        }
        if let Some(scope) = self.search_scope_pill
            && scope.contains(x, y)
        {
            return Some(FinderHit::SearchScopeToggle);
        }
        if self.search_box.contains(x, y) {
            return Some(FinderHit::SearchBox);
        }
        for slot in &self.toolbar_buttons {
            if slot.rect.contains(x, y) {
                return Some(FinderHit::Toolbar(slot.button));
            }
        }
        if self.sidebar_resize_handle.contains(x, y) {
            return Some(FinderHit::SidebarResizeHandle);
        }
        for slot in &self.sidebar_slots {
            if slot.rect.contains(x, y) {
                return Some(FinderHit::SidebarItem(slot.index));
            }
        }
        if self.tab_add_button.contains(x, y) {
            return Some(FinderHit::TabAdd);
        }
        for slot in &self.tab_slots {
            if slot.close_rect.contains(x, y) {
                return Some(FinderHit::TabClose(slot.index));
            }
            if slot.rect.contains(x, y) {
                return Some(FinderHit::Tab(slot.index));
            }
        }
        if let Some(banner) = self.banner {
            if banner.primary_button.contains(x, y) {
                return Some(FinderHit::BannerPrimary);
            }
            if banner.secondary_button.contains(x, y) {
                return Some(FinderHit::BannerSecondary);
            }
        }
        for crumb in &self.breadcrumbs {
            if crumb.rect.contains(x, y) {
                return Some(FinderHit::Breadcrumb(crumb.index));
            }
        }
        if let Some(slider) = self.icon_size_slider
            && slider.contains(x, y)
        {
            return Some(FinderHit::IconSizeSlider);
        }
        None
    }

    fn hit_test_inspector(&self, x: i32, y: i32) -> Option<FinderHit> {
        let inspector = self.inspector?;
        if !inspector.panel.contains(x, y) {
            return None;
        }
        for (tag, rect) in inspector.tag_rects {
            if rect.contains(x, y) {
                return Some(FinderHit::InspectorTag(tag));
            }
        }
        if inspector.open_button.contains(x, y) {
            return Some(FinderHit::InspectorOpen);
        }
        if inspector.copy_path_button.contains(x, y) {
            return Some(FinderHit::InspectorCopyPath);
        }
        None
    }

    fn hit_test_content(&self, x: i32, y: i32) -> Option<FinderHit> {
        for header in &self.list_column_headers {
            if header.rect.contains(x, y) {
                return Some(FinderHit::ListColumnHeader(header.key));
            }
        }
        if !self.content_stage.contains(x, y) {
            return None;
        }
        for row in &self.list_rows {
            if let Some(chevron) = row.chevron_rect
                && chevron.contains(x, y)
            {
                return Some(FinderHit::ListChevron(row.path.clone()));
            }
            if row.rect.contains(x, y) {
                return Some(FinderHit::Item {
                    path: row.path.clone(),
                    col_index: None,
                });
            }
        }
        for tile in &self.icon_tiles {
            if tile.rect.contains(x, y) {
                return Some(FinderHit::Item {
                    path: tile.path.clone(),
                    col_index: None,
                });
            }
        }
        for pane in &self.column_panes {
            if pane.rect.contains(x, y) {
                for row in &pane.rows {
                    if row.rect.contains(x, y) {
                        return Some(FinderHit::Item {
                            path: row.path.clone(),
                            col_index: Some(pane.col_index),
                        });
                    }
                }
            }
        }
        for thumb in &self.gallery_thumbs {
            if thumb.rect.contains(x, y) {
                return Some(FinderHit::Item {
                    path: thumb.path.clone(),
                    col_index: None,
                });
            }
        }
        Some(FinderHit::EmptyContent)
    }

    #[must_use]
    pub fn is_caption_drag_point(&self, x: i32, y: i32) -> bool {
        self.drag_strip.contains(x, y) && self.hit_test(x, y).is_none()
    }
}

#[must_use]
pub fn compute_finder_layout(input: &FinderLayoutInput<'_>) -> FinderLayout {
    let dpi = input.dpi.max(96);
    let width = input.width.max(1);
    let height = input.height.max(1);
    let mut layout = FinderLayout {
        width,
        height,
        dpi,
        ..FinderLayout::default()
    };
    let Some(tab) = input.tabs.get(input.active_tab) else {
        return layout;
    };
    let toolbar_h = scale(input.metrics.toolbar_height, dpi);
    layout.toolbar = Rect::new(0, 0, width, toolbar_h);
    layout.drag_strip = layout.toolbar;

    let sidebar_w = if input.flags.has(FinderFlags::SIDEBAR) {
        scale(
            input
                .metrics
                .sidebar_width
                .clamp(FINDER_SIDEBAR_MIN_POINTS, FINDER_SIDEBAR_MAX_POINTS),
            dpi,
        )
        .min((width * 2) / 5)
    } else {
        0
    };
    layout.sidebar = Rect::new(0, toolbar_h, sidebar_w, (height - toolbar_h).max(0));
    if sidebar_w > 0 {
        let handle_w = scale(6, dpi);
        layout.sidebar_resize_handle = Rect::new(
            sidebar_w - handle_w / 2,
            toolbar_h,
            handle_w,
            layout.sidebar.height,
        );
    }

    build_toolbar_layout(&mut layout, input, tab, sidebar_w);
    build_sidebar_layout(&mut layout, input, tab);

    let main_x = sidebar_w;
    let main_w = (width - sidebar_w).max(1);
    let mut top_y = toolbar_h;

    if input.tabs.len() > 1 {
        let tab_h = scale(input.metrics.tab_bar_height, dpi);
        layout.tab_bar = Rect::new(main_x, top_y, main_w, tab_h);
        build_tabs_layout(&mut layout, input.tabs, input.active_tab, dpi);
        top_y += tab_h;
    }

    let bottom_y = build_bottom_bars(&mut layout, input, tab, main_x, main_w, top_y);
    top_y = build_banner(&mut layout, input, tab, main_x, main_w, top_y);
    build_stage_and_overlays(&mut layout, input, tab, main_x, main_w, top_y, bottom_y);
    layout
}

fn build_bottom_bars(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
    main_x: i32,
    main_w: i32,
    top_y: i32,
) -> i32 {
    let dpi = layout.dpi;
    let status_h = if input.flags.has(FinderFlags::STATUS_BAR) {
        scale(input.metrics.status_bar_height, dpi)
    } else {
        0
    };
    let path_h = if input.flags.has(FinderFlags::PATH_BAR) {
        scale(input.metrics.path_bar_height, dpi)
    } else {
        0
    };
    let bottom_y = (layout.height - status_h - path_h).max(top_y);
    if path_h > 0 {
        layout.path_bar = Rect::new(main_x, bottom_y, main_w, path_h);
        build_breadcrumbs_layout(layout, &tab.location, dpi);
    }
    if status_h > 0 {
        layout.status_bar = Rect::new(main_x, bottom_y + path_h, main_w, status_h);
        if tab.view_mode == FinderViewMode::Icon {
            let sw = scale(88, dpi);
            let sh = scale(14, dpi);
            let sx = layout.status_bar.right() - sw - scale(14, dpi);
            let sy = layout.status_bar.y + (status_h - sh) / 2;
            layout.icon_size_slider = Some(Rect::new(sx, sy, sw, sh));
        }
    }
    bottom_y
}

fn build_banner(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
    main_x: i32,
    main_w: i32,
    top_y: i32,
) -> i32 {
    if !input.flags.has(FinderFlags::CONFIRM_PERM_DELETE) && tab.location != FinderLocation::Trash {
        return top_y;
    }
    let dpi = layout.dpi;
    let bh = scale(34, dpi);
    let banner_rect = Rect::new(main_x, top_y, main_w, bh);
    let btn_w = scale(108, dpi);
    let btn_h = scale(22, dpi);
    let btn_y = top_y + (bh - btn_h) / 2;
    let pad = scale(12, dpi);
    let gap = scale(8, dpi);
    let primary = Rect::new(banner_rect.right() - pad - btn_w, btn_y, btn_w, btn_h);
    let secondary = Rect::new(primary.x - gap - btn_w, btn_y, btn_w, btn_h);
    layout.banner = Some(FinderBannerLayout {
        rect: banner_rect,
        primary_button: primary,
        secondary_button: secondary,
    });
    top_y + bh
}

fn build_stage_and_overlays(
    layout: &mut FinderLayout,
    input: &FinderLayoutInput<'_>,
    tab: &FinderTab,
    main_x: i32,
    main_w: i32,
    top_y: i32,
    bottom_y: i32,
) {
    let dpi = layout.dpi;
    let show_inspector = input.flags.has(FinderFlags::INSPECTOR)
        || (tab.view_mode == FinderViewMode::Gallery && input.flags.has(FinderFlags::PREVIEW_PANE));
    let inspector_w = if show_inspector && main_w >= scale(520, dpi) {
        scale(input.metrics.inspector_width, dpi)
    } else {
        0
    };
    let stage_w = (main_w - inspector_w).max(1);
    let stage_h = (bottom_y - top_y).max(1);

    if tab.view_mode == FinderViewMode::List {
        let hdr_h = scale(FINDER_LIST_HEADER_HEIGHT_POINTS, dpi);
        layout.list_header = Rect::new(main_x, top_y, stage_w, hdr_h);
        layout.content_stage = Rect::new(main_x, top_y + hdr_h, stage_w, (stage_h - hdr_h).max(1));
    } else {
        layout.content_stage = Rect::new(main_x, top_y, stage_w, stage_h);
    }

    if inspector_w > 0 {
        let panel = Rect::new(main_x + stage_w, top_y, inspector_w, stage_h);
        layout.inspector = Some(layout_inspector(panel, dpi));
    }

    match tab.view_mode {
        FinderViewMode::Icon => layout_icon_view(layout, tab, input.metrics.icon_size, dpi),
        FinderViewMode::List => layout_list_view(layout, tab, input.list_rows, dpi),
        FinderViewMode::Column => {
            layout_column_view(
                layout,
                tab,
                input.active_columns,
                input.metrics.column_width,
                dpi,
            );
        }
        FinderViewMode::Gallery => layout_gallery_view(layout, tab, dpi),
    }

    if input.flags.has(FinderFlags::QUICK_LOOK) {
        layout.quick_look = Some(layout_quick_look(layout.width, layout.height, dpi));
    }
    if input.flags.has(FinderFlags::GO_TO_FOLDER) {
        layout.go_to = Some(layout_go_to_overlay(layout.width, layout.height, dpi));
    }
}
