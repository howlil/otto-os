# Changelog

All notable changes to Otto OS are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/howlil/otto-os/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/howlil/otto-os/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/howlil/otto-os/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/howlil/otto-os/releases/tag/v1.0.0
