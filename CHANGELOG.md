# Changelog

All notable changes to Otto OS are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.1] - 2026-11-23

### Changed

- **Unified Control Center layout and typography**: aligned `Focus` and `Battery` tiles onto the same horizontal `32 pt` circular badge and vertically centered two-line layout as `Wi-Fi` and `Bluetooth`; unified `Display` and `Sound` into matching full-width `328 × 26 pt` capsule sliders with the `Sound` mute toggle embedded in the leading cap; standardized `12 pt` card padding, `68 pt` tile/slider heights, `72 pt` Media card framing with a `32 pt` Play/Pause disc, concentric `10 pt`/`14 pt` hover pills and subpanel footer buttons, and `13 pt SemiBold` card titles paired with `11 pt Medium` tabular (`tnum`) readouts.

### Fixed

- **Prevented Dock occlusion during hover magnification motion**: `OttoDockBar` now handles AppBar notifications (`ABN_POSCHANGED`, `ABN_FULLSCREENAPP`), re-registers on `TaskbarCreated`, and re-asserts `HWND_TOPMOST` on hover/motion entry, unhide, app activation, reservation updates, and window geometry changes (while yielding to active Otto overlays), keeping magnified Dock icons and tooltips above maximized windows.
- **Fixed Top Bar Search and Dock Apps Spotlight toggle flow**: clicking the Top Bar Search icon or Dock Apps icon while Spotlight is already open in that mode now closes Spotlight cleanly instead of reopening it; switching between `Search` and `Apps` works in one click; a `250 ms` focus-loss grace window absorbs `WM_KILLFOCUS` pointer-trigger races; and opening Tray or Settings from the Top Bar dismisses Spotlight cleanly.

## [1.3.0] - 2026-11-23

### Added

- **Dedicated Notch Screen Time (`Time`) tab**: view Today's total screen time, Focus share ring, Top 3 Most Used apps with real application icons, and an interactive 7-day stacked screen/focus chart directly inside the expanded Notch, with a quick `Details ›` shortcut to Focus Activity in Otto Settings.
- **Switchable Appearance styles, themes, and accent colors**: choose between `Liquid Glass`, `Frosted`, `Tinted`, and `Solid` surface styles, `Auto | Light | Dark` theme mode, and 7 WCAG-calibrated accent colors (including Windows System accent) with instant live previews in `Otto Settings → Appearance`.
- **In-app Software Updates**: check for new releases and download/install verified SHA-256 patches directly from `Otto Settings → About` with silent relaunch (`/RELAUNCH`) and zero background polling.
- **Process & Memory Buffer Audit (`Otto Settings → General`)**: inspect running applications with 3-tier safety classification (`SafeBackground`, `ActiveWindow`, `SystemProtected`), toggle manual `EcoQoS` throttling, and safely end headless background memory hogs.
- **Redesigned Focus Settings**: split Focus into dedicated `Block apps` and `Activity` tabs with live search, `[Blocked | Recent | All apps]` filters, real app icons, instant toggle switches, and a full 7-day interactive usage breakdown.

### Changed

- **Compact Notification Center redesign**: slimmer `328 pt` shell with concentric radii, adaptive card heights (`50 / 64 / 76 pt`) that remove blank whitespace on short alerts, real Win32 and MSIX/UWP application icons (WinRT `GetLogo` with Shell/catalog fallback), inline `+N` stack badges, tabular timestamps, and quiet borderless-at-rest header/footer actions.
- **Pure-app Focus blocklist catalog**: filtered out Windows system services, `System32` admin utilities, folders, documentation/help shortcuts, and updaters, while deduplicating MSIX/Win32 entries and scrubbing non-domain filenames from website activity.
- **Otto Settings native Mica backdrop**: normalized the Settings window to Windows 11 Mica with uniform neutral cards and hairline dividers.

### Fixed

- Eliminated translucent stack-card bleed-through on collapsed Notification Center stacks by clipping recessed stack lips strictly beneath the front card edge.
- Improved cold-start reliability of Genie restore initialization under heavy GPU load.

## [1.2.0] - 2026-11-22

### Added

- **Focus overlay with Back**: a blocked app or browser window is covered by a card with a **Back** button, and Esc does the same. Back sends a browser back one page, or activates the window below for an app. Nothing is closed.
- **More Focus presets**: Epic Games Launcher, Riot Client, and Roblox join the blocked apps. Threads, Snapchat, Pinterest, Tumblr, Bluesky, 9GAG, Discord web, Roblox web, Poki, CrazyGames, Miniclip, and Y8 join the blocked websites.
- **7-day screen time chart** on the Focus settings page. Each day shows total active time, with its Focus share in a darker tone.

### Changed

- Blocked apps and websites are covered with the Back overlay instead of being auto-minimized.
- The Focus settings page no longer repeats the same stats. Today's totals sit in the header, usage lists show Blocked or Allowed status only, and blocks are edited only in the Blocked Apps and Blocked Websites sections.

## [1.1.0] - 2026-11-22

### Added

- Standalone **Otto Settings** window (`OttoSettings`) consolidating module toggles, configuration pages, and update checks in a dedicated native Direct2D window.
- **Focus Mode, Screen Time & Distraction Blocking**: local Today and 7-day rolling app, website, and search query analytics with zero-extension browser address-bar inspection, configurable app/website blocklist with auto-minimize and Notch HUD alert while Focus is active, notification and Dock badge suppression, and a dedicated **Focus** dashboard page in Otto Settings.
- **Inline Quick Note Add & Edit in Notch**: `Ctrl+Alt+Q`, `+ Add`, or clicking the empty `Tasks` card now opens an inline composer directly inside the Notch `Tasks` tab instead of a separate floating popup, with in-place editing of existing notes via the per-row pencil button or right-click.

### Changed

- Isolated debug/local development data to `%LOCALAPPDATA%\otto-os-dev` while production/installed builds keep `%LOCALAPPDATA%\otto-os`, and preserved the installed `%LOCALAPPDATA%\Programs\Otto OS\otto-os.exe` path when toggling Launch at Login from Cargo target directories.
- Reduced idle memory and GPU footprint via lazy Notch panel surface allocation, idle background worker teardown, and backdrop capture cache release.
- Updated release packaging manifests and installer metadata for `v1.1.0` (`otto-os-setup-1.1.0-x64.exe`).

### Fixed

- Eliminated Top Bar and glass surface flickering/disappearing during screen recording and screen sharing (Cap, OBS, Discord, Windows Graphics Capture) when **Visible in Screenshots / Screen Share** is enabled, while preserving Clear Liquid Glass appearance without self-capture feedback loops.

## [1.0.0] - 2026-11-21

### Added

- Independent **Top Bar** and **Notch** modules: disable the Top Bar while keeping the Notch, Control Center, Otto Settings, Spotlight, and Dock taskbar hiding active, or disable the Notch for a full-width Top Bar.
- Always-available **Otto Settings** access when the Top Bar is off: Windows System Tray icon (left-click toggles Otto Settings, right-click opens the Otto quick menu), global `Ctrl+Alt+,` shortcut, Dock right-click menu entry, and idle Notch right-click quick menu.
- Six-area **Otto Settings** overview inside the Bento **Control Center** (`Ctrl+Alt+C`) with live procedural previews, Wi-Fi/Bluetooth/Audio output controls, DDC/CI external monitor brightness, Launch at Login, capture visibility toggle, and manual GitHub Release update checks.
- **Dynamic Island** and **Classic Notch** styles with Split Compact dual-activity swap, Focus (Pomodoro, Quick Timer, Stopwatch), Music playback with synced lyrics drawer, Pocket file drop shelf, ephemeral Clipboard History (`Clip`), opt-in Quick Capture & Tasks (`Ctrl+Alt+Q`), Privacy indicators (Camera, Microphone, Location), Call Activity, and System HUD (Volume, Brightness, Battery, Caps Lock, Bluetooth).
- Local **Spotlight** application search and full Apps catalog (`Ctrl+Alt+Space`) with keyboard navigation and reduced-motion-safe morphing.
- Floating **Dock** with squircle icons, hover magnification, overflow handling, visual Downloads grid stack, badges and download progress indicators, Recycle Bin status, and Always Visible / Auto-Hide / Reserve Screen Space modes.
- **Window Controls** covering native caption buttons with Windows 11 rounded corners and hardware-accelerated DirectComposition **Genie** minimize and restore animations.
- Single low-power GPU **Clear Liquid Glass** composition with opaque and Windows High Contrast accessibility fallbacks, UI Automation (`WM_GETOBJECT`) providers, full Bar/Dock/Control Center keyboard traversal, and opt-in **Background Cleanup** throttling.
- Production Windows 11 22H2+ (build 22621+) packaging: `--quit` and `--restore-shell` lifecycle CLI, crash-safe shell recovery (`shell_recovery.json`), rolling zero-telemetry local logs, per-user Inno Setup installer (`otto-os-setup-1.0.0-x64.exe`), portable ZIP archive, SHA-256 checksums, and WinGet manifests.

[Unreleased]: https://github.com/howlil/otto-os/compare/v1.3.1...HEAD
[1.3.1]: https://github.com/howlil/otto-os/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/howlil/otto-os/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/howlil/otto-os/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/howlil/otto-os/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/howlil/otto-os/releases/tag/v1.0.0
