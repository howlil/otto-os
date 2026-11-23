# Contributing to Otto OS

Thank you for your interest in contributing to Otto OS! Whether you are reporting a bug, improving documentation, or submitting a Rust pull request, this guide explains how the repository works so your changes pass review and CI smoothly.

## Before You Start

- Check existing [Issues](https://github.com/howlil/otto-os/issues) to avoid duplicate reports or overlapping work.
- Review the **Known Limitations & Non-Goals** in [`README.md`](README.md#known-limitations--non-goals) before proposing large new features.
- For substantial architecture, storage, network, or new dependency changes, open an issue first to align on the contract in [`.agents/engineering.md`](.agents/engineering.md).

## Development Environment

### Prerequisites
- **Windows 11 22H2+** (build `22621+`, `x64`).
- **Rust toolchain** matching [`rust-toolchain.toml`](rust-toolchain.toml) (Rust edition 2024, `rustfmt`, `clippy`, target `x86_64-pc-windows-msvc`).
- **Windows 10/11 SDK** (required for `rc.exe` resource compilation in `crates/app/build.rs`).
- Optional (only if editing `landing/`): [Bun](https://bun.sh/).

### Running Locally
Debug builds isolate their data directory to `%LOCALAPPDATA%\otto-os-dev` so local testing will not overwrite an installed production configuration:

```powershell
cargo run -p otto-os
```

You can also override the data directory explicitly with `OTTO_OS_DATA_DIR`.

If a development crash ever leaves the Windows taskbar or work area altered, run:

```powershell
cargo run -p otto-os -- --restore-shell
```

## Architecture & Repository Rules

Otto OS enforces strict architectural boundaries via automated tests in [`crates/app/src/host/fitness_tests.rs`](crates/app/src/host/fitness_tests.rs). Please keep these rules in mind before coding:

1. **Crate dependency direction**:
   - `otto-os → otto-render → otto-core`
   - `otto-os → otto-platform → otto-core`
   - `otto-core` must remain pure and platform-independent (zero Win32, Direct2D, or outward crate imports).
   - `otto-render` must never import `otto-platform` or `otto-os`.
2. **Single source of truth for layout**:
   - All layout geometry and hit targets belong in `otto-core`. Rendering (`otto-render`) and input dispatch (`otto-os`) consume the same resolved `Layout`.
3. **Comment-free Rust source code**:
   - Rust files under `crates/*/src/` must **not** contain `//`, `///`, `//!`, or `/* ... */` comments. This is enforced by `workspace_rust_sources_remain_comment_free` in `cargo test`.
   - Use clear names, small functions, and behavioral tests to convey intent, and record durable architectural rationale in [`.agents/engineering.md`](.agents/engineering.md).
4. **Module layout & host state**:
   - Crate `src/` roots contain only `lib.rs` (or `main.rs`) and feature/domain folders.
   - Do not introduce new `thread_local!` singletons in `crates/app` (only `host/mod.rs` may hold `HOST`) or scatter `const WM_*` definitions outside `crates/app/src/host/win_msg.rs`.
5. **Resource safety & FFI**:
   - Keep `unsafe` blocks localized to `otto-platform` or `otto-render` boundaries. Every OS/GDI/COM resource must have a clear RAII/`Drop` release path.
6. **Zero telemetry & strict privacy**:
   - Never add analytics, background network calls, or log user secrets / clipboard contents.

## Verification Gate

Run the local verification gate before opening a pull request:

```powershell
.\scripts\gate.ps1
```

Or run each step individually:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace -- --test-threads=1
```

If your changes touch `landing/`:

```powershell
cd landing
bun install --frozen-lockfile
bun x tsc --noEmit
bun test
bun run build
```

For UI, rendering, AppBar, Dock, or Genie behavior changes, also run the interactive Windows E2E acceptance suite on an unlocked desktop:

```powershell
.\tests\e2e\windows-e2e.ps1 -Scenario all
```

## Pull Request Guidelines

- Keep each PR focused on one coherent change (avoid mixing unrelated refactors, formatting sweeps, and feature work).
- Add or update focused unit/regression tests for behavioral changes and bug fixes.
- Update [`CHANGELOG.md`](CHANGELOG.md) under `## [Unreleased]` for user-visible changes.
- Never commit secrets, certificates, local `target/` output, or machine-specific artifacts.
