use crate::layout::{Rect, scale};

pub const DOWNLOADS_STACK_MAX_ITEMS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadsStackEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadsStackTile {
    pub index: usize,
    pub rect: Rect,
    pub thumb_rect: Rect,
    pub label_rect: Rect,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadsStackHit {
    Item(usize),
    OpenFolder,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadsStackLayout {
    pub window: Rect,
    pub header_rect: Rect,
    pub open_folder_rect: Rect,
    pub empty_rect: Option<Rect>,
    pub corner_radius: f32,
    pub tile_radius: f32,
    pub columns: usize,
    pub tiles: Vec<DownloadsStackTile>,
}

#[must_use]
pub fn compute_downloads_stack_layout(
    center_x: i32,
    bottom_y: i32,
    bounds: Rect,
    dpi: u32,
    items: &[DownloadsStackEntry],
) -> DownloadsStackLayout {
    let pad = scale(12, dpi);
    let header_h = scale(26, dpi);
    let header_gap = scale(8, dpi);
    let footer_gap = scale(8, dpi);
    let footer_h = scale(28, dpi);
    let cell_w = scale(84, dpi);
    let cell_h = scale(82, dpi);
    let cell_gap = scale(8, dpi);
    let empty_h = scale(52, dpi);
    let margin = scale(6, dpi);

    let count = items.len().min(DOWNLOADS_STACK_MAX_ITEMS);
    let columns = if count == 0 {
        3
    } else if count <= 3 {
        count.max(2)
    } else {
        4
    };
    let rows = if count == 0 {
        0
    } else {
        count.div_ceil(columns)
    };

    let min_grid_w = cell_w * 3 + cell_gap * 2;
    let grid_w =
        (columns as i32 * cell_w + (columns.saturating_sub(1) as i32) * cell_gap).max(min_grid_w);
    let grid_h = if count == 0 {
        empty_h
    } else {
        rows as i32 * cell_h + (rows.saturating_sub(1) as i32) * cell_gap
    };

    let win_w = grid_w + pad * 2;
    let win_h = pad + header_h + header_gap + grid_h + footer_gap + footer_h + pad;

    let min_x = bounds.x + margin;
    let min_y = bounds.y + margin;
    let win_x = (center_x - win_w / 2).clamp(min_x, (bounds.right() - win_w - margin).max(min_x));
    let win_y = (bottom_y - win_h).clamp(min_y, (bounds.bottom() - win_h - margin).max(min_y));

    let header_rect = Rect {
        x: pad,
        y: pad,
        width: grid_w,
        height: header_h,
    };

    let grid_top = pad + header_h + header_gap;
    let tiles = build_downloads_stack_tiles(
        items, count, columns, pad, grid_top, cell_w, cell_h, cell_gap, dpi,
    );

    let empty_rect = (count == 0).then_some(Rect {
        x: pad,
        y: grid_top,
        width: grid_w,
        height: empty_h,
    });
    let open_folder_rect = Rect {
        x: pad,
        y: grid_top + grid_h + footer_gap,
        width: grid_w,
        height: footer_h,
    };

    DownloadsStackLayout {
        window: Rect {
            x: win_x,
            y: win_y,
            width: win_w,
            height: win_h,
        },
        header_rect,
        open_folder_rect,
        empty_rect,
        corner_radius: scale(14, dpi) as f32,
        tile_radius: scale(8, dpi) as f32,
        columns,
        tiles,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_downloads_stack_tiles(
    items: &[DownloadsStackEntry],
    count: usize,
    columns: usize,
    pad: i32,
    grid_top: i32,
    cell_w: i32,
    cell_h: i32,
    cell_gap: i32,
    dpi: u32,
) -> Vec<DownloadsStackTile> {
    let thumb_size = scale(46, dpi);
    let thumb_top_pad = scale(8, dpi);
    let label_gap = scale(4, dpi);
    let mut tiles = Vec::with_capacity(count);
    for (index, item) in items.iter().take(count).enumerate() {
        let col = (index % columns) as i32;
        let row = (index / columns) as i32;
        let tile_x = pad + col * (cell_w + cell_gap);
        let tile_y = grid_top + row * (cell_h + cell_gap);
        let rect = Rect {
            x: tile_x,
            y: tile_y,
            width: cell_w,
            height: cell_h,
        };
        let thumb_rect = Rect {
            x: tile_x + (cell_w - thumb_size) / 2,
            y: tile_y + thumb_top_pad,
            width: thumb_size,
            height: thumb_size,
        };
        let label_y = thumb_rect.bottom() + label_gap;
        let label_rect = Rect {
            x: tile_x + scale(4, dpi),
            y: label_y,
            width: (cell_w - scale(8, dpi)).max(1),
            height: (rect.bottom() - label_y - scale(4, dpi)).max(1),
        };
        tiles.push(DownloadsStackTile {
            index,
            rect,
            thumb_rect,
            label_rect,
            is_dir: item.is_dir,
        });
    }
    tiles
}

#[must_use]
pub fn downloads_stack_hit_test(
    layout: &DownloadsStackLayout,
    x: i32,
    y: i32,
) -> DownloadsStackHit {
    if layout.open_folder_rect.contains(x, y) {
        return DownloadsStackHit::OpenFolder;
    }
    if let Some(tile) = layout.tiles.iter().find(|tile| tile.rect.contains(x, y)) {
        return DownloadsStackHit::Item(tile.index);
    }
    DownloadsStackHit::None
}

#[must_use]
pub fn next_downloads_stack_focus(
    current: DownloadsStackHit,
    item_count: usize,
    columns: usize,
    delta_col: i32,
    delta_row: i32,
) -> DownloadsStackHit {
    if item_count == 0 || columns == 0 {
        return DownloadsStackHit::OpenFolder;
    }
    match current {
        DownloadsStackHit::None => DownloadsStackHit::Item(0),
        DownloadsStackHit::OpenFolder => {
            if delta_row < 0 {
                DownloadsStackHit::Item(item_count - 1)
            } else if delta_row > 0 || delta_col != 0 {
                DownloadsStackHit::Item(0)
            } else {
                DownloadsStackHit::OpenFolder
            }
        }
        DownloadsStackHit::Item(idx) => {
            if delta_col != 0 {
                let next = (idx as isize + delta_col as isize).rem_euclid(item_count as isize);
                return DownloadsStackHit::Item(next as usize);
            }
            match delta_row.cmp(&0) {
                std::cmp::Ordering::Greater => {
                    let next = idx + columns;
                    if next < item_count {
                        DownloadsStackHit::Item(next)
                    } else {
                        DownloadsStackHit::OpenFolder
                    }
                }
                std::cmp::Ordering::Less => {
                    if idx >= columns {
                        DownloadsStackHit::Item(idx - columns)
                    } else {
                        DownloadsStackHit::OpenFolder
                    }
                }
                std::cmp::Ordering::Equal => DownloadsStackHit::Item(idx),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(n: usize) -> Vec<DownloadsStackEntry> {
        (0..n)
            .map(|i| DownloadsStackEntry {
                name: format!("Item {i}.png"),
                is_dir: i % 3 == 0,
            })
            .collect()
    }

    #[test]
    fn grid_layout_places_items_in_rows_clamps_to_work_area_and_hits_tiles_and_footer() {
        let bounds = Rect {
            x: 0,
            y: 32,
            width: 1280,
            height: 768,
        };
        let layout = compute_downloads_stack_layout(1260, 790, bounds, 96, &entries(7));
        assert_eq!(layout.columns, 4);
        assert_eq!(layout.tiles.len(), 7);
        assert!(layout.window.right() <= bounds.right() - 6);
        assert!(layout.window.bottom() <= bounds.bottom() - 6);
        assert_eq!(layout.tiles[0].rect.y, layout.tiles[3].rect.y);
        assert!(layout.tiles[4].rect.y > layout.tiles[0].rect.bottom());

        let first = layout.tiles[0].rect;
        assert_eq!(
            downloads_stack_hit_test(&layout, first.x + 10, first.y + 10),
            DownloadsStackHit::Item(0)
        );
        let footer = layout.open_folder_rect;
        assert_eq!(
            downloads_stack_hit_test(&layout, footer.x + 10, footer.y + 10),
            DownloadsStackHit::OpenFolder
        );
        assert_eq!(
            downloads_stack_hit_test(&layout, 0, 0),
            DownloadsStackHit::None
        );

        let empty = compute_downloads_stack_layout(640, 780, bounds, 96, &[]);
        assert!(empty.tiles.is_empty());
        assert!(empty.empty_rect.is_some());
        assert_eq!(
            next_downloads_stack_focus(DownloadsStackHit::Item(2), 7, 4, 0, 1),
            DownloadsStackHit::Item(6)
        );
        assert_eq!(
            next_downloads_stack_focus(DownloadsStackHit::Item(6), 7, 4, 0, 1),
            DownloadsStackHit::OpenFolder
        );
        assert_eq!(
            next_downloads_stack_focus(DownloadsStackHit::OpenFolder, 7, 4, 0, -1),
            DownloadsStackHit::Item(6)
        );
    }
}
