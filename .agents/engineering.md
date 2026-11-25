# Otto OS — Engineering Contract

> Active live invariants only (`<= 8 KB`). Full v0.1–v1.3.1 decision history lives in [archive/decisions-v1.md](archive/decisions-v1.md).

## 1. System Architecture

Otto OS is a single-process modular-monolith Windows 11 (`22621+`) shell surface hosting layered windows on the primary monitor plus the standalone DWM Mica `OttoSettings` window:

- **`OttoNotchBar` + `OttoNotchPanel`**: Primary-monitor top bar (`AppBar` with `SPI_SETWORKAREA` fallback), brand icon, active app menus, Obsidian Black center Notch / Dynamic Island (Focus, Screen Time, Pocket/Shelf, Clip, Tasks, Media + Lyrics, HUD, Notifications, dual compact secondary circle), and right status cluster (`Tray -> Wi-Fi/Volume/Battery -> Search -> Control Center -> Notifications -> Clock/Calendar`).
- **`OttoDockBar`**: Floating bottom Dock (Finder, Apps, pinned/running apps grouped 1 per exe, Downloads grid stack, divider, `$Recycle.Bin` probe Trash) with Always / Auto-Hide / Reserve-Space modes (`DockReservation`) and `WM_DROPFILES`.
- **`OttoWindowControls`, `OttoControlCenter`, `OttoSpotlight`, `OttoPopupMenu`, `OttoGenie`, `OttoFocusShield`, `OttoQuickCapture`, `OttoSettings`, `OttoFinder`**: Dedicated surfaces sharing core layout and render kits. `PluginId::WindowControls` owns non-topmost per-window `OttoWindowControls` cover overlays (up to 24 external windows) covering native `- □ ×` caption buttons with sampled row-band fills while excluding `OttoSettings` and `OttoFinder` (which render native top-left traffic lights inside their `WM_NCCALCSIZE` frames and still act as occluders). `OttoSettings` and `OttoFinder` share `otto-render::kit::canvas::HwndCanvas` and `otto-platform::settings` Mica window helpers (`create_finder_window`), dispatching through one shared `settings_window_proc` in `host/messages.rs`.

### Crate Boundaries & Dependency Direction

```text
otto-os (crates/app) ──► otto-platform (crates/platform) ──► otto-core (crates/core)
                     ──► otto-render   (crates/render)   ──► otto-core (crates/core)
```

| Crate | Owns | Forbidden |
|---|---|---|
| `otto-core` | Pure models, layout/hit-testing, formatting, motion, glass math, capability ports, `ExperimentalFlags` | Win32, COM, D2D, rendering, outward crate imports, `unsafe` (`0`) |
| `otto-platform` | Win32/COM/WinRT adapters, hooks, background workers, atomic persistence, env reading | Rendering or app orchestration |
| `otto-render` | Direct2D, DirectWrite, D3D11 HLSL shaders, GDI/DIB/brush/geometry ownership (`Drop`), design tokens | Platform actions, OS queries, `otto-platform` or `otto-os` imports |
| `otto-os` | Process entry, `Host` lifecycle/registry, centralized `win_msg`, feature controllers, `fitness_tests.rs` | Duplicating core layout/motion policy or adding unguarded `unsafe` |

## 2. Core Contracts & Invariants

- **Single source of truth (`G3`)**: `Metrics` -> `compute_layout*` -> `Layout` + `HitTarget` in `otto-core` drive both rendering and input dispatch; `otto-render` never performs OS queries.
- **State & threading (`S1`–`S4`)**: Window message loop runs sequentially on the main thread. Only `host/mod.rs` holds `HOST` `thread_local!`; no new `thread_local!` or global singletons may be added (`F3`, `F7`). Every worker/hook has an explicit owner and `Host::shutdown_session` stop order before HWND destruction (`G2`).
- **Capability ports (`K3`)**: External effects cross narrow feature ports (`NotchPort`, `DockPort`, `ControlCenterPort`, `UpdatesPort`, `NotificationCenterPort`), never a mega `PlatformPort`.
- **Resource & failure policy (`G1`, `G4`)**: Startup/required-surface failures propagate as `Result` and abort; optional plugin startup failures isolate to `Failed` without degrading Top Bar/Popup Menu/Spotlight. Failed OS queries return `Option`/safe defaults and never fabricate success. Every GDI/COM/Win32 handle pairs acquisition with `Drop` (`0` leaks over 10,000 cycles).
- **Shell safety**: `SessionShutdownGuard` and the main-thread panic hook restore hidden windows and desktop work area (`shell_recovery.json` / `--restore-shell`). Fonts register process-private (`FR_PRIVATE`).
- **Data isolation & config (`K5`)**: Debug builds isolate data to `%LOCALAPPDATA%\otto-os-dev`; release uses `%LOCALAPPDATA%\otto-os` (`OTTO_OS_DATA_DIR` overrides both). `plugins.txt` (`version=1`) uses atomic write-through replacement, preserves required surfaces, ignores unknown keys, and leaves `version > 1` untouched.
- **Experimental flags (`OTTO_EXPERIMENTAL_FLAGS`)**: Dark-launch experiments register in `otto-core::runtime::flags` (`ExperimentalFlag`), are read once via `otto_platform::read_experimental_flags()`, are logged at startup when non-empty, and are never persisted into `plugins.txt` until promoted.
- **Notch companion property**: `OttoOS.NotchRect` packs collapsed island `(x: i16, y: i16, w: u16, h: u16)` little-endian into 64 bits on `OttoNotchBar` in physical client pixels (`pack_notch_rect` / `unpack_notch_rect`).
- **Web companion (`landing/`)**: Astro + React 19 + Tailwind + Cloudflare Pages Functions (`waitlist`, `beta-downloads`, `downloads-badge`) + D1 SQL (`migrations/*.sql`). Enforced by Biome, `astro check`, `tsc`, Bun unit/guardrail/D1 tests, and `scripts/smoke-build.ts`.

## 3. Architectural Fitness Gates (`fitness_tests.rs`)

Automated in `cargo test -p otto-os host::fitness_tests`:

- `F1`–`F4`: Crate dependency isolation, `notch/` isolation, single `HOST` `thread_local!`, `render` isolation, folder-only crate roots, centralized `win_msg`, comment-free Rust sources.
- `F5`–`F7`: Zero wildcard re-exports in crate roots/`mod.rs`, render primitive boundaries (`CreateTextFormat` only in `render/lib.rs`), bounded `thread_local!` inventory.
- `F8`–`F11`: Ratchets on prod `#[allow]` counts (`<= 85` total + per-lint caps), per-crate prod `unsafe` budgets (`core=0`, `app<=28`, `render<=355`, `platform<=661`), per-file prod line ceilings (`<= 800` new files; ratcheted ceilings for 18 legacy files), and `app` feature sibling-coupling edges.
- `F12`: Unified forge hygiene (`.agents/{engineering,product,DESIGN}.md <= 12 KB`, `AGENTS.md <= 8 KB`, zero duplicate `DEC-*` IDs).

## 4. Active Decisions (Post-Archive)

Historical `DEC-1`..`DEC-93` are preserved in [archive/decisions-v1.md](archive/decisions-v1.md). Key active baseline decisions:

- **DEC-92 [two-way] (2026-11-24)**: Retired 24-HWND floating `OttoWindowControls` overlays in favor of zero-overlay Top Bar controls + native `WM_NCCALCSIZE`/`WM_NCHITTEST` traffic lights in `OttoSettings`; tightened `WH_MOUSE_LL` minimize interception strictly to DWM caption windows while Genie runs.
- **DEC-93 [two-way] (2026-11-24)**: Enforced workspace Tier-A safety `deny` lints, `clippy.toml`, `deny.toml`, Biome + token-parity tests in `landing/`, render token/primitive consolidation (`appearance/tokens.rs`, `kit/draw.rs`), port splits (`UpdatesPort`, `NotificationCenterPort`), safe Win32 wrappers, and fitness checks `F5`–`F11`.
- **DEC-94 [two-way] (2026-11-25)**: Hardened anti-slop software factory: wired `cargo-deny` into CI/release/local gates, converted `F8`–`F11` into strict per-lint/per-crate/per-file/all-feature ratchets, added Biome cognitive-complexity & `landing/` file-size/D1/build-smoke gates, added `-Scenario full` + `scripts/release-verify.ps1`, introduced `OTTO_EXPERIMENTAL_FLAGS`, archived historical `.agents/` docs under `F12`, and added `CODEOWNERS`, anti-slop PR template, and issue/release forward-pressure workflows.
- **DEC-95 [two-way] (2026-11-25)**: Added native built-in `OttoFinder` (`crates/{core,platform,render,app}/src/finder/`) behind `AppPreferences::finder_enabled` (default `true`, additive `plugins.txt` v1 key) with zero `explorer.exe` injection, zero new external crates, shared `HwndCanvas` keeping `otto-render` `unsafe` within `F9`, shared `create_mica_window` + safe `std::fs` keeping `otto-platform` `unsafe` within `F9`, and `FinderDirWatcher` RAII shutdown guaranteeing `0%` idle CPU when hidden.
- **DEC-96 [two-way] (2026-11-25)**: Superseded DEC-92 by restoring non-topmost per-window `OttoWindowControls` cover overlays (covering native `- □ ×` on external windows) and removing traffic lights from `OttoNotchBar`, while retaining native client-area traffic lights inside `OttoSettings` and `OttoFinder` (excluded from external overlays while remaining normal occluders) and fixing `WM_LBUTTONUP` capture-release ordering (`release_capture_if_owned` after taking press state) plus `OttoFinder` `WM_ACTIVATE` focus handoff.
- **DEC-97 [two-way] (2026-11-25)**: Hardened `OttoWindowControls` and custom Mica frames: stripped `WS_SYSMENU` + handled `WM_NCACTIVATE` on `OttoSettings`/`OttoFinder` so DWM never bleeds native `- □ ×` over translucent Mica; fixed `is_full_screen` and auto-hide `effective_bar_height` for maximized custom-chrome apps (Electron/Chromium); rejected transparent `WS_EX_LAYERED` / non-captioned ghost overlays; added cached async `WM_GETTITLEBARINFOEX` + UIA caption probe (`caption_probe.rs`), DPI-normalized `screen_caption_bounds`, and `button_top_offset` vertical centering; split atomic `BeginDeferWindowPos` Z-order + fast-path single-HWND drag sync (`overlay_zorder.rs`, `MOVESIZESTART`/`END`, `CAPTURESTART`).
- **DEC-98 [two-way] (2026-11-26)**: Upgraded `OttoControlCenter` to an iPadOS/macOS-style full-monitor blurred canvas (`capture_screen_scaled_now` + `box_blur_in_place`) hosting a 4-column modular, drag-to-move, drag-to-resize Liquid Glass tile grid (`1×1`, `2×1`, `1×2` vertical sliders, `2×2`, `4×1`, `4×2`) and `+ Add a Control` Gallery, persisted atomically in `%LOCALAPPDATA%\otto-os(-dev)\control_center.txt` (`version=1`) without altering `plugins.txt` v1.

## 5. Release & Canary Pipeline

1. **Version parity**: `Cargo.toml` `[workspace.package].version` (`X.Y.Z`) mirrors `crates/app/otto-os.manifest` (`X.Y.Z.0`), `packaging/installer/otto-os.iss`, `packaging/winget/*.yaml`, and a non-empty `[X.Y.Z]` section in `CHANGELOG.md`.
2. **Canary soak**: Tag `vX.Y.Z-rc.1` (published as GitHub pre-release) and run `.\scripts\release-verify.ps1 -E2EScenario full` on unlocked Windows 11 hardware (including the 13 `#[ignore]` native/leak tests, `windows-e2e.ps1 -Scenario full`, MSIX smoke test, and `--restore-shell` / in-app updater upgrade check) before tagging stable `vX.Y.Z`.
3. **Immutable releases**: `.github/workflows/release.yml` re-runs `fmt`, `clippy -D warnings`, `cargo-deny`, unit tests, `check-e2e.ps1`, signs when `WINDOWS_CERT_PFX_BASE64`/`WINDOWS_CERT_PASSWORD` exist, and publishes Inno installer, portable ZIP, WinGet manifests, and `SHA256SUMS.txt`. Post-release fixes roll forward as `vX.Y.(Z+1)`.
