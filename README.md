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

- Windows, because the application uses Win32, Direct2D, DirectWrite, GDI, shell APIs, and Windows media/audio interfaces.
- Rust with edition 2024 support.
- A working Windows desktop session for runtime behavior and native acceptance testing.

## Build and verify

From the repository root:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo check --workspace
cargo clippy --workspace --all-targets --all-features
```

Run the application locally with:

```powershell
cargo run -p otto-os
```

To disable the optional backdrop blur during development:

```powershell
cargo run -p otto-os -- --no-blur
```

### Build an MSIX package

The repository includes a reproducible Windows SDK packaging lane. The Store identity, publisher, version, certificate, and final brand assets are supplied at build time rather than committed to source control.

Place the final PNG artwork in `packaging/msix/Assets`, then run PowerShell from the repository root:

```powershell
.\packaging\msix\build.ps1 `
  -IdentityName 'your-store-identity' `
  -Publisher 'CN=Your Publisher' `
  -PublisherDisplayName 'Your Publisher' `
  -Version '0.1.0.0' `
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

The scenarios are `success` for the primary notch and Control Center journey, `false` for outside-click and single-instance rejection, `edge` for empty media and rapid hotkeys, and `worst` for teardown while the panel and popup are active. A non-zero exit is a release-gate failure. Inspect the named screenshots and report entries to distinguish runtime failure, visual signal failure, input-delivery warning, and environment contamination.

## Runtime configuration

Optional plugin configuration is stored at:

```text
%LOCALAPPDATA%\otto-os\plugins.txt
```

The file uses a small versioned format:

```text
version=1
enabled=top_bar,dock,window_controls,genie,control_center
```

The popup menu and top bar remain required host surfaces. Optional plugins can be disabled without removing the required host path. The Control Center is accessed from the two-toggle icon between Search and Clock in the top bar.

## Project status

Otto OS is under active development. The workspace has executable core models, native platform adapters, rendering modules, runtime orchestration, and unit coverage. A production release requires the native Windows acceptance pass described in the engineering contract.

## Contributing

Keep changes inside the workspace boundaries and preserve the dependency direction. Prefer existing abstractions, keep layout ownership in `otto-core`, keep Win32 calls behind `otto-platform`, and keep rendering responsibilities in `otto-render`.

Before submitting a change, run the verification commands above and review the resulting diff. Do not commit secrets, generated `target/` output, or private machine-specific data.

## License

MIT. See the package metadata in [`Cargo.toml`](Cargo.toml).
