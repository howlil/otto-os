# Otto OS engineering contract

## System

Otto OS is a single-process Windows shell surface hosting layered windows on the primary monitor:
1. `OttoNotchBar` + `OttoNotchPanel`: `OttoNotchBar` reserves the primary monitor's top strip via `SPI_SETWORKAREA`, hides the native Windows taskbar while active, renders a layered macOS notch-style top bar with SF Pro typography and SF Symbol vector icons. Left side renders the Apple logo (``), the bold active application name (`Finder`, `Code`, `Chrome`, etc.), and global application menus (`File`, `Edit`, `View`, `Window`, `Help`) that open custom frosted-glass popup menus via `OttoPopupMenu` with hover-scrubbing across menu items and macOS shortcut symbols (`⌘`, `⌥`, `⇧`, `⌃`). Center Notch smoothly expands horizontally (`200pt ↔ 292pt` for active compact Pomodoro or Now Playing Live Activity, `200pt ↔ 356pt` for Volume/Mute, Brightness, Charging/Low Battery, and Caps Lock HUD events), and expands into a `380 × 116 pt` (`22pt` radius) Dynamic Island card via `OttoNotchPanel` on hover (~80ms debounce), global shortcut (`Alt+N` to toggle expand/close, `Alt+P` to start/pause), or phase-completion alert notification with native chime. When media is playing, the compact notch displays a vibrant Apple Music glyph, title, and animated 4-bar equalizer waveform; when both Pomodoro and media are active, dual live activities render the Pomodoro countdown on the left and equalizer on the right. Expanded notch includes a segmented tab switcher (`Focus` / `Now Playing`) with full track metadata and Previous, Play/Pause, Next controls. Right side renders System Tray Overflow (`^`), Status (`Wi-Fi`, dynamic `Speaker`, `Battery %` with scroll-wheel volume control), Spotlight Search (`magnifyingglass` -> `Win+S`), Control Center (`switch.2` -> custom frosted-glass popover with Sound and Display sliders, media block, and settings shortcuts), and Clock (left-click opens macOS monthly calendar popover with date grid, navigation, and today highlight).
2. `OttoDockBar`: renders a floating macOS Sonoma/Sequoia bottom Dock with Finder (File Explorer), Launchpad (Start/Apps grid), pinned and running applications (grouped 1 icon per executable with multi-window cycling on click), Downloads folder stack, a vertical separator, and dynamic Trash (Recycle Bin empty vs full queried via `SHQueryRecycleBinW`). All icons are rendered as uniform macOS Squircles (vector Apple icons for Finder, Launchpad, Trash/TrashFull, Terminal, Settings, Browser, Notes, Code, Music, and Folder; normalized squircle-wrapped bitmaps for third-party `.exe` icons). Features cosine fish-eye hover magnification, tooltip pills, launch/click bounce animation, smooth slot expand/shrink (`0 ↔ 48pt`) on app open/close, running/active indicator dots, auto-hide only when a normal application window is maximized or fullscreen, and a custom frosted-glass right-click context menu positioned centered above the Dock icon (open window list with active checkmark, `Keep in Dock`, `New Window`, `Show / Minimize`, `Empty Trash...`, `Quit`).
3. `OttoWindowControls`: renders macOS Green/Yellow/Red (`ToggleMaximize`, `Minimize`, `Close`) Window Control buttons anchored to the top-right of the active foreground application window over a multi-point sampled titlebar cover plate that hides the native `— □ ×` caption buttons. Following official Apple HIG terminology, buttons represent Close, Miniaturize, and Zoom/Full Screen. Window re-assertion uses explicit `ShowWindow(SW_SHOWNOACTIVATE)` and `SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW)` upon foreground transitions to guarantee continuous visibility. Yellow minimize uses the app-owned Genie overlay with cubic Hermite S-curve funnel suction physics, elastic bottom stretch, and 128-strip subpixel resolution; right-clicking opens the custom frosted-glass Window Tiling menu (`Enter/Exit Full Screen`, `Tile Window to Left/Right of Screen`, `Minimize`, `Close`).
4. `OttoPopupMenu`: renders a non-blocking per-pixel-alpha frosted-glass macOS Sonoma popup menu (`PopupMenuRenderer`) shared by Top Bar menus, Dock context menus, Window Controls tiling menus, and the Control Center. `MenuRow::Slider` rows are drag-adjustable; the app dispatches slider values to `set_system_volume` / `set_display_brightness`. The Control Center (left-click on the Top Bar `switch.2` slot) shows Sound and Display sliders, a Now Playing block (title, artist, Play/Pause, Next, Previous via Windows Global System Media Transport Controls in `otto-platform::media`), and shortcuts to Wi-Fi, Bluetooth, Focus, Display, and Sound settings. Wi-Fi, Bluetooth, and Focus do not toggle in place; they open Windows Settings.

## Structure and ownership

- `otto-core`: platform-independent shapes and rules: `Rect`, `Metrics`, `Layout`, `HitTarget`, `BatteryStatus`, `ClockTime`, `SystemStatus`, `NowPlaying`, `MediaCommand`, `NotchExpandedTab`, `ThemeMode`, `ResolvedTheme`, `days_in_month`, `month_cells`, `month_title`, `shift_month`, `weekday_of`, `AppleIconKind`, `DockInput`, `DockMetrics`, `DockItemLayout`, `DockLayout`, `compute_dock_layout`, `select_dock_click_window`, `MenuRow`, `MenuAnchor`, `PopupMenuMetrics`, `PopupMenuLayout`, `compute_popup_menu_layout`, `build_apple_menu_rows`, `build_calendar_rows`, `build_control_center_rows`, `build_top_bar_menu_rows`, `build_window_tiling_menu_rows`, `build_traffic_tiling_menu_rows`, `GenieGeometry`, `GenieStrip`, `compute_genie_geometry`, `step_toward`, `smooth_damp`, `MotionMetrics`, `MotionPreference`, `animate_value`, `advance_bounce_phase`, `advance_genie_progress`, `bounce_offset`, `WindowControlAction`, `WindowControlsMetrics`, `WindowControlsLayout`, and `compute_window_controls_layout`.
- `otto-platform`: Win32 adapters for work-area reservation, taskbar visibility, DPI and monitor queries, native flyouts, system status, motion-preference queries, blur, single-instance ownership, window enumeration, high-resolution shell icon extraction (`SHIL_JUMBO` / `SHIL_EXTRALARGE`) with macOS squircle normalization, Recycle Bin state and emptying (`SHQueryRecycleBinW` / `SHEmptyRecycleBinW`), maximized/fullscreen window detection, HWND bounds and capture, native minimize/restore, temporary DWM cloaking, explicit native-transition policy, compositor flush, active window tracking/actions, multi-point titlebar color sampling, Windows system dark mode detection (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`), low-level keyboard hook multimedia key interception (`WH_KEYBOARD_LL` for `VK_VOLUME_UP`, `VK_VOLUME_DOWN`, `VK_VOLUME_MUTE`), native Windows OSD suppression (`NativeHUD`), and pinned app list persistence (`%LOCALAPPDATA%\otto-os\pinned.txt`).
- `otto-render`: Direct2D, DirectWrite, and GDI resource ownership and drawing (`Renderer`, `DockRenderer`, `WindowControlsRenderer`, `GenieRenderer`, `PopupMenuRenderer`, and `LiquidGlassPalette`). It receives resolved core data and presents pixels to layered windows.
- `otto-os`: process entry, window creation (`OttoNotchBar`, `OttoNotchPanel`, `OttoDockBar`, `OttoWindowControls`, `OttoGenie`, and `OttoPopupMenu`), message routing, timers, hover/animation state, Genie snapshot lifetime, runtime theme state coordination (`ThemeState`), and orchestration of platform and render operations.
- Within `otto-os`, the top-bar surface is composed by a root `Bar` orchestrator, a `NotchController` for Notch/Pomodoro/media state, and a `BarControlsController` for non-Notch menu and status actions. The root owns window geometry and presentation; feature controllers return domain updates or effects.

The dependency graph is:

```text
otto-os → otto-core
otto-os → otto-platform → otto-core
otto-os → otto-render → otto-core
```

No reverse dependency or platform dependency may enter `otto-core`.

## Shapes and flow

```text
Windows monitor/status/input
  → platform adapters
  → core values and resolved Layout
  → app-owned Bar state
  → render drawing and layered-window presentation
```

`Metrics` is the design input. `compute_layout` is the only layout resolver. `Layout` is the shared contract for rendering and hit testing. `HitTarget` is the only input result consumed by the app dispatcher. `SystemStatus` is a snapshot; the app refreshes the view only when the snapshot changes.

External application HWND state transitions remain native Win32 actions. For minimize and restore, the app presents a captured HWND snapshot through one bounded Genie overlay, commits its first frame, cloaks the source HWND, and performs the native state change with native transitions suppressed. Restore evaluates the exact inverse of minimize against the full snapshot; core Genie geometry and motion policy remain pure and reduced-motion-safe.

## State and execution

`Bar` owns the app bar registration, renderer, monitor snapshot, metrics, resolved layout, system status, hover target, and mouse tracking. The message loop is sequential on the window thread. Status polling is periodic, shell visibility is reasserted on each tick, and display or DPI changes rebuild the monitor-dependent renderer and layout before presenting again.

The process owns `InstanceGuard` for its lifetime. `AppBar` owns registration and taskbar restoration. `Renderer` owns Direct2D, DirectWrite, GDI, DIB, and DC resources and releases them through `Drop`. No spawned work may outlive the window thread without an explicit owner and cancellation rule.

## Failure policy

- Startup failures from window, app-bar, renderer, or required native resources propagate as `Result` and stop startup.
- Native flyout and shell calls are best effort; a failed call does not fabricate success.
- Blur is optional: if the OS rejects it, rendering continues with zero blur tint.
- A failed status query yields a safe default snapshot.
- A failed window capture or missing Dock target falls back to native minimize or restore and never leaves a visible overlay running.
- A failed presentation is transient at the runtime boundary; the next timer or state transition may retry it.
- No low-level Win32 or graphics error is exposed as a new domain protocol without an owning recovery policy.

## Design invariants

- All geometry is in physical pixels after DPI scaling from the `Metrics` point values.
- The bar spans the primary monitor width and uses the resolved notch and slot rectangles for both paint and interaction.
- Slot ordering is Start, Notch, Quick Settings, then Clock. Slots must remain within the bar and must not overlap the notch unexpectedly.
- Render geometry, typography, colors, icon proportions, and hover treatment are owned by `otto-render`; their dimensions derive from `Layout` or named render constants.
- Platform modules perform native conversion and validation once. Core and render code operate on typed values rather than raw handles or Win32 structs.
- Unsafe blocks stay at the narrowest FFI or resource boundary. Resource acquisition, use, and release must be visibly paired in the owning type.

## Change rules

1. Search for an existing owner before adding a helper, constant, or abstraction.
2. Change one ownership boundary at a time and keep the dependency direction intact.
3. Add or update the cheapest faithful test for every changed core rule or failure policy.
4. Keep comments out of source. If a rule must be durable, update this contract instead of adding inline explanation.
5. Do not add a dependency, persistence layer, network path, service, or global state without an explicit contract and verification command.

## Decisions

- DEC-1: `otto-os` owns the transitional `Host` registry, surface creation, plugin configuration, and feature lifecycle; feature modules remain static Rust plugins until their boundaries are independently migrated.
- DEC-2: Runtime activation uses an allowlisted user-level plugin configuration at `%LOCALAPPDATA%\otto-os\plugins.txt`; the Top Bar and Popup Menu remain required host surfaces.
- DEC-3: Platform-independent capability contracts begin in `otto-core`; Windows implementations remain in `otto-platform` and are injected into feature-owned state.
- DEC-4: Dynamic DLL loading is out of scope until static lifecycle, resource cleanup, ABI ownership, and failure isolation have release evidence.
- DEC-5: Windows media reads and commands run through a platform-owned COM worker with a short-lived cache; Host shutdown owns worker termination.
- DEC-6: Official Apple macOS HIG nomenclature ("Window Controls" with Close, Miniaturize, and Zoom/Full Screen) replaces legacy informal "traffic light" nomenclature across core, render, platform, and host modules while preserving backward compatibility for configuration parsing and transitional aliases.
- DEC-7: The top-bar Notch feature uses the `NotchPort` capability boundary for media, task-view, and Pomodoro-chime effects; `BarControlsController` remains snapshot/action based, while `Bar` owns menu anchors and platform execution. No renderer split or dynamic Notch plugin loading is introduced by this refactor.
- D-1: Optional plugin startup failures disable only that plugin and preserve the required host path; dependency violations are rejected before surface activation.
- D-2: Control Center is the first migrated capability boundary; its status, media, slider, and shell actions use `ControlCenterPort`.
- D-3: Plugin configuration writes use a platform-owned write-through replacement so a process or machine interruption does not leave a partial config file.
- DEC-8: Dual Theme Material System (`ThemeMode`: `Auto`, `Light`, `Dark`; `ResolvedTheme`: `Light`, `Dark`). `otto-core` models theme mode and resolution. `otto-platform` queries `AppsUseLightTheme` in Windows registry to determine system preference. `otto-render` provides `LiquidGlassPalette` implementing authentic Apple macOS Liquid Glass / Vibrancy materials (frosted translucent white with specular rim and dark graphite text for Light mode, deep smoke acrylic with specular white highlight for Dark mode). The Center Notch and Dynamic Island stay Obsidian Black (`#000000`) across all themes matching MacBook hardware notch aesthetics.
- DEC-9: Global Control Center Theme Switching. `ControlCenterPort` and popup menu rows include interactive Theme controls (`CMD_CC_THEME_TOGGLE`, `CMD_CC_THEME_LIGHT`, `CMD_CC_THEME_DARK`, `CMD_CC_THEME_AUTO`). Clicking updates `ThemeState` in `otto-os`, which synchronizes `ResolvedTheme` across `OttoNotchBar`, `OttoDockBar`, and `OttoPopupMenu`, triggering immediate repaint across all active layered surfaces.
- DEC-10: OS Event Single Source of Truth & Windows OSD Suppression. Low-level keyboard hook (`WH_KEYBOARD_LL`) in `otto-platform::os_events` intercepts native multimedia keys (`VK_VOLUME_UP`, `VK_VOLUME_DOWN`, `VK_VOLUME_MUTE`). When intercepted, Otto OS applies direct WASAPI scalar volume adjustments and returns `LRESULT(1)` to suppress the Windows Explorer volume flyout. In addition, periodic maintenance suppresses and hides native Windows on-screen displays (`NativeHUD`), ensuring Otto OS Notch HUD remains the sole source of truth for volume and mute feedback.

## Verification

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets --all-features`
- Core geometry and formatting tests must remain runnable without a Windows window system.
- PF1 (Genie continuity) [important] | core unit proof for inverse geometry, shared boundaries, full-snapshot sampling, and bounded presentation | stage: iter | `cargo test -p otto-core`.
- PF2 (plugin lifecycle) [important] | app unit proof for config parsing and required-plugin preservation | stage: iter | `cargo test -p otto-os`.
- PF3 (control-center port) [important] | workspace compile and unit proof for typed status/media/action boundary | stage: pr | `cargo test --workspace`.
- Capture fidelity, DWM cloak timing, frame pacing, and visual continuity still require Windows manual acceptance at release.
