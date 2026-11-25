# Otto OS — Local Engineering & Software Factory Rules

## Scope

This repository is a Rust 2024 monorepo for **Otto OS** (a native Windows 11 desktop shell with Top Bar, Dynamic Island/Notch, Dock, Control Center, Spotlight, zero-overlay Window Controls, Genie, Focus Shield, and Otto Settings) plus its Astro/Cloudflare landing site (`landing/`), Windows installers/MSIX/WinGet packaging (`packaging/`), and interactive E2E suite (`tests/e2e/`).

Keep changes inside this workspace and preserve the crate boundaries in `.agents/engineering.md`.

## Stack & Verification Gate

- **Rust workspace**: Edition 2024 (`rust-toolchain.toml`), `windows` 0.62, `windows-numerics` 0.3, lints in `Cargo.toml` and `clippy.toml`, dependency policy in `deny.toml`.
- **Landing site**: Bun, Astro, React 19, Tailwind CSS, Biome (`landing/biome.json`), Cloudflare Pages Functions + D1 SQL (`landing/migrations/`).
- **Local bootstrap (once per clone)**: `.\scripts\setup-dev.ps1` (configures `core.hooksPath = .githooks` and checks local tools).
- **Canonical local gate**: `.\scripts\gate.ps1` — runs all required checks matching CI:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo deny check --all-features` (enforced in CI; install locally via `cargo install cargo-deny --locked`)
  - `cargo test -p otto-core -p otto-render -p otto-os -- --test-threads=1` (includes `host::fitness_tests` `F1`–`F12`)
  - `cargo test -p otto-platform -- --skip the_screen_grabber_returns_a_frame_of_the_requested_size --test-threads=1`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-e2e.ps1` + PowerShell AST syntax check
  - `landing/`: `bun x biome ci .`, `bun run check`, `bun x tsc --noEmit`, `bun test`, `bun run build`, `bun run smoke`
- **Interactive desktop & pre-release gate**: `.\scripts\release-verify.ps1 -E2EScenario full` (runs `gate.ps1`, `the_screen_grabber_returns_a_frame_of_the_requested_size`, all `13` `#[ignore]` native/handle-leak tests, release build, `tests/e2e/windows-e2e.ps1 -Scenario full`, and `packaging/msix/smoke-test.ps1`).

Run the relevant checks before declaring any change verified; a failed pre-existing gate remains a reported blocker and must never be hidden behind a weaker command.

## Expected-Pain Planning Policy

Before writing code, classify the task by **Expected Pain = (change likelihood) × (reversal/migration cost)**:

- **Deep** (`otto-core` layout/UIA/port schemas, persistent file/SQL formats, Win32 hooks/AppBar/recovery/process throttling, `updates.rs`, packaging, CI/CD, new dependencies): align on a concise spec + visual table/flow and rollback path with human sign-off before coding.
- **Medium** (new feature slices behind existing ports, render shaders/DPI changes, splitting legacy god files, `landing/functions/` endpoints): write a brief interface/edge-case/acceptance spec first.
- **Light** (isolated `landing/` tweaks, scoped `otto-core` bug fix with failing test, dark-launched prototype behind `OTTO_EXPERIMENTAL_FLAGS`): iterate quickly behind a reversible boundary.

## Codebase Design Rules

- **Comment-free Rust source**: `crates/*/src/` must contain zero `//`, `///`, `//!`, or block comments (`workspace_rust_sources_remain_comment_free`). Record durable rationale in `.agents/engineering.md`.
- **Single source of truth**: Layout metrics and hit targets live exclusively in `otto-core`; rendering (`otto-render`) and input dispatch (`otto-os`) consume the same resolved `Layout`.
- **Crate boundaries (`F1`–`F4`)**: `otto-os -> otto-render -> otto-core` and `otto-os -> otto-platform -> otto-core`. `otto-core` never imports Win32/render or uses `unsafe`; `otto-render` never invokes platform actions.
- **Ratchets (`F5`–`F12`)**: Never weaken `fitness_tests.rs` budgets (`#[allow]`, `unsafe`, file lines, sibling coupling, doc sizes) to make a change pass—refactor or split instead.
- **Dark-launch isolation**: Gate uncertain experiments behind `OTTO_EXPERIMENTAL_FLAGS` (`otto-core::runtime::flags`) rather than mutating `plugins.txt` v1 or user settings.
- **Error & resource discipline**: Use `Result` for startup/resource failures, `Option` for absence, RAII/`Drop` for every OS/graphics handle, and separate mechanical formatting sweeps from behavioral PRs.

## Safety & Project Memory

- Never commit secrets, certificates, user data, or `target/` output. Never force-push, publish, or alter external state without explicit authorization.
- **Live project memory** (keep each `<= 8 KB` / hard cap `12 KB`):
  - Architecture & contracts: `.agents/engineering.md` (history in `.agents/archive/decisions-v1.md`)
  - Product scope & invariants: `.agents/product.md` (history in `.agents/archive/product-v1.md`)
  - Native UI/HIG design rules: `.agents/DESIGN.md` (history in `.agents/archive/design-v1.md`)
  - Landing site contracts: `landing/PRODUCT.md` and `landing/DESIGN.md`
