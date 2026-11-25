use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::HWND;
use windows::core::{Error, Result};

use crate::{bar, control_center, dock, genie, menu, settings_window, spotlight, window_controls};
use otto_core::{
    AppPreferences, AppearancePreferences, BrandIcon, DockPreferences, MenuBarPreferences,
    NotchStyle, SystemAppearance,
};

use super::plugins::PluginId;
use super::plugins::SurfaceSlot;
use super::plugins::TOGGLEABLE_PLUGINS;
use super::plugins::toggleable_plugin_index;

pub(super) struct Surfaces {
    pub(super) bar: HWND,
    pub(super) panel: HWND,
    pub(super) menu: HWND,
    pub(super) dock: Option<HWND>,
    pub(super) window_controls: Option<HWND>,
    pub(super) genie: Option<HWND>,
    pub(super) control_center: Option<HWND>,
    pub(super) spotlight: Option<HWND>,
}

impl Surfaces {
    pub(super) fn get(&self, slot: SurfaceSlot) -> Option<HWND> {
        match slot {
            SurfaceSlot::Bar => Some(self.bar),
            SurfaceSlot::Panel => Some(self.panel),
            SurfaceSlot::Menu => Some(self.menu),
            SurfaceSlot::Dock => self.dock,
            SurfaceSlot::WindowControls => self.window_controls,
            SurfaceSlot::Genie => self.genie,
            SurfaceSlot::ControlCenter => self.control_center,
            SurfaceSlot::Spotlight => self.spotlight,
        }
    }

    pub(super) fn store(&mut self, slot: SurfaceSlot, hwnd: HWND) -> Result<()> {
        let target = match slot {
            SurfaceSlot::Dock => &mut self.dock,
            SurfaceSlot::WindowControls => &mut self.window_controls,
            SurfaceSlot::Genie => &mut self.genie,
            SurfaceSlot::ControlCenter => &mut self.control_center,
            SurfaceSlot::Spotlight => &mut self.spotlight,
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

    pub(super) fn take(&mut self, slot: SurfaceSlot) -> Option<HWND> {
        match slot {
            SurfaceSlot::Dock => self.dock.take(),
            SurfaceSlot::WindowControls => self.window_controls.take(),
            SurfaceSlot::Genie => self.genie.take(),
            SurfaceSlot::ControlCenter => self.control_center.take(),
            SurfaceSlot::Spotlight => self.spotlight.take(),
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
    pub(crate) spotlight: RefCell<Option<spotlight::SpotlightSurface>>,
    pub(crate) settings_window: RefCell<Option<settings_window::SettingsWindow>>,
    pub(crate) finder_window: RefCell<Option<crate::finder::FinderWindow>>,
    pub(crate) creating_finder: Cell<bool>,
    pub(crate) bar_hwnd: Cell<isize>,
    pub(crate) dock_hwnd: Cell<isize>,
    pub(crate) secondary_bar_hwnds: RefCell<Vec<isize>>,
    pub(crate) secondary_dock_hwnds: RefCell<Vec<isize>>,
    pub(crate) system_appearance: Cell<SystemAppearance>,
    pub(crate) appearance_preferences: Cell<AppearancePreferences>,
    pub(crate) blur_supported: Cell<bool>,
    pub(crate) glass_enabled: Cell<bool>,
    pub(crate) online_lyrics_enabled: Cell<bool>,
    pub(crate) pending_online_lyrics: Cell<Option<bool>>,
    pub(crate) dock_preferences: Cell<DockPreferences>,
    pub(crate) pending_dock_preferences: Cell<Option<DockPreferences>>,
    pub(crate) focus_active: Cell<bool>,
    pub(crate) top_bar_enabled: Cell<bool>,
    pub(crate) notch_enabled: Cell<bool>,
    pub(crate) dock_enabled: Cell<bool>,
    pub(crate) window_controls_enabled: Cell<bool>,
    pub(crate) genie_enabled: Cell<bool>,
    pub(crate) hotkeys_enabled: Cell<bool>,
    pub(crate) capture_visible: Cell<bool>,
    pub(crate) brand_icon: Cell<BrandIcon>,
    pub(crate) dynamic_island_enabled: Cell<bool>,
    pub(crate) privacy_indicators_enabled: Cell<bool>,
    pub(crate) clipboard_history_enabled: Cell<bool>,
    pub(crate) drop_shelf_enabled: Cell<bool>,
    pub(crate) notch_hud_enabled: Cell<bool>,
    pub(crate) media_module_enabled: Cell<bool>,
    pub(crate) focus_module_enabled: Cell<bool>,
    pub(crate) background_cleanup_enabled: Cell<bool>,
    pub(crate) finder_enabled: Cell<bool>,
    pub(crate) menu_bar: Cell<MenuBarPreferences>,
    pub(crate) quick_capture_enabled: Cell<bool>,
    pub(crate) onboarding_seen: Cell<bool>,
    pub(crate) pending_plugin_toggles: [Cell<Option<bool>>; 4],
    pub(crate) pending_capture_visible: Cell<Option<bool>>,
    pub(crate) pending_hotkeys_enabled: Cell<Option<bool>>,
    pub(crate) pending_brand_icon: Cell<Option<BrandIcon>>,
    pub(crate) pending_notch_enabled: Cell<Option<bool>>,
    pub(crate) pending_dynamic_island: Cell<Option<bool>>,
    pub(crate) pending_privacy_indicators: Cell<Option<bool>>,
    pub(crate) pending_clipboard_history: Cell<Option<bool>>,
    pub(crate) pending_drop_shelf: Cell<Option<bool>>,
    pub(crate) pending_notch_hud: Cell<Option<bool>>,
    pub(crate) pending_media_module: Cell<Option<bool>>,
    pub(crate) pending_focus_module: Cell<Option<bool>>,
    pub(crate) pending_background_cleanup: Cell<Option<bool>>,
    pub(crate) pending_finder_enabled: Cell<Option<bool>>,
    pub(crate) pending_menu_bar: Cell<Option<MenuBarPreferences>>,
    pub(crate) pending_quick_capture: Cell<Option<bool>>,
    pub(crate) pending_onboarding_seen: Cell<Option<bool>>,
    pub(crate) pending_appearance: Cell<Option<AppearancePreferences>>,
    pub(crate) pending_end_session: Cell<bool>,
    pub(crate) pending_modal_action: RefCell<Option<bar::DeferredModalAction>>,
    pub(crate) pending_destroy_hwnds: RefCell<Vec<isize>>,
    pub(crate) focus_activity: RefCell<otto_core::FocusActivityState>,
    pub(crate) focus_shield: RefCell<crate::focus_shield::FocusShield>,
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
            spotlight: RefCell::new(None),
            settings_window: RefCell::new(None),
            finder_window: RefCell::new(None),
            creating_finder: Cell::new(false),
            bar_hwnd: Cell::new(0),
            dock_hwnd: Cell::new(0),
            secondary_bar_hwnds: RefCell::new(Vec::new()),
            secondary_dock_hwnds: RefCell::new(Vec::new()),
            system_appearance: Cell::new(SystemAppearance::DEFAULT),
            appearance_preferences: Cell::new(AppearancePreferences::default()),
            blur_supported: Cell::new(false),
            glass_enabled: Cell::new(false),
            online_lyrics_enabled: Cell::new(online_lyrics_enabled),
            pending_online_lyrics: Cell::new(None),
            dock_preferences: Cell::new(DockPreferences::default()),
            pending_dock_preferences: Cell::new(None),
            focus_active: Cell::new(false),
            top_bar_enabled: Cell::new(true),
            notch_enabled: Cell::new(true),
            dock_enabled: Cell::new(true),
            window_controls_enabled: Cell::new(true),
            genie_enabled: Cell::new(true),
            hotkeys_enabled: Cell::new(true),
            capture_visible: Cell::new(false),
            brand_icon: Cell::new(BrandIcon::Otto),
            dynamic_island_enabled: Cell::new(false),
            privacy_indicators_enabled: Cell::new(true),
            clipboard_history_enabled: Cell::new(true),
            drop_shelf_enabled: Cell::new(true),
            notch_hud_enabled: Cell::new(true),
            media_module_enabled: Cell::new(true),
            focus_module_enabled: Cell::new(true),
            background_cleanup_enabled: Cell::new(false),
            finder_enabled: Cell::new(true),
            menu_bar: Cell::new(MenuBarPreferences::default()),
            quick_capture_enabled: Cell::new(false),
            onboarding_seen: Cell::new(false),
            pending_plugin_toggles: [const { Cell::new(None) }; 4],
            pending_capture_visible: Cell::new(None),
            pending_hotkeys_enabled: Cell::new(None),
            pending_brand_icon: Cell::new(None),
            pending_notch_enabled: Cell::new(None),
            pending_dynamic_island: Cell::new(None),
            pending_privacy_indicators: Cell::new(None),
            pending_clipboard_history: Cell::new(None),
            pending_drop_shelf: Cell::new(None),
            pending_notch_hud: Cell::new(None),
            pending_media_module: Cell::new(None),
            pending_focus_module: Cell::new(None),
            pending_background_cleanup: Cell::new(None),
            pending_finder_enabled: Cell::new(None),
            pending_menu_bar: Cell::new(None),
            pending_quick_capture: Cell::new(None),
            pending_onboarding_seen: Cell::new(None),
            pending_appearance: Cell::new(None),
            pending_end_session: Cell::new(false),
            pending_modal_action: RefCell::new(None),
            pending_destroy_hwnds: RefCell::new(Vec::new()),
            focus_activity: RefCell::new(otto_platform::load_focus_activity()),
            focus_shield: RefCell::new(crate::focus_shield::FocusShield::default()),
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

    pub(crate) fn set_secondary_bar_hwnds(&self, hwnds: Vec<isize>) {
        if let Ok(mut slot) = self.secondary_bar_hwnds.try_borrow_mut() {
            *slot = hwnds;
        }
    }

    pub(crate) fn set_secondary_dock_hwnds(&self, hwnds: Vec<isize>) {
        if let Ok(mut slot) = self.secondary_dock_hwnds.try_borrow_mut() {
            *slot = hwnds;
        }
    }

    pub(crate) fn overlay_anchor_hwnds(&self) -> Vec<isize> {
        let mut anchors = Vec::new();
        let primary_bar = self.bar_hwnd.get();
        if primary_bar != 0 {
            anchors.push(primary_bar);
        }
        let primary_dock = self.dock_hwnd.get();
        if primary_dock != 0 {
            anchors.push(primary_dock);
        }
        if let Ok(bars) = self.secondary_bar_hwnds.try_borrow() {
            anchors.extend(bars.iter().copied().filter(|&hwnd| hwnd != 0));
        }
        if let Ok(docks) = self.secondary_dock_hwnds.try_borrow() {
            anchors.extend(docks.iter().copied().filter(|&hwnd| hwnd != 0));
        }
        anchors
    }

    pub(crate) fn brand_icon(&self) -> BrandIcon {
        self.brand_icon.get()
    }

    pub(crate) fn notch_style(&self) -> NotchStyle {
        if self.dynamic_island_enabled.get() {
            NotchStyle::DynamicIsland
        } else {
            NotchStyle::ClassicNotch
        }
    }

    pub(super) fn apply_preferences(&self, preferences: AppPreferences) {
        self.capture_visible.set(preferences.capture_visible);
        self.hotkeys_enabled.set(preferences.hotkeys_enabled);
        self.brand_icon.set(preferences.brand_icon);
        self.notch_enabled.set(preferences.notch_enabled);
        self.dynamic_island_enabled.set(preferences.dynamic_island);
        self.privacy_indicators_enabled
            .set(preferences.privacy_indicators);
        self.clipboard_history_enabled
            .set(preferences.clipboard_history);
        self.drop_shelf_enabled.set(preferences.drop_shelf);
        self.notch_hud_enabled.set(preferences.notch_hud);
        self.media_module_enabled.set(preferences.media_module);
        self.focus_module_enabled.set(preferences.focus_module);
        self.background_cleanup_enabled
            .set(preferences.background_cleanup);
        self.finder_enabled.set(preferences.finder_enabled);
        self.menu_bar.set(preferences.menu_bar);
        self.quick_capture_enabled.set(preferences.quick_capture);
        self.onboarding_seen.set(preferences.onboarding_seen);
        self.appearance_preferences.set(preferences.appearance);
    }
}
