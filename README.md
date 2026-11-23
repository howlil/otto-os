<div align="center">

  <img src="assets/otto-os-orbit.svg" alt="Otto OS logo" width="88" height="88" />

  <h1>Otto OS</h1>

  <p><strong>A native top bar, Dynamic Island, and floating Dock for Windows 11.</strong></p>

  <p>Bring a cohesive, macOS-inspired desktop experience to Windows — built in Rust, modular by design, and local-first.</p>

  <p>
    <a href="https://github.com/howlil/otto-os/releases/latest"><img src="https://img.shields.io/github/v/release/howlil/otto-os?style=flat-square&label=release&color=075bef" alt="Latest release" /></a>
    <a href="https://github.com/howlil/otto-os/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/howlil/otto-os/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status" /></a>
    <a href="https://github.com/howlil/otto-os/releases"><img src="https://otto-os-landing.pages.dev/api/downloads-badge" alt="Application downloads" /></a>
    <img src="https://img.shields.io/badge/Windows_11-22H2%2B_x64-0078D4?style=flat-square" alt="Windows 11 22H2 or newer, x64" />
    <a href="LICENSE"><img src="https://img.shields.io/github/license/howlil/otto-os?style=flat-square" alt="MIT license" /></a>
  </p>

  <p>
    <a href="https://github.com/howlil/otto-os/releases/latest"><strong>Download Otto OS</strong></a>
    &nbsp;·&nbsp;
    <a href="https://otto-os-landing.pages.dev"><strong>Explore interactive demos</strong></a>
    &nbsp;·&nbsp;
    <a href="#installation"><strong>Get started</strong></a>
    &nbsp;·&nbsp;
    <a href="docs/reference.md"><strong>Documentation</strong></a>
  </p>

</div>

---

## See Otto OS in action

The [official Otto OS website](https://otto-os-landing.pages.dev) includes interactive demonstrations of the Dynamic Island, top bar, Dock/Genie motion, Focus, Spotlight, and appearance settings.

> **Product visuals:** Actual Windows desktop screenshots and recordings are being prepared. The [media capture guide](docs/media/README.md) describes the required shots and safe capture settings. We do not substitute concept renders for real application screenshots.

## Why Otto OS?

- **Truly native:** A single Rust process using Windows APIs, without Electron or a WebView.
- **Make it yours:** Enable or disable the Top Bar, Notch, Dock, Window Controls, Genie, and Control Center independently at runtime.
- **Local-first:** No accounts or background analytics. Optional online lyrics and manually initiated update checks are explicit.
- **Designed for safe recovery:** Closing or uninstalling restores Windows shell state; a dedicated `--restore-shell` command is available if needed.

## Features

| Experience | What you get |
| --- | --- |
| **Top Bar** | Contextual menus, tray and system status, calendar, notifications, and configurable auto-hide. |
| **Dynamic Island & Notch** | Focus timers, Screen Time, music and synced lyrics, quick tasks, clipboard shelf, and system HUD. |
| **Floating Dock** | Pinned and running apps, magnification, downloads and Recycle Bin stacks, and GPU-accelerated Genie animations. |
| **Window Controls** | Native caption overlays for minimize, maximize, close, and tiling actions with safety fallbacks. |
| **Control Center & Spotlight** | Wi-Fi, audio, brightness, and battery controls; fast local app search with `Ctrl+Alt+Space`. |
| **Appearance & Focus** | Liquid Glass and other surface styles, adjustable accents, and non-destructive distraction blocking. |

For capabilities, constraints, and keyboard shortcuts, see the [full feature reference](docs/reference.md#features).

## Installation

**Requirements:** Windows 11 22H2 or newer (build `22621+`), x64. Windows 10 is not supported.

1. Open the [latest GitHub Release](https://github.com/howlil/otto-os/releases/latest).
2. Download `otto-os-setup-<version>-x64.exe` (**recommended**, per-user install without UAC) or `otto-os-<version>-windows-x64.zip` (**portable**, extract and run `otto-os.exe`).
3. Open **Otto Settings** from the system tray or press `Ctrl+Alt+,` to configure modules.

| Package | Purpose |
| --- | --- |
| **Installer** (`.exe`) | Installs to `%LOCALAPPDATA%\Programs\Otto OS` with per-user install and clean upgrade/uninstall behavior. |
| **Portable ZIP** | Runs directly after extraction. |
| **`SHA256SUMS.txt`** | Verifies the downloaded release files. |

**Signing notice:** Public releases are currently unsigned. Windows Defender SmartScreen may warn on first launch. Verify the file against the release's `SHA256SUMS.txt` before deciding whether to run it. You can check the file in PowerShell:

```powershell
Get-FileHash .\otto-os-setup-<version>-x64.exe -Algorithm SHA256
```

For detailed recovery options and all shortcuts, use the [user and system reference](docs/reference.md#keyboard-shortcuts--recovery).

## Privacy, safety & limitations

Otto OS runs locally with no account, telemetry, or cloud sync. Optional network access is limited to GitHub for user-triggered update operations and LRCLIB when online synced lyrics are enabled. Clipboard history is kept in RAM, and elevated windows retain their native Windows controls.

Because Otto OS modifies desktop shell surfaces, review the [system-access details](docs/reference.md#privacy-network--system-access) and [known limitations](docs/reference.md#known-limitations--non-goals). The tray menu offers a normal exit; `otto-os.exe --restore-shell` provides an explicit recovery path.

## Project growth

Downloads count the **Windows installer EXE and portable ZIP** across published GitHub Releases, matching the [website's download metric](https://otto-os-landing.pages.dev). Checksums and WinGet manifests are excluded. These are download requests, **not unique users or installations**. The historical chart is refreshed daily, so it may briefly lag behind the live badge.

<p align="center">
  <a href="https://github.com/howlil/otto-os/releases">
    <img src="https://raw.githubusercontent.com/howlil/otto-os/repo-growth-data/assets/repo-growth.svg" alt="Otto OS growth: application downloads by date and release, stars, and forks" width="900" />
  </a>
</p>

## For developers

Otto OS is a Rust 2024 workspace with `otto-core`, `otto-platform`, `otto-render`, and `otto-os`. The codebase uses native Windows APIs and requires Windows SDK/MSVC tools for development.

```powershell
git clone https://github.com/howlil/otto-os.git
cd otto-os
cargo run -p otto-os
```

The [engineering reference](docs/reference.md#architecture) covers crate boundaries, build prerequisites, tests, packaging, and configuration. For contributions, read [CONTRIBUTING.md](CONTRIBUTING.md).

## Community & project information

[Release history](CHANGELOG.md) · [Report security vulnerabilities](SECURITY.md) · [Code of Conduct](CODE_OF_CONDUCT.md) · [License](LICENSE) · [Complete documentation](docs/reference.md)

---

<sub>Otto OS is an independent open-source project and is not affiliated with, endorsed by, or sponsored by Apple Inc. or Microsoft Corporation.</sub>
