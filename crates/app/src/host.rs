use std::cell::{Cell, RefCell};
use std::fs;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, HCURSOR, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW, SW_HIDE,
    ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WNDCLASSEXW, WNDPROC, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result, w};

use crate::{bar, control_center, dock, genie, menu, window_controls};
use otto_core::{
    AppPreferences, BrandIcon, DockPreferences, NotchStyle, SystemAppearance, ThemeMode,
};
use otto_platform::{
    replace_file_atomically, restore_hidden_windows, resume_background_tasks,
    shutdown_background_tasks,
};

const CONFIG_FILE: &str = "plugins.txt";
const SUPPORTED_CONFIG_VERSION: u32 = 1;
const TOGGLEABLE_PLUGINS: [PluginId; 3] =
    [PluginId::Dock, PluginId::WindowControls, PluginId::Genie];

const fn toggleable_plugin_index(plugin: PluginId) -> Option<usize> {
    match plugin {
        PluginId::Dock => Some(0),
        PluginId::WindowControls => Some(1),
        PluginId::Genie => Some(2),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PluginId {
    PopupMenu,
    TopBar,
    Dock,
    WindowControls,
    Genie,
    ControlCenter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PluginState {
    Disabled,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Copy)]
enum SurfaceSlot {
    Bar,
    Panel,
    Menu,
    Dock,
    WindowControls,
    Genie,
    ControlCenter,
}

#[derive(Clone, Copy)]
struct SurfaceDescriptor {
    class_name: PCWSTR,
    wnd_proc: WNDPROC,
    slot: SurfaceSlot,
    topmost: bool,
}

type PluginStart = fn(&Host) -> Result<()>;
type PluginStop = fn(&RuntimeContext);

#[derive(Clone, Copy)]
struct PluginDescriptor {
    id: PluginId,
    required: bool,
    dependencies: &'static [PluginId],
    surface: Option<SurfaceDescriptor>,
    start: Option<PluginStart>,
    stop: PluginStop,
}

const NO_DEPENDENCIES: &[PluginId] = &[];
const POPUP_DEPENDENCY: &[PluginId] = &[PluginId::PopupMenu];

const DOCK_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoDockBar"),
    wnd_proc: Some(dock_window_proc),
    slot: SurfaceSlot::Dock,
    topmost: true,
};
const WINDOW_CONTROLS_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: window_controls::OVERLAY_CLASS,
    wnd_proc: Some(window_controls_window_proc),
    slot: SurfaceSlot::WindowControls,
    topmost: false,
};
const GENIE_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoGenie"),
    wnd_proc: Some(genie_window_proc),
    slot: SurfaceSlot::Genie,
    topmost: true,
};
const CONTROL_CENTER_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoControlCenter"),
    wnd_proc: Some(control_center_window_proc),
    slot: SurfaceSlot::ControlCenter,
    topmost: true,
};

const PLUGIN_DESCRIPTORS: &[PluginDescriptor] = &[
    PluginDescriptor {
        id: PluginId::PopupMenu,
        required: true,
        dependencies: NO_DEPENDENCIES,
        surface: None,
        start: Some(start_popup_menu),
        stop: stop_popup_menu,
    },
    PluginDescriptor {
        id: PluginId::TopBar,
        required: true,
        dependencies: POPUP_DEPENDENCY,
        surface: None,
        start: Some(start_top_bar),
        stop: stop_top_bar,
    },
    PluginDescriptor {
        id: PluginId::Genie,
        required: false,
        dependencies: NO_DEPENDENCIES,
        surface: Some(GENIE_SURFACE),
        start: Some(start_genie),
        stop: stop_genie,
    },
    PluginDescriptor {
        id: PluginId::Dock,
        required: false,
        dependencies: POPUP_DEPENDENCY,
        surface: Some(DOCK_SURFACE),
        start: Some(start_dock),
        stop: stop_dock,
    },
    PluginDescriptor {
        id: PluginId::WindowControls,
        required: false,
        dependencies: POPUP_DEPENDENCY,
        surface: Some(WINDOW_CONTROLS_SURFACE),
        start: Some(start_window_controls),
        stop: stop_window_controls,
    },
    PluginDescriptor {
        id: PluginId::ControlCenter,
        required: false,
        dependencies: &[PluginId::PopupMenu, PluginId::TopBar],
        surface: Some(CONTROL_CENTER_SURFACE),
        start: Some(start_control_center),
        stop: stop_control_center,
    },
];

impl PluginId {
    const fn name(self) -> &'static str {
        match self {
            Self::PopupMenu => "popup_menu",
            Self::TopBar => "top_bar",
            Self::Dock => "dock",
            Self::WindowControls => "window_controls",
            Self::Genie => "genie",
            Self::ControlCenter => "control_center",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "popup_menu" => Some(Self::PopupMenu),
            "top_bar" => Some(Self::TopBar),
            "dock" => Some(Self::Dock),
            "window_controls" | "traffic_light" => Some(Self::WindowControls),
            "genie" => Some(Self::Genie),
            "control_center" => Some(Self::ControlCenter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigSource {
    Defaults,
    Supported,
    Unsupported,
}

#[derive(Clone, Debug)]
struct PluginConfig {
    enabled: Vec<PluginId>,
    online_lyrics: bool,
    dock: DockPreferences,
    preferences: AppPreferences,
    source: ConfigSource,
}

fn config_source(contents: &str) -> ConfigSource {
    let Some(version) = contents
        .lines()
        .find_map(|line| line.strip_prefix("version="))
    else {
        return ConfigSource::Supported;
    };
    match version.trim().parse::<u32>() {
        Ok(version) if (1..=SUPPORTED_CONFIG_VERSION).contains(&version) => ConfigSource::Supported,
        _ => ConfigSource::Unsupported,
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: vec![
                PluginId::PopupMenu,
                PluginId::TopBar,
                PluginId::Dock,
                PluginId::WindowControls,
                PluginId::Genie,
                PluginId::ControlCenter,
            ],
            online_lyrics: false,
            dock: DockPreferences::default(),
            preferences: AppPreferences::default(),
            source: ConfigSource::Defaults,
        }
    }
}

impl PluginConfig {
    fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&contents)
    }

    fn parse(contents: &str) -> Self {
        let source = config_source(contents);
        if source == ConfigSource::Unsupported {
            return Self {
                source,
                ..Self::default()
            };
        }
        let Some(enabled) = contents
            .lines()
            .find_map(|line| line.strip_prefix("enabled="))
        else {
            return Self {
                source,
                ..Self::default()
            };
        };
        let mut config = Self {
            source,
            enabled: Vec::new(),
            online_lyrics: contents
                .lines()
                .find_map(|line| line.strip_prefix("online_lyrics="))
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("true")),
            dock: DockPreferences::parse_config(contents),
            preferences: AppPreferences::parse_config(contents),
        };
        for name in enabled.split(',') {
            if let Some(plugin) = PluginId::parse(name)
                && !config.enabled.contains(&plugin)
            {
                config.enabled.push(plugin);
            }
        }
        config.ensure_required();
        config
    }

    fn save(&self) -> Result<()> {
        if self.source == ConfigSource::Unsupported {
            return Err(Error::new(
                windows::Win32::Foundation::E_ACCESSDENIED,
                "The Otto configuration file was written by a newer version and is left untouched",
            ));
        }
        let Some(path) = config_path() else {
            return Ok(());
        };
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        fs::create_dir_all(parent).map_err(io_error)?;
        let enabled = self
            .enabled
            .iter()
            .map(|plugin| plugin.name())
            .collect::<Vec<_>>()
            .join(",");
        replace_file_atomically(
            &path,
            format!(
                "version={SUPPORTED_CONFIG_VERSION}\nenabled={enabled}\nonline_lyrics={}\n{}\n{}\n",
                self.online_lyrics,
                self.dock.config_lines().trim_end_matches('\n'),
                self.preferences.config_lines().trim_end_matches('\n'),
            )
            .as_bytes(),
        )
    }

    fn ensure_required(&mut self) {
        if !self.enabled.contains(&PluginId::PopupMenu) {
            self.enabled.push(PluginId::PopupMenu);
        }
        if !self.enabled.contains(&PluginId::TopBar) {
            self.enabled.push(PluginId::TopBar);
        }
    }

    fn is_enabled(&self, plugin: PluginId) -> bool {
        self.enabled.contains(&plugin)
    }

    fn set_enabled(&mut self, plugin: PluginId, enabled: bool) {
        if enabled {
            if !self.enabled.contains(&plugin) {
                self.enabled.push(plugin);
            }
        } else {
            self.enabled.retain(|candidate| *candidate != plugin);
        }
        self.ensure_required();
    }
}

struct Surfaces {
    bar: HWND,
    panel: HWND,
    menu: HWND,
    dock: Option<HWND>,
    window_controls: Option<HWND>,
    genie: Option<HWND>,
    control_center: Option<HWND>,
}

impl Surfaces {
    fn get(&self, slot: SurfaceSlot) -> Option<HWND> {
        match slot {
            SurfaceSlot::Bar => Some(self.bar),
            SurfaceSlot::Panel => Some(self.panel),
            SurfaceSlot::Menu => Some(self.menu),
            SurfaceSlot::Dock => self.dock,
            SurfaceSlot::WindowControls => self.window_controls,
            SurfaceSlot::Genie => self.genie,
            SurfaceSlot::ControlCenter => self.control_center,
        }
    }

    fn store(&mut self, slot: SurfaceSlot, hwnd: HWND) -> Result<()> {
        let target = match slot {
            SurfaceSlot::Dock => &mut self.dock,
            SurfaceSlot::WindowControls => &mut self.window_controls,
            SurfaceSlot::Genie => &mut self.genie,
            SurfaceSlot::ControlCenter => &mut self.control_center,
            SurfaceSlot::Bar | SurfaceSlot::Panel | SurfaceSlot::Menu => {
                return Err(Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    "Base surface cannot be registered as an optional surface",
                ));
            }
        };
        *target = Some(hwnd);
        Ok(())
    }

    fn take(&mut self, slot: SurfaceSlot) -> Option<HWND> {
        match slot {
            SurfaceSlot::Dock => self.dock.take(),
            SurfaceSlot::WindowControls => self.window_controls.take(),
            SurfaceSlot::Genie => self.genie.take(),
            SurfaceSlot::ControlCenter => self.control_center.take(),
            SurfaceSlot::Bar | SurfaceSlot::Panel | SurfaceSlot::Menu => None,
        }
    }
}

pub(crate) struct RuntimeContext {
    pub(crate) bar: RefCell<Option<bar::Bar>>,
    pub(crate) menu: RefCell<Option<menu::PopupMenu>>,
    pub(crate) dock: RefCell<Option<dock::Dock>>,
    pub(crate) window_controls: RefCell<Option<window_controls::WindowControls>>,
    pub(crate) genie: RefCell<Option<genie::Genie>>,
    pub(crate) control_center: RefCell<Option<control_center::ControlCenterSurface>>,
    pub(crate) bar_hwnd: Cell<isize>,
    pub(crate) dock_hwnd: Cell<isize>,
    pub(crate) window_controls_hwnd: Cell<isize>,
    pub(crate) theme_mode: Cell<ThemeMode>,
    pub(crate) system_appearance: Cell<SystemAppearance>,
    pub(crate) blur_supported: Cell<bool>,
    pub(crate) glass_enabled: Cell<bool>,
    pub(crate) online_lyrics_enabled: Cell<bool>,
    pub(crate) pending_online_lyrics: Cell<Option<bool>>,
    pub(crate) dock_preferences: Cell<DockPreferences>,
    pub(crate) pending_dock_preferences: Cell<Option<DockPreferences>>,
    pub(crate) focus_active: Cell<bool>,
    pub(crate) dock_enabled: Cell<bool>,
    pub(crate) window_controls_enabled: Cell<bool>,
    pub(crate) genie_enabled: Cell<bool>,
    pub(crate) hotkeys_enabled: Cell<bool>,
    pub(crate) capture_visible: Cell<bool>,
    pub(crate) classic_logo_enabled: Cell<bool>,
    pub(crate) dynamic_island_enabled: Cell<bool>,
    pub(crate) liquid_glass_pref: Cell<bool>,
    pub(crate) privacy_indicators_enabled: Cell<bool>,
    pub(crate) clipboard_history_enabled: Cell<bool>,
    pub(crate) drop_shelf_enabled: Cell<bool>,
    pub(crate) notch_hud_enabled: Cell<bool>,
    pub(crate) media_module_enabled: Cell<bool>,
    pub(crate) focus_module_enabled: Cell<bool>,
    pub(crate) onboarding_seen: Cell<bool>,
    pub(crate) pending_plugin_toggles: [Cell<Option<bool>>; 3],
    pub(crate) pending_theme_mode: Cell<Option<ThemeMode>>,
    pub(crate) pending_capture_visible: Cell<Option<bool>>,
    pub(crate) pending_hotkeys_enabled: Cell<Option<bool>>,
    pub(crate) pending_classic_logo: Cell<Option<bool>>,
    pub(crate) pending_dynamic_island: Cell<Option<bool>>,
    pub(crate) pending_liquid_glass: Cell<Option<bool>>,
    pub(crate) pending_privacy_indicators: Cell<Option<bool>>,
    pub(crate) pending_clipboard_history: Cell<Option<bool>>,
    pub(crate) pending_drop_shelf: Cell<Option<bool>>,
    pub(crate) pending_notch_hud: Cell<Option<bool>>,
    pub(crate) pending_media_module: Cell<Option<bool>>,
    pub(crate) pending_focus_module: Cell<Option<bool>>,
    pub(crate) pending_onboarding_seen: Cell<Option<bool>>,
    pub(crate) pending_end_session: Cell<bool>,
    pub(crate) pending_modal_action: RefCell<Option<bar::DeferredModalAction>>,
    pub(crate) pending_destroy_hwnds: RefCell<Vec<isize>>,
}

impl RuntimeContext {
    pub(crate) fn new(online_lyrics_enabled: bool) -> Self {
        Self {
            bar: RefCell::new(None),
            menu: RefCell::new(None),
            dock: RefCell::new(None),
            window_controls: RefCell::new(None),
            genie: RefCell::new(None),
            control_center: RefCell::new(None),
            bar_hwnd: Cell::new(0),
            dock_hwnd: Cell::new(0),
            window_controls_hwnd: Cell::new(0),
            theme_mode: Cell::new(ThemeMode::Auto),
            system_appearance: Cell::new(SystemAppearance::DEFAULT),
            blur_supported: Cell::new(false),
            glass_enabled: Cell::new(false),
            online_lyrics_enabled: Cell::new(online_lyrics_enabled),
            pending_online_lyrics: Cell::new(None),
            dock_preferences: Cell::new(DockPreferences::default()),
            pending_dock_preferences: Cell::new(None),
            focus_active: Cell::new(false),
            dock_enabled: Cell::new(true),
            window_controls_enabled: Cell::new(true),
            genie_enabled: Cell::new(true),
            hotkeys_enabled: Cell::new(true),
            capture_visible: Cell::new(false),
            classic_logo_enabled: Cell::new(false),
            dynamic_island_enabled: Cell::new(false),
            liquid_glass_pref: Cell::new(true),
            privacy_indicators_enabled: Cell::new(true),
            clipboard_history_enabled: Cell::new(true),
            drop_shelf_enabled: Cell::new(true),
            notch_hud_enabled: Cell::new(true),
            media_module_enabled: Cell::new(true),
            focus_module_enabled: Cell::new(true),
            onboarding_seen: Cell::new(false),
            pending_plugin_toggles: [const { Cell::new(None) }; 3],
            pending_theme_mode: Cell::new(None),
            pending_capture_visible: Cell::new(None),
            pending_hotkeys_enabled: Cell::new(None),
            pending_classic_logo: Cell::new(None),
            pending_dynamic_island: Cell::new(None),
            pending_liquid_glass: Cell::new(None),
            pending_privacy_indicators: Cell::new(None),
            pending_clipboard_history: Cell::new(None),
            pending_drop_shelf: Cell::new(None),
            pending_notch_hud: Cell::new(None),
            pending_media_module: Cell::new(None),
            pending_focus_module: Cell::new(None),
            pending_onboarding_seen: Cell::new(None),
            pending_end_session: Cell::new(false),
            pending_modal_action: RefCell::new(None),
            pending_destroy_hwnds: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn queue_plugin_toggle(&self, plugin: PluginId, enabled: bool) {
        if let Some(index) = toggleable_plugin_index(plugin) {
            self.pending_plugin_toggles[index].set(Some(enabled));
        }
    }

    pub(crate) fn take_plugin_toggles(&self) -> Vec<(PluginId, bool)> {
        let mut out = Vec::new();
        for (index, plugin) in TOGGLEABLE_PLUGINS.into_iter().enumerate() {
            if let Some(enabled) = self.pending_plugin_toggles[index].take() {
                out.push((plugin, enabled));
            }
        }
        out
    }

    pub(crate) fn queue_destroy_hwnd(&self, hwnd: isize) {
        if hwnd != 0
            && let Ok(mut pending) = self.pending_destroy_hwnds.try_borrow_mut()
        {
            pending.push(hwnd);
        }
    }

    pub(crate) fn take_pending_destroy_hwnds(&self) -> Vec<HWND> {
        let Ok(mut pending) = self.pending_destroy_hwnds.try_borrow_mut() else {
            return Vec::new();
        };
        pending.drain(..).map(|raw| HWND(raw as *mut _)).collect()
    }

    pub(crate) fn brand_icon(&self) -> BrandIcon {
        if self.classic_logo_enabled.get() {
            BrandIcon::ClassicApple
        } else {
            BrandIcon::Otto
        }
    }

    pub(crate) fn notch_style(&self) -> NotchStyle {
        if self.dynamic_island_enabled.get() {
            NotchStyle::DynamicIsland
        } else {
            NotchStyle::ClassicNotch
        }
    }

    fn apply_preferences(&self, preferences: AppPreferences) {
        self.theme_mode.set(preferences.theme_mode);
        self.capture_visible.set(preferences.capture_visible);
        self.hotkeys_enabled.set(preferences.hotkeys_enabled);
        self.classic_logo_enabled.set(preferences.classic_logo);
        self.dynamic_island_enabled.set(preferences.dynamic_island);
        self.liquid_glass_pref.set(preferences.liquid_glass);
        self.privacy_indicators_enabled
            .set(preferences.privacy_indicators);
        self.clipboard_history_enabled
            .set(preferences.clipboard_history);
        self.drop_shelf_enabled.set(preferences.drop_shelf);
        self.notch_hud_enabled.set(preferences.notch_hud);
        self.media_module_enabled.set(preferences.media_module);
        self.focus_module_enabled.set(preferences.focus_module);
        self.onboarding_seen.set(preferences.onboarding_seen);
    }
}

struct Host {
    instance: HINSTANCE,
    cursor: HCURSOR,
    ex_style: WINDOW_EX_STYLE,
    blur: bool,
    config: PluginConfig,
    surfaces: Option<Surfaces>,
    started: Vec<PluginId>,
    states: Vec<(PluginId, PluginState)>,
    shutting_down: bool,
    runtime: RuntimeContext,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static MODAL_ACTION_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

struct ModalActionGuard;

impl Drop for ModalActionGuard {
    fn drop(&mut self) {
        MODAL_ACTION_ACTIVE.with(|flag| flag.set(false));
    }
}

fn busy_host_error() -> Error {
    Error::new(windows::Win32::Foundation::E_FAIL, "Otto host is busy")
}

fn not_running_error() -> Error {
    Error::new(
        windows::Win32::Foundation::E_FAIL,
        "Otto host is not running",
    )
}

fn with_host_read(action: impl FnOnce(&Host)) {
    HOST.with(|cell| {
        if let Ok(guard) = cell.try_borrow()
            && let Some(host) = guard.as_ref()
        {
            action(host);
        }
    });
}

fn with_host_mut<T>(action: impl FnOnce(&mut Host) -> Result<T>) -> Result<T> {
    HOST.with(|cell| {
        let mut guard = cell.try_borrow_mut().map_err(|_| busy_host_error())?;
        let Some(host) = guard.as_mut() else {
            return Err(not_running_error());
        };
        action(host)
    })
}

fn destroy_hwnds(hwnds: &[HWND]) {
    for &hwnd in hwnds {
        if !hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

pub fn start(blur: bool) -> Result<()> {
    resume_background_tasks();
    let instance: HINSTANCE =
        unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? }.into();
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };
    let config = PluginConfig::load();
    let online_lyrics_enabled = config.online_lyrics;
    let mut host = Host {
        instance,
        cursor,
        ex_style: WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
        blur,
        config,
        surfaces: None,
        started: Vec::new(),
        states: PLUGIN_DESCRIPTORS
            .iter()
            .map(|descriptor| (descriptor.id, PluginState::Disabled))
            .collect(),
        shutting_down: false,
        runtime: RuntimeContext::new(online_lyrics_enabled),
    };
    host.runtime
        .system_appearance
        .set(otto_platform::read_system_appearance());
    host.runtime.dock_preferences.set(host.config.dock);
    host.runtime.apply_preferences(host.config.preferences);
    host.runtime.blur_supported.set(blur);
    host.runtime.glass_enabled.set(
        blur && host.config.preferences.liquid_glass
            && host.runtime.system_appearance.get().transparency,
    );
    if !host.config.preferences.media_module {
        otto_platform::shutdown_media_task();
    }
    if !host.config.preferences.privacy_indicators {
        otto_platform::shutdown_privacy_task();
    }
    otto_platform::set_volume_hud_hook_enabled(host.config.preferences.notch_hud);
    host.sync_plugin_flags();
    if let Err(error) = host.initialize() {
        let hwnds = host.shutdown();
        destroy_hwnds(&hwnds);
        return Err(error);
    }
    let pending_hwnds = host.runtime.take_pending_destroy_hwnds();
    destroy_hwnds(&pending_hwnds);
    if let Some(seen) = host.runtime.pending_onboarding_seen.take() {
        host.config.preferences.onboarding_seen = seen;
        let _ = host.config.save();
        host.runtime.onboarding_seen.set(seen);
    }
    HOST.with(|cell| {
        let mut guard = cell.try_borrow_mut().map_err(|_| busy_host_error())?;
        *guard = Some(host);
        Ok(())
    })
}

pub fn run_message_loop() {
    unsafe {
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

pub fn apply_cli_overrides() -> Result<()> {
    let overrides = std::env::args().filter_map(|arg| {
        if let Some(name) = arg.strip_prefix("--enable-plugin=") {
            return PluginId::parse(name).map(|plugin| (plugin, true));
        }
        arg.strip_prefix("--disable-plugin=")
            .and_then(PluginId::parse)
            .map(|plugin| (plugin, false))
    });
    for (plugin, enabled) in overrides {
        set_plugin_enabled(plugin, enabled)?;
    }
    Ok(())
}

pub fn stop() {
    let hwnds = HOST.with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else {
            return Vec::new();
        };
        guard.as_mut().map(Host::shutdown).unwrap_or_default()
    });
    destroy_hwnds(&hwnds);
    let host = HOST.with(|cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut guard| guard.take())
    });
    drop(host);
    otto_render::release_shared_factories();
}

pub fn shutdown_session() {
    otto_platform::uninstall_keyboard_hook();
    stop();
    let _ = otto_platform::restore_shell_from_recovery();
    otto_platform::uninstall_window_event_hooks();
}

pub(crate) fn log_present_error<E: std::fmt::Display>(
    surface: &str,
    result: std::result::Result<(), E>,
) {
    if let Err(error) = result {
        otto_platform::log_error("render", &format!("{surface} present failed: {error}"));
    }
}

pub fn set_plugin_enabled(plugin: PluginId, enabled: bool) -> Result<()> {
    let mut pending_hwnds = Vec::new();
    let result = with_host_mut(|host| {
        let outcome = host.set_plugin_enabled(plugin, enabled, PluginConfig::save);
        pending_hwnds = host.runtime.take_pending_destroy_hwnds();
        outcome
    });
    destroy_hwnds(&pending_hwnds);
    if result.is_ok() {
        match plugin {
            PluginId::ControlCenter => {
                with_host_read(|host| {
                    bar::refresh_plugin_layout(
                        &host.runtime,
                        host.config.is_enabled(PluginId::ControlCenter),
                    );
                });
            }
            PluginId::Dock => {
                with_host_read(|host| {
                    bar::set_dock_taskbar_hidden(
                        &host.runtime,
                        enabled && host.config.dock.hide_taskbar,
                    );
                });
            }
            _ => {}
        }
    }
    result
}

pub fn set_online_lyrics_enabled(enabled: bool) -> Result<()> {
    with_host_mut(|host| {
        let previous = host.config.online_lyrics;
        host.config.online_lyrics = enabled;
        if let Err(error) = host.config.save() {
            host.config.online_lyrics = previous;
            return Err(error);
        }
        host.runtime.online_lyrics_enabled.set(enabled);
        Ok(())
    })
}

pub fn set_dock_preferences(preferences: DockPreferences) -> Result<()> {
    let result = with_host_mut(|host| {
        let previous = host.config.dock;
        host.config.dock = preferences;
        if let Err(error) = host.config.save() {
            host.config.dock = previous;
            return Err(error);
        }
        host.runtime.dock_preferences.set(preferences);
        Ok(())
    });
    if result.is_ok() {
        with_host_read(|host| {
            bar::set_dock_taskbar_hidden(
                &host.runtime,
                host.config.is_enabled(PluginId::Dock) && host.config.dock.hide_taskbar,
            );
        });
    }
    result
}

fn update_preferences(update: impl FnOnce(&mut AppPreferences)) -> Result<AppPreferences> {
    with_host_mut(|host| {
        let previous = host.config.preferences;
        update(&mut host.config.preferences);
        if let Err(error) = host.config.save() {
            host.config.preferences = previous;
            return Err(error);
        }
        let updated = host.config.preferences;
        host.runtime.apply_preferences(updated);
        Ok(updated)
    })
}

fn config_path() -> Option<PathBuf> {
    otto_platform::data_dir().map(|root| root.join(CONFIG_FILE))
}

fn io_error(_error: std::io::Error) -> Error {
    Error::new(
        windows::Win32::Foundation::E_FAIL,
        "Unable to persist Otto plugin configuration",
    )
}

fn descriptor(plugin: PluginId) -> &'static PluginDescriptor {
    PLUGIN_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.id == plugin)
        .expect("every PluginId must have a descriptor")
}

impl Host {
    fn sync_plugin_flags(&self) {
        self.runtime
            .dock_enabled
            .set(self.config.is_enabled(PluginId::Dock));
        self.runtime
            .window_controls_enabled
            .set(self.config.is_enabled(PluginId::WindowControls));
        self.runtime
            .genie_enabled
            .set(self.config.is_enabled(PluginId::Genie));
    }

    fn set_state(&mut self, plugin: PluginId, state: PluginState) {
        if let Some((_, current)) = self
            .states
            .iter_mut()
            .find(|(candidate, _)| *candidate == plugin)
        {
            *current = state;
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let bar = create_overlay_window(
            self.instance,
            self.cursor,
            w!("OttoNotchBar"),
            Some(bar_window_proc),
            self.ex_style,
        )?;
        let panel = match create_overlay_window(
            self.instance,
            self.cursor,
            w!("OttoNotchPanel"),
            Some(bar_window_proc),
            self.ex_style,
        ) {
            Ok(panel) => panel,
            Err(error) => {
                unsafe {
                    let _ = DestroyWindow(bar);
                }
                return Err(error);
            }
        };
        let menu = match create_overlay_window(
            self.instance,
            self.cursor,
            w!("OttoPopupMenu"),
            Some(menu_window_proc),
            self.ex_style,
        ) {
            Ok(menu) => menu,
            Err(error) => {
                unsafe {
                    let _ = DestroyWindow(panel);
                    let _ = DestroyWindow(bar);
                }
                return Err(error);
            }
        };
        self.runtime.bar_hwnd.set(bar.0 as isize);
        self.surfaces = Some(Surfaces {
            bar,
            panel,
            menu,
            dock: None,
            window_controls: None,
            genie: None,
            control_center: None,
        });

        for descriptor in PLUGIN_DESCRIPTORS {
            if !self.config.is_enabled(descriptor.id) {
                continue;
            }
            match self.start_plugin(descriptor.id) {
                Ok(()) => {}
                Err(error) if descriptor.required => return Err(error),
                Err(error) => report_plugin_failure(descriptor.id, &error),
            }
        }
        self.sync_plugin_flags();
        Ok(())
    }

    fn start_plugin(&mut self, plugin: PluginId) -> Result<()> {
        let descriptor = descriptor(plugin);
        if self.started.contains(&plugin) {
            return Ok(());
        }
        if !dependencies_running(&self.started, descriptor.dependencies) {
            self.set_state(plugin, PluginState::Failed);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
                self.sync_plugin_flags();
            }
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Plugin dependency is not running",
            ));
        }
        self.set_state(plugin, PluginState::Starting);
        if descriptor.surface.is_some()
            && let Err(error) = self.create_surface(plugin)
        {
            self.set_state(plugin, PluginState::Failed);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
                self.sync_plugin_flags();
            }
            return Err(error);
        }
        let result = descriptor.start.map_or(Ok(()), |start| start(self));
        if let Err(error) = result {
            self.queue_surface_destroy(plugin);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
                self.sync_plugin_flags();
            }
            self.set_state(plugin, PluginState::Failed);
            return Err(error);
        }
        self.started.push(plugin);
        self.set_state(plugin, PluginState::Running);
        self.sync_plugin_flags();
        Ok(())
    }

    fn set_plugin_enabled(
        &mut self,
        plugin: PluginId,
        enabled: bool,
        persist: impl FnOnce(&PluginConfig) -> Result<()>,
    ) -> Result<()> {
        if descriptor(plugin).required {
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "The Otto top bar is required",
            ));
        }
        if enabled == self.config.is_enabled(plugin) {
            return Ok(());
        }
        if enabled {
            self.enable_plugin(plugin, persist)
        } else {
            self.disable_plugin(plugin, persist)
        }
    }

    fn enable_plugin(
        &mut self,
        plugin: PluginId,
        persist: impl FnOnce(&PluginConfig) -> Result<()>,
    ) -> Result<()> {
        self.config.set_enabled(plugin, true);
        if let Err(error) = self.start_plugin(plugin) {
            self.config.set_enabled(plugin, false);
            self.sync_plugin_flags();
            return Err(error);
        }
        if let Err(error) = persist(&self.config) {
            self.stop_plugin(plugin);
            self.config.set_enabled(plugin, false);
            self.sync_plugin_flags();
            return Err(error);
        }
        self.sync_plugin_flags();
        Ok(())
    }

    fn disable_plugin(
        &mut self,
        plugin: PluginId,
        persist: impl FnOnce(&PluginConfig) -> Result<()>,
    ) -> Result<()> {
        if let Some(dependent) = enabled_dependent(plugin, &self.config) {
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                format!("Disable {} before {}", dependent.name(), plugin.name()),
            ));
        }
        self.config.set_enabled(plugin, false);
        if let Err(error) = persist(&self.config) {
            self.config.set_enabled(plugin, true);
            self.sync_plugin_flags();
            return Err(error);
        }
        self.stop_plugin(plugin);
        self.sync_plugin_flags();
        Ok(())
    }

    fn create_surface(&mut self, plugin: PluginId) -> Result<HWND> {
        if self.surfaces.is_none() {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surfaces are not initialized",
            ));
        }
        let Some(descriptor) = descriptor(plugin).surface else {
            return Err(Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Plugin does not own a dynamic surface",
            ));
        };
        #[cfg(test)]
        if self.instance.is_invalid() {
            let fake = HWND(100 as _);
            if let Some(surfaces) = self.surfaces.as_mut() {
                surfaces.store(descriptor.slot, fake)?;
                return Ok(fake);
            }
        }
        let base_ex_style = if descriptor.topmost {
            self.ex_style
        } else {
            WINDOW_EX_STYLE(self.ex_style.0 & !WS_EX_TOPMOST.0)
        };
        let ex_style = if matches!(descriptor.slot, SurfaceSlot::Genie) {
            WINDOW_EX_STYLE(
                (base_ex_style.0 & !WS_EX_LAYERED.0)
                    | WS_EX_NOREDIRECTIONBITMAP.0
                    | WS_EX_TRANSPARENT.0,
            )
        } else {
            base_ex_style
        };
        let surface = create_overlay_window(
            self.instance,
            self.cursor,
            descriptor.class_name,
            descriptor.wnd_proc,
            ex_style,
        )?;
        let surfaces = self.surfaces.as_mut().ok_or_else(|| {
            Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surfaces are not initialized",
            )
        })?;
        if let Err(error) = surfaces.store(descriptor.slot, surface) {
            unsafe {
                let _ = DestroyWindow(surface);
            }
            return Err(error);
        }
        match descriptor.slot {
            SurfaceSlot::Dock => self.runtime.dock_hwnd.set(surface.0 as isize),
            SurfaceSlot::WindowControls => {
                self.runtime.window_controls_hwnd.set(surface.0 as isize);
            }
            _ => {}
        }
        Ok(surface)
    }

    fn queue_surface_destroy(&mut self, plugin: PluginId) {
        let Some(descriptor) = descriptor(plugin).surface else {
            return;
        };
        match descriptor.slot {
            SurfaceSlot::Dock => self.runtime.dock_hwnd.set(0),
            SurfaceSlot::WindowControls => self.runtime.window_controls_hwnd.set(0),
            _ => {}
        }
        let Some(surfaces) = self.surfaces.as_mut() else {
            return;
        };
        if let Some(surface) = surfaces.take(descriptor.slot) {
            #[cfg(test)]
            if self.instance.is_invalid() {
                return;
            }
            unsafe {
                let _ = ShowWindow(surface, SW_HIDE);
            }
            self.runtime.queue_destroy_hwnd(surface.0 as isize);
        }
    }

    fn stop_plugin(&mut self, plugin: PluginId) {
        if !self.started.contains(&plugin) {
            return;
        }
        self.stop_started_plugin(plugin);
    }

    fn stop_started_plugin(&mut self, plugin: PluginId) {
        self.set_state(plugin, PluginState::Stopping);
        (descriptor(plugin).stop)(&self.runtime);
        self.started.retain(|started| *started != plugin);
        self.queue_surface_destroy(plugin);
        self.set_state(plugin, PluginState::Disabled);
        self.sync_plugin_flags();
    }

    fn shutdown(&mut self) -> Vec<HWND> {
        self.shutting_down = true;
        let started = std::mem::take(&mut self.started);
        for plugin in reverse_lifecycle_order(&started) {
            self.stop_started_plugin(plugin);
        }
        let _ = restore_hidden_windows();
        shutdown_background_tasks();
        if let Some(surfaces) = self.surfaces.take() {
            let allow_real_hwnd = {
                #[cfg(test)]
                {
                    !self.instance.is_invalid()
                }
                #[cfg(not(test))]
                {
                    true
                }
            };
            if allow_real_hwnd {
                self.runtime.queue_destroy_hwnd(surfaces.menu.0 as isize);
                self.runtime.queue_destroy_hwnd(surfaces.panel.0 as isize);
                self.runtime.queue_destroy_hwnd(surfaces.bar.0 as isize);
            }
        }
        for (_, state) in &mut self.states {
            *state = PluginState::Disabled;
        }
        self.runtime.take_pending_destroy_hwnds()
    }
}

fn dependencies_running(started: &[PluginId], dependencies: &[PluginId]) -> bool {
    dependencies
        .iter()
        .all(|dependency| started.contains(dependency))
}

fn reverse_lifecycle_order(started: &[PluginId]) -> impl Iterator<Item = PluginId> + '_ {
    started.iter().rev().copied()
}

fn enabled_dependent(plugin: PluginId, config: &PluginConfig) -> Option<PluginId> {
    PLUGIN_DESCRIPTORS
        .iter()
        .find(|candidate| {
            config.is_enabled(candidate.id) && candidate.dependencies.contains(&plugin)
        })
        .map(|candidate| candidate.id)
}

fn surface(host: &Host, slot: SurfaceSlot) -> Result<HWND> {
    host.surfaces
        .as_ref()
        .and_then(|surfaces| surfaces.get(slot))
        .ok_or_else(|| {
            Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto surface is not initialized",
            )
        })
}

fn start_popup_menu(host: &Host) -> Result<()> {
    menu::start(&host.runtime, surface(host, SurfaceSlot::Menu)?.0 as isize)
}

fn start_top_bar(host: &Host) -> Result<()> {
    bar::start(
        &host.runtime,
        surface(host, SurfaceSlot::Bar)?.0 as isize,
        surface(host, SurfaceSlot::Panel)?.0 as isize,
        host.blur,
        host.config.is_enabled(PluginId::ControlCenter),
    )
}

fn start_dock(host: &Host) -> Result<()> {
    dock::start(&host.runtime, surface(host, SurfaceSlot::Dock)?.0 as isize)
}

fn start_window_controls(host: &Host) -> Result<()> {
    window_controls::start(
        &host.runtime,
        surface(host, SurfaceSlot::WindowControls)?.0 as isize,
    )
}

fn start_genie(host: &Host) -> Result<()> {
    genie::start(&host.runtime, surface(host, SurfaceSlot::Genie)?.0 as isize)
}

fn start_control_center(host: &Host) -> Result<()> {
    control_center::start(
        &host.runtime,
        surface(host, SurfaceSlot::ControlCenter)?.0 as isize,
    );
    Ok(())
}

fn stop_popup_menu(runtime: &RuntimeContext) {
    menu::stop(runtime);
}

fn stop_top_bar(runtime: &RuntimeContext) {
    bar::stop(runtime);
}

fn stop_dock(runtime: &RuntimeContext) {
    dock::stop(runtime);
}

fn stop_window_controls(runtime: &RuntimeContext) {
    window_controls::stop(runtime);
}

fn stop_genie(runtime: &RuntimeContext) {
    genie::stop(runtime);
}

fn stop_control_center(runtime: &RuntimeContext) {
    control_center::stop(runtime);
}

fn report_plugin_failure(plugin: PluginId, error: &Error) {
    let message = format!("Otto plugin {} failed to start: {error}\0", plugin.name());
    let wide: Vec<u16> = message.encode_utf16().collect();
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(PCWSTR(wide.as_ptr()));
    }
}

fn create_overlay_window(
    instance: HINSTANCE,
    cursor: HCURSOR,
    class_name: PCWSTR,
    wnd_proc: WNDPROC,
    ex_style: WINDOW_EX_STYLE,
) -> Result<HWND> {
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: wnd_proc,
            hInstance: instance,
            hCursor: cursor,
            lpszClassName: class_name,
            ..WNDCLASSEXW::default()
        };
        let _ = RegisterClassExW(&class);
        CreateWindowExW(
            ex_style,
            class_name,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
    }
}

fn should_post_quit(hwnd: isize) -> bool {
    HOST.with(|cell| {
        cell.try_borrow().ok().is_some_and(|guard| {
            guard.as_ref().is_some_and(|host| {
                !host.shutting_down
                    && host
                        .surfaces
                        .as_ref()
                        .is_some_and(|surfaces| surfaces.bar.0 as isize == hwnd)
            })
        })
    })
}

struct PendingPreferences {
    theme_mode: Option<ThemeMode>,
    capture_visible: Option<bool>,
    hotkeys_enabled: Option<bool>,
    classic_logo: Option<bool>,
    dynamic_island: Option<bool>,
    liquid_glass: Option<bool>,
    privacy_indicators: Option<bool>,
    clipboard_history: Option<bool>,
    drop_shelf: Option<bool>,
    notch_hud: Option<bool>,
    media_module: Option<bool>,
    focus_module: Option<bool>,
    onboarding_seen: Option<bool>,
}

impl PendingPreferences {
    fn has_change(&self) -> bool {
        self.theme_mode.is_some()
            || self.capture_visible.is_some()
            || self.hotkeys_enabled.is_some()
            || self.classic_logo.is_some()
            || self.dynamic_island.is_some()
            || self.liquid_glass.is_some()
            || self.privacy_indicators.is_some()
            || self.clipboard_history.is_some()
            || self.drop_shelf.is_some()
            || self.notch_hud.is_some()
            || self.media_module.is_some()
            || self.focus_module.is_some()
            || self.onboarding_seen.is_some()
    }

    fn affects_ui(&self) -> bool {
        self.theme_mode.is_some()
            || self.capture_visible.is_some()
            || self.hotkeys_enabled.is_some()
            || self.classic_logo.is_some()
            || self.dynamic_island.is_some()
            || self.liquid_glass.is_some()
            || self.privacy_indicators.is_some()
            || self.clipboard_history.is_some()
            || self.drop_shelf.is_some()
            || self.notch_hud.is_some()
            || self.media_module.is_some()
            || self.focus_module.is_some()
    }
}

pub(crate) fn is_host_active() -> bool {
    HOST.with(|cell| {
        cell.try_borrow()
            .ok()
            .is_some_and(|guard| guard.as_ref().is_some_and(|host| !host.shutting_down))
    })
}

struct PendingDispatch {
    online_lyrics: Option<bool>,
    dock: Option<DockPreferences>,
    plugin_toggles: Vec<(PluginId, bool)>,
    preferences: PendingPreferences,
    end_session: bool,
    modal: Option<bar::PreparedModalAction>,
}

fn dispatch_runtime_message(
    action: impl FnOnce(&RuntimeContext) -> Option<LRESULT>,
) -> Option<LRESULT> {
    let (result, pending) = HOST.with(|cell| {
        let guard = cell.try_borrow().ok()?;
        let host = guard.as_ref()?;
        let result = action(&host.runtime);
        let online_lyrics = host.runtime.pending_online_lyrics.take();
        let dock = host.runtime.pending_dock_preferences.take();
        let plugin_toggles = host.runtime.take_plugin_toggles();
        let preferences = PendingPreferences {
            theme_mode: host.runtime.pending_theme_mode.take(),
            capture_visible: host.runtime.pending_capture_visible.take(),
            hotkeys_enabled: host.runtime.pending_hotkeys_enabled.take(),
            classic_logo: host.runtime.pending_classic_logo.take(),
            dynamic_island: host.runtime.pending_dynamic_island.take(),
            liquid_glass: host.runtime.pending_liquid_glass.take(),
            privacy_indicators: host.runtime.pending_privacy_indicators.take(),
            clipboard_history: host.runtime.pending_clipboard_history.take(),
            drop_shelf: host.runtime.pending_drop_shelf.take(),
            notch_hud: host.runtime.pending_notch_hud.take(),
            media_module: host.runtime.pending_media_module.take(),
            focus_module: host.runtime.pending_focus_module.take(),
            onboarding_seen: host.runtime.pending_onboarding_seen.take(),
        };
        let end_session = host.runtime.pending_end_session.take();
        let raw_modal = host
            .runtime
            .pending_modal_action
            .try_borrow_mut()
            .ok()
            .and_then(|mut slot| slot.take());
        let modal = if !host.shutting_down && !end_session && !MODAL_ACTION_ACTIVE.with(Cell::get) {
            raw_modal.and_then(|modal| bar::prepare_modal_action(&host.runtime, modal))
        } else {
            None
        };
        Some((
            result,
            PendingDispatch {
                online_lyrics,
                dock,
                plugin_toggles,
                preferences,
                end_session,
                modal,
            },
        ))
    })?;
    let mut plugin_error = None;
    let had_plugin_toggles = !pending.plugin_toggles.is_empty();
    for (plugin, enabled) in pending.plugin_toggles {
        if let Err(error) = set_plugin_enabled(plugin, enabled)
            && plugin_error.is_none()
        {
            plugin_error = Some(error.message());
        }
    }
    let mut dock_changed = false;
    if let Some(preferences) = pending.dock {
        if set_dock_preferences(preferences).is_ok() {
            dock_changed = true;
            with_host_read(|host| dock::apply_preferences(&host.runtime, preferences));
        } else if plugin_error.is_none() {
            plugin_error = Some(String::from("Could not save Dock preferences"));
        }
    }
    let mut lyrics_changed = false;
    if let Some(enabled) = pending.online_lyrics {
        if set_online_lyrics_enabled(enabled).is_ok() {
            lyrics_changed = true;
            with_host_read(|host| bar::set_online_lyrics_enabled(&host.runtime, enabled));
        } else if plugin_error.is_none() {
            plugin_error = Some(String::from("Could not save Online Lyrics preference"));
        }
    }
    let prefs_changed = apply_pending_preference_changes(&pending.preferences);
    let glass_changed = pending.preferences.liquid_glass.is_some();
    if had_plugin_toggles || plugin_error.is_some() {
        with_host_read(|host| control_center::set_error(&host.runtime, plugin_error));
    }
    if had_plugin_toggles || dock_changed || lyrics_changed || (prefs_changed && !glass_changed) {
        with_host_read(|host| control_center::repaint(&host.runtime));
    }
    if let Some(prepared) = pending.modal {
        MODAL_ACTION_ACTIVE.with(|flag| flag.set(true));
        let _modal_guard = ModalActionGuard;
        let completed = bar::run_prepared_modal_action(prepared);
        with_host_read(|host| {
            if !host.shutting_down {
                bar::finish_modal_action(&host.runtime, completed);
            }
        });
    }
    if pending.end_session {
        shutdown_session();
        unsafe { PostQuitMessage(0) };
    }
    result
}

fn apply_pending_preference_changes(pending: &PendingPreferences) -> bool {
    if pending.has_change()
        && let Ok(updated) = update_preferences(|prefs| {
            if let Some(mode) = pending.theme_mode {
                prefs.theme_mode = mode;
            }
            if let Some(visible) = pending.capture_visible {
                prefs.capture_visible = visible;
            }
            if let Some(enabled) = pending.hotkeys_enabled {
                prefs.hotkeys_enabled = enabled;
            }
            if let Some(classic) = pending.classic_logo {
                prefs.classic_logo = classic;
            }
            if let Some(dynamic_island) = pending.dynamic_island {
                prefs.dynamic_island = dynamic_island;
            }
            if let Some(liquid_glass) = pending.liquid_glass {
                prefs.liquid_glass = liquid_glass;
            }
            if let Some(privacy_indicators) = pending.privacy_indicators {
                prefs.privacy_indicators = privacy_indicators;
            }
            if let Some(clipboard_history) = pending.clipboard_history {
                prefs.clipboard_history = clipboard_history;
            }
            if let Some(drop_shelf) = pending.drop_shelf {
                prefs.drop_shelf = drop_shelf;
            }
            if let Some(notch_hud) = pending.notch_hud {
                prefs.notch_hud = notch_hud;
            }
            if let Some(media_module) = pending.media_module {
                prefs.media_module = media_module;
            }
            if let Some(focus_module) = pending.focus_module {
                prefs.focus_module = focus_module;
            }
            if let Some(seen) = pending.onboarding_seen {
                prefs.onboarding_seen = seen;
            }
        })
    {
        sync_pending_platform_modules(pending, &updated);
        with_host_read(|host| {
            if let Some(glass) = pending.liquid_glass {
                host.runtime.liquid_glass_pref.set(glass);
                crate::theme_state::sync_effective_glass(&host.runtime);
            }
            if pending.capture_visible.is_some() {
                bar::set_capture_visible(&host.runtime, updated.capture_visible);
                dock::set_capture_visible(&host.runtime, updated.capture_visible);
                control_center::set_capture_visible(&host.runtime, updated.capture_visible);
                menu::set_capture_visible(&host.runtime, updated.capture_visible);
            }
            if pending.hotkeys_enabled.is_some() {
                bar::set_hotkeys_enabled(&host.runtime, updated.hotkeys_enabled);
            }
            if pending.classic_logo.is_some() {
                bar::set_brand_icon(&host.runtime, updated.brand_icon());
            }
            if pending.dynamic_island.is_some() {
                let style = updated.notch_style();
                bar::set_notch_style(&host.runtime, style);
                window_controls::set_notch_style(&host.runtime, style);
            }
            if pending.privacy_indicators.is_some() {
                bar::set_privacy_indicators_enabled(&host.runtime, updated.privacy_indicators);
            }
            if pending.clipboard_history.is_some() {
                bar::set_clipboard_history_enabled(&host.runtime, updated.clipboard_history);
            }
            if pending.drop_shelf.is_some() {
                bar::set_drop_shelf_enabled(&host.runtime, updated.drop_shelf);
            }
            if pending.notch_hud.is_some() {
                bar::set_notch_hud_enabled(&host.runtime, updated.notch_hud);
            }
            if pending.media_module.is_some() {
                bar::set_media_module_enabled(&host.runtime, updated.media_module);
            }
            if pending.focus_module.is_some() {
                bar::set_focus_module_enabled(&host.runtime, updated.focus_module);
            }
        });
    }
    pending.affects_ui()
}

fn sync_pending_platform_modules(
    pending: &PendingPreferences,
    updated: &otto_core::AppPreferences,
) {
    if pending.privacy_indicators.is_some() {
        if updated.privacy_indicators {
            otto_platform::resume_privacy_task();
        } else {
            otto_platform::shutdown_privacy_task();
        }
    }
    if pending.media_module.is_some() {
        if updated.media_module {
            otto_platform::resume_media_task();
        } else {
            otto_platform::shutdown_media_task();
        }
    }
    if pending.notch_hud.is_some() {
        otto_platform::set_volume_hud_hook_enabled(updated.notch_hud);
    }
}

fn safe_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    handler: impl FnOnce() -> Option<LRESULT>,
) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(handler));
    if let Ok(Some(result)) = result {
        return result;
    }
    if message == WM_DESTROY && should_post_quit(hwnd.0 as isize) {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn bar_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            bar::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

unsafe extern "system" fn dock_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            dock::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

unsafe extern "system" fn window_controls_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            window_controls::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

unsafe extern "system" fn genie_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            genie::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

unsafe extern "system" fn control_center_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            control_center::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

unsafe extern "system" fn menu_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    safe_window_proc(hwnd, message, wparam, lparam, || {
        dispatch_runtime_message(|runtime| {
            menu::handle_message(runtime, hwnd.0 as isize, message, wparam, lparam)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigSource, Error, HCURSOR, HINSTANCE, HOST, HWND, Host, LRESULT, MODAL_ACTION_ACTIVE,
        ModalActionGuard, PLUGIN_DESCRIPTORS, PluginConfig, PluginId, PluginState, Result,
        RuntimeContext, SurfaceSlot, Surfaces, WINDOW_EX_STYLE, bar, dependencies_running,
        descriptor, dispatch_runtime_message, enabled_dependent, reverse_lifecycle_order,
        set_dock_preferences, set_online_lyrics_enabled, set_plugin_enabled, should_post_quit,
        update_preferences,
    };

    struct TestHostGuard;

    impl Drop for TestHostGuard {
        fn drop(&mut self) {
            HOST.with(|cell| {
                if let Ok(mut guard) = cell.try_borrow_mut() {
                    let _ = guard.take();
                }
            });
        }
    }

    fn host_with_required_plugins() -> Host {
        let mut config = PluginConfig::default();
        config.set_enabled(PluginId::ControlCenter, false);
        Host {
            instance: HINSTANCE::default(),
            cursor: HCURSOR::default(),
            ex_style: WINDOW_EX_STYLE::default(),
            blur: false,
            config,
            surfaces: Some(Surfaces {
                bar: hwnd(1),
                panel: hwnd(2),
                menu: hwnd(3),
                dock: None,
                window_controls: None,
                genie: None,
                control_center: None,
            }),
            started: vec![PluginId::PopupMenu, PluginId::TopBar],
            states: PLUGIN_DESCRIPTORS
                .iter()
                .map(|descriptor| (descriptor.id, PluginState::Disabled))
                .collect(),
            shutting_down: false,
            runtime: RuntimeContext::new(false),
        }
    }

    fn failing_save(_config: &PluginConfig) -> Result<()> {
        Err(Error::new(
            windows::Win32::Foundation::E_FAIL,
            "simulated save failure",
        ))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn working_save(_config: &PluginConfig) -> Result<()> {
        Ok(())
    }

    fn is_running(host: &Host, plugin: PluginId) -> bool {
        host.started.contains(&plugin)
    }

    #[test]
    fn failed_save_while_enabling_leaves_the_plugin_stopped_and_disabled() {
        let mut host = host_with_required_plugins();
        let result = host.set_plugin_enabled(PluginId::ControlCenter, true, failing_save);
        assert!(result.is_err());
        assert!(!is_running(&host, PluginId::ControlCenter));
        assert!(!host.config.is_enabled(PluginId::ControlCenter));
    }

    #[test]
    fn failed_save_while_disabling_leaves_the_plugin_running_and_enabled() {
        let mut host = host_with_required_plugins();
        host.set_plugin_enabled(PluginId::ControlCenter, true, working_save)
            .unwrap();
        assert!(is_running(&host, PluginId::ControlCenter));
        let result = host.set_plugin_enabled(PluginId::ControlCenter, false, failing_save);
        assert!(result.is_err());
        assert!(is_running(&host, PluginId::ControlCenter));
        assert!(host.config.is_enabled(PluginId::ControlCenter));
    }

    #[test]
    fn successful_toggle_keeps_runtime_and_configuration_in_step() {
        let mut host = host_with_required_plugins();
        host.set_plugin_enabled(PluginId::ControlCenter, true, working_save)
            .unwrap();
        host.set_plugin_enabled(PluginId::ControlCenter, false, working_save)
            .unwrap();
        assert!(!is_running(&host, PluginId::ControlCenter));
        assert!(!host.config.is_enabled(PluginId::ControlCenter));
        host.stop_plugin(PluginId::ControlCenter);
        host.stop_plugin(PluginId::ControlCenter);
        assert_eq!(host.started, vec![PluginId::PopupMenu, PluginId::TopBar]);
    }

    #[test]
    fn enabling_a_plugin_whose_dependency_is_not_running_is_rejected() {
        let mut host = host_with_required_plugins();
        host.started.clear();
        let result = host.set_plugin_enabled(PluginId::ControlCenter, true, working_save);
        assert!(result.is_err());
        assert!(!host.config.is_enabled(PluginId::ControlCenter));
    }

    #[test]
    fn newer_configuration_version_is_preserved_and_never_overwritten() {
        let config = PluginConfig::parse("version=2\nenabled=popup_menu,top_bar\nfuture=1\n");
        assert_eq!(config.source, ConfigSource::Unsupported);
        assert!(config.is_enabled(PluginId::TopBar));
        assert!(config.is_enabled(PluginId::Dock));
        assert!(config.save().is_err());
        assert_eq!(
            PluginConfig::parse("version=banana\n").source,
            ConfigSource::Unsupported
        );
    }

    #[test]
    fn current_and_legacy_configuration_versions_are_accepted() {
        assert_eq!(
            PluginConfig::parse("version=1\nenabled=dock\n").source,
            ConfigSource::Supported
        );
        assert_eq!(
            PluginConfig::parse("enabled=dock\n").source,
            ConfigSource::Supported
        );
        assert_eq!(PluginConfig::default().source, ConfigSource::Defaults);
    }

    fn hwnd(value: usize) -> HWND {
        HWND(value as *mut _)
    }

    #[test]
    fn config_keeps_required_top_bar_and_ignores_unknown_plugins() {
        let config = PluginConfig::parse("version=1\nenabled=dock,unknown\n");
        assert!(config.is_enabled(PluginId::TopBar));
        assert!(config.is_enabled(PluginId::Dock));
        assert!(!config.is_enabled(PluginId::WindowControls));
    }

    #[test]
    fn dock_preferences_load_from_config_and_default_when_absent() {
        let legacy = PluginConfig::parse(
            "version=1
enabled=dock
online_lyrics=false
",
        );
        assert_eq!(legacy.dock, otto_core::DockPreferences::default());
        let saved = PluginConfig::parse(
            "version=1
enabled=dock
dock_autohide=true
dock_magnification=false
dock_icon_size=64
",
        );
        assert!(saved.dock.autohide);
        assert!(!saved.dock.magnification);
        assert_eq!(saved.dock.icon_size, 64);
    }

    #[test]
    fn config_defaults_all_plugins_when_enabled_field_is_missing() {
        let config = PluginConfig::parse("version=1\n");
        assert!(config.is_enabled(PluginId::TopBar));
        assert!(config.is_enabled(PluginId::ControlCenter));
        assert!(!config.online_lyrics);
    }

    #[test]
    fn config_parses_online_lyrics_as_an_explicit_opt_in() {
        let config =
            PluginConfig::parse("version=1\nenabled=popup_menu,top_bar\nonline_lyrics=true\n");
        assert!(config.online_lyrics);
        let disabled = PluginConfig::parse("version=1\nenabled=popup_menu,top_bar\n");
        assert!(!disabled.online_lyrics);
    }

    #[test]
    fn descriptors_are_declared_in_dependency_order() {
        let mut started = Vec::new();
        for descriptor in PLUGIN_DESCRIPTORS {
            assert!(
                dependencies_running(&started, descriptor.dependencies),
                "{} dependencies must start first",
                descriptor.id.name()
            );
            started.push(descriptor.id);
        }
    }

    #[test]
    fn descriptors_register_surfaces_and_virtual_features() {
        assert!(descriptor(PluginId::Dock).surface.is_some());
        assert!(descriptor(PluginId::WindowControls).surface.is_some());
        assert!(descriptor(PluginId::Genie).surface.is_some());
        assert!(descriptor(PluginId::ControlCenter).surface.is_some());
        assert!(descriptor(PluginId::Dock).start.is_some());
        assert!(descriptor(PluginId::ControlCenter).start.is_some());
    }

    #[test]
    fn surface_registry_stores_and_takes_dynamic_slots() {
        let mut surfaces = Surfaces {
            bar: hwnd(1),
            panel: hwnd(2),
            menu: hwnd(3),
            dock: None,
            window_controls: None,
            genie: None,
            control_center: None,
        };
        let dock = hwnd(4);
        let control_center = hwnd(5);
        surfaces.store(SurfaceSlot::Dock, dock).unwrap();
        surfaces
            .store(SurfaceSlot::ControlCenter, control_center)
            .unwrap();
        assert_eq!(surfaces.get(SurfaceSlot::Dock), Some(dock));
        assert_eq!(
            surfaces.get(SurfaceSlot::ControlCenter),
            Some(control_center)
        );
        assert_eq!(surfaces.take(SurfaceSlot::Dock), Some(dock));
        assert_eq!(
            surfaces.take(SurfaceSlot::ControlCenter),
            Some(control_center)
        );
        assert_eq!(surfaces.get(SurfaceSlot::Dock), None);
        assert_eq!(surfaces.get(SurfaceSlot::ControlCenter), None);
        assert!(surfaces.store(SurfaceSlot::Menu, hwnd(6)).is_err());
    }

    #[test]
    fn enabled_dependents_block_dependency_disable() {
        let mut config = PluginConfig::default();
        assert_eq!(enabled_dependent(PluginId::Genie, &config), None);
        assert_eq!(
            enabled_dependent(PluginId::TopBar, &config),
            Some(PluginId::ControlCenter)
        );
        config.set_enabled(PluginId::ControlCenter, false);
        assert_eq!(enabled_dependent(PluginId::TopBar, &config), None);
    }

    #[test]
    fn shutdown_order_reverses_start_order() {
        let started = vec![
            PluginId::PopupMenu,
            PluginId::TopBar,
            PluginId::Genie,
            PluginId::Dock,
            PluginId::WindowControls,
            PluginId::ControlCenter,
        ];
        let stopped = reverse_lifecycle_order(&started).collect::<Vec<_>>();
        assert_eq!(
            stopped,
            vec![
                PluginId::ControlCenter,
                PluginId::WindowControls,
                PluginId::Dock,
                PluginId::Genie,
                PluginId::TopBar,
                PluginId::PopupMenu,
            ]
        );
    }

    #[test]
    fn preferences_round_trip_through_plugin_config() {
        let mut config = PluginConfig::default();
        config.preferences.theme_mode = otto_core::ThemeMode::Dark;
        config.preferences.capture_visible = true;
        config.preferences.hotkeys_enabled = false;
        config.preferences.classic_logo = true;
        config.preferences.onboarding_seen = true;
        let serialized = format!(
            "version=1\nenabled=popup_menu,top_bar\nonline_lyrics=false\n{}\n{}\n",
            config.dock.config_lines().trim_end_matches('\n'),
            config.preferences.config_lines().trim_end_matches('\n'),
        );
        let parsed = PluginConfig::parse(&serialized);
        assert_eq!(parsed.source, ConfigSource::Supported);
        assert_eq!(parsed.preferences, config.preferences);
    }

    #[test]
    fn reentrant_host_borrow_mut_returns_error_instead_of_panicking() {
        let _cleanup = TestHostGuard;
        HOST.with(|cell| *cell.borrow_mut() = Some(host_with_required_plugins()));
        HOST.with(|cell| {
            let _borrow = cell.borrow_mut();
            assert!(set_plugin_enabled(PluginId::ControlCenter, true).is_err());
            assert!(set_online_lyrics_enabled(true).is_err());
            assert!(set_dock_preferences(otto_core::DockPreferences::default()).is_err());
            assert!(update_preferences(|_| {}).is_err());
        });
    }

    #[test]
    fn host_shutdown_marks_shutting_down_while_host_remains_registered() {
        struct BackgroundResumeGuard;
        impl Drop for BackgroundResumeGuard {
            fn drop(&mut self) {
                otto_platform::resume_background_tasks();
            }
        }

        let _cleanup = TestHostGuard;
        let _resume = BackgroundResumeGuard;
        let mut host = host_with_required_plugins();
        host.set_plugin_enabled(PluginId::ControlCenter, true, working_save)
            .unwrap();
        assert!(
            host.surfaces
                .as_ref()
                .and_then(|surfaces| surfaces.get(SurfaceSlot::ControlCenter))
                .is_some()
        );
        HOST.with(|cell| *cell.borrow_mut() = Some(host));
        assert!(should_post_quit(1));
        assert!(!should_post_quit(2));
        let hwnds = HOST.with(|cell| {
            cell.borrow_mut()
                .as_mut()
                .map(Host::shutdown)
                .unwrap_or_default()
        });
        assert!(hwnds.is_empty());
        assert!(!should_post_quit(1));
        HOST.with(|cell| {
            let borrow = cell.borrow();
            let host = borrow
                .as_ref()
                .expect("host remains present during shutdown");
            assert!(host.shutting_down);
            assert!(host.started.is_empty());
        });
    }

    #[test]
    fn nested_modal_flag_blocks_preparing_second_modal_action() {
        let _cleanup = TestHostGuard;
        let host = host_with_required_plugins();
        *host.runtime.pending_modal_action.borrow_mut() =
            Some(bar::DeferredModalAction::PickShelfFiles);
        HOST.with(|cell| *cell.borrow_mut() = Some(host));
        MODAL_ACTION_ACTIVE.with(|flag| flag.set(true));
        let _modal_guard = ModalActionGuard;
        let result = dispatch_runtime_message(|_| Some(LRESULT(7)));
        assert_eq!(result, Some(LRESULT(7)));
        HOST.with(|cell| {
            let borrow = cell.borrow();
            let host = borrow.as_ref().expect("host remains registered");
            assert!(host.runtime.pending_modal_action.borrow().is_none());
        });
    }

    #[test]
    fn pending_destroy_hwnds_are_queued_and_drained_once() {
        let runtime = RuntimeContext::new(false);
        runtime.queue_destroy_hwnd(0);
        runtime.queue_destroy_hwnd(42);
        runtime.queue_destroy_hwnd(99);
        assert_eq!(
            runtime.take_pending_destroy_hwnds(),
            vec![hwnd(42), hwnd(99)]
        );
        assert!(runtime.take_pending_destroy_hwnds().is_empty());
    }

    #[test]
    fn end_session_dispatch_stops_host_and_is_idempotent() {
        let _cleanup = TestHostGuard;
        let host = host_with_required_plugins();
        HOST.with(|cell| *cell.borrow_mut() = Some(host));
        let result = dispatch_runtime_message(|runtime| {
            runtime.pending_end_session.set(true);
            Some(LRESULT(0))
        });
        assert_eq!(result, Some(LRESULT(0)));
        assert!(HOST.with(|cell| cell.borrow().is_none()));
        super::shutdown_session();
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        let posted_quit = unsafe {
            windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                &mut msg,
                None,
                windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
                windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
                windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
            )
        };
        assert!(posted_quit.as_bool());
    }

    #[test]
    fn queued_plugin_toggles_preserve_multiple_plugins_without_clobbering() {
        let runtime = RuntimeContext::new(false);
        runtime.queue_plugin_toggle(PluginId::Dock, false);
        runtime.queue_plugin_toggle(PluginId::Genie, false);
        runtime.queue_plugin_toggle(PluginId::WindowControls, true);
        assert_eq!(
            runtime.take_plugin_toggles(),
            vec![
                (PluginId::Dock, false),
                (PluginId::WindowControls, true),
                (PluginId::Genie, false),
            ]
        );
        assert!(runtime.take_plugin_toggles().is_empty());
    }
}
