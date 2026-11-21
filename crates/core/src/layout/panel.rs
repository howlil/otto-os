use crate::notch::clipboard::CLIPBOARD_PAGE_SIZE;
use crate::notch::media::NotchExpandedTab;
use crate::notch::pomodoro::FocusSubMode;
use crate::notch::pomodoro_view::PomodoroSurface;
use crate::notch::shelf::SHELF_PAGE_SIZE;
use crate::notch::tasks::TASKS_PAGE_SIZE;

use super::clipboard::compute_clipboard_controls;
use super::geometry::Rect;
use super::geometry::lerp_points;
use super::geometry::scale;
use super::input::NotchLayoutInput;
use super::metrics::Metrics;
use super::shelf::compute_shelf_controls;
use super::tasks::compute_tasks_controls;

#[derive(Default)]
pub(super) struct ExpandedNotchControls {
    pub(super) notch_tab_strip: Rect,
    pub(super) icon: Rect,
    pub(super) time: Rect,
    pub(super) subtitle: Rect,
    pub(super) session_label: Rect,
    pub(super) segments: [Rect; 4],
    pub(super) footer_left: Rect,
    pub(super) footer_right: Rect,
    pub(super) presets: [Rect; 4],
    pub(super) focus_modes: [Rect; 3],
    pub(super) play_button: Rect,
    pub(super) reset_button: Rect,
    pub(super) skip_button: Rect,
    pub(super) alert_start_button: Rect,
    pub(super) alert_extend_button: Rect,
    pub(super) notch_tab_pomodoro: Rect,
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

pub(super) const PANEL_PAD_POINTS: i32 = 16;
pub(super) const PANEL_TAB_TOP_POINTS: i32 = 10;
pub(super) const PANEL_TAB_HEIGHT_POINTS: i32 = 24;
pub(super) const PANEL_TAB_ACTIVE_WIDTH_POINTS: i32 = 64;
pub(super) const PANEL_TAB_ICON_WIDTH_POINTS: i32 = 24;
pub(super) const PANEL_TAB_GAP_POINTS: i32 = 4;
pub(super) const PANEL_TAB_STRIP_INSET_POINTS: i32 = 2;
pub(super) const PANEL_SESSION_LABEL_WIDTH_POINTS: i32 = 72;
pub(super) const FOCUS_MODE_WIDTH_POINTS: i32 = 24;
pub(super) const PANEL_HERO_CENTER_POINTS: i32 = 62;
pub(super) const PANEL_HERO_ICON_POINTS: i32 = 48;
pub(super) const PANEL_HERO_TEXT_GAP_POINTS: i32 = 12;
pub(super) const PANEL_TIME_HEIGHT_POINTS: i32 = 32;
pub(super) const PANEL_SUBTITLE_HEIGHT_POINTS: i32 = 16;
pub(super) const PANEL_PRIMARY_BUTTON_POINTS: i32 = 40;
pub(super) const PANEL_SECONDARY_BUTTON_POINTS: i32 = 32;
pub(super) const PANEL_BUTTON_GAP_POINTS: i32 = 8;
pub(super) const PANEL_PROGRESS_TOP_POINTS: i32 = 98;
pub(super) const PANEL_PROGRESS_HEIGHT_POINTS: i32 = 4;
pub(super) const PANEL_SEGMENT_GAP_POINTS: i32 = 6;
pub(super) const PANEL_FOOTER_TOP_POINTS: i32 = 108;
pub(super) const PANEL_FOOTER_HEIGHT_POINTS: i32 = 20;
pub(super) const PANEL_FOOTER_LEFT_SHARE_PERCENT: i32 = 64;
pub(super) const PANEL_PRESET_TOP_POINTS: i32 = 98;
pub(super) const PANEL_PRESET_WIDTH_POINTS: i32 = 58;
pub(super) const PANEL_PRESET_HEIGHT_POINTS: i32 = 24;
pub(super) const MEDIA_LYRICS_TOGGLE_POINTS: i32 = 24;
pub(super) const MEDIA_SOURCE_LABEL_WIDTH_POINTS: i32 = 96;
pub(super) const MEDIA_TITLE_HEIGHT_POINTS: i32 = 20;
pub(super) const MEDIA_ARTIST_HEIGHT_POINTS: i32 = 16;
pub(super) const MEDIA_REASON_HEIGHT_POINTS: i32 = 28;
pub(super) const MEDIA_EQUALIZER_WIDTH_POINTS: i32 = 24;
pub(super) const MEDIA_EQUALIZER_HEIGHT_POINTS: i32 = 10;
pub(super) const MEDIA_SEEK_TOP_POINTS: i32 = 98;
pub(super) const MEDIA_SEEK_HEIGHT_POINTS: i32 = 24;
pub(super) const MEDIA_DRAWER_TOP_GAP_POINTS: i32 = 8;
pub(super) const MEDIA_DRAWER_BOTTOM_GAP_POINTS: i32 = 12;
pub(super) const PANEL_ALERT_TITLE_TOP_POINTS: i32 = 40;
pub(super) const PANEL_ALERT_TITLE_HEIGHT_POINTS: i32 = 24;
pub(super) const PANEL_ALERT_BUTTON_HEIGHT_POINTS: i32 = 32;
pub(super) const PANEL_ALERT_START_WIDTH_POINTS: i32 = 104;
pub(super) const PANEL_ALERT_EXTEND_WIDTH_POINTS: i32 = 72;
pub(super) const PRIVACY_COMPACT_MIC_WIDTH_POINTS: i32 = 40;
pub(super) const PRIVACY_PANEL_MIC_WIDTH_POINTS: i32 = 88;

pub(super) fn compute_focus_mode_rects(
    surface: PomodoroSurface,
    session_left: i32,
    tab_y: i32,
    tab_h: i32,
    dpi: u32,
) -> [Rect; 3] {
    if surface == PomodoroSurface::Alert {
        return [Rect::default(); 3];
    }
    let gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let item_w = scale(FOCUS_MODE_WIDTH_POINTS, dpi);
    let total_w = item_w * 3 + gap * 2;
    let x0 = session_left - gap - total_w;
    let r0 = Rect {
        x: x0,
        y: tab_y,
        width: item_w,
        height: tab_h,
    };
    let r1 = Rect {
        x: r0.right() + gap,
        y: tab_y,
        width: item_w,
        height: tab_h,
    };
    let r2 = Rect {
        x: r1.right() + gap,
        y: tab_y,
        width: item_w,
        height: tab_h,
    };
    [r0, r1, r2]
}

#[allow(clippy::too_many_lines)]
pub(super) fn compute_expanded_notch_controls(
    notch: Rect,
    dpi: u32,
    notch_input: &NotchLayoutInput,
    metrics: &Metrics,
) -> ExpandedNotchControls {
    let oy = notch.y;
    let lyrics_progress = notch_input.lyrics_progress.clamp(0.0, 1.0);
    let surface = notch_input.pomodoro_surface;
    let active_tab = notch_input.active_tab;
    let media_present = notch_input.media_present;
    let panel_height_points = metrics.notch_expanded_height;
    let lyrics_height_points = metrics.notch_lyrics_height;
    let pad_x = scale(PANEL_PAD_POINTS, dpi);
    let tab_y = oy + scale(PANEL_TAB_TOP_POINTS, dpi);
    let tab_h = scale(PANEL_TAB_HEIGHT_POINTS, dpi);
    let tab_active_w = scale(PANEL_TAB_ACTIVE_WIDTH_POINTS, dpi);
    let tab_icon_w = scale(PANEL_TAB_ICON_WIDTH_POINTS, dpi);
    let tab_gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let strip_inset = scale(PANEL_TAB_STRIP_INSET_POINTS, dpi);
    let tab_width = |tab: NotchExpandedTab| {
        if active_tab == tab {
            tab_active_w
        } else {
            tab_icon_w
        }
    };

    let mut cursor_x = notch.x + pad_x;
    let mut first_tab_x: Option<i32> = None;
    let mut strip_right = cursor_x;
    let mut allocate_tab = |tab: NotchExpandedTab, enabled: bool| -> Rect {
        if !enabled {
            return Rect::default();
        }
        let rect = Rect {
            x: cursor_x,
            y: tab_y,
            width: tab_width(tab),
            height: tab_h,
        };
        first_tab_x.get_or_insert(rect.x);
        strip_right = rect.right();
        cursor_x = rect.right() + tab_gap;
        rect
    };

    let notch_tab_pomodoro = allocate_tab(NotchExpandedTab::Pomodoro, notch_input.focus_enabled);
    let notch_tab_shelf = allocate_tab(NotchExpandedTab::Shelf, notch_input.shelf_enabled);
    let notch_tab_clipboard =
        allocate_tab(NotchExpandedTab::Clipboard, notch_input.clipboard_enabled);
    let notch_tab_tasks = allocate_tab(NotchExpandedTab::Tasks, notch_input.tasks_enabled);
    let notch_tab_media = allocate_tab(NotchExpandedTab::Media, media_present);
    let notch_tab_strip = match first_tab_x {
        Some(left) => Rect {
            x: left - strip_inset,
            y: tab_y - strip_inset,
            width: strip_right - left + strip_inset * 2,
            height: tab_h + strip_inset * 2,
        },
        None => Rect::default(),
    };
    let session_label_w = scale(PANEL_SESSION_LABEL_WIDTH_POINTS, dpi);
    let session_label = Rect {
        x: notch.right() - pad_x - session_label_w,
        y: tab_y,
        width: session_label_w,
        height: tab_h,
    };
    let focus_modes = compute_focus_mode_rects(surface, session_label.x, tab_y, tab_h, dpi);

    let content_left = notch.x + pad_x;
    let content_right = notch.right() - pad_x;
    let content_width = content_right - content_left;
    let hero_cy = oy + scale(PANEL_HERO_CENTER_POINTS, dpi);
    let icon_size = scale(PANEL_HERO_ICON_POINTS, dpi);
    let text_gap = scale(PANEL_HERO_TEXT_GAP_POINTS, dpi);
    let primary = scale(PANEL_PRIMARY_BUTTON_POINTS, dpi);
    let secondary = scale(PANEL_SECONDARY_BUTTON_POINTS, dpi);
    let btn_gap = scale(PANEL_BUTTON_GAP_POINTS, dpi);
    let alert = surface == PomodoroSurface::Alert;
    let setup = surface == PomodoroSurface::Setup;

    let icon = Rect {
        x: content_left,
        y: hero_cy - icon_size / 2,
        width: icon_size,
        height: icon_size,
    };
    let text_left = icon.right() + text_gap;

    let mut play_button = Rect::default();
    let mut reset_button = Rect::default();
    let mut skip_button = Rect::default();
    if !alert {
        if setup
            || (notch_input.focus_mode == FocusSubMode::Stopwatch
                && surface == PomodoroSurface::Ready)
        {
            play_button = Rect {
                x: content_right - primary,
                y: hero_cy - primary / 2,
                width: primary,
                height: primary,
            };
        } else {
            let buttons = hero_buttons(content_right, hero_cy, primary, secondary, btn_gap);
            play_button = buttons.primary;
            reset_button = buttons.left;
            skip_button = if notch_input.focus_mode == FocusSubMode::Pomodoro
                || (notch_input.focus_mode == FocusSubMode::Stopwatch
                    && surface == PomodoroSurface::Running)
            {
                buttons.right
            } else {
                Rect::default()
            };
        }
    }
    let text_right = if alert {
        content_right
    } else if !reset_button.is_empty() {
        reset_button.x - btn_gap
    } else {
        play_button.x - btn_gap
    };
    let text_width = (text_right - text_left).max(0);

    let (time, subtitle) = if alert {
        let title_h = scale(PANEL_ALERT_TITLE_HEIGHT_POINTS, dpi);
        let title_top = oy + scale(PANEL_ALERT_TITLE_TOP_POINTS, dpi);
        (
            Rect {
                x: text_left,
                y: title_top,
                width: text_width,
                height: title_h,
            },
            Rect {
                x: text_left,
                y: title_top + title_h,
                width: text_width,
                height: scale(PANEL_SUBTITLE_HEIGHT_POINTS, dpi),
            },
        )
    } else {
        let time_h = scale(PANEL_TIME_HEIGHT_POINTS, dpi);
        let subtitle_h = scale(PANEL_SUBTITLE_HEIGHT_POINTS, dpi);
        let block_top = hero_cy - i32::midpoint(time_h, subtitle_h);
        (
            Rect {
                x: text_left,
                y: block_top,
                width: text_width,
                height: time_h,
            },
            Rect {
                x: text_left,
                y: block_top + time_h,
                width: text_width,
                height: subtitle_h,
            },
        )
    };

    let mut segments = [Rect::default(); 4];
    let mut footer_left = Rect::default();
    let mut footer_right = Rect::default();
    if !alert && !setup {
        let segment_gap = scale(PANEL_SEGMENT_GAP_POINTS, dpi);
        let segment_w = (content_width - segment_gap * 3) / 4;
        let segment_y = oy + scale(PANEL_PROGRESS_TOP_POINTS, dpi);
        let segment_h = scale(PANEL_PROGRESS_HEIGHT_POINTS, dpi);
        for (index, segment) in segments.iter_mut().enumerate() {
            let x = content_left + (segment_w + segment_gap) * index as i32;
            let width = if index == 3 {
                content_right - x
            } else {
                segment_w
            };
            *segment = Rect {
                x,
                y: segment_y,
                width,
                height: segment_h,
            };
        }
        let footer_y = oy + scale(PANEL_FOOTER_TOP_POINTS, dpi);
        let footer_h = scale(PANEL_FOOTER_HEIGHT_POINTS, dpi);
        let footer_left_w = content_width * PANEL_FOOTER_LEFT_SHARE_PERCENT / 100;
        footer_left = Rect {
            x: content_left,
            y: footer_y,
            width: footer_left_w,
            height: footer_h,
        };
        footer_right = Rect {
            x: content_left + footer_left_w,
            y: footer_y,
            width: content_width - footer_left_w,
            height: footer_h,
        };
    }

    let mut presets = [Rect::default(); 4];
    if setup {
        let preset_w = scale(PANEL_PRESET_WIDTH_POINTS, dpi);
        let preset_h = scale(PANEL_PRESET_HEIGHT_POINTS, dpi);
        let preset_y = oy + scale(PANEL_PRESET_TOP_POINTS, dpi);
        for (index, preset) in presets.iter_mut().enumerate() {
            *preset = Rect {
                x: content_left + (preset_w + btn_gap) * index as i32,
                y: preset_y,
                width: preset_w,
                height: preset_h,
            };
        }
    }

    let alert_btn_h = scale(PANEL_ALERT_BUTTON_HEIGHT_POINTS, dpi);
    let alert_start_w = scale(PANEL_ALERT_START_WIDTH_POINTS, dpi);
    let alert_extend_w = scale(PANEL_ALERT_EXTEND_WIDTH_POINTS, dpi);
    let alert_y = oy + scale(panel_height_points - PANEL_PAD_POINTS, dpi) - alert_btn_h;
    let alert_start_x = content_right - alert_start_w;
    let alert_extend_x = alert_start_x - btn_gap - alert_extend_w;
    let alert_start_button = Rect {
        x: alert_start_x,
        y: alert_y,
        width: alert_start_w,
        height: alert_btn_h,
    };
    let alert_extend_button = Rect {
        x: alert_extend_x,
        y: alert_y,
        width: alert_extend_w,
        height: alert_btn_h,
    };

    let media_cover_art = icon;
    let lyrics_toggle_size = scale(MEDIA_LYRICS_TOGGLE_POINTS, dpi);
    let media_lyrics_toggle = Rect {
        x: content_right - lyrics_toggle_size,
        y: tab_y,
        width: lyrics_toggle_size,
        height: lyrics_toggle_size,
    };
    let source_width = scale(MEDIA_SOURCE_LABEL_WIDTH_POINTS, dpi);
    let media_source_label = Rect {
        x: media_lyrics_toggle.x - btn_gap - source_width,
        y: tab_y,
        width: source_width,
        height: tab_h,
    };

    let media_buttons = hero_buttons(content_right, hero_cy, primary, secondary, btn_gap);
    let media_text_width = (media_buttons.left.x - btn_gap - text_left).max(0);
    let title_h = scale(MEDIA_TITLE_HEIGHT_POINTS, dpi);
    let artist_h = scale(MEDIA_ARTIST_HEIGHT_POINTS, dpi);
    let media_title = Rect {
        x: text_left,
        y: icon.y,
        width: media_text_width,
        height: title_h,
    };
    let media_artist = Rect {
        x: text_left,
        y: icon.y + title_h,
        width: media_text_width,
        height: artist_h,
    };
    let media_reason = Rect {
        x: text_left,
        y: icon.y + title_h,
        width: media_text_width,
        height: scale(MEDIA_REASON_HEIGHT_POINTS, dpi),
    };
    let equalizer_h = scale(MEDIA_EQUALIZER_HEIGHT_POINTS, dpi);
    let media_equalizer = Rect {
        x: text_left,
        y: icon.bottom() - equalizer_h,
        width: scale(MEDIA_EQUALIZER_WIDTH_POINTS, dpi),
        height: equalizer_h,
    };

    let seek_travel = (lyrics_height_points - panel_height_points).max(0);
    let media_seek_y = oy
        + scale(
            lerp_points(
                MEDIA_SEEK_TOP_POINTS,
                MEDIA_SEEK_TOP_POINTS + seek_travel,
                lyrics_progress,
            ),
            dpi,
        );
    let media_seekbar = Rect {
        x: content_left,
        y: media_seek_y,
        width: content_width,
        height: scale(MEDIA_SEEK_HEIGHT_POINTS, dpi),
    };
    let drawer_top = icon.bottom() + scale(MEDIA_DRAWER_TOP_GAP_POINTS, dpi);
    let media_lyrics_drawer = Rect {
        x: content_left,
        y: drawer_top,
        width: content_width,
        height: (media_seek_y - scale(MEDIA_DRAWER_BOTTOM_GAP_POINTS, dpi) - drawer_top).max(0),
    };
    let shelf = compute_shelf_controls(
        oy,
        content_left,
        content_right,
        tab_y,
        tab_h,
        notch_input.shelf,
        notch_input.privacy.panel_mic_control,
        dpi,
    );
    let clipboard = compute_clipboard_controls(
        oy,
        content_left,
        content_right,
        tab_y,
        tab_h,
        notch_input.clipboard,
        notch_input.privacy.panel_mic_control,
        dpi,
    );
    let tasks = compute_tasks_controls(
        oy,
        content_left,
        content_right,
        tab_y,
        tab_h,
        notch_input.tasks,
        notch_input.privacy.panel_mic_control,
        dpi,
    );
    let privacy_mic_button = match active_tab {
        NotchExpandedTab::Shelf => shelf.mic_button,
        NotchExpandedTab::Clipboard => clipboard.mic_button,
        NotchExpandedTab::Tasks => tasks.mic_button,
        NotchExpandedTab::Media if notch_input.privacy.panel_mic_control => {
            let mic_w = scale(PRIVACY_PANEL_MIC_WIDTH_POINTS, dpi);
            Rect {
                x: content_right - mic_w,
                y: tab_y,
                width: mic_w,
                height: tab_h,
            }
        }
        _ => Rect::default(),
    };

    ExpandedNotchControls {
        notch_tab_strip,
        icon,
        time,
        subtitle,
        session_label,
        segments,
        footer_left,
        footer_right,
        presets,
        focus_modes,
        play_button,
        reset_button,
        skip_button,
        alert_start_button,
        alert_extend_button,
        notch_tab_pomodoro,
        notch_tab_shelf,
        notch_tab_clipboard,
        notch_tab_tasks,
        notch_tab_media,
        shelf_add_button: shelf.add_button,
        shelf_clear_button: shelf.clear_button,
        shelf_pager_button: shelf.pager_button,
        shelf_dropzone: shelf.dropzone,
        shelf_cards: shelf.cards,
        shelf_copy_buttons: shelf.copy_buttons,
        shelf_remove_buttons: shelf.remove_buttons,
        clipboard_pause_button: clipboard.pause_button,
        clipboard_clear_button: clipboard.clear_button,
        clipboard_pager_button: clipboard.pager_button,
        clipboard_body: clipboard.body,
        clipboard_rows: clipboard.rows,
        clipboard_remove_buttons: clipboard.remove_buttons,
        tasks_add_button: tasks.add_button,
        tasks_clear_button: tasks.clear_button,
        tasks_pager_button: tasks.pager_button,
        tasks_editor: tasks.editor,
        tasks_editor_input: tasks.editor_input,
        tasks_body: tasks.body,
        tasks_rows: tasks.rows,
        tasks_check_buttons: tasks.check_buttons,
        tasks_edit_buttons: tasks.edit_buttons,
        tasks_remove_buttons: tasks.remove_buttons,
        privacy_mic_button,
        media_cover_art,
        media_source_label,
        media_title,
        media_artist,
        media_reason,
        media_equalizer,
        media_lyrics_toggle,
        media_lyrics_drawer,
        media_seekbar,
        media_play_button: media_buttons.primary,
        media_prev_button: media_buttons.left,
        media_next_button: media_buttons.right,
    }
}
