# Otto OS — Product Contract

> Active live product contract (`<= 8 KB`). Full v1.0–v1.3.1 historical spec and parity archive live in [archive/product-v1.md](archive/product-v1.md). Companion landing copy rules live in [landing/PRODUCT.md](../landing/PRODUCT.md).

## 1. Product Vision & Target Platform

Otto OS brings a quiet, native macOS-inspired desktop shell and Dynamic Island workflow to **Windows 11 22H2+ (build `22621+`, `x86_64-pc-windows-msvc`)** without taking over external application state or sending telemetry. Windows 10 builds are explicitly blocked at startup with a diagnostic dialog before mutating shell state.

## 2. Live Feature Surfaces

- **Top Bar (`OttoNotchBar`)**: Edge-to-edge primary-monitor menu bar reserving work area, showing Brand icon (Orbit O default; Classic Apple opt-in for non-Store builds), active app name and contextual menus (`File`, `Edit`, `View`, `Window`, `Help`), and right cluster (`Tray Overflow -> Wi-Fi -> Volume -> Battery -> Search -> Control Center -> Notifications -> Date/Time`). Supports Auto-Hide modes, per-item visibility toggles, Menu Bar Background toggle, and Notch Stays Visible.
- **Notch / Dynamic Island (`OttoNotchPanel`)**: Obsidian Black (`#000000`) hardware pill supporting Classic Notch and Dynamic Island styles, dual compact live activities (`NotchSecondary` circle swap/dismiss), transient OS HUD (Volume, Brightness, Battery, Caps Lock, Focus Blocked), Privacy camera/mic/location dots + mic mute, and expanded tabs:
  - **Focus (Pomodoro)**: End-to-end Apple HIG Focus system with customizable duration (`1–180m` steppers, scroll, click-to-type, presets), Focus Categories (`Coding`, `Belajar`, `Kerja`, `Hobby`, up to 12 custom modes with rename & delete) with Allowlist/Blocklist **Focus Shield** enforcement, **Background Relief** (auto EcoQoS throttling with restore + explicit opt-in graceful `WM_CLOSE` per app), local **Smart App Suggester**, 2-tab `Otto Settings -> Focus` (`Focus Modes` | `Insights`), and **Screen Time (`Time`)** Today/7-Day/`Map` GitHub-style **Contribution Heatmap** (16-week in Notch + 26-week per-category & combined in `Insights`).
  - **Music**: GSMTC cover art, seekbar, transport controls, and explicit opt-in synced lyrics (`LRCLIB`, cache-only by default).
  - **Pocket (Shelf)** (ephemeral 12-item drag/drop holding tray) & **Clip** (RAM-only 12-item clipboard history excluding sensitive formats).
  - **Tasks (Quick Capture)**: Opt-in local task list (`%LOCALAPPDATA%\otto-os\tasks.txt`, up to 50 tasks) with inline Notch add/edit (`Ctrl+Alt+Q`).
  - **Notification Events**: Peek/Banner/Ringing call alerts (dormant until packaged MSIX notification access is granted).
- **Dock (`OttoDockBar`)**: Primary-monitor floating bottom Dock with squircle icons, hover magnification, bounce/badges/progress, click-to-focus active app semantics, drag-reorder/unpin, `WM_DROPFILES`, Downloads grid stack, dynamic `$Recycle.Bin` Trash, and three exclusive modes (`Always`, `Auto-Hide`, `Reserve Screen Space`). Disabling Dock restores `Shell_TrayWnd`.
- **Window Controls & Genie**: Per-window floating `OttoWindowControls` cover overlays replacing native `- □ ×` caption buttons on external windows + native top-left client-area traffic lights inside `OttoSettings` and `OttoFinder`; Alt+Click app-wide/fullscreen semantics; 600 ms hover / right-click Window Tiling menu; GPU-accelerated Genie minimize/restore with blank-frame detection and safe native Win32 fallback.
- **Control Center, Spotlight, Calendar & Notification Center**: Full-screen blurred iPadOS/macOS-style modular 4-column Liquid Glass Control Center with Edit Mode drag-to-resize (`1×1`, `2×1`, `1×2` vertical sliders, `2×2`, `4×1`, `4×2`), drag-to-reorder, and `+ Add a Control` Gallery (Connectivity, Wi-Fi, Bluetooth, Focus, Battery, Display DDC/CI + internal brightness, WASAPI Sound, Mute, Night Light, Now Playing, Dark Mode, Screenshot, Lock Screen, Task View, Otto Finder, Spotlight, Otto Settings); local `shell:AppsFolder` Spotlight search (`Ctrl+Alt+Space`, no file/web index); fixed 6-week Calendar popup; grouped Notification Center.
- **Otto Settings (`OttoSettings`)**: Standalone DWM Mica top-level window owning all preferences, runtime module plug-in/out controls (`Modules`), switchable Appearance (`LiquidGlass`, `Frosted`, `Tinted`, `Solid`; `Auto`/`Light`/`Dark`; 7 Accent presets), Background Cleanup (opt-in CPU hog EcoQoS + safe process audit, never auto-terminates), shortcuts (`Ctrl+Alt+*`), and explicit user-triggered SHA-256 verified in-app updates.
- **Otto Finder (`OttoFinder`)**: Built-in native DWM Mica + Direct2D macOS-style File Explorer (toggleable via `Otto Settings -> Modules / General` with instant `explorer.exe` fallback) supporting all 4 Finder views (`Icon`, `List` with expandable `▸/▾` folder tree, `Column` / Miller Columns + preview pane, `Gallery`), multi-tab browsing, Sidebar (Favorites, Drives, Trash, 7 local color Tags), `Space` Quick Look preview, `Ctrl+I` Get Info Inspector, `Ctrl+Shift+G` Go to Folder, hybrid macOS+Windows keyboard ergonomics (`Enter`/`F2` stem-selected inline rename, `Ctrl+O`/`Ctrl+↓`/double-click open, `Delete`/`Ctrl+Backspace` Recycle Bin, `Ctrl+Alt+V` Move Item Here over `CF_HDROP`), live directory watching, and Dock/Top Bar integration.

## 3. Explicit Non-Goals & Privacy Contract

- **Zero telemetry**: No analytics, remote crash reporting, background update checks, or silent network calls. Only explicit opt-in online lyrics and user-clicked `Check for Updates...` access the network over verified HTTPS.
- **Zero secret logging**: Rolling local diagnostics (`%LOCALAPPDATA%\otto-os\logs\`) never log clipboard contents, task text, keystrokes, or secrets.
- **Out of scope**: Apple ID / iCloud / CloudKit sync, AirDrop / AirPlay / Continuity Camera, purple system-audio recording indicator, macOS sandbox/Mach IPC emulation, file/web search in Spotlight, or automatic process termination / working-set purging.

## 4. Non-Negotiable Quality Invariants

| Invariant | Target | Verification Path |
|---|---|---|
| Idle CPU usage | `0%` when settled (no active animations) | Event-driven hooks + `windows-e2e.ps1 -Scenario stall` |
| Idle memory footprint | `<= 60 MB` private working set | Release build live check |
| Graphics/OS handle safety | `0` leaked HDC/HBITMAP/HICON/COM/USER objects over 10,000 cycles | `cargo test -p otto-render -- --ignored --test-threads=1` |
| Shell recovery safety | Work area, taskbar, and hidden windows always restore cleanly | `--quit`, `--restore-shell`, panic hook, `windows-e2e.ps1 -Scenario recovery` |

## 5. Open Production & Store Gaps (Post-v1.3.1)

1. **Authenticode signing & trust verification**: Configure CI signing secrets (`WINDOWS_CERT_PFX_BASE64` / `WINDOWS_CERT_PASSWORD`) and add `WinVerifyTrust` / pinned signature verification in `updates.rs` beyond `SHA256SUMS.txt`.
2. **Distribution & Store qualification**: Publish and automate `howlil.OttoOS` WinGet manifest PRs; ship signed MSIX / Sparse Package (`M13`, Store-safe flag gating `BrandIcon::ClassicApple`) to unlock `UserNotificationListener`.
3. **Future evaluation**: `aarch64-pc-windows-msvc` release builds and multi-monitor Top Bar/Dock support.
