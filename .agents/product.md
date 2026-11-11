# Otto OS Product Contract

## 1. Product Vision and Target Platform

Otto OS transforms the Windows 11 desktop experience into an authentic, highly responsive macOS Golden Gate environment, complemented by the hardware Dynamic Island interaction model found on the MacBook Pro PROLED XDR display.

- Target Operating System: Windows 11 24H2+ (x86_64-pc-windows-msvc)
- Core Language & Framework: Rust 2024 edition, Win32 APIs, Direct2D, DirectWrite, GDI
- Architecture: Single-process modular monolith across four crates (`otto-os`, `otto-platform`, `otto-render`, `otto-core`)

---

## 2. Boundary Classification

### 2.1 macOS Golden Gate General Shell (Core Parity)
- **Top Bar**: Edge-to-edge menu bar reserving monitor work area, displaying Apple logo (``), bold foreground application name, and contextual application menus (`File`, `Edit`, `View`, `Window`, `Help`). Right-hand system status area displays Tray Overflow, Wi-Fi, Audio Volume, Battery percentage, Control Center, and Clock.
- **Dock**: Floating bottom dock with uniform macOS squircles, cosine fish-eye magnification, running/active indicators, Downloads stack, and dynamic Recycle Bin. Clicking active app asserts focus and brings windows to front (no Windows minimize-toggle).
- **Window Controls**: Traffic lights anchored to top-left of active window: Red (Close), Yellow (Genie Minimize), Green (Zoom/Maximize; Alt+Click for F11 Fullscreen; right-click for Tiling menu).
- **Control Center**: Frosted-glass modular popover with interactive Sound and Display sliders, now playing block, and quick settings toggles/shortcuts.
- **Notification Center**: Triggered by clicking the Menu Bar date/time, hosting a monthly calendar grid and Windows notification summary.

### 2.2 MacBook Pro PROLED XDR Dynamic Island (Hardware Parity)
- **Hardware Notch**: Obsidian Black (`#000000`) center pill, visually continuous with the display bezel across all themes.
- **Interaction Semantics**: Click/tap to expand into dynamic card; swipe up or click outside to collapse.
- **Activity State Machine**:
  - Collapsed: 200pt idle notch.
  - Compact Single Activity: 292pt with now playing icon + title or Pomodoro badge.
  - Compact Dual Live Activity: Pomodoro countdown on left pill, animated 4-bar equalizer waveform on right pill.
  - Transient HUD: 356pt pill for Volume, Brightness, Charging, and Caps Lock events.
  - Expanded Island: 380x116pt card with Focus / Now Playing segmented tabs.

### 2.3 Otto OS Product Extensions
- **Pomodoro Focus Timer**: Customizable work/break cycles, badge states, alert hold, chime notifications, and wheel adjustment.
- **Synced Lyrics Drawer**: Expanding notch drawer (280px) displaying synced 3-line auto-scrolling lyrics with active karaoke highlight.

### 2.4 Infeasible / Unsupported Integrations (Explicit Non-Goals)
- Apple ID authentication, iCloud Keychain, and CloudKit synchronization.
- AirDrop, AirPlay, and continuity camera.
- Native macOS sandbox emulation or Mach IPC.
- macOS system preferences daemon replacement.

---

## 3. Parity Matrix

| Feature | macOS Reference | Windows Implementation | Prerequisite | Fallback | Verification Gate |
|---|---|---|---|---|---|
| **Top Bar** | macOS Menubar | Layered `OttoNotchBar` via `SPI_SETWORKAREA` | Primary monitor rect | Standard Windows taskbar | `VER_1.5`, `VER_2.1` |
| **Dynamic Island** | MacBook Pro PROLED XDR | `OttoNotchPanel` Direct2D Bezier Path | Win32 message hook | Compact pill | `VER_2.5`, `VER_5.1` |
| **Window Controls**| Traffic lights (Top-Left) | `OttoWindowControls` Direct2D overlay | `EVENT_OBJECT_LOCATIONCHANGE` | Native caption buttons | `VER_1.4`, `VER_3.3` |
| **Genie Effect** | macOS Window Minimize | Cubic Hermite mesh suction via Direct2D | HWND screen capture | Win32 native minimize | `VER_1.2`, `VER_3.2` |
| **Dock** | macOS Dock | Layered `OttoDockBar` with squircle icons | Shell icon extraction | Explorer taskbar | `VER_2.2`, `VER_3.2` |
| **Control Center** | Control Center | `OttoPopupMenu` with live WASAPI / GSMTC | Windows Core Audio / WinRT | Windows Action Center | `VER_2.5`, `VER_4.1` |
| **Lyrics** | Apple Music Lyrics | LRCLIB WinRT HttpClient + DirectWrite | Explicit user opt-in | Track metadata only | `VER_1.1`, `VER_4.2` |

---

## 4. Quality and Resource Invariants

- Idle CPU usage: 0% when no animations or transitions are active.
- Private working set: $\le 60\text{ MB}$ at idle.
- Graphics handles: Zero leaked HDC, HBITMAP, HICON, or Direct2D COM pointers over 10,000 cycles.
- Shell safety: Work area and taskbar must restore completely on graceful shutdown, crash recovery, or `--restore-shell`.
