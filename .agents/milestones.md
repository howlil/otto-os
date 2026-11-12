# Otto OS Milestones

Single index of milestone numbers. Every number from M00 to M13 appears once, even when it has no definition yet, so gaps stay visible instead of silently disappearing.

Rules:
- A number is never reused or renumbered. Sub-steps use a suffix (M07.6) under their parent.
- Status is one of: Done, Partial, Open, Undefined.
- Evidence points to a DEC in [engineering.md](engineering.md) or a commit. A milestone without evidence cannot be marked Done.

Last updated: 2026-11-12, branch `otto/m07-dock`.

| Milestone | Name | Evidence | Status | Open items |
|---|---|---|---|---|
| M00 | Baseline and contracts | `dda651c` (Rust 1.97.1 pin, product and engineering contracts) | Done | None |
| M01 | Safety and resources | RAII render resources, pixel limits (DEC-31), shell recovery | Done | None |
| M02 | Lifecycle | AppBar registration, recovery, graceful shutdown (DEC-26, DEC-28) | Done | None |
| M03 | Bar model and window identity | DEC-27 | Done | None |
| M04 | Bar keyboard and accessibility | DEC-39 (Ctrl+F2 focus mode), DEC-50 (Tab, Dock focus, capture audit) | Partial | UI Automation provider (deferred to its own slice), keyboard focus in the notch panel and popups, live keyboard E2E run |
| M05 | Material and visual reference | DEC-40, DEC-41, DEC-42 | Partial | Versioned visual reference set per surface, mode, scale, and wallpaper |
| M06 | Interaction model | Not separated in the notes; overlaps M03 and M04 | Undefined | Define the scope, or fold into M04 |
| M07 | Dock gate | DEC-47 | Done | None |
| M07.6 | Dock preferences | DEC-48 | Partial | Live check of the settings menu |
| M08 | Window controls and Genie | DEC-33, DEC-35 | Done | None |
| M09 | No evidence in repo | None | Undefined | Needs a definition or an owner |
| M10 | Media and Dynamic Island | DEC-43 to DEC-46 | Partial | M10.3 has no record; see sub-steps |
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

- **G1 Idle CPU.** Target is 0%. Current state is in DEC-49: the UI thread is about 2.7 ms per second, and the remaining cost is in the media and audio worker threads.
- **G2 Render resource proof.** Done. Covered by `otto-render::resource_tests`.

## Open questions

- Do M09, M11, and M12 exist in a document or ticket outside this repo? If so, paste their definitions and this file gets updated.
