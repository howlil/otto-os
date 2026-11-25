## Summary

<!-- Describe the user outcome this change delivers and why. Link related issues below. -->

Closes #

## Expected-Pain Tier & Planning

- [ ] **Light** (reversible tweak, scoped core bug fix, or dark-launched behind `OTTO_EXPERIMENTAL_FLAGS`)
- [ ] **Medium** (feature slice behind existing ports, render refinement, legacy god-file split, or landing API)
- [ ] **Deep** (layout/UIA/port schema, persistent file/SQL migration, Win32 hooks/AppBar/recovery/updater, packaging, CI, or new dependency)

<!-- For Medium/Deep changes, paste a concise interface/flow table or link the aligned spec/issue/HIG reference. -->

## Changes

-

## AI Assistance & Human Ownership

- **AI assistance**: `None` / `Assisted` / `Agent-authored, human-audited`
- [ ] A human owner has read the entire diff, verified every line earns its place, and takes responsibility for production behavior.

## Verification

- [ ] `.\scripts\gate.ps1` (fmt, check, `clippy -D warnings`, `cargo-deny`, workspace tests + `F1`–`F12`, `check-e2e.ps1`, PowerShell syntax, and `landing/` checks + smoke)
- [ ] Interactive Windows check: `.\tests\e2e\windows-e2e.ps1 -Scenario <targeted|full>` or `.\scripts\release-verify.ps1` (required for UI, AppBar, Dock, Genie, hook, or rendering changes)
- [ ] Real-behavior evidence attached below (E2E report summary, screenshot, or verification log for Medium/Deep changes)

## Anti-Slop Checklist

- [ ] Focused diff: mechanical formatting/CRLF sweeps are not mixed with behavioral changes
- [ ] Crate boundaries and fitness ratchets (`F1`–`F12`) preserved without weakening budgets
- [ ] Rust source files under `crates/*/src/` remain comment-free (durable rationale recorded in `.agents/engineering.md` if needed)
- [ ] Uncertain or experimental work is isolated behind `OTTO_EXPERIMENTAL_FLAGS` (no speculative `plugins.txt` schema edits)
- [ ] `CHANGELOG.md` updated under `[Unreleased]` (for user-visible changes)
- [ ] No secrets, certificates, or machine-specific artifacts committed
