pub const CMD_NONE: usize = 0;
pub const CMD_QUIT: usize = 1;
pub const CMD_START: usize = 2;
pub const CMD_SETTINGS: usize = 3;
pub const CMD_TASKMGR: usize = 4;
pub const CMD_LOCK: usize = 5;
pub const CMD_SLEEP: usize = 6;
pub const CMD_RESTART: usize = 7;
pub const CMD_SHUTDOWN: usize = 8;

pub const CMD_APP_ACTIVATE: usize = 10;
pub const CMD_WIN_MINIMIZE: usize = 11;
pub const CMD_WIN_MAXIMIZE: usize = 12;
pub const CMD_WIN_CLOSE: usize = 13;
pub const CMD_SHOW_DESKTOP: usize = 14;
pub const CMD_TILE_LEFT: usize = 15;
pub const CMD_TILE_RIGHT: usize = 16;
pub const CMD_TASK_VIEW: usize = 17;
pub const CMD_APP_HIDE: usize = 18;
pub const CMD_APP_HIDE_OTHERS: usize = 19;
pub const CMD_APP_QUIT: usize = 25;
pub const CMD_APP_SHOW_ALL: usize = 27;
pub const CMD_APP_SETTINGS: usize = 28;
pub const CMD_WIN_BRING_ALL_TO_FRONT: usize = 29;

pub const CMD_FILE_NEW: usize = 20;
pub const CMD_FILE_NEW_TAB: usize = 21;
pub const CMD_FILE_OPEN: usize = 22;
pub const CMD_FILE_SAVE: usize = 23;
pub const CMD_FILE_SAVE_AS: usize = 24;
pub const CMD_FILE_NEW_FOLDER: usize = 26;

pub const CMD_EDIT_UNDO: usize = 30;
pub const CMD_EDIT_REDO: usize = 31;
pub const CMD_EDIT_CUT: usize = 32;
pub const CMD_EDIT_COPY: usize = 33;
pub const CMD_EDIT_PASTE: usize = 34;
pub const CMD_EDIT_SELECT_ALL: usize = 35;
pub const CMD_EDIT_FIND: usize = 36;

pub const CMD_VIEW_FULLSCREEN: usize = 40;
pub const CMD_VIEW_ZOOM_IN: usize = 41;
pub const CMD_VIEW_ZOOM_OUT: usize = 42;
pub const CMD_VIEW_ACTUAL_SIZE: usize = 43;
pub const CMD_VIEW_AS_ICONS: usize = 44;
pub const CMD_VIEW_AS_LIST: usize = 45;
pub const CMD_VIEW_AS_DETAILS: usize = 46;

pub const CMD_HELP_F1: usize = 51;

pub const CMD_CC_VOLUME: usize = 60;
pub const CMD_CC_BRIGHTNESS: usize = 61;
pub const CMD_CC_MEDIA_TOGGLE: usize = 62;
pub const CMD_CC_MEDIA_NEXT: usize = 63;
pub const CMD_CC_MEDIA_PREVIOUS: usize = 64;
pub const CMD_CC_WIFI: usize = 65;
pub const CMD_CC_BLUETOOTH: usize = 66;
pub const CMD_CC_FOCUS: usize = 67;
pub const CMD_CC_DISPLAY: usize = 68;
pub const CMD_CC_SOUND: usize = 69;
pub const CMD_CC_CALENDAR: usize = 70;
pub const CMD_CAL_PREVIOUS: usize = 71;
pub const CMD_CAL_NEXT: usize = 72;
pub const CMD_CAL_TODAY: usize = 73;
pub const CMD_CAL_SETTINGS: usize = 74;
pub const CMD_CC_THEME_TOGGLE: usize = 75;
pub const CMD_CC_THEME_LIGHT: usize = 76;
pub const CMD_CC_THEME_DARK: usize = 77;
pub const CMD_CC_THEME_AUTO: usize = 78;
pub const CMD_CC_LYRICS_ONLINE: usize = 79;
pub const CMD_OTTO_SETTINGS: usize = 80;
pub const CMD_OPEN_LOGS: usize = 81;
pub const CMD_NOTIF_CLEAR_ALL: usize = 82;
pub const CMD_NOTIF_REQUEST_ACCESS: usize = 83;
pub const CMD_CONTROL_CENTER: usize = 84;
pub const CMD_TOGGLE_TOP_BAR: usize = 85;
pub const CMD_TOGGLE_NOTCH: usize = 86;
pub const CMD_TOGGLE_DOCK: usize = 87;
pub const CMD_OVERFLOW_MENU_BASE: usize = 90;
pub const CMD_DOCK_PIN: usize = 100;
pub const CMD_DOCK_TOGGLE: usize = 101;
pub const CMD_DOCK_NEW_WINDOW: usize = 102;
pub const CMD_DOCK_EMPTY_TRASH: usize = 103;
pub const CMD_DOCK_QUIT: usize = 104;
pub const CMD_DOCK_HIDE: usize = 105;
pub const CMD_DOCK_WINDOW_BASE: usize = 200;
pub const CMD_NOTIF_DISMISS_BASE: usize = 1000;
pub const MAX_VISIBLE_NOTIFICATIONS: usize = 5;
pub const NOTIFICATION_CARD_HEIGHT: i32 = 68;
pub const NOTIFICATION_CARD_WIDTH: i32 = 320;

pub(super) const NOTIFICATION_BADGE_COLORS: [[u8; 3]; 7] = [
    [255, 69, 58],
    [255, 159, 10],
    [48, 209, 88],
    [10, 132, 255],
    [191, 90, 242],
    [100, 210, 255],
    [255, 55, 95],
];
