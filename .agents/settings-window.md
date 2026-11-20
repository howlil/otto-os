# Otto Settings window — design plan (draft)

Status: proposal, no code changed. Supersedes the product line "without a standalone Settings window" and DEC-79 (inline Otto Settings overview inside Control Center) once accepted.

## Goal

One real Windows window, sidebar on the left and page on the right, that holds every Otto setting and lets every feature be plugged in or out at runtime. The tray icon is the canonical way to reach it.

## Access: tray icon is the source of truth

- `Host` already owns `TrayIcon` (DEC-80): left-click opens Settings, right-click opens the quick menu. That stays the canonical entry and can never be switched off.
- Every other entry calls the same `open_settings(page)`: `Ctrl+Alt+,`, Apple menu, Control Center cog, Dock right-click, idle Notch right-click. No entry owns its own copy of the UI.
- Windows 11 puts new tray icons in the overflow flyout. An app cannot pin itself, so first run shows a one-time hint to drag the Otto icon onto the bar.
- Recovery stays independent of any module: tray plus `Ctrl+Alt+,`, `Ctrl+Alt+B`, `Ctrl+Alt+D`, `--restore-shell`.

## Window

- Class `OttoSettings`: normal activatable top-level window (not `WS_EX_NOACTIVATE`, not topmost, not layered glass). Single instance; a second open focuses the first. Default 920 x 620, minimum 720 x 480, resizable, centered on the primary work area.
- Material: DWM system backdrop (Mica) with Windows dark mode, solid fallback when transparency is off or high contrast is on. DEC-57 rejected DWM backdrops only for layered no-activate glass; a normal window has none of those limits.
- Drawing: custom Direct2D, like every other Otto surface. Layout and hit regions live in `otto-core` and are consumed by both render and input. Rejected: WinUI/XAML (new runtime and dependency), common controls (no coherent dark mode), webview (runtime, network surface).
- Tokens from `DESIGN.md`: 4 pt grid, 6 pt control radius, `#202124` surface, `#F5F5F7` text, `#A8ABB2` muted, `#F04438` primary. Light mode follows Windows.
- Sidebar 220 pt (collapses to a 56 pt icon rail under 760 pt width). Content column max 640 pt, rows 44 pt, group headings 11 pt muted.
- Motion: page crossfade and switch-thumb slide from `MotionMetrics`; reduced motion settles immediately.
- Keyboard and UIA: Tab into sidebar, Up/Down to change page, Space toggles, every row has a UIA name and state. Search box deferred (needs a native `EDIT` child like Quick Capture).

## Pages (sidebar)

| Page | Contents | Maps from today |
| --- | --- | --- |
| Overview | health, modules on/off count, active shortcuts, reset to defaults | new |
| Modules | master plug in / plug off list with status | new |
| Top Bar | auto-hide mode, background, keep notch, show in bar (Tray, Status, Search, Notifications, Clock), classic logo | MenuBar |
| Notch | style (Classic / Dynamic Island), HUD, privacy dots; tools: Media (+ online lyrics), Focus, Clipboard, Pocket, Quick Capture | Notch + NotchTools |
| Dock | behavior (always / auto-hide / reserve space), icon size, magnification, Downloads, Trash, hide Windows taskbar | Dock |
| Window Controls | on/off, Genie on/off | Window + Genie |
| Spotlight | on/off, shortcut | new page |
| Control Center | on/off, shortcut | new page |
| Appearance | Liquid Glass, show in screenshots | Appearance |
| Shortcuts | all hotkeys, master hotkey switch, recovery keys read-only | Hotkeys |
| General | launch at login, Background Cleanup, open logs, check for updates | System |
| About | version, recovery, reset | new |

## Single source of truth for settings

- `otto-core` keeps one descriptor per setting: id, page, section, label, control (toggle, choice, slider), requirement, config key, default. `OttoSettingKind`, `OttoSettingItem`, `OttoSettingsSnapshot` and `is_item_available` already do most of this; the window reuses them and `OttoSettingsArea` grows into `SettingsPage`.
- One write path: the window emits typed actions into the same functions the Control Center uses today (`queue_plugin_toggle`, preference setters). No second setter, no new persistence; `plugins.txt` stays additive and backward compatible.

## Plug in / plug off

- Core adds `ModuleId` (a plugin or a preference master), `parent`, `locked`, and a read-only `ModuleStatus` (Off, Starting, Running, Failed with reason) published from `Host` (`PluginState` is private today).
- Toggleable today: Dock, Window Controls, Genie, Top Bar, Notch, and the Notch tools. New: Spotlight and Control Center become user-toggleable through the existing `user_can_toggle` path (DEC-78 pattern). Popup Menu stays infrastructure and is not listed.
- Locked: tray icon, Settings window, recovery hotkeys.
- Cascade: switching a parent off pauses children without clearing their own flags (DEC-78). In the Modules list a paused child stays visible, muted, with "Needs Notch". Setting rows inside a page stay hidden, not disabled (DEC-79).
- A failed start shows `Failed`, a reason, Retry and Open logs, and never blocks the rest.
- Applies live. Nothing in the current set needs a restart.

## Phases

1. Contract: update `product.md`, add DEC-81 to `engineering.md`, settle the open questions below.
2. Core: `settings_window` module (pages, modules, layout, hit targets, scroll) with tests.
3. Platform and app: `OttoSettings` window, single instance, DWM backdrop, entry-point routing.
4. Render: `SettingsRenderer` on the shared brush palette and text formats.
5. Modules: expose `ModuleStatus`, make Spotlight and Control Center toggleable.
6. Migration: Control Center cog opens the window; remove the inline overview from `control_center.rs` (asks first, it is a large deletion).
7. Keyboard, UIA, reduced motion, native tests, then desktop E2E (ask before running it).

Gate per phase: `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo check --workspace`, `cargo clippy --workspace --all-targets --all-features`.

## Open questions

1. Tray as canonical entry: confirm this is what "source of truth for access" means.
2. Control Center cog: replace the inline overview (recommended) or keep both.
3. Paused modules shown muted with a reason (recommended) or hidden.
4. Shortcut rebinding in v1: recommended no, read-only list first.
5. Otto's own Window Controls overlay may also cover the Settings window caption; check on Windows whether own-process windows are excluded, and decide if Settings should opt out.
