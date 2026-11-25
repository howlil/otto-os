use std::time::{Instant, SystemTime, UNIX_EPOCH};

use otto_core::{
    CallTracker, ClipboardPayload, ClipboardState, ClockTime, CompactContent, CompactSlotKind,
    FocusBlocklist, FocusCategoryStore, FocusStatsRange, HeatmapFilter, LyricsStatus,
    MotionPreference, NotchAction, NotchExpandedTab, NotchHudKind, NotificationQueue, NowPlaying,
    PendingSeek, PomodoroClock, PomodoroState, PrivacyState, ShelfPayload, ShelfState, TasksState,
    is_openable_shelf_url, is_valid_shelf_file_path,
};

mod alerts;
mod focus;
mod hud;
mod input;
mod island;
mod media;
mod modules;
mod motion;
mod privacy;
mod tools;

const ALERT_HOLD_MS: u64 = 10_000;
const HOVER_DEBOUNCE_MS: u64 = 80;
const PEEK_FADE_SECONDS: f32 = 0.18;
const GLOW_BREATH_FLOOR: f32 = 0.7;
const GLOW_SCENE_VISIBLE: f32 = 0.05;
const CLIPBOARD_COPIED_BADGE_MS: u64 = 1_200;

struct ActivePeek {
    text: String,
    shown_at: Instant,
    until: Instant,
}

struct MediaPeekState {
    label: &'static str,
    title: String,
    shown_at: Instant,
    until: Instant,
}

pub(crate) fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PanelTrigger {
    #[default]
    Closed,
    Hovered,
    Pinned,
    PinnedHovered,
}

impl PanelTrigger {
    const fn is_open(self) -> bool {
        !matches!(self, Self::Closed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotchEffect {
    Platform(NotchAction),
    PickShelfFiles,
    CopyClipboard(ClipboardPayload),
    SaveTasks,
    OpenTaskEditor {
        initial_text: String,
        allow_keep_open: bool,
    },
    CloseTaskEditor {
        restore_focus: bool,
    },
    OpenFocusActivitySettings,
    OpenFocusModeSettings,
    SelectFocusCategory(String),
    UpdateFocusDuration {
        category_id: String,
        minutes: u32,
    },
    RecordCompletedSession {
        category_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskSubmitResult {
    Saved,
    Unchanged,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotchHotkey {
    ToggleNotch,
    TogglePomodoro,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct NotchUpdate {
    pub changed: bool,
    pub effect: Option<NotchEffect>,
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct NotchController {
    hud_kind: Option<NotchHudKind>,
    hud_expansion: f32,
    hud_level: f32,
    hud_until: Option<Instant>,
    pomodoro: PomodoroState,
    privacy: PrivacyState,
    shelf: ShelfState,
    clipboard: ClipboardState,
    clipboard_copied_until: Option<Instant>,
    tasks: TasksState,
    opened_by_task_editor: bool,
    shelf_drag_leave_until: Option<Instant>,
    opened_by_shelf_drag: bool,
    modal_picker_open: bool,
    drag_out_active: bool,
    compact_expansion: f32,
    compact_velocity: f32,
    secondary_expansion: f32,
    secondary_velocity: f32,
    compact_swap_blend: f32,
    panel_expansion: f32,
    panel_velocity: f32,
    press_active: bool,
    press_progress: f32,
    press_velocity: f32,
    hover_since: Option<Instant>,
    panel_trigger: PanelTrigger,
    alert_until: Option<Instant>,
    notifications: NotificationQueue,
    notification_unread_count: usize,
    dnd_active: bool,
    call: CallTracker,
    call_shown: bool,
    clock: PomodoroClock,
    epoch: Instant,
    last_anim_instant: Instant,
    media: Option<NowPlaying>,
    notch_tab: NotchExpandedTab,
    previous_tab: NotchExpandedTab,
    anim_phase: f32,
    lyrics_expanded: bool,
    lyrics_progress: f32,
    stats_progress: f32,
    stats_range: FocusStatsRange,
    stats_selected_day: Option<usize>,
    tab_blend: f32,
    motion: MotionPreference,
    lyrics: Option<otto_core::LyricsSheet>,
    lyrics_status: LyricsStatus,
    pending_seek: Option<PendingSeek>,
    seek_drag: Option<f32>,
    media_primary: bool,
    dismissed: Option<CompactContent>,
    dismissed_secondary: Option<CompactContent>,
    last_primary_slot: Option<CompactSlotKind>,
    last_secondary_slot: Option<CompactSlotKind>,
    hover_blocked: bool,
    clock_time: ClockTime,
    presented: bool,
    glow_presence: f32,
    dim_presence: f32,
    pulse_presence: f32,
    scene_step_at: Instant,
    peek: Option<ActivePeek>,
    media_peek: Option<MediaPeekState>,
    media_paused_at: Option<Instant>,
    media_dim_presence: f32,
    lyrics_in_compact: bool,
    focus_enabled: bool,
    shelf_enabled: bool,
    clipboard_enabled: bool,
    tasks_enabled: bool,
    media_enabled: bool,
    privacy_enabled: bool,
    hud_enabled: bool,
    compact_title_points: f32,
    compact_title_target: f32,
    compact_title_measured: Option<(String, f32)>,
    focus_blocklist: FocusBlocklist,
    focus_categories: FocusCategoryStore,
    category_picker_open: bool,
    custom_time_input: Option<String>,
    focus_relief_summary: String,
    heatmap_filter: HeatmapFilter,
    heatmap_selected_cell: Option<usize>,
}

impl NotchController {
    pub(crate) fn new(media: Option<NowPlaying>, notch_tab: NotchExpandedTab) -> Self {
        let initial_primary = media
            .as_ref()
            .is_some_and(NowPlaying::is_active)
            .then_some(CompactSlotKind::Media);
        Self {
            hud_kind: None,
            hud_expansion: 0.0,
            hud_level: 0.0,
            hud_until: None,
            pomodoro: PomodoroState::new(),
            privacy: PrivacyState::default(),
            shelf: ShelfState::default(),
            clipboard: ClipboardState::default(),
            clipboard_copied_until: None,
            tasks: TasksState::default(),
            opened_by_task_editor: false,
            shelf_drag_leave_until: None,
            opened_by_shelf_drag: false,
            modal_picker_open: false,
            drag_out_active: false,
            compact_expansion: 0.0,
            compact_velocity: 0.0,
            secondary_expansion: 0.0,
            secondary_velocity: 0.0,
            compact_swap_blend: 1.0,
            panel_expansion: 0.0,
            panel_velocity: 0.0,
            press_active: false,
            press_progress: 0.0,
            press_velocity: 0.0,
            hover_since: None,
            panel_trigger: PanelTrigger::Closed,
            alert_until: None,
            notifications: NotificationQueue::default(),
            notification_unread_count: 0,
            dnd_active: false,
            call: CallTracker::default(),
            call_shown: false,
            clock: PomodoroClock::default(),
            epoch: Instant::now(),
            last_anim_instant: Instant::now(),
            media,
            notch_tab,
            previous_tab: notch_tab,
            anim_phase: 0.0,
            lyrics_expanded: false,
            lyrics_progress: 0.0,
            stats_progress: 0.0,
            stats_range: FocusStatsRange::Today,
            stats_selected_day: None,
            tab_blend: 1.0,
            motion: MotionPreference::Full,
            lyrics: None,
            lyrics_status: LyricsStatus::Idle,
            pending_seek: None,
            seek_drag: None,
            media_primary: false,
            dismissed: None,
            dismissed_secondary: None,
            last_primary_slot: initial_primary,
            last_secondary_slot: None,
            hover_blocked: false,
            clock_time: ClockTime::default(),
            presented: true,
            glow_presence: 0.0,
            dim_presence: 0.0,
            pulse_presence: 0.0,
            scene_step_at: Instant::now(),
            peek: None,
            media_peek: None,
            media_paused_at: None,
            media_dim_presence: 0.0,
            lyrics_in_compact: false,
            focus_enabled: true,
            shelf_enabled: true,
            clipboard_enabled: true,
            tasks_enabled: false,
            media_enabled: true,
            privacy_enabled: true,
            hud_enabled: true,
            compact_title_points: 0.0,
            compact_title_target: 0.0,
            compact_title_measured: None,
            focus_blocklist: FocusBlocklist::default(),
            focus_categories: FocusCategoryStore::default(),
            category_picker_open: false,
            custom_time_input: None,
            focus_relief_summary: String::new(),
            heatmap_filter: HeatmapFilter::All,
            heatmap_selected_cell: None,
        }
    }
}

fn sanitize_shelf_payload(payload: ShelfPayload) -> Option<ShelfPayload> {
    match payload {
        ShelfPayload::File(path) => {
            is_valid_shelf_file_path(&path).then_some(ShelfPayload::File(path))
        }
        ShelfPayload::Url(url) => is_openable_shelf_url(&url).then_some(ShelfPayload::Url(url)),
        ShelfPayload::Text(text) => (!text.trim().is_empty()).then_some(ShelfPayload::Text(text)),
    }
}

#[cfg(test)]
mod tests;
