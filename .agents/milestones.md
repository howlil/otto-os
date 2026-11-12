# Otto OS Milestones

Single index of milestone numbers. Every number from M00 to M13 appears once, even when it has no definition yet, so gaps stay visible instead of silently disappearing.

Rules:
- A number is never reused or renumbered. Sub-steps use a suffix (M07.6) under their parent.
- Status is one of: Done, Partial, Open, Undefined.
- Priority is one of: P1 (blocks the next release or is a verified defect), P2 (planned work with a clear scope), P3 (later or unscoped).
- Evidence points to a DEC in [engineering.md](engineering.md) or a commit. A milestone without evidence cannot be marked Done.

Last updated: 2026-11-12, branch `otto/m07-dock`.

## Priority order

| Priority | Milestone | Why | Next concrete step |
|---|---|---|---|
| P1 | M04 keyboard and accessibility | Tab, Dock focus, and capture ownership are committed (`a5c3b77`) but only unit-tested. The last live run did not show the bar handing focus to the Dock. | Run `keyboard-dock` with the desktop in front (the preflight now refuses otherwise). If it fails on a clean desktop, instrument the hook to see which keys reach Otto. |
| P1 | M05 visual reference | Harness exists (DEC-51), but the manifest is empty, so the scenario always fails. | Record references with `-RecordReference` on a clean desktop. Then add scale and wallpaper. |
| P2 | M07.6 Dock preferences | Code is done; the live settings menu has never been checked. | Check the Dock right-click menu live as part of the next desktop session. |
| P2 | G1 idle CPU | Target 0% is not met. Remaining cost is in the media and audio worker threads. DEC-52 halves idle media polling (not yet measured). | Measure idle CPU on a quiet desktop. Then replace the media poll with SMTC events. |
| P2 | M13 MSIX qualification | Not started. Blocks shipping, but has no detailed scope yet. | Write the qualification checklist (install, update, uninstall, shell restore) before any code. |
| P3 | M04 UI Automation | Deferred on purpose. Needs Narrator verification. | Separate slice after P1 is green. |
| P3 | M04 notch and popup keyboard focus | Not started. Popups poll keys and cannot take focus. | Decide whether popups should share the focus model. |
| P3 | M10.1 to M10.6 follow-ups | All Done. Nothing required. | None. |

## Full index

| Milestone | Name | Evidence | Status | Open items |
|---|---|---|---|---|
| M00 | Baseline and contracts | `dda651c` (Rust 1.97.1 pin, product and engineering contracts) | Done | None |
| M01 | Safety and resources | RAII render resources, pixel limits (DEC-31), shell recovery | Done | None |
| M02 | Lifecycle | AppBar registration, recovery, graceful shutdown (DEC-26, DEC-28) | Done | None |
| M03 | Bar model and window identity | DEC-27 | Done | None |
| M04 | Bar keyboard and accessibility | DEC-39, DEC-50, commits `a5c3b77` and `40824c8` | Partial | Live `keyboard-dock` run; UIA provider; notch and popup focus |
| M05 | Material and visual reference | DEC-40, DEC-41, DEC-42, DEC-51 (harness only) | Partial | Record references; scale and wallpaper; menus, notch, high contrast |
| M06 | Interaction model | Not separated in the notes; overlaps M03 and M04 | Undefined | Define the scope, or fold into M04 |
| M07 | Dock gate | DEC-47 | Done | None |
| M07.6 | Dock preferences | DEC-48 | Partial | Live check of the settings menu |
| M08 | Window controls and Genie | DEC-33, DEC-35 | Done | None |
| M09 | No evidence in repo | None | Undefined | Needs a definition or an owner |
| M10 | Media and Dynamic Island | DEC-43 to DEC-46 | Partial | M10.3 has no record |
| M10.1 | Media session model | DEC-43 | Done | None |
| M10.2 | Media commands | DEC-44 | Done | None |
| M10.3 | No record | None | Undefined | Needs a definition |
| M10.4 | Lyrics state | DEC-45 | Done | None |
| M10.5 | Seek | DEC-44 | Done | None |
| M10.6 | Island state machine | DEC-46 | Done | None |
| M11 | No evidence in repo | None | Undefined | Needs a definition or an owner |
| M12 | No evidence in repo | None | Undefined | Needs a definition or an owner |
| M13 | MSIX qualification | Not started | Open | Entire milestone |

## Cross-cutting work not tied to a number

- **G1 Idle CPU.** Target 0%. DEC-49: the UI thread is about 2.7 ms per second; the rest is in the media and audio worker threads.
- **G2 Render resource proof.** Done. Covered by `otto-render::resource_tests`.

## Open questions

- Do M09, M11, and M12 exist in a document or ticket outside this repo? If so, paste their definitions and this file gets updated.
- Is M13 a release blocker? That decides whether it moves ahead of G1.
