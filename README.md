# Otto OS

Otto OS is a native Windows desktop shell surface inspired by the spatial language of macOS. It provides a notch-style top bar, Dynamic Island-style interactions, a floating Dock, custom window controls, and native Windows integrations in a single Rust workspace.

The project is intentionally designed around a small, explicit set of native boundaries: pure behavior and geometry live in `otto-core`, Win32 and shell integration live in `otto-platform`, rendering lives in `otto-render`, and `otto-os` coordinates windows, messages, timers, and feature state.

## Features

- Notch-style top bar with active application context, application menus, status indicators, clock, search, tray overflow, and Control Center.
- Dynamic Island-style expanded surface for Pomodoro, media playback, and system HUD events.
- Floating Dock with application grouping, launch and focus actions, hover magnification, pinned entries, Downloads, and Trash state.
- macOS-style window controls for close, miniaturize, and zoom/full-screen actions.
- Genie-style minimize and restore feedback using a bounded window snapshot overlay.
- Native Windows media, volume, brightness, battery, display, taskbar, app-bar, recycle-bin, DPI, and foreground-window integration.
- Light, dark, and automatic theme modes with shared rendering across the top bar, Dock, and popup menus.
- Reduced-motion-aware animation policies and explicit cleanup for owned native and graphics resources.

## Architecture

```text
Windows monitor, shell, and input state
                 │
                 ▼
        otto-platform adapters
                 │
                 ▼
          otto-core values
                 │
                 ▼
        otto-os application state
                 │
       ┌─────────┴─────────┐
       ▼                   ▼
 otto-render          native actions
```

The workspace dependency direction is:

```text
otto-os → otto-render → otto-core
otto-os → otto-platform → otto-core
```

### Crates

| Crate | Responsibility |
| --- | --- |
| `otto-core` | Platform-independent models, layout, hit testing, formatting, motion, menu rows, theme resolution, and tests. |
| `otto-platform` | Win32, shell, media, audio, display, taskbar, app-bar, window, filesystem, and native event integration. |
| `otto-render` | Direct2D, DirectWrite, GDI, layered-window presentation, palette, Dock, menu, Genie, and window-control rendering. |
| `otto-os` | Process startup, plugin lifecycle, window creation, message dispatch, timers, feature orchestration, and runtime state. |

The full system contract is documented in [`.agents/engineering.md`](.agents/engineering.md). Visual and motion guardrails are documented in [`.agents/DESIGN.md`](.agents/DESIGN.md).

## Requirements

- Windows 11 22H2 or newer (build 22621+, x64), because the application uses Win32, Direct2D, DirectWrite, GDI, shell APIs, DWM backdrops (`DWMWA_SYSTEMBACKDROP_TYPE`), and Windows media/audio/display interfaces.
- Rust with edition 2024 support and Windows 10/11 SDK (`rc.exe` for application resources).
- An unlocked interactive Windows desktop session for runtime behavior, desktop pixel tests, and native acceptance testing.

## Build and verify

From the repository root:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run the application locally with:

```powershell
cargo run -p otto-os
```

Supported CLI flags:

- `--no-blur`: disable the optional backdrop blur during development.
- `--quit`: cleanly close an active Otto OS instance and restore shell state before upgrades or uninstall.
- `--restore-shell`: restore any saved taskbar and work-area recovery state.

Global keyboard shortcuts:

- `Ctrl+Alt+,`: toggle Otto Settings (always active, even with Top Bar or Global Hotkeys disabled).
- `Ctrl+Alt+B` / `Ctrl+Alt+D`: toggle the Top Bar or Dock from anywhere (always active).
- `Ctrl+Alt+C`: toggle Control Center.
- `Ctrl+Alt+Space`: toggle Spotlight Search.
- `Ctrl+Alt+N` / `Ctrl+Alt+P` / `Ctrl+Alt+Q`: toggle expanded Notch, Pomodoro timer, or Quick Capture (when Global Hotkeys are enabled).
- `Ctrl+F2`: focus the top bar for keyboard navigation.

Otto Settings is also accessible from the Windows System Tray icon (left-click opens Otto Settings, right-click opens the quick menu), the Dock right-click menu, or right-clicking the idle Notch when the Top Bar is disabled.

### Build the per-user installer

Build a per-user Windows `.exe` installer (Inno Setup 6, no UAC elevation required; invokes `--quit` cleanly on upgrade/uninstall):

```powershell
.\packaging\installer\build.ps1
```

WinGet manifest templates (`howlil.OttoOS`) live under `packaging/winget/` and automated CI/Release workflows live under `.github/workflows/`.

### Build an MSIX package (Microsoft Store lane)

The repository includes a reproducible Windows SDK packaging lane. Store identity, publisher, version, certificate, and Store PNG brand assets under `packaging/msix/Assets` are supplied at build time rather than committed to source control:

```powershell
.\packaging\msix\build.ps1 `
  -IdentityName 'your-store-identity' `
  -Publisher 'CN=Your Publisher' `
  -PublisherDisplayName 'Your Publisher' `
  -Version '1.0.0.0' `
  -CertificatePath 'C:\path\to\publisher.pfx' `
  -CertificatePassword (Read-Host 'PFX password' -AsSecureString)
```

Without `-CertificatePath`, the script produces an unsigned MSIX for local inspection and prints that it is not ready for Store submission. The script requires `makeappx.exe`, `signtool.exe` when signing, and the Windows 10/11 SDK.

The workspace gate covers formatting, unit and documentation tests, compilation, and Clippy. Native Windows acceptance is still required for capture fidelity, DWM cloak timing, frame pacing, DPI behavior, and visual continuity.

For a bounded startup and graceful-shutdown smoke test against a built executable:

```powershell
.\packaging\msix\smoke-test.ps1 -ExecutablePath 'target\release\otto-os.exe'
```

### Run Windows E2E acceptance

The native E2E runner launches an isolated Otto process for each scenario, drives Win32 input, checks visible surface topology and geometry, waits for visual stability, captures screenshots, and writes a JSON report under `artifacts/e2e`:

```powershell
.\tests\e2e\windows-e2e.ps1 -Scenario all
```

`-Scenario all` runs `success`, `false`, `edge`, `worst`, `fullscreen`, `genie`, `native-minimize`, `window-cover`, `dock`, `keyboard`, `keyboard-dock`, and `stall`; standalone scenarios include `recovery`, `appearance`, `audio`, `visual-reference`, and `dock-settings`. A non-zero exit is a release-gate failure. Inspect the named screenshots and report entries to distinguish runtime failure, visual signal failure, input-delivery warning, and environment contamination.

## Runtime configuration and diagnostics

Optional plugin state and user preferences are stored atomically at:

```text
%LOCALAPPDATA%\otto-os\plugins.txt
```

The file uses a versioned key/value format:

```text
version=1
enabled=top_bar,dock,window_controls,genie,control_center
online_lyrics=false
dock_autohide=false
dock_magnification=true
dock_icon_size=48
theme_mode=auto
capture_visible=false
hotkeys_enabled=true
classic_logo=false
onboarding_seen=false
```

Rolling local diagnostic logs (zero telemetry, excluding clipboard contents) are written to `%LOCALAPPDATA%\otto-os\logs\`.

The popup menu and Spotlight remain required host surfaces. The Top Bar, Notch, Dock, Window Controls, Genie, and Control Center can be toggled independently while keeping Otto Settings reachable via the Windows System Tray icon and `Ctrl+Alt+,`.

## Project status

Otto OS `v1.3.0` is released for Windows 11 22H2+ (x64) via GitHub Releases (per-user installer, portable ZIP, SHA-256 checksums, and WinGet manifests). Release history is tracked in [`CHANGELOG.md`](CHANGELOG.md), and architecture contracts live in [`.agents/engineering.md`](.agents/engineering.md).

## Contributing

Keep changes inside the workspace boundaries and preserve the dependency direction. Prefer existing abstractions, keep layout ownership in `otto-core`, keep Win32 calls behind `otto-platform`, and keep rendering responsibilities in `otto-render`.

Before submitting a change, run the verification commands above and review the resulting diff. Do not commit secrets, generated `target/` output, or private machine-specific data.

## License

MIT. See [`LICENSE`](LICENSE) and the package metadata in [`Cargo.toml`](Cargo.toml).
