use crate::layout::{Rect, scale};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpotlightView {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotlightHit {
    SearchField,
    SearchTab,
    AppsTab,
    GridView,
    ListView,
    Result(usize),
    Drag,
    Resize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotlightMove {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotlightResultLayout {
    pub index: usize,
    pub tile: Rect,
    pub icon: Rect,
    pub label: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpotlightLayout {
    pub window: Rect,
    pub search_field: Rect,
    pub search_tab: Rect,
    pub apps_tab: Rect,
    pub grid_view: Rect,
    pub list_view: Rect,
    pub drag_handle: Rect,
    pub resize_handle: Rect,
    pub empty_state: Rect,
    pub results: Vec<SpotlightResultLayout>,
    pub columns: usize,
    pub view: SpotlightView,
    pub corner_radius: f32,
}

impl SpotlightLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<SpotlightHit> {
        if self.resize_handle.contains(x, y) {
            return Some(SpotlightHit::Resize);
        }
        if self.search_field.contains(x, y) {
            return Some(SpotlightHit::SearchField);
        }
        if self.search_tab.contains(x, y) {
            return Some(SpotlightHit::SearchTab);
        }
        if self.apps_tab.contains(x, y) {
            return Some(SpotlightHit::AppsTab);
        }
        if self.grid_view.contains(x, y) {
            return Some(SpotlightHit::GridView);
        }
        if self.list_view.contains(x, y) {
            return Some(SpotlightHit::ListView);
        }
        self.results
            .iter()
            .find(|result| result.tile.contains(x, y))
            .map(|result| SpotlightHit::Result(result.index))
            .or_else(|| {
                self.drag_handle
                    .contains(x, y)
                    .then_some(SpotlightHit::Drag)
            })
    }
}

#[must_use]
pub fn compute_spotlight_layout(
    origin: (i32, i32),
    width: i32,
    height: i32,
    dpi: u32,
    view: SpotlightView,
    result_count: usize,
    first_visible: usize,
) -> SpotlightLayout {
    let width = width.max(scale(420, dpi));
    let height = height.max(scale(300, dpi));
    let inset = scale(16, dpi);
    let search_height = scale(44, dpi);
    let tab_height = scale(30, dpi);
    let header_top = scale(18, dpi);
    let tabs_top = header_top + search_height + scale(12, dpi);
    let results_top = tabs_top + tab_height + scale(12, dpi);
    let results_bottom = height - scale(34, dpi);
    let results_height = (results_bottom - results_top).max(0);
    let content_width = (width - inset * 2).max(0);
    let search_field = Rect {
        x: inset,
        y: header_top,
        width: content_width,
        height: search_height,
    };
    let search_tab = Rect {
        x: inset,
        y: tabs_top,
        width: scale(76, dpi),
        height: tab_height,
    };
    let apps_tab = Rect {
        x: search_tab.right() + scale(6, dpi),
        y: tabs_top,
        width: scale(76, dpi),
        height: tab_height,
    };
    let list_width = scale(42, dpi);
    let view_gap = scale(4, dpi);
    let list_view = Rect {
        x: width - inset - list_width,
        y: tabs_top,
        width: list_width,
        height: tab_height,
    };
    let grid_view = Rect {
        x: list_view.x - view_gap - list_width,
        y: tabs_top,
        width: list_width,
        height: tab_height,
    };
    let (columns, results) = spotlight_result_layouts(
        view,
        SpotlightResultsRegion {
            width,
            dpi,
            inset,
            content_width,
            top: results_top,
            height: results_height,
        },
        result_count,
        first_visible,
    );
    let resize_size = scale(18, dpi);
    SpotlightLayout {
        window: Rect {
            x: origin.0,
            y: origin.1,
            width,
            height,
        },
        search_field,
        search_tab,
        apps_tab,
        grid_view,
        list_view,
        drag_handle: Rect {
            x: 0,
            y: 0,
            width,
            height: results_top,
        },
        resize_handle: Rect {
            x: width - resize_size,
            y: height - resize_size,
            width: resize_size,
            height: resize_size,
        },
        empty_state: Rect {
            x: inset,
            y: results_top,
            width: content_width,
            height: results_height,
        },
        results,
        columns,
        view,
        corner_radius: scale(28, dpi) as f32,
    }
}

#[derive(Clone, Copy)]
pub(super) struct SpotlightResultsRegion {
    pub(super) width: i32,
    pub(super) dpi: u32,
    pub(super) inset: i32,
    pub(super) content_width: i32,
    pub(super) top: i32,
    pub(super) height: i32,
}

pub(super) fn spotlight_result_layouts(
    view: SpotlightView,
    region: SpotlightResultsRegion,
    result_count: usize,
    first_visible: usize,
) -> (usize, Vec<SpotlightResultLayout>) {
    match view {
        SpotlightView::Grid => grid_result_layouts(region, result_count, first_visible),
        SpotlightView::List => list_result_layouts(region, result_count, first_visible),
    }
}

pub(super) fn grid_result_layouts(
    region: SpotlightResultsRegion,
    result_count: usize,
    first_visible: usize,
) -> (usize, Vec<SpotlightResultLayout>) {
    let columns = 6;
    let start = first_visible.min(result_count) / columns * columns;
    let gap = scale(8, region.dpi);
    let cell_width = (region.content_width - gap * (columns as i32 - 1)).max(0) / columns as i32;
    let row_height = scale(108, region.dpi);
    let visible_rows = (region.height / row_height.max(1)).max(1) as usize;
    let end = start
        .saturating_add(visible_rows.saturating_mul(columns))
        .min(result_count);
    let results = (start..end)
        .map(|index| {
            let offset = index - start;
            let column = offset % columns;
            let row = offset / columns;
            let x = region.inset + column as i32 * (cell_width + gap);
            let y = region.top + row as i32 * row_height;
            let icon_size = scale(58, region.dpi).min((cell_width - scale(12, region.dpi)).max(0));
            let icon = Rect {
                x: x + (cell_width - icon_size) / 2,
                y: y + scale(9, region.dpi),
                width: icon_size,
                height: icon_size,
            };
            SpotlightResultLayout {
                index,
                tile: Rect {
                    x,
                    y,
                    width: cell_width,
                    height: row_height,
                },
                icon,
                label: Rect {
                    x: x + scale(4, region.dpi),
                    y: icon.bottom() + scale(4, region.dpi),
                    width: (cell_width - scale(8, region.dpi)).max(0),
                    height: scale(30, region.dpi),
                },
            }
        })
        .collect();
    (columns, results)
}

pub(super) fn list_result_layouts(
    region: SpotlightResultsRegion,
    result_count: usize,
    first_visible: usize,
) -> (usize, Vec<SpotlightResultLayout>) {
    let start = first_visible.min(result_count);
    let row_height = scale(54, region.dpi);
    let visible_rows = (region.height / row_height.max(1)).max(1) as usize;
    let end = start.saturating_add(visible_rows).min(result_count);
    let results = (start..end)
        .map(|index| {
            let y = region.top + (index - start) as i32 * row_height;
            let icon_size = scale(36, region.dpi);
            let icon = Rect {
                x: region.inset + scale(10, region.dpi),
                y: y + (row_height - icon_size) / 2,
                width: icon_size,
                height: icon_size,
            };
            SpotlightResultLayout {
                index,
                tile: Rect {
                    x: region.inset,
                    y,
                    width: region.content_width,
                    height: row_height,
                },
                icon,
                label: Rect {
                    x: icon.right() + scale(12, region.dpi),
                    y: y + scale(4, region.dpi),
                    width: (region.width - region.inset - icon.right() - scale(12, region.dpi))
                        .max(0),
                    height: row_height - scale(8, region.dpi),
                },
            }
        })
        .collect();
    (1, results)
}
