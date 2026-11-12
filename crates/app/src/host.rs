use std::cell::{Cell, RefCell};
use std::fs;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, HCURSOR, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW,
    TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WNDCLASSEXW, WNDPROC, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result, w};

use crate::{bar, dock, genie, menu, window_controls};
use otto_core::{SystemAppearance, ThemeMode};
use otto_platform::{
    replace_file_atomically, restore_hidden_windows, shutdown_audio_task, shutdown_media_task,
};

const CONFIG_FILE: &str = "plugins.txt";
const SUPPORTED_CONFIG_VERSION: u32 = 1;

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
}

#[derive(Clone, Copy)]
struct SurfaceDescriptor {
    class_name: PCWSTR,
    wnd_proc: WNDPROC,
    slot: SurfaceSlot,
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
const WINDOW_CONTROLS_DEPENDENCIES: &[PluginId] = &[PluginId::PopupMenu, PluginId::Genie];

const DOCK_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoDockBar"),
    wnd_proc: Some(dock_window_proc),
    slot: SurfaceSlot::Dock,
};
const WINDOW_CONTROLS_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoWindowControls"),
    wnd_proc: Some(window_controls_window_proc),
    slot: SurfaceSlot::WindowControls,
};
const GENIE_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoGenie"),
    wnd_proc: Some(genie_window_proc),
    slot: SurfaceSlot::Genie,
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
        dependencies: WINDOW_CONTROLS_DEPENDENCIES,
        surface: Some(WINDOW_CONTROLS_SURFACE),
        start: Some(start_window_controls),
        stop: stop_window_controls,
    },
    PluginDescriptor {
        id: PluginId::ControlCenter,
        required: false,
        dependencies: &[PluginId::PopupMenu, PluginId::TopBar],
        surface: None,
        start: None,
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
                "version={SUPPORTED_CONFIG_VERSION}\nenabled={enabled}\nonline_lyrics={}\n",
                self.online_lyrics
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
        }
    }

    fn store(&mut self, slot: SurfaceSlot, hwnd: HWND) -> Result<()> {
        let target = match slot {
            SurfaceSlot::Dock => &mut self.dock,
            SurfaceSlot::WindowControls => &mut self.window_controls,
            SurfaceSlot::Genie => &mut self.genie,
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
    pub(crate) theme_mode: Cell<ThemeMode>,
    pub(crate) system_appearance: Cell<SystemAppearance>,
    pub(crate) online_lyrics_enabled: Cell<bool>,
    pub(crate) pending_online_lyrics: Cell<Option<bool>>,
}

impl RuntimeContext {
    pub(crate) fn new(online_lyrics_enabled: bool) -> Self {
        Self {
            bar: RefCell::new(None),
            menu: RefCell::new(None),
            dock: RefCell::new(None),
            window_controls: RefCell::new(None),
            genie: RefCell::new(None),
            theme_mode: Cell::new(ThemeMode::Auto),
            system_appearance: Cell::new(SystemAppearance::DEFAULT),
            online_lyrics_enabled: Cell::new(online_lyrics_enabled),
            pending_online_lyrics: Cell::new(None),
        }
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
}

pub fn start(blur: bool) -> Result<()> {
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
    if let Err(error) = host.initialize() {
        host.shutdown();
        return Err(error);
    }
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    Ok(())
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
    HOST.with(|cell| {
        if let Some(mut host) = cell.borrow_mut().take() {
            host.shutdown();
        }
    });
}

pub fn set_plugin_enabled(plugin: PluginId, enabled: bool) -> Result<()> {
    let result = HOST.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(host) = guard.as_mut() else {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto host is not running",
            ));
        };
        host.set_plugin_enabled(plugin, enabled, PluginConfig::save)
    });
    if result.is_ok() && plugin == PluginId::ControlCenter {
        HOST.with(|cell| {
            if let Some(host) = cell.borrow().as_ref() {
                bar::refresh_plugin_layout(
                    &host.runtime,
                    host.config.is_enabled(PluginId::ControlCenter),
                );
            }
        });
    }
    result
}

pub fn set_online_lyrics_enabled(enabled: bool) -> Result<()> {
    HOST.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(host) = guard.as_mut() else {
            return Err(Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Otto host is not running",
            ));
        };
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
        self.surfaces = Some(Surfaces {
            bar,
            panel,
            menu,
            dock: None,
            window_controls: None,
            genie: None,
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
            }
            return Err(error);
        }
        let result = descriptor.start.map_or(Ok(()), |start| start(self));
        if let Err(error) = result {
            self.destroy_surface(plugin);
            if !descriptor.required {
                self.config.set_enabled(plugin, false);
            }
            self.set_state(plugin, PluginState::Failed);
            return Err(error);
        }
        self.started.push(plugin);
        self.set_state(plugin, PluginState::Running);
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
            return Err(error);
        }
        if let Err(error) = persist(&self.config) {
            self.stop_plugin(plugin);
            self.config.set_enabled(plugin, false);
            return Err(error);
        }
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
            return Err(error);
        }
        self.stop_plugin(plugin);
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
        let surface = create_overlay_window(
            self.instance,
            self.cursor,
            descriptor.class_name,
            descriptor.wnd_proc,
            self.ex_style,
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
        Ok(surface)
    }

    fn destroy_surface(&mut self, plugin: PluginId) {
        let Some(descriptor) = descriptor(plugin).surface else {
            return;
        };
        let Some(surfaces) = self.surfaces.as_mut() else {
            return;
        };
        if let Some(surface) = surfaces.take(descriptor.slot) {
            unsafe {
                let _ = DestroyWindow(surface);
            }
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
        self.destroy_surface(plugin);
        self.set_state(plugin, PluginState::Disabled);
    }

    fn shutdown(&mut self) {
        self.shutting_down = true;
        let started = std::mem::take(&mut self.started);
        for plugin in reverse_lifecycle_order(&started) {
            self.stop_started_plugin(plugin);
        }
        let _ = restore_hidden_windows();
        shutdown_media_task();
        shutdown_audio_task();
        if let Some(surfaces) = self.surfaces.take() {
            unsafe {
                let _ = DestroyWindow(surfaces.menu);
                let _ = DestroyWindow(surfaces.panel);
                let _ = DestroyWindow(surfaces.bar);
            }
        }
        otto_render::release_shared_factories();
        for (_, state) in &mut self.states {
            *state = PluginState::Disabled;
        }
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
    menu::close(runtime);
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

fn dispatch_runtime_message(
    action: impl FnOnce(&RuntimeContext) -> Option<LRESULT>,
) -> Option<LRESULT> {
    let (result, pending_online_lyrics) = HOST.with(|cell| {
        let guard = cell.try_borrow().ok()?;
        let host = guard.as_ref()?;
        let result = action(&host.runtime);
        let pending = host.runtime.pending_online_lyrics.take();
        Some((result, pending))
    })?;
    if let Some(enabled) = pending_online_lyrics
        && set_online_lyrics_enabled(enabled).is_ok()
    {
        HOST.with(|cell| {
            if let Some(host) = cell.borrow().as_ref() {
                bar::set_online_lyrics_enabled(&host.runtime, enabled);
            }
        });
    }
    result
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
        ConfigSource, Error, HCURSOR, HINSTANCE, HWND, Host, PLUGIN_DESCRIPTORS, PluginConfig,
        PluginId, PluginState, Result, RuntimeContext, SurfaceSlot, Surfaces, WINDOW_EX_STYLE,
        dependencies_running, descriptor, enabled_dependent, reverse_lifecycle_order,
    };

    fn host_with_required_plugins() -> Host {
        let mut config = PluginConfig::default();
        config.set_enabled(PluginId::ControlCenter, false);
        Host {
            instance: HINSTANCE::default(),
            cursor: HCURSOR::default(),
            ex_style: WINDOW_EX_STYLE::default(),
            blur: false,
            config,
            surfaces: None,
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
        assert!(descriptor(PluginId::ControlCenter).surface.is_none());
        assert!(descriptor(PluginId::Dock).start.is_some());
        assert!(descriptor(PluginId::ControlCenter).start.is_none());
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
        };
        let dock = hwnd(4);
        surfaces.store(SurfaceSlot::Dock, dock).unwrap();
        assert_eq!(surfaces.get(SurfaceSlot::Dock), Some(dock));
        assert_eq!(surfaces.take(SurfaceSlot::Dock), Some(dock));
        assert_eq!(surfaces.get(SurfaceSlot::Dock), None);
        assert!(surfaces.store(SurfaceSlot::Menu, hwnd(5)).is_err());
    }

    #[test]
    fn enabled_dependents_block_dependency_disable() {
        let mut config = PluginConfig::default();
        assert_eq!(
            enabled_dependent(PluginId::Genie, &config),
            Some(PluginId::WindowControls)
        );
        config.set_enabled(PluginId::WindowControls, false);
        assert_eq!(enabled_dependent(PluginId::Genie, &config), None);
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
}
