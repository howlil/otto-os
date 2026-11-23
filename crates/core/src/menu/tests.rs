use super::builders::*;
use super::commands::*;
use super::layout::*;
use super::notifications::*;
use super::rows::*;
use super::top_bar::*;
use crate::app_model::WindowInfo;
use crate::app_model::{
    AppIdentity, WindowIdentity, WindowPresence, WindowRole, WindowTraits, group_windows,
};
use crate::bar::calendar::CALENDAR_POPUP_WIDTH;
use crate::bar::calendar::calendar_row_height;
use crate::bar::calendar::compute_calendar_layout;
use crate::layout::HitTarget;
use crate::layout::Rect;
use crate::layout::scale;

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
};
const TEXT: EstimatedMenuText = EstimatedMenuText { dpi: 96 };

fn placement(bounds: Rect, anchor: MenuAnchor, scroll_offset: i32) -> PopupMenuPlacement {
    PopupMenuPlacement {
        bounds,
        dpi: 96,
        anchor,
        scroll_offset,
    }
}

fn layout_for(rows: &[MenuRow], anchor: MenuAnchor) -> PopupMenuLayout {
    compute_popup_menu_layout(
        &PopupMenuMetrics::MACOS_SONOMA,
        placement(SCREEN, anchor, 0),
        rows,
        &TEXT,
    )
}

fn window(traits: WindowTraits) -> WindowInfo {
    WindowInfo {
        identity: WindowIdentity {
            hwnd: 1,
            process_id: 2,
            thread_id: 3,
            generation: 4,
        },
        app: AppIdentity::executable("code.exe"),
        app_name: String::from("Code"),
        executable: String::from("code.exe"),
        title: String::from("main.rs"),
        role: WindowRole::Primary,
        presence: WindowPresence::Visible,
        zoomed: false,
        full_screen: false,
        active: true,
        traits,
    }
}

fn context(traits: WindowTraits) -> MenuContext<'static> {
    let info = window(traits);
    let app = group_windows(vec![info.clone()]).remove(0);
    MenuContext {
        app_name: "Code",
        file_manager: false,
        tabbed: false,
        window: Some(WindowMenuState::of(&info)),
        app: AppMenuState::of(&app, true),
        app_windows: &[],
    }
}

fn ids(rows: &[MenuRow], only_enabled: bool) -> Vec<usize> {
    rows.iter()
        .filter_map(|row| match row {
            MenuRow::Item { id, enabled, .. } if *enabled || !only_enabled => Some(*id),
            _ => None,
        })
        .collect()
}

#[test]
fn popup_menu_layout_sizes_and_hits_enabled_rows() {
    let rows = build_top_bar_menu_rows(HitTarget::WindowMenu, &context(WindowTraits::STANDARD));
    let layout = layout_for(&rows, MenuAnchor::TopBar { x: 200, y: 34 });
    assert!(layout.window.width >= 216);
    assert_eq!(layout.rows.len(), rows.len());
    let first_rect = layout.rows[0].rect;
    assert_eq!(layout.hit_test(first_rect.x + 8, first_rect.y + 8), Some(0));
    assert_eq!(layout.command_at(0), Some(CMD_WIN_MINIMIZE));
    let sep_rect = layout.rows[2].rect;
    assert_eq!(layout.hit_test(sep_rect.x + 8, sep_rect.y + 4), None);
    assert!(!layout.is_scrollable());
}

#[test]
fn keyboard_navigation_cycles_interactive_rows_and_skips_separators() {
    let rows = build_top_bar_menu_rows(HitTarget::FileMenu, &context(WindowTraits::STANDARD));
    let layout = layout_for(&rows, MenuAnchor::TopBar { x: 100, y: 34 });
    let first = layout.next_interactive_row(None);
    assert_eq!(first, Some(0));
    let second = layout.next_interactive_row(first);
    assert_eq!(second, Some(1));
    let third = layout.next_interactive_row(second);
    assert_eq!(third, Some(3));
    assert_eq!(layout.prev_interactive_row(third), Some(1));
    assert_eq!(
        layout.prev_interactive_row(first),
        layout.interactive_rows().last().copied()
    );
}

#[test]
fn type_to_select_cycles_through_rows_sharing_a_first_letter() {
    let rows = build_top_bar_menu_rows(HitTarget::FileMenu, &context(WindowTraits::STANDARD));
    let layout = layout_for(&rows, MenuAnchor::TopBar { x: 100, y: 34 });
    let save = layout.find_row_by_prefix(&rows, 's', None);
    let save_as = layout.find_row_by_prefix(&rows, 'S', save);
    assert_eq!(save, Some(3));
    assert_eq!(save_as, Some(4));
    assert_eq!(layout.find_row_by_prefix(&rows, 's', save_as), save);
    assert_eq!(layout.find_row_by_prefix(&rows, 'z', None), None);
}

#[test]
fn menus_without_a_target_window_offer_no_window_or_shortcut_actions() {
    let bare = MenuContext::without_window("Finder");
    for target in APPLICATION_MENUS {
        let rows = build_top_bar_menu_rows(target, &bare);
        let enabled = ids(&rows, true);
        assert!(
            enabled
                .iter()
                .all(|id| matches!(*id, CMD_TASK_VIEW | CMD_SHOW_DESKTOP)),
            "{target:?} offered {enabled:?}"
        );
        assert!(!rows.is_empty());
    }
}

#[test]
fn unsupported_window_actions_are_absent_and_blocked_ones_are_disabled() {
    let mut fixed = WindowTraits::STANDARD;
    fixed.maximizable = false;
    fixed.resizable = false;
    let rows = build_top_bar_menu_rows(HitTarget::WindowMenu, &context(fixed));
    let all = ids(&rows, false);
    assert!(all.contains(&CMD_WIN_MINIMIZE));
    assert!(!all.contains(&CMD_WIN_MAXIMIZE));
    assert!(!all.contains(&CMD_TILE_LEFT));

    let mut elevated = WindowTraits::STANDARD;
    elevated.elevated = true;
    let rows = build_top_bar_menu_rows(HitTarget::EditMenu, &context(elevated));
    assert!(ids(&rows, true).is_empty());
    assert!(ids(&rows, false).contains(&CMD_EDIT_PASTE));
}

#[test]
fn full_screen_row_appears_only_for_applications_that_support_it() {
    let plain = build_top_bar_menu_rows(HitTarget::ViewMenu, &context(WindowTraits::STANDARD));
    assert!(!ids(&plain, false).contains(&CMD_VIEW_FULLSCREEN));
    let mut capable = WindowTraits::STANDARD;
    capable.full_screen_shortcut = true;
    let rows = build_top_bar_menu_rows(HitTarget::ViewMenu, &context(capable));
    assert!(ids(&rows, true).contains(&CMD_VIEW_FULLSCREEN));
}

#[test]
fn app_menu_separates_hide_quit_and_window_close() {
    let rows = build_top_bar_menu_rows(HitTarget::AppMenu, &context(WindowTraits::STANDARD));
    let enabled = ids(&rows, true);
    assert!(enabled.contains(&CMD_APP_SETTINGS));
    assert!(enabled.contains(&CMD_APP_HIDE));
    assert!(enabled.contains(&CMD_APP_HIDE_OTHERS));
    assert!(enabled.contains(&CMD_APP_QUIT));
    assert!(!enabled.contains(&CMD_TASKMGR));
    assert!(!enabled.contains(&CMD_WIN_CLOSE));
    assert!(!enabled.contains(&CMD_WIN_MINIMIZE));
    assert!(!enabled.contains(&CMD_APP_SHOW_ALL));
}

#[test]
fn macos_shortcut_glyphs_appear_on_apple_app_file_and_window_menus() {
    fn find_shortcut(rows: &[MenuRow], target_id: usize) -> Option<&str> {
        rows.iter().find_map(|row| match row {
            MenuRow::Item { id, shortcut, .. } if *id == target_id => Some(shortcut.as_str()),
            _ => None,
        })
    }
    let apple = build_apple_menu_rows();
    assert_eq!(find_shortcut(&apple, CMD_TASKMGR), Some("⌥⌘⎋"));
    assert_eq!(find_shortcut(&apple, CMD_LOCK), Some("⌃⌘Q"));

    let ctx = context(WindowTraits::STANDARD);
    let app = build_top_bar_menu_rows(HitTarget::AppMenu, &ctx);
    assert_eq!(find_shortcut(&app, CMD_APP_SETTINGS), Some("⌘,"));
    assert_eq!(find_shortcut(&app, CMD_APP_HIDE), Some("⌘H"));
    assert_eq!(find_shortcut(&app, CMD_APP_HIDE_OTHERS), Some("⌥⌘H"));
    assert_eq!(find_shortcut(&app, CMD_APP_QUIT), Some("⌘Q"));

    let file = build_top_bar_menu_rows(HitTarget::FileMenu, &ctx);
    assert_eq!(find_shortcut(&file, CMD_WIN_CLOSE), Some("⌘W"));

    let window_rows = build_top_bar_menu_rows(HitTarget::WindowMenu, &ctx);
    assert_eq!(find_shortcut(&window_rows, CMD_WIN_MINIMIZE), Some("⌘M"));
    assert!(!ids(&window_rows, false).contains(&CMD_WIN_BRING_ALL_TO_FRONT));
}

#[test]
fn window_menu_lists_app_windows_and_bring_all_to_front() {
    let entries = [
        WindowMenuEntry {
            title: String::from("main.rs"),
            active: true,
        },
        WindowMenuEntry {
            title: String::from("lib.rs"),
            active: false,
        },
    ];
    let mut ctx = context(WindowTraits::STANDARD);
    ctx.app_windows = &entries;
    let rows = build_top_bar_menu_rows(HitTarget::WindowMenu, &ctx);
    let enabled = ids(&rows, true);
    assert!(enabled.contains(&CMD_WIN_BRING_ALL_TO_FRONT));
    assert!(enabled.contains(&CMD_DOCK_WINDOW_BASE));
    assert!(enabled.contains(&(CMD_DOCK_WINDOW_BASE + 1)));
    assert!(rows.iter().any(|row| matches!(
        row,
        MenuRow::Item {
            id,
            label,
            checked: true,
            enabled: true,
            ..
        } if *id == CMD_DOCK_WINDOW_BASE && label == "main.rs"
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        MenuRow::Item {
            id,
            label,
            checked: false,
            enabled: true,
            ..
        } if *id == CMD_DOCK_WINDOW_BASE + 1 && label == "lib.rs"
    )));
}

#[test]
fn file_manager_menu_maps_labels_to_distinct_commands() {
    let mut finder = context(WindowTraits::STANDARD);
    finder.file_manager = true;
    let file = ids(&build_top_bar_menu_rows(HitTarget::FileMenu, &finder), true);
    assert!(file.contains(&CMD_FILE_NEW_FOLDER));
    assert!(file.contains(&CMD_FILE_NEW));
    assert!(!file.contains(&CMD_FILE_SAVE));
    let view = ids(&build_top_bar_menu_rows(HitTarget::ViewMenu, &finder), true);
    assert_eq!(
        &view[..3],
        &[CMD_VIEW_AS_ICONS, CMD_VIEW_AS_LIST, CMD_VIEW_AS_DETAILS]
    );
}

#[test]
fn above_dock_anchor_centers_and_clamps_within_bounds() {
    let state = WindowMenuState::of(&window(WindowTraits::STANDARD));
    let rows = build_window_tiling_menu_rows(&state);
    let layout = layout_for(
        &rows,
        MenuAnchor::AboveDock {
            center_x: 960,
            bottom_y: 1000,
        },
    );
    let center = layout.window.x + layout.window.width / 2;
    assert!((center - 960).abs() <= 1);
    assert_eq!(layout.window.bottom(), 1000);
}

#[test]
fn popup_is_placed_inside_the_anchor_monitor_work_area() {
    let secondary = Rect {
        x: -1280,
        y: 40,
        width: 1280,
        height: 720,
    };
    let rows = build_apple_menu_rows();
    let layout = compute_popup_menu_layout(
        &PopupMenuMetrics::MACOS_SONOMA,
        placement(secondary, MenuAnchor::BelowPoint { x: -40, y: 0 }, 0),
        &rows,
        &TEXT,
    );
    assert!(layout.window.x >= secondary.x);
    assert!(layout.window.right() <= secondary.right());
    assert!(layout.window.y >= secondary.y);
    assert!(layout.window.bottom() <= secondary.bottom());
}

#[test]
fn tall_menu_scrolls_so_every_row_stays_reachable() {
    let rows: Vec<MenuRow> = (0..60)
        .map(|index| MenuRow::item(CMD_DOCK_WINDOW_BASE + index, format!("Window {index}"), ""))
        .collect();
    let small = Rect {
        x: 0,
        y: 0,
        width: 800,
        height: 400,
    };
    let anchor = MenuAnchor::BelowPoint { x: 10, y: 10 };
    let mut layout = compute_popup_menu_layout(
        &PopupMenuMetrics::MACOS_SONOMA,
        placement(small, anchor, 0),
        &rows,
        &TEXT,
    );
    assert!(layout.is_scrollable());
    assert!(layout.window.bottom() <= small.bottom());
    let mut reached = Vec::new();
    for index in 0..rows.len() {
        let offset = layout.scroll_revealing(index);
        layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            placement(small, anchor, offset),
            &rows,
            &TEXT,
        );
        let row = layout.rows[index];
        assert!(row.rect.y >= layout.viewport.y);
        assert!(row.rect.bottom() <= layout.viewport.bottom());
        if layout.hit_test(row.rect.x + 4, row.rect.y + 4) == Some(index) {
            reached.push(index);
        }
    }
    assert_eq!(reached.len(), rows.len());
    assert_eq!(layout.scrolled_by(10_000), layout.max_scroll());
    assert_eq!(layout.scrolled_by(-10_000), 0);
}

#[test]
fn rows_scrolled_outside_the_viewport_are_not_hit() {
    let rows: Vec<MenuRow> = (0..60)
        .map(|index| MenuRow::item(300 + index, format!("Row {index}"), ""))
        .collect();
    let small = Rect {
        x: 0,
        y: 0,
        width: 800,
        height: 300,
    };
    let layout = compute_popup_menu_layout(
        &PopupMenuMetrics::MACOS_SONOMA,
        placement(small, MenuAnchor::BelowPoint { x: 10, y: 10 }, 48),
        &rows,
        &TEXT,
    );
    let hidden = layout.rows[0];
    assert!(!layout.is_row_reachable(&hidden));
    assert_eq!(layout.hit_test(hidden.rect.x + 4, hidden.rect.y + 4), None);
    assert_eq!(layout.hit_test(8, 2), None);
}

#[test]
fn shortcut_column_is_sized_from_measured_text() {
    struct Wide;
    impl MenuTextMeasure for Wide {
        fn label_width(&self, text: &str) -> i32 {
            text.chars().count() as i32 * 9
        }
        fn shortcut_width(&self, _text: &str) -> i32 {
            64
        }
    }
    let rows = vec![MenuRow::item(1, "Save", "Ctrl+Shift+S")];
    let layout = compute_popup_menu_layout(
        &PopupMenuMetrics::MACOS_SONOMA,
        placement(SCREEN, MenuAnchor::BelowPoint { x: 10, y: 10 }, 0),
        &rows,
        &Wide,
    );
    assert!((layout.shortcut_col_width - 64.0).abs() < f32::EPSILON);
    let label_right =
        layout.window.width as f32 - layout.right_pad - layout.shortcut_col_width - 24.0 - 5.0;
    assert!(label_right >= layout.check_col_width + 36.0);
}

#[test]
fn calendar_row_is_a_fixed_height_grid_and_only_the_header_controls_are_interactive() {
    let rows = build_calendar_rows(2026, 11, Some(11));
    let layout = layout_for(&rows, MenuAnchor::BelowPoint { x: 1500, y: 34 });
    let cal = layout.rows[0].rect;
    assert!(layout.rows[0].is_calendar);
    assert_eq!(cal.height, calendar_row_height(96));
    assert!(layout.window.width >= scale(CALENDAR_POPUP_WIDTH, 96));
    assert_eq!(layout.hit_test(cal.x + 10, cal.y + 10), None);
    assert_eq!(layout.command_at(2), Some(CMD_CAL_SETTINGS));
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row, MenuRow::Item { .. }))
            .count(),
        1
    );
}

#[test]
fn calendar_header_controls_are_hit_through_the_menu_layout() {
    let rows = build_calendar_rows(2026, 11, Some(11));
    let layout = layout_for(&rows, MenuAnchor::BelowPoint { x: 1500, y: 34 });
    let calendar = compute_calendar_layout(layout.rows[0].rect, layout.dpi, 2026, 11);
    for (control, rect) in calendar.controls {
        let (x, y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
        assert_eq!(layout.calendar_control_at(&rows, x, y), Some(control));
    }
    let cell = calendar.cells[10].rect;
    assert_eq!(
        layout.calendar_control_at(&rows, cell.x + 4, cell.y + 4),
        None
    );
    let settings = layout.rows[2].rect;
    assert_eq!(
        layout.calendar_control_at(&rows, settings.x + 4, settings.y + 4),
        None
    );
}

#[test]
fn top_bar_menu_targets_are_recognized() {
    assert!(is_top_bar_menu_target(HitTarget::Start));
    assert!(is_top_bar_menu_target(HitTarget::AppMenu));
    assert!(is_top_bar_menu_target(HitTarget::HelpMenu));
    assert!(is_top_bar_menu_target(HitTarget::MenuOverflow));
    assert!(!is_top_bar_menu_target(HitTarget::Notch));
}

#[test]
fn overflow_rows_round_trip_to_their_menu_targets() {
    let hidden = [
        HitTarget::WindowMenu,
        HitTarget::HelpMenu,
        HitTarget::Search,
    ];
    let rows = build_menu_overflow_rows(&hidden, "Code");
    let commands = ids(&rows, true);
    assert_eq!(commands.len(), 3);
    assert_eq!(
        overflow_menu_target(commands[0]),
        Some(HitTarget::WindowMenu)
    );
    assert_eq!(overflow_menu_target(commands[1]), Some(HitTarget::HelpMenu));
    assert_eq!(overflow_menu_target(commands[2]), Some(HitTarget::Search));
    assert_eq!(overflow_menu_target(CMD_OVERFLOW_MENU_BASE + 13), None);
    assert_eq!(overflow_menu_target(3), None);
}

#[test]
fn tiling_menu_reports_when_no_action_is_possible() {
    let mut locked = WindowTraits::STANDARD;
    locked.closable = false;
    locked.minimizable = false;
    locked.maximizable = false;
    locked.resizable = false;
    let rows = build_window_tiling_menu_rows(&WindowMenuState::of(&window(locked)));
    assert_eq!(rows.len(), 1);
    assert!(ids(&rows, true).is_empty());
}

#[test]
fn notification_rows_group_apps_and_cover_access_modes() {
    let populated = NotificationCenterSnapshot {
        access: NotificationAccess::Allowed,
        items: (0..7)
            .map(|i| NotificationSummary {
                id: 100 + i,
                app_id: if i == 1 {
                    String::from("Contoso.Chat")
                } else {
                    String::from("Contoso.Mail")
                },
                app_name: if i == 1 {
                    String::from("Chat")
                } else {
                    String::from("Mail")
                },
                title: format!("Message {i}"),
                body: String::from("Body preview"),
                timestamp_label: format_relative_age(120),
                icon_bytes: None,
            })
            .collect(),
    };
    let rows = build_notification_rows(&populated);
    let notif_rows: Vec<_> = rows
        .iter()
        .filter(|row| matches!(row, MenuRow::Notification { .. }))
        .collect();
    assert_eq!(notif_rows.len(), MAX_VISIBLE_NOTIFICATIONS);
    assert!(rows.iter().any(|row| matches!(
        row,
        MenuRow::Item { label, enabled: false, .. } if label == "Mail (4)"
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        MenuRow::Item { label, enabled: false, .. } if label == "Chat"
    )));
    assert_eq!(
        notification_dismiss_index(CMD_NOTIF_DISMISS_BASE + 2, 5),
        Some(2)
    );
    assert_eq!(
        notification_dismiss_index(CMD_NOTIF_DISMISS_BASE + 5, 5),
        None
    );
    assert_eq!(notification_dismiss_index(CMD_NOTIF_CLEAR_ALL, 5), None);

    let layout = layout_for(&rows, MenuAnchor::BelowPoint { x: 1500, y: 34 });
    assert!(layout.window.width >= 284);
    assert!(rows.iter().any(|row| matches!(
        row,
        MenuRow::Item { id, .. } if *id == CMD_NOTIF_CLEAR_ALL
    )));

    let denied = NotificationCenterSnapshot {
        access: NotificationAccess::Denied,
        items: Vec::new(),
    };
    let denied_rows = build_notification_rows(&denied);
    assert!(denied_rows.iter().any(|row| matches!(
        row,
        MenuRow::Item { id, .. } if *id == CMD_NOTIF_REQUEST_ACCESS
    )));

    let unpackaged = NotificationCenterSnapshot::default();
    let unpackaged_rows = build_notification_rows(&unpackaged);
    assert!(unpackaged_rows.iter().any(|row| matches!(
        row,
        MenuRow::Item { id, .. } if *id == CMD_CC_CALENDAR
    )));
}

#[test]
fn popup_menu_shell_and_row_pills_are_concentric() {
    let metrics = PopupMenuMetrics::MACOS_SONOMA;
    assert_eq!(metrics.corner_radius, 13);
    assert_eq!(metrics.row_corner_radius, 8);
    assert_eq!(metrics.pad_x, 5);
    assert_eq!(metrics.pad_y, 5);
    assert_eq!(
        metrics.corner_radius - metrics.pad_x,
        metrics.row_corner_radius
    );
    let rows = vec![MenuRow::item(1, "New Window", "⌘N")];
    let layout = layout_for(&rows, MenuAnchor::TopBar { x: 100, y: 34 });
    assert!((layout.corner_radius - 13.0).abs() < f32::EPSILON);
    assert!((layout.row_corner_radius - 8.0).abs() < f32::EPSILON);
    assert_eq!(layout.rows[0].rect.x, 5);
    assert_eq!(layout.rows[0].rect.y, 5);
}

#[test]
fn notification_badge_uses_a_stable_initial_and_color_per_app() {
    let mail = notification_badge("Mail");
    assert_eq!(mail.initial, 'M');
    assert_eq!(mail, notification_badge("Mail"));
    assert_eq!(notification_badge("  slack").initial, 'S');
    assert_eq!(notification_badge("").initial, '?');
    assert_eq!(notification_badge("!!").initial, '?');
}

#[test]
fn notification_badge_ink_meets_wcag_aa_for_all_palette_colors() {
    let linear = |v: u8| {
        let c = f32::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    for color in super::commands::NOTIFICATION_BADGE_COLORS {
        let bg = 0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2]);
        let fg = if badge_ink_is_dark(color) { 0.0 } else { 1.0 };
        let (hi, lo) = if fg > bg { (fg, bg) } else { (bg, fg) };
        let ratio = (hi + 0.05) / (lo + 0.05);
        assert!(
            ratio >= 3.2,
            "color {color:?} ratio {ratio:.2} below accessible threshold"
        );
    }
}

#[test]
fn tray_menu_exposes_settings_control_center_module_checks_and_quit() {
    let rows = build_tray_menu_rows(false, true, false);
    let commands = ids(&rows, true);
    assert_eq!(
        commands,
        vec![
            CMD_OTTO_SETTINGS,
            CMD_CONTROL_CENTER,
            CMD_TOGGLE_TOP_BAR,
            CMD_TOGGLE_NOTCH,
            CMD_TOGGLE_DOCK,
            CMD_OPEN_LOGS,
            CMD_QUIT,
        ]
    );
    let checked = |target: usize| {
        rows.iter().find_map(|row| match row {
            MenuRow::Item { id, checked, .. } if *id == target => Some(*checked),
            _ => None,
        })
    };
    assert_eq!(checked(CMD_TOGGLE_TOP_BAR), Some(false));
    assert_eq!(checked(CMD_TOGGLE_NOTCH), Some(true));
    assert_eq!(checked(CMD_TOGGLE_DOCK), Some(false));
}
