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
- DEC-11: Event-Driven Window Controls & Adaptive Animation Timer. `otto-platform` installs a Win32 out-of-context hook (`SetWinEventHook`) filtering for `EVENT_SYSTEM_FOREGROUND` and `EVENT_OBJECT_LOCATIONCHANGE` with `OBJID_WINDOW` and `WINEVENT_SKIPOWNPROCESS`. Events are posted directly to the WindowControls surface (`WM_APP_WINDOW_SYNC`), eliminating continuous 25ms polling and GDI titlebar pixel sampling while idle. When the target window moves or foreground transitions occur, an adaptive 16ms animation timer is armed to drive smooth cubic follow motion and titlebar sampling until the overlay reaches rest and settles (`settle_ticks == 0`), upon which `KillTimer` disarms the loop to ensure 0% idle CPU usage.
- DEC-17: Apple Dynamic Island 3-Tier Notch Media Layout, Interactive Scrubber, and DirectWrite Trimming Invariants. To resolve text and control collision when titles or lyrics are long, the expanded media notch layout (`Metrics::MACBOOK_PRO_14.notch_expanded_height: 144`) is strictly divided into three vertically segregated tiers:
  1. Header Tier (y: 36..80): 44x44 squircle cover art, song title and artist/album, and live right-aligned audio equalizer glyphs. Title and artist text formats enforce DirectWrite `DWRITE_TRIMMING_GRANULARITY_CHARACTER` with `CreateEllipsisTrimmingSign` and `D2D1_DRAW_TEXT_OPTIONS_CLIP`, ensuring glyphs never bleed into neighboring widgets.
  2. Scrubber Tier (y: 90..104): Full-width interactive seekbar backed by Windows GSMTC `TimelineProperties` (`position_ms`, `duration_ms`). Left side displays elapsed time (`mm:ss`) and right side displays remaining time (`-mm:ss`). Click interactions hit `HitTarget::MediaSeekbar`, mapping relative horizontal position to milliseconds and emitting `MediaCommand::Seek(target_ms)` through `TryChangePlaybackPositionAsync`.
  3. Controls Tier (y: 110..140): Centered playback control triad (Previous, Play/Pause 34px button, Next), avoiding horizontal collision with text and maintaining visual balance.
- DEC-18: Direct2D Resource Palette and Notch Geometry Caching. To eliminate per-frame Direct2D COM object allocation churn (`CreateSolidColorBrush` and `CreatePathGeometry`), all renderers (`Renderer`, `DockRenderer`, `PopupMenuRenderer`, and `WindowControlsRenderer`) utilize pre-allocated persistent brushes (`BrushPalette`) with in-place `SetColor(&color)` mutation alongside an 8-slot scratch brush pool for ephemeral tinting. Top bar and panel notch bezier path geometries are cached in `CachedNotchGeometry`, matching on bitwise-exact dimensions (`Rect`, `corner_radius_bits`, `ear_radius_bits`) and returning cloned COM pointers (`ID2D1PathGeometry`) with zero-allocation atomic `AddRef` instead of rebuilding and tessellating bezier geometry sinks every frame.

## Modular system contract

The target is a modular monolith, not a service graph or a runtime DLL ecosystem. A feature is replaceable when its state, native effects, renderer, and lifecycle are behind one narrow entry point. A feature is not replaceable merely because it has its own Rust file.

### Responsibilities and state ownership

- R1 Host composition: `otto-os::host` owns descriptors, dependencies, activation, surface registration, configuration, and shutdown ordering.
- R2 Feature runtime: each feature owns its controller state and dispatches typed core events to domain effects. It may not create, destroy, or mutate a sibling feature.
- R3 Core policy: `otto-core` owns domain values, layout, hit testing, motion, menu models, and capability contracts. It never owns Win32 handles or render resources.
- R4 Platform adapters: `otto-platform` owns Win32, shell, hooks, COM workers, persistence adapters, and native resource conversion. It returns domain values or explicit failure/absence.
- R5 Render adapters: `otto-render` owns Direct2D, DirectWrite, GDI, layered-window presentation, and feature drawing. It consumes core values and never invokes platform actions.
- S1 Host lifecycle | owner `Host` | every required surface starts before dependants; every feature stops before its HWND is destroyed; concurrent writers: window thread only.
- S2 Feature state | owner of the feature runtime | derived layout can be recomputed from core inputs; no second mutable owner; concurrent writers: owning window thread.
- S3 Native hooks and workers | owner of the platform capability that created them, with shutdown called by `Host` | unregister/terminate before owner teardown; no callback may target a stopped surface.
- S4 Render resources | owner of the renderer instance | acquisition, use, and `Drop` remain paired; no feature creates a second device/resource lifetime without a contract.
- S5 Plugin configuration | owner `PluginConfig` plus the atomic file adapter | unknown optional entries are ignored, required entries are restored, and a failed write does not replace the last valid file. Durability target remains A1.

### Contracts and critical flows

- K1 `Host -> feature`: descriptor and dependency state enter `start`; result is `Running` or isolated `Failed`; `stop` is idempotent and releases hooks/timers before `Host` destroys the surface.
- K2 `feature -> core`: typed snapshots, hit targets, actions, and effects cross the boundary; raw HWNDs and Win32 structs do not.
- K3 `feature -> platform`: use a capability-scoped port or adapter such as `NotchPort` or `ControlCenterPort`; do not create a mega `PlatformPort`.
- K4 `feature -> render`: resolved `Layout` and feature snapshots enter a renderer; the renderer returns presentation failure without performing native policy.
- K5 `PluginConfig`: `version=1` and an allowlist are the compatibility contract. Future versions accept N-1 during migration, preserve required plugins, and ignore unknown optional names.
- Startup: config -> dependency validation -> required surfaces -> optional feature start -> running message loop. A required failure aborts startup; an optional failure disables only that feature.
- Runtime enable/disable: request -> dependency check -> feature start/stop -> state update -> atomic config write. On failure, the prior running/config state remains the valid outcome.
- Shutdown: stop event sources and feature controllers -> destroy feature HWNDs -> restore shared shell state -> stop platform workers. Stale native messages are ignored after feature state is gone.

### Guarantees and challenge

- G1 [H]: when an optional feature fails to start or stop, required Top Bar and Popup Menu behavior remains available and the feature is marked disabled; measured by app lifecycle tests and Windows smoke acceptance.
- G2 [H]: when a feature is stopped, hooks, timers, workers, renderer resources, and owned HWNDs are released in dependency order; measured by lifecycle tests plus Windows resource/hook acceptance.
- G3 [H]: when layout is recomputed, rendering and hit testing consume the same core `Layout`; measured by core geometry tests and app input/render integration checks.
- G4 [M]: when a platform call is absent or fails, the feature receives `Option`, a safe snapshot, or an explicit error according to its port contract; it never fabricates native success.

| Flow | Failure | Blast radius | Containment and recovery |
|---|---|---|---|
| Optional start | surface, renderer, or adapter init fails | one feature | mark `Failed`, destroy only its surface, retain required host path |
| Stop/shutdown | hook callback, timer, or worker outlives feature | feature and possibly process teardown | unregister/cancel first, then destroy HWND; host owns final worker shutdown |
| Config write | interruption during replacement | future activation only | keep previous valid file; load defaults and required plugins when unreadable |
| Capability call | Windows API or external app state unavailable | current interaction | return absence/default/error; native source of truth remains authoritative |

### Consequential decisions and debt

- DEC-12 [one-way]: keep lifecycle and dependency resolution in a host-owned registry; feature modules expose one lifecycle facade and never manage sibling surfaces. Driver G1/G2. Trade-off: `Host` remains a composition root until descriptors are extracted behind a registry contract. Revisit when independent feature release or third-party extension is required.
- DEC-13 [one-way]: add capability-scoped ports only where an external effect must be replaceable or testable; keep ports in `otto-core` and implementations in `otto-platform`. Driver K3. Trade-off: more explicit adapter code, but no cross-feature god interface.
- DEC-14 [two-way]: retain one render resource owner while extracting feature draw delegates incrementally from `render/src/lib.rs`. Driver G2. Trade-off: renderer delegates need explicit resource access; duplicating D2D/DIB ownership is forbidden.
- DEC-15 [two-way]: no new `thread_local!`, process singleton, or global mutable state. Migrate existing app registries toward a Host-owned runtime context as features are touched. Driver S1/S2. Exception: platform hooks/workers remain platform-owned and must have explicit shutdown.
- DEC-16 [two-way]: Control Center remains a virtual feature over Popup Menu and Top Bar until it needs an independent surface or release cadence. Driver K1. Trade-off: it is not independently renderable today, but it avoids a duplicate window and state owner.
- DEC-17 [two-way]: Apple Dynamic Island 3-tier media layout, seekbar scrubbing, and typography ellipsis trimming. Header tier (cover art squircle, title, artist, audio equalizer) + Tier 2 (progress seekbar with elapsed/duration timestamps) + Tier 3 (playback action buttons: prev, play/pause, next). Layout metrics resolved in `otto-core`; DirectWrite ellipsis trimming with fixed width boundaries prevents text overlap with controls.
- DEC-18 [two-way]: Real album art extraction via WinRT GSMTC stream and synced lyrics drawer. Platform reads `IRandomAccessStreamReference` on track transition without blocking tick; render decodes via WIC (`IWICImagingFactory`) into `ID2D1Bitmap` with single-slot memory caching. Lyrics are parsed from LRC format (`otto-core`), fetched from LRCLIB via WinRT `HttpClient` and cached locally in `%LOCALAPPDATA%/otto-os/lyrics_cache/`. Expanded notch supports a toggleable lyrics drawer (expanding to 280px) displaying a 3-line auto-scrolling synced window alongside a 1-line active karaoke lyric in the header tier.
- D4: static `PluginId` matches and per-feature thread-local registries | valid transitional path for current binary | pay down when adding the next feature by introducing a lifecycle facade/context.
- D5: broad `otto-platform` re-exports plus mixed `actions.rs` and `dock.rs` responsibilities | reduces initial wiring | split by capability before adding more native integrations.
- D6: `otto-render/src/lib.rs` is the shared resource owner and a large drawing hotspot | protects resource cleanup | extract draw delegates without changing the device/presentation owner.
- D7: no automated architecture fitness check exists yet | repository is small and Cargo catches crate-level direction | add boundary checks before external/static plugin work.

## Fitness

- F1: `otto-core` has no dependency on platform/render/app; signal `Cargo.toml` plus `cargo check -p otto-core`; pass when the dependency direction remains one-way; mode enforce through Cargo.
- F2: every feature has one public lifecycle entry and sibling access goes through Host or a typed port; signal review plus focused app tests; pass when no new direct sibling/global path is added; mode warn until a boundary test exists, exception D4.
- F3: no new app `thread_local!` or mutable singleton; signal `rg`/code review; pass when new feature state is Host-owned or explicitly scoped to its runtime; mode warn during D4 migration.
- F4: platform capability exports stay narrow and render has no platform action imports; signal `cargo metadata` plus import review; pass when a capability can be replaced without editing unrelated features; mode warn, pay down through D5/D6.

## Assumptions and risks

- A1 [blocking for dynamic plugins or independent deployment]: no `.agents/product.md`, runtime hot-reload requirement, third-party plugin ABI, availability target, or performance SLO is defined. This contract therefore assumes one process, statically linked Rust features, one UI thread, and no independent deployment. Decide these requirements before choosing DLLs or services.
- A2: Windows-native capture, DWM timing, hooks, and visual continuity require manual acceptance on Windows; compile and unit tests cannot prove them.
- Risks / trade-offs: host composition is intentionally centralized; current platform/render files have high change fan-out; static linking gives reliable resource ownership but limits external runtime extensibility.

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
