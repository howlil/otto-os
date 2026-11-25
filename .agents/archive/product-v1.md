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
- **Otto Settings Window**: A normal Windows window with a page sidebar (Overview, Modules, Top Bar, Notch, Dock, Window Controls, Spotlight, Control Center, Appearance, Shortcuts, General, About) holds every Otto setting. The Modules page plugs each feature in or out at runtime and shows whether it is Off, Starting, Running, or Failed; a module whose parent is off is shown paused with the reason. The tray icon is the canonical way to open it and cannot be turned off; `Ctrl+Alt+,`, the Otto menu, the Control Center settings control, and the Dock menu open the same window.
- **Spotlight**: Search opens a focused local-app search surface; `Ctrl+Alt+Space` toggles it and Dock Apps opens its full app catalog. Search ranks installed app names as the query changes; arrows move selection, Return launches, Escape closes, and Tab switches list/grid. The surface can move and resize, morphs from its trigger, and settles immediately with reduced motion. Search covers `shell:AppsFolder` apps only, not files or web results.
- **Status Cluster Actions**: Wi-Fi opens Wi-Fi controls, Volume opens Sound controls, and Battery opens Windows Power & Battery settings. Only Volume accepts wheel adjustment; if Control Center is disabled, Wi-Fi and Volume open their matching Windows Settings pages.
- **Dock**: Floating bottom dock with uniform macOS squircles, cosine fish-eye magnification, running/active indicators, Downloads stack, and dynamic Recycle Bin. Dock Behavior has three exclusive modes: Always visible (default; floats over windows), Auto-Hide Dock (hides while any window covers it and shows again when nothing does; the bottom edge reveals it), and Reserve Screen Space (taskbar-like: maximized windows stop above the Dock; needs Hide Windows Taskbar). Every mode hides the Dock for a truly fullscreen app. Clicking active app asserts focus and brings windows to front (no Windows minimize-toggle).
- **Window Controls**: Zero-overlay macOS traffic lights (`[Red (Close), Yellow (Genie Minimize), Green (Zoom/Maximize)]`) rendered inside the Top Bar (between the brand icon and the active app name) to control the foreground window without covering external app titlebars or fighting Z-order, plus native client-area traffic lights inside `OttoSettings`'s borderless-caption Mica frame (`WM_NCCALCSIZE`/`WM_NCHITTEST`). Hover shows internal glyphs (`×`, `−`, `+`/`⤢`), press-and-release cancels if the pointer leaves the circle, Alt+Click on Green toggles Fullscreen, Alt+Click on Yellow/Red minimizes or politely closes (`WM_CLOSE`) every primary window of the active app, and 600 ms hover on Green or right-click opens the Window Tiling menu. Elevated or non-controllable foreground windows dim the Top Bar circles instead of fabricating actions; external windows keep their own native caption buttons and corner preference untouched.
- **Control Center**: Frosted-glass modular popover with interactive Sound and Display sliders, now playing block, and quick settings toggles/shortcuts.
- **Calendar & Notification Center**: Date/time opens a monthly calendar modelled on the Apple calendar: a `Month Year` header with previous, today and next controls, a single-letter weekday row, a fixed six-week grid with neighbouring-month days dimmed, a red filled disc on today, and a quiet Date & Time Settings row (Left/Right arrows also change month). The adjacent bell independently opens grouped Windows notification summaries with per-item dismiss, Clear All, and permission recovery. Opening any top-bar popup closes the competing Otto popup.

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
- **Quick Capture & Tasks**: Opt-in (`Quick Capture` in Otto Settings, default off). `Ctrl+Alt+Q`, `+ Add`, or clicking the empty `Tasks` card opens an inline one-line input directly inside the Notch `Tasks` tab (expanding the card height so visible tasks remain underneath): Enter saves and closes, Shift+Enter saves and keeps adding, Esc or clicking outside cancels, and focus returns to the previous app. Existing tasks can be edited inline via the per-row pencil button or right-clicking the task row (`Enter` updates the note in place while preserving done/active state, `Esc` cancels). The `Tasks` tab lists up to 50 tasks (4 per page, newest first): the circle marks done, clicking the text makes it the one active task, which replaces the subtitle on the Focus tab, `x` removes, and `Clear` removes done tasks. Tasks persist as plain text in `%LOCALAPPDATA%\otto-os\tasks.txt`; nothing is synced or sent anywhere, and a full list drops the oldest done task before refusing a new one.
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
| **Window Controls**| Traffic lights `[Red, Yellow, Green]` | Zero-overlay controls inside `OttoNotchBar` + native `WM_NCCALCSIZE` controls in `OttoSettings` | Foreground window inspection | Native caption buttons untouched | `VER_1.4`, `VER_3.3` |
| **Genie Effect** | macOS Window Minimize | Cubic Hermite mesh suction via Direct2D | HWND screen capture | Win32 native minimize | `VER_1.2`, `VER_3.2` |
| **Dock** | macOS Dock | Layered `OttoDockBar` with squircle icons | Shell icon extraction | Explorer taskbar | `VER_2.2`, `VER_3.2` |
| **Control Center** | Control Center | `OttoPopupMenu` with live WASAPI / GSMTC | Windows Core Audio / WinRT | Windows Action Center | `VER_2.5`, `VER_4.1` |
| **Quick Capture** | Reminders quick entry | Inline `OttoQuickCapture` native `EDIT` aligned inside the `Tasks` notch tab (add & edit) | Explicit user opt-in | Module off: no hotkey, no tab, no file access | `VER_2.5` |
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
- **In-Menu Settings & First-Run Onboarding**: Expose checkable settings rows directly inside the Apple/Otto menu and Control Center (Dock, Window Controls, Genie, Launch at Login, Capture Visibility, Hotkeys); the standalone Otto Settings window (section 2.1, Otto Settings Window) is the full home for every setting; persist all preferences including `theme_mode`; auto-populate default Dock pins detected on first run (Terminal, default Browser, Settings, Notepad) and show a one-time welcome hint.
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

### Closed in v1.0.0 – v1.3.1
- Root `LICENSE` (MIT), `build.rs` executable icon and `VERSIONINFO`, Orbit O default branding, crate-versioned `User-Agent`, Inno Setup per-user installer (`--quit`, Launch at Login), GitHub Actions CI & Release workflows, WinGet templates, MSIX Store assets and `MinVersion="10.0.22621.0"`, Windows 11 22H2 startup gate, DDC/CI brightness, conflict-safe `Ctrl+Alt+*` hotkeys, standalone Otto Settings window, rolling zero-telemetry local logs, `MessageBoxW` fatal error dialogs, and core `M04` UIA fragment/invoke providers.
- OSS repository governance files (`CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, GitHub Issue/PR templates, Dependabot) and refreshed user-first `README.md`.

### Open Gaps for Production & Store Qualification
1. **Authenticode Code Signing & Cryptographic Update Verification**:
   - Configure CI Authenticode signing secrets (`WINDOWS_CERT_PFX_BASE64` / `WINDOWS_CERT_PASSWORD`, e.g. SignPath OSS or Azure Trusted Signing) so public installers and executables no longer trigger SmartScreen warnings.
   - Add cryptographic publisher verification (`WinVerifyTrust` / pinned public-key signature) in `crates/platform/src/control_center/updates.rs` beyond same-origin `SHA256SUMS.txt`.
2. **Distribution Channels & Platform Parity**:
   - Publish `howlil.OttoOS` to `microsoft/winget-pkgs` (and automate manifest PRs on release tags).
   - Ship signed **MSIX / Sparse Package** (`M13` Store qualification, Store-safe build flag gating `BrandIcon::ClassicApple`) to unlock `UserNotificationListener` Notification Center and Notch toast events for end users.
   - Evaluate `aarch64-pc-windows-msvc` (Windows 11 ARM64) release builds and multi-monitor Top Bar/Dock support.
3. **Performance & Visual Release Evidence**:
   - Measure and record post-DEC-69 release SLIs (idle CPU `<= 1%`, discrete GPU `0%`, p95 frame `<= 16.6 ms`) and refresh `tests/visual/reference/manifest.json` across the 4 material styles (`Liquid Glass`, `Frosted`, `Tinted`, `Solid`).
   - `.ics` calendar integration remains deferred pending an explicit network/privacy specification.
