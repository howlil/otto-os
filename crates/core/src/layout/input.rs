use crate::bar::menu_bar::BarItemSet;
use crate::notch::clipboard::ClipboardLayoutInput;
use crate::notch::island::CompactSlotKind;
use crate::notch::media::NotchExpandedTab;
use crate::notch::pomodoro::FocusSubMode;
use crate::notch::pomodoro_view::PomodoroSurface;
use crate::notch::tasks::TasksLayoutInput;

use super::notification::NotificationLayoutInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrivacyLayoutState {
    pub compact_dots: bool,
    pub panel_mic_control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShelfLayoutInput {
    pub visible_count: u8,
    pub has_pager: bool,
    pub has_items: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent layout feature flags"
)]
pub struct NotchLayoutInput {
    pub hud_expansion: f32,
    pub compact_expansion: f32,
    pub secondary_expansion: f32,
    pub compact_swap_blend: f32,
    pub compact_primary: Option<CompactSlotKind>,
    pub compact_secondary: Option<CompactSlotKind>,
    pub panel_expansion: f32,
    pub pomodoro_surface: PomodoroSurface,
    pub focus_mode: FocusSubMode,
    pub active_tab: NotchExpandedTab,
    pub previous_tab: NotchExpandedTab,
    pub top_bar_enabled: bool,
    pub notch_enabled: bool,
    pub control_center_enabled: bool,
    pub focus_enabled: bool,
    pub shelf_enabled: bool,
    pub clipboard_enabled: bool,
    pub tasks_enabled: bool,
    pub media_present: bool,
    pub lyrics_progress: f32,
    pub tab_blend: f32,
    pub compact_title_points: i32,
    pub media_live: bool,
    pub press_progress: f32,
    pub privacy: PrivacyLayoutState,
    pub shelf: ShelfLayoutInput,
    pub clipboard: ClipboardLayoutInput,
    pub hidden_items: BarItemSet,
    pub notification: Option<NotificationLayoutInput>,
    pub tasks: TasksLayoutInput,
}

impl Default for NotchLayoutInput {
    fn default() -> Self {
        Self {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            secondary_expansion: 0.0,
            compact_swap_blend: 1.0,
            compact_primary: None,
            compact_secondary: None,
            panel_expansion: 0.0,
            pomodoro_surface: PomodoroSurface::Setup,
            focus_mode: FocusSubMode::Pomodoro,
            active_tab: NotchExpandedTab::Pomodoro,
            previous_tab: NotchExpandedTab::Pomodoro,
            top_bar_enabled: true,
            notch_enabled: true,
            control_center_enabled: true,
            focus_enabled: true,
            shelf_enabled: true,
            clipboard_enabled: true,
            tasks_enabled: true,
            media_present: true,
            lyrics_progress: 0.0,
            tab_blend: 1.0,
            compact_title_points: 0,
            media_live: false,
            press_progress: 0.0,
            privacy: PrivacyLayoutState::default(),
            shelf: ShelfLayoutInput::default(),
            clipboard: ClipboardLayoutInput::default(),
            hidden_items: BarItemSet::default(),
            notification: None,
            tasks: TasksLayoutInput::default(),
        }
    }
}
