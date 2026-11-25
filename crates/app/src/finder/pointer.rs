use std::time::{Duration, Instant};

use otto_core::{
    FINDER_ICON_SIZE_MAX, FINDER_ICON_SIZE_MIN, FinderFlags, FinderHit, FinderLocation, FinderPort,
    FinderToolbarButton, FinderUndoAction, FinderViewMode, Rect, WindowControlAction,
    build_finder_action_menu_rows, build_finder_context_menu_rows, build_finder_sort_menu_rows,
    confirm_permanent_delete, open_selected, submit_go_to_folder,
};
use otto_platform::{
    WindowsFinderPort, capture_mouse, client_to_screen, current_process_id,
    invalidate_settings_window, is_virtual_key_down, kill_window_timer, minimize_settings_window,
    release_capture_if_owned, set_window_timer, toggle_settings_window_maximize,
};

use crate::host::RuntimeContext;
use crate::win_msg::{FINDER_SPRING_TIMER, GREEN_HOVER_TIMER};
use crate::window_controls::{GREEN_HOVER_MS, open_tiling_menu};

use super::lifecycle::close;
use super::{
    FinderWindow, UI_DRAGGING_SIDEBAR, UI_DRAGGING_SLIDER, UI_GREEN_HOVER_ARMED, with_window,
};

const DOUBLE_CLICK_MS: u64 = 420;
const SPRING_LOAD_MS: u32 = 600;

pub(super) fn on_mouse_move(runtime: &RuntimeContext, x: i32, y: i32) {
    let mut needs_repaint = false;
    let mut arm_green = None;
    let mut disarm_green = None;
    let _ = with_window(runtime, |window| {
        if window.flag(UI_DRAGGING_SIDEBAR) {
            let pts = (x * 96) / window.dpi.max(96) as i32;
            window.state.set_sidebar_width(pts);
            window.relayout();
            needs_repaint = true;
            return;
        }
        if window.flag(UI_DRAGGING_SLIDER)
            && let Some(slider) = window.layout.icon_size_slider
        {
            update_icon_size_from_x(window, slider, x);
            needs_repaint = true;
            return;
        }
        if let Some((sx, sy)) = window.marquee_start {
            let rect = Rect::new(sx.min(x), sy.min(y), (x - sx).abs(), (y - sy).abs());
            window.marquee_rect = Some(rect);
            select_marquee_items(window, rect);
            needs_repaint = true;
            return;
        }
        if window.drag_paths.is_some() {
            handle_drag_spring_hover(window, x, y);
        }
        let hit = window.layout.hit_test(x, y);
        if window.hovered != hit {
            let on_green = matches!(
                hit,
                Some(FinderHit::WindowControl(
                    WindowControlAction::ToggleMaximize
                ))
            );
            if on_green && !window.flag(UI_GREEN_HOVER_ARMED) {
                window.set_flag(UI_GREEN_HOVER_ARMED, true);
                arm_green = Some(window.hwnd);
            } else if !on_green && window.flag(UI_GREEN_HOVER_ARMED) {
                window.set_flag(UI_GREEN_HOVER_ARMED, false);
                disarm_green = Some(window.hwnd);
            }
            window.hovered = hit;
            needs_repaint = true;
        }
    });
    if let Some(hwnd) = arm_green {
        let _ = set_window_timer(hwnd, GREEN_HOVER_TIMER, GREEN_HOVER_MS);
    }
    if let Some(hwnd) = disarm_green {
        kill_window_timer(hwnd, GREEN_HOVER_TIMER);
    }
    if needs_repaint && let Some(hwnd) = with_window(runtime, |w| w.hwnd) {
        invalidate_settings_window(hwnd);
    }
}

fn handle_drag_spring_hover(window: &mut FinderWindow, x: i32, y: i32) {
    let hover_folder = match window.layout.hit_test(x, y) {
        Some(FinderHit::Item { path, .. }) => window
            .state
            .find_entry_by_path(&path)
            .filter(otto_core::FinderEntry::is_dir)
            .map(|e| e.path),
        _ => None,
    };
    match hover_folder {
        Some(path) => {
            let same = window
                .spring_target
                .as_ref()
                .is_some_and(|(prev, _)| prev.eq_ignore_ascii_case(&path));
            if !same {
                window.spring_target = Some((path, Instant::now()));
                let _ = set_window_timer(window.hwnd, FINDER_SPRING_TIMER, SPRING_LOAD_MS);
            }
        }
        None => {
            if window.spring_target.take().is_some() {
                kill_window_timer(window.hwnd, FINDER_SPRING_TIMER);
            }
        }
    }
}

pub(super) fn on_spring_timer(runtime: &RuntimeContext) {
    let port = WindowsFinderPort;
    with_window(runtime, |window| {
        kill_window_timer(window.hwnd, FINDER_SPRING_TIMER);
        let Some((path, started)) = window.spring_target.take() else {
            return;
        };
        if window.drag_paths.is_none() || started.elapsed() < Duration::from_millis(400) {
            return;
        }
        if window.state.tab().view_mode == FinderViewMode::List {
            window.state.toggle_folder_expand(&port, &path);
        } else {
            window.navigate_to(FinderLocation::Path(path));
        }
        window.relayout();
        invalidate_settings_window(window.hwnd);
    });
}

pub(super) fn on_left_down(runtime: &RuntimeContext, x: i32, y: i32) {
    let ctrl = is_virtual_key_down(0x11);
    let shift = is_virtual_key_down(0x10);
    let port = WindowsFinderPort;
    let mut capture_hwnd = None;
    let mut should_repaint = false;
    let _ = with_window(runtime, |window| {
        let hit = window.layout.hit_test(x, y);
        window.pressed.clone_from(&hit);
        if !matches!(hit, Some(FinderHit::SearchBox))
            && window.state.flags.has(FinderFlags::SEARCH_FOCUSED)
        {
            window.state.flags.set(FinderFlags::SEARCH_FOCUSED, false);
            should_repaint = true;
        }
        if window.state.rename.is_some() {
            let _ = window.state.commit_rename(&port);
            window.relayout();
            should_repaint = true;
        }
        match hit {
            Some(FinderHit::SidebarResizeHandle) => {
                window.set_flag(UI_DRAGGING_SIDEBAR, true);
                capture_hwnd = Some(window.hwnd);
            }
            Some(FinderHit::IconSizeSlider) => {
                window.set_flag(UI_DRAGGING_SLIDER, true);
                if let Some(slider) = window.layout.icon_size_slider {
                    update_icon_size_from_x(window, slider, x);
                }
                capture_hwnd = Some(window.hwnd);
                should_repaint = true;
            }
            Some(FinderHit::EmptyContent) => {
                window.state.tab_mut().selected_paths.clear();
                window.marquee_start = Some((x, y));
                window.marquee_rect = None;
                capture_hwnd = Some(window.hwnd);
                should_repaint = true;
            }
            Some(FinderHit::Item { path, col_index }) => {
                handle_item_down(window, port, &path, col_index, ctrl, shift);
                capture_hwnd = Some(window.hwnd);
                should_repaint = true;
            }
            Some(_) => {
                capture_hwnd = Some(window.hwnd);
                should_repaint = true;
            }
            None => {}
        }
    });
    if let Some(hwnd) = capture_hwnd {
        capture_mouse(hwnd);
    }
    if should_repaint && let Some(hwnd) = with_window(runtime, |w| w.hwnd) {
        invalidate_settings_window(hwnd);
    }
}

fn handle_item_down(
    window: &mut FinderWindow,
    port: WindowsFinderPort,
    path: &str,
    col_index: Option<usize>,
    ctrl: bool,
    shift: bool,
) {
    let now = Instant::now();
    let is_double = window.last_click.as_ref().is_some_and(|(prev, at)| {
        prev.eq_ignore_ascii_case(path) && at.elapsed() <= Duration::from_millis(DOUBLE_CLICK_MS)
    });
    if is_double && !ctrl && !shift {
        window.last_click = None;
        window.drag_paths = None;
        window.state.select_path(&port, path, false, false);
        open_selected(&mut window.state, &port);
        window.relayout();
        window.sync_watcher();
        return;
    }
    window.last_click = Some((String::from(path), now));
    if let Some(ci) = col_index {
        window.state.select_miller_item(&port, ci, path, ctrl);
    } else {
        window.state.select_path(&port, path, ctrl, shift);
    }
    window.drag_paths = Some(window.state.tab().selected_paths.clone());
    window.relayout();
    window.sync_watcher();
}

pub(super) fn on_left_up(runtime: &RuntimeContext, x: i32, y: i32) {
    let mut capture_hwnd = None;
    let mut action_close = false;
    let mut action_minimize = None;
    let mut action_maximize = None;
    let mut open_sort_at = None;
    let mut open_action_at = None;
    let port = WindowsFinderPort;

    let _ = with_window(runtime, |window| {
        capture_hwnd = Some(window.hwnd);
        window.set_flag(UI_DRAGGING_SIDEBAR, false);
        window.set_flag(UI_DRAGGING_SLIDER, false);
        window.marquee_start = None;
        window.marquee_rect = None;
        kill_window_timer(window.hwnd, FINDER_SPRING_TIMER);
        window.spring_target = None;

        let pressed = window.pressed.take();
        let hit = window.layout.hit_test(x, y);
        if let Some(dragged) = window.drag_paths.take()
            && try_complete_internal_drop(window, port, &dragged, hit.as_ref())
        {
            window.relayout();
            invalidate_settings_window(window.hwnd);
            return;
        }
        if pressed != hit {
            invalidate_settings_window(window.hwnd);
            return;
        }
        let Some(hit) = hit else {
            return;
        };
        match hit {
            FinderHit::WindowControl(WindowControlAction::Close) => action_close = true,
            FinderHit::WindowControl(WindowControlAction::Minimize) => {
                action_minimize = Some(window.hwnd);
            }
            FinderHit::WindowControl(WindowControlAction::ToggleMaximize) => {
                action_maximize = Some(window.hwnd);
            }
            FinderHit::Toolbar(FinderToolbarButton::SortMenu) => {
                open_sort_at = Some((window.hwnd, x, y));
            }
            FinderHit::Toolbar(FinderToolbarButton::ActionMenu) => {
                open_action_at = Some((window.hwnd, x, y));
            }
            other => {
                apply_click_hit(window, port, other);
                window.relayout();
                window.sync_watcher();
                invalidate_settings_window(window.hwnd);
            }
        }
    });

    if let Some(hwnd) = capture_hwnd {
        let _ = release_capture_if_owned(hwnd);
    }

    if action_close {
        close(runtime);
    } else if let Some(hwnd) = action_minimize {
        if !crate::genie::minimize_window(runtime, hwnd) {
            minimize_settings_window(hwnd);
        }
    } else if let Some(hwnd) = action_maximize {
        toggle_settings_window_maximize(hwnd);
    } else if let Some((hwnd, px, py)) = open_sort_at {
        open_finder_popup(runtime, hwnd, px, py, FinderPopupKind::Sort);
    } else if let Some((hwnd, px, py)) = open_action_at {
        open_finder_popup(runtime, hwnd, px, py, FinderPopupKind::Action);
    }
}

fn try_complete_internal_drop(
    window: &mut FinderWindow,
    port: WindowsFinderPort,
    dragged: &[String],
    hit: Option<&FinderHit>,
) -> bool {
    let dest_dir = match hit {
        Some(FinderHit::SidebarItem(idx)) => window
            .state
            .sidebar_items
            .get(*idx)
            .and_then(|item| item.location.fs_path().map(str::to_owned)),
        Some(FinderHit::Item { path, .. }) => {
            if dragged.iter().any(|src| src.eq_ignore_ascii_case(path)) {
                None
            } else {
                window
                    .state
                    .find_entry_by_path(path)
                    .filter(otto_core::FinderEntry::is_dir)
                    .map(|entry| entry.path)
            }
        }
        _ => None,
    };
    let Some(dest_dir) = dest_dir else {
        return false;
    };
    let copy_mode = is_virtual_key_down(0x11);
    let outcome = port.paste_paths(dragged, &dest_dir, !copy_mode);
    let action = if copy_mode { "Copy" } else { "Move" };
    let mutated = outcome.any_mutated();
    if mutated {
        let completed = outcome.completed_pairs();
        if !copy_mode && !completed.is_empty() {
            window
                .state
                .push_undo(FinderUndoAction::Move { moves: completed });
        }
        window.state.reload_active_tab(&port);
    }
    window.state.status_notice = outcome.summary_notice(action);
    mutated || window.state.status_notice.is_some()
}

fn apply_click_hit(window: &mut FinderWindow, port: WindowsFinderPort, hit: FinderHit) {
    match hit {
        FinderHit::Toolbar(FinderToolbarButton::ToggleSidebar) => {
            window.state.flags.toggle(FinderFlags::SIDEBAR);
        }
        FinderHit::Toolbar(FinderToolbarButton::Back) => {
            let _ = window.state.go_back(&port);
        }
        FinderHit::Toolbar(FinderToolbarButton::Forward) => {
            let _ = window.state.go_forward(&port);
        }
        FinderHit::Toolbar(FinderToolbarButton::ViewMode(mode)) => {
            window.state.set_view_mode(&port, mode);
        }
        FinderHit::Toolbar(FinderToolbarButton::ToggleInspector) => {
            window.state.flags.toggle(FinderFlags::INSPECTOR);
            window.state.refresh_preview(&port);
        }
        FinderHit::SearchBox => window.state.flags.set(FinderFlags::SEARCH_FOCUSED, true),
        FinderHit::SearchClear => {
            window.state.tab_mut().search_query.clear();
            window.state.reload_active_tab(&port);
        }
        FinderHit::SearchScopeToggle => {
            window.state.flags.toggle(FinderFlags::SEARCH_RECURSIVE);
            window.state.reload_active_tab(&port);
        }
        FinderHit::SidebarItem(idx) => {
            if let Some(item) = window.state.sidebar_items.get(idx).cloned() {
                window.navigate_to(item.location);
            }
        }
        FinderHit::Tab(idx) => window.state.select_tab(&port, idx),
        FinderHit::TabClose(idx) => {
            let _ = window.state.close_tab(&port, idx);
        }
        FinderHit::TabAdd => {
            let loc = window.state.tab().location.clone();
            window.state.open_tab(&port, loc);
        }
        FinderHit::ListColumnHeader(key) => window.state.set_sort(&port, key),
        FinderHit::ListChevron(path) => window.state.toggle_folder_expand(&port, &path),
        FinderHit::Breadcrumb(idx) => {
            if let Some(crumb) = window.layout.breadcrumbs.get(idx).cloned() {
                window.navigate_to(crumb.breadcrumb.location);
            }
        }
        other => apply_overlay_click_hit(window, port, &other),
    }
}

fn apply_overlay_click_hit(window: &mut FinderWindow, port: WindowsFinderPort, hit: &FinderHit) {
    match hit {
        FinderHit::QuickLookClose | FinderHit::QuickLookBackdrop => {
            window.state.flags.set(FinderFlags::QUICK_LOOK, false);
        }
        FinderHit::QuickLookOpen => {
            window.state.flags.set(FinderFlags::QUICK_LOOK, false);
            open_selected(&mut window.state, &port);
        }
        FinderHit::QuickLookPrev => window.state.step_selection(&port, -1, false),
        FinderHit::QuickLookNext => window.state.step_selection(&port, 1, false),
        FinderHit::InspectorTag(tag) => window.state.toggle_tag_on_selection(&port, *tag),
        FinderHit::InspectorOpen => open_selected(&mut window.state, &port),
        FinderHit::InspectorCopyPath => {
            if let Some(target) = window
                .state
                .tab()
                .primary_selected_path()
                .map(str::to_owned)
                .or_else(|| window.state.active_dir_path())
            {
                let _ = port.copy_text_to_clipboard(&target);
                window.state.status_notice = Some(String::from("Path copied"));
            }
        }
        FinderHit::GoToGo => submit_go_to_folder(&mut window.state, &port),
        FinderHit::GoToCancel => {
            window.state.flags.set(FinderFlags::GO_TO_FOLDER, false);
            window.state.go_to.error = None;
        }
        FinderHit::BannerPrimary => {
            if window.state.flags.has(FinderFlags::CONFIRM_PERM_DELETE) {
                confirm_permanent_delete(&mut window.state, &port);
            } else if window.state.tab().location == FinderLocation::Trash {
                if !port.empty_trash() {
                    window.state.status_notice = Some(String::from("Could not empty Trash"));
                }
                window.state.reload_active_tab(&port);
            }
        }
        FinderHit::BannerSecondary => {
            if window.state.flags.has(FinderFlags::CONFIRM_PERM_DELETE) {
                window
                    .state
                    .flags
                    .set(FinderFlags::CONFIRM_PERM_DELETE, false);
                window.state.pending_perm_delete.clear();
            } else if window.state.tab().location == FinderLocation::Trash {
                let paths = window.state.tab().selected_paths.clone();
                if !paths.is_empty() {
                    let outcome = port.restore_trash_paths(&paths);
                    let failed = outcome.failed_sources();
                    if outcome.any_mutated() {
                        window.state.reload_active_tab(&port);
                    }
                    window.state.tab_mut().selected_paths = failed;
                    window.state.status_notice = outcome.summary_notice("Restore");
                }
            }
        }
        _ => {}
    }
}

pub(super) fn on_right_up(runtime: &RuntimeContext, x: i32, y: i32) {
    let port = WindowsFinderPort;
    let mut green_hwnd = None;
    let mut ctx_hwnd = None;
    let _ = with_window(runtime, |window| {
        let hit = window.layout.hit_test(x, y);
        match hit {
            Some(FinderHit::WindowControl(WindowControlAction::ToggleMaximize)) => {
                green_hwnd = Some(window.hwnd);
            }
            Some(FinderHit::Item { path, col_index }) => {
                if !window
                    .state
                    .tab()
                    .selected_paths
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(&path))
                {
                    if let Some(ci) = col_index {
                        window.state.select_miller_item(&port, ci, &path, false);
                    } else {
                        window.state.select_path(&port, &path, false, false);
                    }
                    window.relayout();
                    invalidate_settings_window(window.hwnd);
                }
                ctx_hwnd = Some(window.hwnd);
            }
            Some(FinderHit::EmptyContent) => {
                window.state.tab_mut().selected_paths.clear();
                window.relayout();
                invalidate_settings_window(window.hwnd);
                ctx_hwnd = Some(window.hwnd);
            }
            _ => {}
        }
    });
    if let Some(hwnd) = green_hwnd {
        let (sx, sy) = client_to_screen(hwnd, x, y);
        open_tiling_menu(runtime, hwnd, current_process_id(), sx, sy);
    } else if let Some(hwnd) = ctx_hwnd {
        open_finder_popup(runtime, hwnd, x, y, FinderPopupKind::Context);
    }
}

#[derive(Clone, Copy)]
enum FinderPopupKind {
    Context,
    Sort,
    Action,
}

fn open_finder_popup(runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32, kind: FinderPopupKind) {
    let Some(rows) = with_window(runtime, |window| match kind {
        FinderPopupKind::Context => build_finder_context_menu_rows(
            &window.state,
            !window.state.tab().selected_paths.is_empty(),
        ),
        FinderPopupKind::Sort => build_finder_sort_menu_rows(&window.state),
        FinderPopupKind::Action => build_finder_action_menu_rows(&window.state),
    }) else {
        return;
    };
    let (sx, sy) = client_to_screen(hwnd, x, y);
    crate::menu::open_finder_menu(runtime, sx, sy, rows);
}

fn update_icon_size_from_x(window: &mut FinderWindow, slider: Rect, x: i32) {
    let frac = ((x - slider.x) as f32 / slider.width.max(1) as f32).clamp(0.0, 1.0);
    let size = FINDER_ICON_SIZE_MIN
        + ((FINDER_ICON_SIZE_MAX - FINDER_ICON_SIZE_MIN) as f32 * frac).round() as i32;
    window.state.set_icon_size(size);
    window.relayout();
}

fn select_marquee_items(window: &mut FinderWindow, rect: Rect) {
    let mut hits = Vec::new();
    match window.state.tab().view_mode {
        FinderViewMode::Icon => {
            for tile in &window.layout.icon_tiles {
                if tile.rect.intersection(&rect).is_some() {
                    hits.push(tile.path.clone());
                }
            }
        }
        FinderViewMode::List => {
            for row in &window.layout.list_rows {
                if row.rect.intersection(&rect).is_some() {
                    hits.push(row.path.clone());
                }
            }
        }
        FinderViewMode::Gallery => {
            for thumb in &window.layout.gallery_thumbs {
                if thumb.rect.intersection(&rect).is_some() {
                    hits.push(thumb.path.clone());
                }
            }
        }
        FinderViewMode::Column => {}
    }
    window.state.tab_mut().selected_paths = hits;
}
