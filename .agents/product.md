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
- **Menu Bar Settings**: Otto Settings offers Auto-Hide Menu Bar (Full Screen default, Desktop, Always, Never; the top edge reveals it), per-item visibility for Tray, Status, Search, Notifications and Clock, a Menu Bar Background toggle, and Notch Stays Visible (keeps the notch, HUD and activity while the bar hides). Privacy dots add a blue location arrow below camera and microphone.
- **Spotlight**: Search opens a focused local-app search surface; `Ctrl+Alt+Space` toggles it and Dock Apps opens its full app catalog. Search ranks installed app names as the query changes; arrows move selection, Return launches, Escape closes, and Tab switches list/grid. The surface can move and resize, morphs from its trigger, and settles immediately with reduced motion. Search covers `shell:AppsFolder` apps only, not files or web results.
- **Status Cluster Actions**: Wi-Fi opens Wi-Fi controls, Volume opens Sound controls, and Battery opens Windows Power & Battery settings. Only Volume accepts wheel adjustment; if Control Center is disabled, Wi-Fi and Volume open their matching Windows Settings pages.
- **Dock**: Floating bottom dock with uniform macOS squircles, cosine fish-eye magnification, running/active indicators, Downloads stack, and dynamic Recycle Bin. Dock Behavior has three exclusive modes: Always visible (default; floats over windows), Auto-Hide Dock (hides while any window covers it and shows again when nothing does; the bottom edge reveals it), and Reserve Screen Space (taskbar-like: maximized windows stop above the Dock; needs Hide Windows Taskbar). Every mode hides the Dock for a truly fullscreen app. Clicking active app asserts focus and brings windows to front (no Windows minimize-toggle).
- **Window Controls**: Traffic lights covering the top-right native caption strip (`[Yellow (Genie Minimize), Green (Zoom/Maximize), Red (Close)]`; top-left placement is retired so native Windows caption buttons are never duplicated; Alt+Click on Green toggles Fullscreen, Alt+Click on Yellow minimizes and Alt+Click on Red closes every primary window of the same app that supports the command (hidden windows are skipped; close stays a polite `WM_CLOSE`, so apps can still ask to save); right-click opens Tiling menu; elevated/non-caption windows keep native controls). Windows that get Otto controls are also forced to Windows 11 rounded corners when the app has not chosen its own corner preference, and are reset to the system default when Window Controls is turned off or Otto quits (radius stays the system ~8 px).
- **Control Center**: Frosted-glass modular popover with interactive Sound and Display sliders, now playing block, and quick settings toggles/shortcuts.
- **Calendar & Notification Center**: Date/time opens a monthly calendar modelled on the Apple calendar: a `Month Year` header with previous, today and next controls, a single-letter weekday row, a fixed six-week grid with neighbouring-month days dimmed, a red filled disc on today, and a quiet Date & Time Settings row (Left/Right arrows also change month). The adjacent bell independently opens grouped Windows notification summaries with per-item dismiss, Clear All, permission recovery, and a Windows Notifications shortcut. Opening any top-bar popup closes the competing Otto popup.

### 2.2 MacBook Pro PROLED XDR Dynamic Island (Hardware Parity)
- **Hardware Notch**: Obsidian Black (`#000000`) center pill, visually continuous with the display bezel across all themes.
- **Interaction Semantics**: Click/tap to expand into dynamic card; swipe up or click outside to collapse.
- **Activity State Machine**:
  - Collapsed: Classic Notch 200 x 32 pt; Dynamic Island 120 x 32 pt.
  - Compact Single Activity: Classic Notch Focus stays 200 x 32 pt; Dynamic Island Focus grows from 120 x 32 pt idle to 184 x 36 pt active before optional privacy-dot reserve; Pomodoro adds four centered cycle steps that transient event peeks replace. Music keeps its Dynamic Island 292 pt base and 36 pt active height, widening only when its title or synced lyric (after online-lyrics opt-in) needs room, up to 520 pt of content plus the compact privacy-dot reserve when visible; the cover, title or lyric, and equalizer stay together. A paused track stays 8 seconds with a dimmed cover.
  - Compact Dual Live Activity: When both Focus (`Pomodoro` / `QuickTimer` / `Stopwatch`) and Media (`NowPlaying` active or within the 8-second pause hold) are active, Focus occupies the Main Notch by default and Media detaches into a 36x36pt Obsidian Black Secondary Circle on Dynamic Island or 28x28pt on Classic Notch (`HitTarget::NotchSecondary`) 8pt to the right of the Main Notch. Left-clicking the Secondary Circle swaps the primary and secondary modules (`Main Notch <-> Secondary Circle`) and syncs the Expanded Panel tab to the newly promoted primary module; right-clicking dismisses the clicked slot independently.
  - Transient HUD: 356pt pill for Volume, Brightness, Charging, Low battery, and Caps Lock events.
  - Expanded Island: 380x144pt card with Focus / Music segmented tabs (280pt while the lyrics drawer is open).

### 2.3 Otto OS Product Extensions
- **Pomodoro Focus Timer**: Four-session focus, short break and long break cycles; 15, 25, 45 and 60 minute focus presets with wheel fine-tune; start and end times; short event peeks in the compact notch; a last-minute warning and a 5-minute heads-up; a session-complete card with +5 min and Start break; chime notifications.
- **Synced Lyrics Drawer**: Expanding notch drawer (280px) in a bordered card, displaying synced 3-line auto-scrolling lyrics with an active-line pill and karaoke highlight.
- **Music Controls**: Cover, title, artist and source in the Music tab with previous, play/pause and next, a seek bar, a lyrics toggle, colours that follow the cover, peeks when the track changes, plays, pauses or is skipped, and a LIVE state for sources without a length. Unsupported controls name the reason.
- **Notification Events**: a Windows toast arriving while the notch is idle shows as a Peek then a Banner (text) or a Ringing card with Open and Dismiss (call or video call); calls preempt messages, bursts coalesce, and an in-progress call shows a timer and waveform with mute. Messages never close an open panel, Focus and Do Not Disturb suppress banners, and a preview-off setting hides text. Needs notification access, which Windows grants only to a packaged app, so the feature stays dormant until MSIX (M13); incoming calls are recognised by a call-app allowlist plus phrases, and Open and Dismiss act on the window and the toast, not on the call itself.
- **Audio Output Switcher & Privacy Indicators**: Expanded `Audio` tab enumerating active render endpoints with peak metering and 3-role default device switching (`PolicyConfig`), plus one live mic/camera indicator beside the notch (camera takes priority when both are active), a click-through privacy detail view, and a separate expanded header mic-mute control.
- **Quick Capture & Tasks**: Opt-in (`Quick Capture` in Otto Settings, default off). `Ctrl+Alt+Q` or `+ Add` on the `Tasks` tab opens a one-line input under the notch: Enter saves and closes, Shift+Enter saves and keeps typing, Esc cancels, and focus returns to the previous app. The `Tasks` tab lists up to 50 tasks (4 per page, newest first): the circle marks done, clicking the text makes it the one active task, which replaces the subtitle on the Focus tab, `x` removes, and `Clear` removes done tasks. Tasks persist as plain text in `%LOCALAPPDATA%\otto-os\tasks.txt`; nothing is synced or sent anywhere, and a full list drops the oldest done task before refusing a new one.
- **Pocket (File Drop Shelf) & Clipboard (`Clip`)**: Ephemeral 12-item drag-and-drop holding shelf for files, URLs, and text (`Pocket`), and RAM-only 12-item clipboard history (`Clip`) excluding sensitive/opt-out clipboard formats.
- **Background Cleanup**: Opt-in `Background Cleanup` setting in Otto Settings (default off). While on, apps that are not in the foreground and keep using at least 20% of one CPU core for about 40 seconds are slowed down (Windows Efficiency Mode plus lower priority) so the app you are using stays responsive, and they return to normal when you switch to them or turn the setting off. Otto never closes processes, never touches Windows services or elevated apps, and does not clear RAM.

### 2.4 Infeasible / Unsupported Integrations (Explicit Non-Goals)
- Apple ID authentication, iCloud Keychain, and CloudKit synchronization.
- Purple system-audio-recording privacy indicator (no reliable Windows signal).
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
| **Quick Capture** | Reminders quick entry | `OttoQuickCapture` popup with native `EDIT` plus the `Tasks` notch tab | Explicit user opt-in | Module off: no hotkey, no tab, no file access | `VER_2.5` |
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
- **Capture Visibility Toggle**: Control Center exposes a live `Visible in Screenshots / Screen Share` toggle that controls whether Otto surfaces appear in screenshots and screen share while preserving the Liquid Glass appearance.
- **In-Menu Settings & First-Run Onboarding**: Expose checkable settings rows directly inside the Apple/Otto menu and Control Center (Dock, Window Controls, Genie, Launch at Login, Capture Visibility, Hotkeys) without a standalone Settings window; persist all preferences including `theme_mode`; auto-populate default Dock pins detected on first run (Terminal, default Browser, Settings, Notepad) and show a one-time welcome hint.
- **Conflict-Safe Shortcuts**: Replace default `Alt+N` and `Alt+P` global hotkeys with a conflict-safe 3-key chord (`Ctrl+Alt+N` & `Ctrl+Alt+P` or `Win+Alt+N` & `Win+Alt+P`) and provide an option to disable global hotkeys.
- **Shell Off Switch**: Control Center > Core Plugins turns the Top Bar and the Dock off; `Ctrl+Alt+B` and `Ctrl+Alt+D` toggle them from anywhere, including when the bar is gone, and work even with Global Hotkeys off. Turning the Top Bar off also closes the notch, Spotlight and Control Center; turning it on restores whichever of them were enabled. The choice persists across restarts.
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
