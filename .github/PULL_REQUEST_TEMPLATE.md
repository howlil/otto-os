## Summary

<!-- Describe what this change does and why. Link related issues below. -->

Closes #

## Changes

- 

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace -- --test-threads=1` (includes architecture fitness & comment-free checks)
- [ ] Interactive Windows check / `.\tests\e2e\windows-e2e.ps1` (if UI, AppBar, Dock, or rendering changed)
- [ ] `landing/` typecheck, tests, and build (if `landing/` changed)

## Checklist

- [ ] Crate boundaries preserved (`otto-os → otto-render/otto-platform → otto-core`)
- [ ] Rust source files under `crates/*/src/` remain comment-free (durable rationale documented in `.agents/engineering.md` if needed)
- [ ] `CHANGELOG.md` updated under `[Unreleased]` (for user-visible changes)
- [ ] No secrets, certificates, or machine-specific artifacts committed
