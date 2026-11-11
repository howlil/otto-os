# Otto OS — local engineering rules

## Scope

This directory is a Rust 2024 workspace for a native Windows notch-style top bar. Keep changes inside this workspace and preserve the crate boundaries described in `.agents/engineering.md`.

## Stack and commands

- Rust edition 2024 with workspace lint settings in `Cargo.toml`.
- `windows` 0.62 for Win32, Direct2D, DirectWrite, GDI, and shell integration.
- `windows-numerics` 0.3 for render geometry.
- Format: `cargo fmt --all -- --check`.
- Test: `cargo test --workspace`.
- Build check: `cargo check --workspace`.
- Lint: `cargo clippy --workspace --all-targets --all-features`.

## Verification gate

Run formatting, workspace tests, workspace check, and Clippy before declaring a change verified. A Windows-only path must be checked on Windows. A failed pre-existing gate remains a reported blocker; do not hide it with a weaker command.

## Codebase design rules

- Source code is comment-free: do not add `//`, `///`, `//!`, or block comments. Put durable rationale in `.agents/engineering.md`.
- Prefer names, types, small functions, and tests that explain behavior. Do not replace missing explanation with broad abstractions.
- `otto-core` contains pure models, geometry, formatting, and tests. It must not depend on Windows or rendering.
- `otto-platform` owns all Win32 and shell integration. Keep FFI behind narrow safe functions and return domain values to callers.
- `otto-render` owns Direct2D, DirectWrite, GDI, brushes, geometry, and resource cleanup. It consumes core values and does not call platform actions.
- `otto-os` owns the window procedure, message dispatch, timers, runtime state, and orchestration. It coordinates crates but does not duplicate their policies.
- Layout metrics and hit regions have one source of truth in `otto-core`. Rendering and input dispatch consume the same resolved layout.
- Keep visual constants grouped by responsibility. Do not introduce a second copy of notch, slot, DPI, color, or timer values.
- Use `Result` for startup and resource failures, `Option` for absence, and explicit best-effort handling only at the runtime boundary.
- Keep unsafe code local to the platform or render boundary. Every owned OS or graphics resource must have an explicit release path.

## System boundaries

The dependency direction is `app → render → core` and `app → platform → core`; `core` never points outward. Native input enters through the Windows message procedure, is converted into core coordinates, and is dispatched to platform actions. Native status enters through `platform`, becomes `otto-core` data, and is rendered without another OS query.

Do not add persistence, networking, background services, or cross-crate global state without an explicit system contract and a matching verification path.

## Safety

Do not commit secrets or private data. Do not force-push, publish, deploy, delete user data, or change external state without explicit authorization. Keep generated `target/` output out of source changes.

## Project memory

- System contracts and durable architecture: `.agents/engineering.md`.
- Product behavior is represented by the current source and tests until a product contract is added.
