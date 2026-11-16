# Otto OS Product Contract

## 1. Product Vision and Target Platform

Otto OS transforms the Windows 11 desktop experience into an authentic, highly responsive macOS Golden Gate environment, complemented by the hardware Dynamic Island interaction model found on the MacBook Pro PROLED XDR display.

- Target Operating System: Windows 11 22H2+ (build 22621+, x86_64-pc-windows-msvc; Windows 10 unsupported and blocked at startup)
- Core Language & Framework: Rust 2024 edition, Win32 APIs, Direct2D, DirectWrite, GDI
- Architecture: Single-process modular monolith across four crates (`otto-os`, `otto-platform`, `otto-render`, `otto-core`)

---

## 2. Boundary Classification

### 2.1 macOS Golden Gate General Shell (Core Parity)
- **Top Bar**: Edge-to-edge menu bar reserving monitor work area, displaying Apple logo (``), bold foreground application name, and contextual application menus (`File`, `Edit`, `View`, `Window`, `Help`). The right-hand cluster follows `Tray Overflow → Wi-Fi → Audio Volume → Battery → Search → Control Center → Notifications → Date/Time`, with one independent hit target per action and overflow preserving every hidden target.
- **Dock**: Floating bottom dock with uniform macOS squircles, cosine fish-eye magnification, running/active indicators, Downloads stack, and dynamic Recycle Bin. Clicking active app asserts focus and brings windows to front (no Windows minimize-toggle).
- **Window Controls**: Traffic lights covering the top-right native caption strip (`[Yellow (Genie Minimize), Green (Zoom/Maximize), Red (Close)]`; top-left placement is retired so native Windows caption buttons are never duplicated; Alt+Click on Green toggles Fullscreen; right-click opens Tiling menu; elevated/non-caption windows keep native controls).
- **Control Center**: Frosted-glass modular popover with interactive Sound and Display sliders, now playing block, and quick settings toggles/shortcuts.
- **Calendar & Notification Center**: Date/time opens the monthly calendar grid and date settings. The adjacent bell independently opens grouped Windows notification summaries with per-item dismiss, Clear All, permission recovery, and a Windows Notifications shortcut. Opening any top-bar popup closes the competing Otto popup.

### 2.2 MacBook Pro PROLED XDR Dynamic Island (Hardware Parity)
- **Hardware Notch**: Obsidian Black (`#000000`) center pill, visually continuous with the display bezel across all themes.
- **Interaction Semantics**: Click/tap to expand into dynamic card; swipe up or click outside to collapse.
- **Activity State Machine**:
  - Collapsed: 200pt idle notch.
  - Compact Single Activity: 292pt with the cover thumbnail, title (or the synced lyric line after the online-lyrics opt-in), and equalizer; or the Pomodoro ring and time. A paused track stays 8 seconds with a dimmed cover.
  - Compact Dual Live Activity: When both Focus (`Pomodoro` / `QuickTimer` / `Stopwatch`) and Media (`NowPlaying` active or within the 8-second pause hold) are active, Focus occupies the Main Notch by default and Media detaches into a 28x28pt Obsidian Black Secondary Circle (`HitTarget::NotchSecondary`) 8pt to the right of the Main Notch. Left-clicking the Secondary Circle swaps the primary and secondary modules (`Main Notch <-> Secondary Circle`) and syncs the Expanded Panel tab to the newly promoted primary module; right-clicking dismisses the clicked slot independently.
  - Transient HUD: 356pt pill for Volume, Brightness, Charging, Low battery, and Caps Lock events.
  - Expanded Island: 380x144pt card with Focus / Music segmented tabs (280pt while the lyrics drawer is open).

### 2.3 Otto OS Product Extensions
- **Pomodoro Focus Timer**: Four-session focus, short break and long break cycles; 15, 25, 45 and 60 minute focus presets with wheel fine-tune; start and end times; short event peeks in the compact notch; a last-minute warning and a 5-minute heads-up; a session-complete card with +5 min and Start break; chime notifications.
- **Synced Lyrics Drawer**: Expanding notch drawer (280px) in a bordered card, displaying synced 3-line auto-scrolling lyrics with an active-line pill and karaoke highlight.
- **Music Controls**: Cover, title, artist and source in the Music tab with previous, play/pause and next, a seek bar, a lyrics toggle, colours that follow the cover, peeks when the track changes, plays, pauses or is skipped, and a LIVE state for sources without a length. Unsupported controls name the reason.
- **Audio Output Switcher & Privacy Indicators**: Expanded `Audio` tab enumerating active render endpoints with peak metering and 3-role default device switching (`PolicyConfig`), plus live mic/camera consent dots beside the notch and an expanded header mic-mute toggle pill.
- **Pocket (File Drop Shelf) & Clipboard (`Clip`)**: Ephemeral 12-item drag-and-drop holding shelf for files, URLs, and text (`Pocket`), and RAM-only 12-item clipboard history (`Clip`) excluding sensitive/opt-out clipboard formats.

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
| **Window Controls**| Traffic lights (macOS Top-Left; Windows Top-Right cover `[Yellow, Green, Red]`)| `OttoWindowControls` Direct2D overlay | `EVENT_OBJECT_LOCATIONCHANGE` | Native caption buttons | `VER_1.4`, `VER_3.3` |
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

---

## 5. Public Release Contract (Target Scope)

### 5.1 Distribution & Release Channels
- **v1.0 Public Beta**: GitHub Releases (per-user `.exe` installer without UAC elevation) + WinGet package submission via automated GitHub Actions release pipeline + code signing (SignPath / Azure Trusted Signing).
- **v1.1 Microsoft Store**: Signed MSIX package after completing Store prerequisites (Store-compliant branding, asset icons, and accessibility/UIA requirements).
- **Target OS Enforcement**: Windows 11 22H2+ (build 22621+); startup detects unsupported Windows 10 builds and shows a clear diagnostic dialog before mutating shell state.

### 5.2 Installer, Startup, Updates & Shell Behavior
- **Lifecycle CLI & Auto-Start**: Support `--quit` to cleanly close an active instance and restore shell state prior to install/uninstall; offer a per-user `Launch at Login` menu toggle (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
- **Dock ↔ Taskbar Coupling**: Keep Top Bar and Dock on the primary monitor only (leaving Windows `Shell_SecondaryTrayWnd` visible on secondary monitors). Disabling the Dock module automatically restores the primary Windows taskbar (`Shell_TrayWnd`); enabling the Dock hides it.
- **Capture Visibility Toggle**: Control Center exposes a live `Visible in Screenshots / Screen Share` toggle switching between Liquid Glass (`WDA_EXCLUDEFROMCAPTURE`) and DWM Translucent (`include_in_capture`) without restart.
- **In-Menu Settings & First-Run Onboarding**: Expose checkable settings rows directly inside the Apple/Otto menu and Control Center (Dock, Window Controls, Genie, Launch at Login, Capture Visibility, Hotkeys) without a standalone Settings window; persist all preferences including `theme_mode`; auto-populate default Dock pins detected on first run (Terminal, default Browser, Settings, Notepad) and show a one-time welcome hint.
- **Conflict-Safe Shortcuts**: Replace default `Alt+N` and `Alt+P` global hotkeys with a conflict-safe 3-key chord (`Ctrl+Alt+N` & `Ctrl+Alt+P` or `Win+Alt+N` & `Win+Alt+P`) and provide an option to disable global hotkeys.
- **Hardware Brightness**: Support DDC/CI (`Dxva2`) for external/desktop monitors alongside internal `\\.\LCD` displays, and disable or hide the Brightness slider gracefully when no controllable display interface is present.
- **Branding**: Default to original Otto OS vector/app branding (Store-safe), with an optional `Classic Apple Icon` mode reserved for non-Store GitHub builds.
- **Manual Updates**: Passive `Check for Updates...` menu item querying the GitHub Releases API on explicit user click only (no background auto-download).

### 5.3 Diagnostics & Privacy Policy
- **Zero Telemetry**: No analytics, telemetry uploads, or remote crash reporting.
- **Local Bounded Diagnostics**: Rolling log file under `%LOCALAPPDATA%\otto-os\logs\` (strictly excluding clipboard contents or user secrets), `MessageBoxW` notifications for fatal startup/panic failures, and an `Open Logs Folder` menu action.

### 5.4 Release Cutoff Policy
- **Blocking for v1.0 Public Beta**: Close the idle CPU defect toward 0%, add a root `LICENSE` file matching MIT package metadata, align public landing/README copy (remove unimplemented `.ics` / `"jadwal"` claims), and deliver all v1.0 packaging/UX/diagnostics gaps listed below.
- **Deferred to v1.1 (Pre-Store) / Future**: `M04` UI Automation provider (`WM_GETOBJECT`) and full Dock/Notch panel keyboard/Tab completion, `M13` MSIX Store qualification, and `.ics` calendar integration (pending a dedicated `.agents/product.md` network/privacy spec).

---

## 6. Public-Readiness Gap Inventory (Verified Audit)

The following gaps were verified against the repository state and must be closed before their respective target releases:

1. **Packaging, Installer & CI/CD (v1.0 Blocker)**:
   - No per-user `.exe` installer script, no WinGet manifest, and no `.github/workflows/` CI/Release automation or code-signing pipeline.
   - Missing `--quit` CLI handshake in `crates/app/src/main.rs` and no `HKCU\...\Run` Launch-at-Login toggle.
   - `packaging/msix/Assets/` currently contains `0` required PNG assets (`StoreLogo.png`, `Square150x150Logo.png`, `Square44x44Logo.png`), and `AppxManifest.xml.template` still lists `MinVersion="10.0.17763.0"` instead of `10.0.22621.0` (v1.0/v1.1 sync).
2. **Executable Metadata, Branding & Legal (v1.0 Blocker)**:
   - No `LICENSE` file in repository root despite MIT declarations in `Cargo.toml` and `README.md`.
   - `crates/app` lacks `build.rs` / `winresource` for embedded `.exe` icon and Windows version/product metadata.
   - `crates/render/src/lib.rs` (`draw_apple_logo`) defaults to a literal Apple bezier glyph; no default Otto OS brand mark yet.
   - `crates/platform/src/lyrics.rs` hardcodes `"OttoOS/0.1.0 (https://github.com/otto-os)"` instead of crate version metadata.
   - `landing/src/pages/index.astro` advertises `"jadwal"` (schedule), which is not implemented.
3. **Configuration, Menus & Onboarding (v1.0 Blocker)**:
   - `theme_mode` is kept in `RuntimeContext` memory (`crates/app/src/host.rs`, `theme_state.rs`) and not persisted to `plugins.txt`.
   - Top Bar menu and Control Center (`crates/core/src/menu.rs`) lack module toggles (`Dock`, `WindowControls`, `Genie`), `Launch at Login`, `Visible in Screenshots / Screen Share`, `Open Logs Folder`, `Check for Updates...`, and hotkey settings.
   - `crates/app/src/bar.rs` unconditionally calls `hide_system_taskbar()` at startup without automatically restoring `Shell_TrayWnd` when the Dock module is disabled.
   - `crates/platform/src/dock.rs` (`pinned_from_read`) initializes first-run `pinned.txt` as empty (`Vec::new()`) with no default apps or welcome hint, and unconditionally renames `"explorer"` / `"notepad"` to `"Finder"` / `"Notes"`.
4. **Windows Compatibility, Hardware, Shortcuts & Performance (v1.0 Blocker)**:
   - No startup Windows 11 build check (`>= 22621`), despite depending on Win 11 APIs (`DWMWA_SYSTEMBACKDROP_TYPE = 38`, `DWMWA_WINDOW_CORNER_PREFERENCE`, `Segoe UI Variable Display`).
   - `crates/platform/src/status.rs` only drives internal laptop `\\.\LCD` brightness and silently stores `FALLBACK_BRIGHTNESS = 80` on desktops/external monitors (lacks `Win32_Devices_Display` DDC/CI and UI capability feedback).
   - `crates/platform/src/actions.rs` binds global hotkeys `Alt+N` and `Alt+P` with no conflict-safe default or disable toggle.
   - Idle CPU defect remains open in `.agents/engineering.md` (`~0.63–1.67%` single-core vs `0%` target).
5. **Diagnostics & Fatal Error Visibility (v1.0 Blocker)**:
   - `crates/app/src/main.rs` uses `#![windows_subsystem = "windows"]` with `eprintln!` and no rolling file log in `%LOCALAPPDATA%\otto-os\logs\` and no `MessageBoxW` crash/startup failure dialog.
6. **Deferred to v1.1 (Microsoft Store Readiness)**:
   - `M04` full UI Automation provider (`WM_GETOBJECT`) and complete accessibility/focus verification across all surfaces.
   - `M13` MSIX Store qualification, populated Store logo assets, and clean Store branding without Apple-trademark iconography.
