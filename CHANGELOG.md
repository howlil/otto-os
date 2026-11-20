# Changelog

All notable changes to Otto OS are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/howlil/otto-os/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/howlil/otto-os/releases/tag/v1.0.0
