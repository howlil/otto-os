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

The workspace gate covers formatting, unit and documentation tests, compilation, and Clippy. Native Windows acceptance is still required for capture fidelity, DWM cloak timing, frame pacing, DPI behavior, and visual continuity.

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
