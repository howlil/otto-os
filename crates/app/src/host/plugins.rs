use super::messages::control_center_window_proc;
use super::messages::dock_window_proc;
use super::messages::genie_window_proc;
use super::messages::spotlight_window_proc;
use super::messages::window_controls_window_proc;
use std::fs;

use windows::Win32::UI::WindowsAndMessaging::WNDPROC;
use windows::core::{Error, PCWSTR, Result, w};

use crate::window_controls;
use otto_core::{AppPreferences, DockPreferences};
use otto_platform::replace_file_atomically;

use super::Host;
use super::config_path;
use super::io_error;
use super::runtime::RuntimeContext;
use super::start_control_center;
use super::start_dock;
use super::start_genie;
use super::start_popup_menu;
use super::start_spotlight;
use super::start_top_bar;
use super::start_window_controls;
use super::stop_control_center;
use super::stop_dock;
use super::stop_genie;
use super::stop_popup_menu;
use super::stop_spotlight;
use super::stop_top_bar;
use super::stop_window_controls;

pub(super) const CONFIG_FILE: &str = "plugins.txt";
pub(super) const SUPPORTED_CONFIG_VERSION: u32 = 1;
pub(super) const TOGGLEABLE_PLUGINS: [PluginId; 4] = [
    PluginId::Dock,
    PluginId::WindowControls,
    PluginId::Genie,
    PluginId::TopBar,
];

pub(super) const fn toggleable_plugin_index(plugin: PluginId) -> Option<usize> {
    match plugin {
        PluginId::Dock => Some(0),
        PluginId::WindowControls => Some(1),
        PluginId::Genie => Some(2),
        PluginId::TopBar => Some(3),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PluginId {
    PopupMenu,
    TopBar,
    Spotlight,
    Dock,
    WindowControls,
    Genie,
    ControlCenter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PluginState {
    Disabled,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Copy)]
pub(super) enum SurfaceSlot {
    Bar,
    Panel,
    Menu,
    Dock,
    WindowControls,
    Genie,
    ControlCenter,
    Spotlight,
}

#[derive(Clone, Copy)]
pub(super) struct SurfaceDescriptor {
    pub(super) class_name: PCWSTR,
    pub(super) wnd_proc: WNDPROC,
    pub(super) slot: SurfaceSlot,
    pub(super) topmost: bool,
}

pub(super) type PluginStart = fn(&Host) -> Result<()>;
pub(super) type PluginStop = fn(&RuntimeContext);

#[derive(Clone, Copy)]
pub(super) struct PluginDescriptor {
    pub(super) id: PluginId,
    pub(super) required: bool,
    pub(super) dependencies: &'static [PluginId],
    pub(super) surface: Option<SurfaceDescriptor>,
    pub(super) start: Option<PluginStart>,
    pub(super) stop: PluginStop,
}

pub(super) const NO_DEPENDENCIES: &[PluginId] = &[];
pub(super) const POPUP_DEPENDENCY: &[PluginId] = &[PluginId::PopupMenu];

pub(super) const DOCK_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoDockBar"),
    wnd_proc: Some(dock_window_proc),
    slot: SurfaceSlot::Dock,
    topmost: true,
};
pub(super) const WINDOW_CONTROLS_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: window_controls::OVERLAY_CLASS,
    wnd_proc: Some(window_controls_window_proc),
    slot: SurfaceSlot::WindowControls,
    topmost: false,
};
pub(super) const GENIE_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoGenie"),
    wnd_proc: Some(genie_window_proc),
    slot: SurfaceSlot::Genie,
    topmost: true,
};
pub(super) const CONTROL_CENTER_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoControlCenter"),
    wnd_proc: Some(control_center_window_proc),
    slot: SurfaceSlot::ControlCenter,
    topmost: true,
};
pub(super) const SPOTLIGHT_SURFACE: SurfaceDescriptor = SurfaceDescriptor {
    class_name: w!("OttoSpotlight"),
    wnd_proc: Some(spotlight_window_proc),
    slot: SurfaceSlot::Spotlight,
    topmost: true,
};

pub(super) const PLUGIN_DESCRIPTORS: &[PluginDescriptor] = &[
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
        id: PluginId::Spotlight,
        required: true,
        dependencies: POPUP_DEPENDENCY,
        surface: Some(SPOTLIGHT_SURFACE),
        start: Some(start_spotlight),
        stop: stop_spotlight,
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
        dependencies: POPUP_DEPENDENCY,
        surface: Some(CONTROL_CENTER_SURFACE),
        start: Some(start_control_center),
        stop: stop_control_center,
    },
];

impl PluginId {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::PopupMenu => "popup_menu",
            Self::TopBar => "top_bar",
            Self::Spotlight => "spotlight",
            Self::Dock => "dock",
            Self::WindowControls => "window_controls",
            Self::Genie => "genie",
            Self::ControlCenter => "control_center",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "popup_menu" => Some(Self::PopupMenu),
            "top_bar" => Some(Self::TopBar),
            "spotlight" => Some(Self::Spotlight),
            "dock" => Some(Self::Dock),
            "window_controls" | "traffic_light" => Some(Self::WindowControls),
            "genie" => Some(Self::Genie),
            "control_center" => Some(Self::ControlCenter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigSource {
    Defaults,
    Supported,
    Unsupported,
}

#[derive(Clone, Debug)]
pub(super) struct PluginConfig {
    pub(super) enabled: Vec<PluginId>,
    pub(super) online_lyrics: bool,
    pub(super) dock: DockPreferences,
    pub(super) preferences: AppPreferences,
    pub(super) source: ConfigSource,
}

pub(super) fn config_source(contents: &str) -> ConfigSource {
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
                PluginId::Spotlight,
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
    pub(super) fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&contents)
    }

    pub(super) fn parse(contents: &str) -> Self {
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

    pub(super) fn save(&self) -> Result<()> {
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

    pub(super) fn ensure_required(&mut self) {
        if !self.enabled.contains(&PluginId::PopupMenu) {
            self.enabled.push(PluginId::PopupMenu);
        }
        if !self.enabled.contains(&PluginId::Spotlight) {
            self.enabled.push(PluginId::Spotlight);
        }
    }

    pub(super) fn is_enabled(&self, plugin: PluginId) -> bool {
        self.enabled.contains(&plugin)
    }

    pub(super) fn set_enabled(&mut self, plugin: PluginId, enabled: bool) {
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
