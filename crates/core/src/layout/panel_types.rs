use crate::notch::clipboard::CLIPBOARD_PAGE_SIZE;
use crate::notch::focus_stats_view::NOTCH_STATS_DAYS;
use crate::notch::shelf::SHELF_PAGE_SIZE;
use crate::notch::tasks::TASKS_PAGE_SIZE;

use super::geometry::Rect;
use super::model::{NOTCH_CATEGORY_OPTIONS, NOTCH_HEATMAP_FILTERS};

#[derive(Default)]
pub(super) struct ExpandedNotchControls {
    pub(super) notch_tab_strip: Rect,
    pub(super) icon: Rect,
    pub(super) time: Rect,
    pub(super) subtitle: Rect,
    pub(super) session_label: Rect,
    pub(super) category_pill: Rect,
    pub(super) pomodoro_open_settings: Rect,
    pub(super) category_options: [Rect; NOTCH_CATEGORY_OPTIONS],
    pub(super) duration_dec: Rect,
    pub(super) duration_inc: Rect,
    pub(super) segments: [Rect; 4],
    pub(super) footer_left: Rect,
    pub(super) footer_right: Rect,
    pub(super) presets: [Rect; 4],
    pub(super) play_button: Rect,
    pub(super) reset_button: Rect,
    pub(super) skip_button: Rect,
    pub(super) alert_start_button: Rect,
    pub(super) alert_extend_button: Rect,
    pub(super) stats_body: Rect,
    pub(super) stats_range_today: Rect,
    pub(super) stats_range_week: Rect,
    pub(super) stats_range_heatmap: Rect,
    pub(super) stats_open_settings: Rect,
    pub(super) stats_days: [Rect; NOTCH_STATS_DAYS],
    pub(super) stats_heatmap_filters: [Rect; NOTCH_HEATMAP_FILTERS],
    pub(super) stats_heatmap_grid: Rect,
    pub(super) stats_heatmap_cells: Vec<Rect>,
    pub(super) notch_tab_pomodoro: Rect,
    pub(super) notch_tab_stats: Rect,
    pub(super) notch_tab_shelf: Rect,
    pub(super) notch_tab_clipboard: Rect,
    pub(super) notch_tab_tasks: Rect,
    pub(super) notch_tab_media: Rect,
    pub(super) shelf_add_button: Rect,
    pub(super) shelf_clear_button: Rect,
    pub(super) shelf_pager_button: Rect,
    pub(super) shelf_dropzone: Rect,
    pub(super) shelf_cards: [Rect; SHELF_PAGE_SIZE],
    pub(super) shelf_copy_buttons: [Rect; SHELF_PAGE_SIZE],
    pub(super) shelf_remove_buttons: [Rect; SHELF_PAGE_SIZE],
    pub(super) clipboard_pause_button: Rect,
    pub(super) clipboard_clear_button: Rect,
    pub(super) clipboard_pager_button: Rect,
    pub(super) clipboard_body: Rect,
    pub(super) clipboard_rows: [Rect; CLIPBOARD_PAGE_SIZE],
    pub(super) clipboard_remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
    pub(super) tasks_add_button: Rect,
    pub(super) tasks_clear_button: Rect,
    pub(super) tasks_pager_button: Rect,
    pub(super) tasks_editor: Rect,
    pub(super) tasks_editor_input: Rect,
    pub(super) tasks_body: Rect,
    pub(super) tasks_rows: [Rect; TASKS_PAGE_SIZE],
    pub(super) tasks_check_buttons: [Rect; TASKS_PAGE_SIZE],
    pub(super) tasks_edit_buttons: [Rect; TASKS_PAGE_SIZE],
    pub(super) tasks_remove_buttons: [Rect; TASKS_PAGE_SIZE],
    pub(super) privacy_mic_button: Rect,
    pub(super) media_cover_art: Rect,
    pub(super) media_source_label: Rect,
    pub(super) media_title: Rect,
    pub(super) media_artist: Rect,
    pub(super) media_reason: Rect,
    pub(super) media_equalizer: Rect,
    pub(super) media_lyrics_toggle: Rect,
    pub(super) media_lyrics_drawer: Rect,
    pub(super) media_seekbar: Rect,
    pub(super) media_play_button: Rect,
    pub(super) media_prev_button: Rect,
    pub(super) media_next_button: Rect,
}

pub(super) struct HeroButtons {
    pub(super) left: Rect,
    pub(super) primary: Rect,
    pub(super) right: Rect,
}

pub(super) fn hero_buttons(
    content_right: i32,
    hero_cy: i32,
    primary: i32,
    secondary: i32,
    gap: i32,
) -> HeroButtons {
    let right = Rect {
        x: content_right - secondary,
        y: hero_cy - secondary / 2,
        width: secondary,
        height: secondary,
    };
    let middle = Rect {
        x: right.x - gap - primary,
        y: hero_cy - primary / 2,
        width: primary,
        height: primary,
    };
    let left = Rect {
        x: middle.x - gap - secondary,
        y: hero_cy - secondary / 2,
        width: secondary,
        height: secondary,
    };
    HeroButtons {
        left,
        primary: middle,
        right,
    }
}
