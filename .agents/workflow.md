# Otto OS Work Loop

One slice at a time. A slice is finished only when its gate is green and it is committed, or when the user has said not to commit.

## Start a slice

1. Name the milestone it belongs to from [milestones.md](milestones.md), or write "G1" or "G2" for the cross-cutting goals.
2. Write one measurable exit criterion before editing. Example: "UI thread under 5 ms/s in a sample taken with no input."
3. Keep the slice to one concern. If it grows, stop and split it.

## While working

- Build after each logical change, not after many files.
- Diagnostics (`eprintln!`, counters) are allowed for one measurement. Remove them in the same change, and check with `git grep DIAG` before committing.
- Do not start a second slice while the first has uncommitted changes.

## Finish a slice

1. Run `pwsh scripts/gate.ps1`. It must print `GATE PASSED`.
2. Measure against the exit criterion, and write the result in [engineering.md](engineering.md) as a DEC or an update to one.
3. Update the status in [milestones.md](milestones.md).
4. Commit with one clear message. Commits are made only after the user approves.

## Measurement notes

- Idle CPU depends on what the desktop is doing. Sample only when `GetLastInputInfo` shows no input, and compare against a baseline built from HEAD in the same session.
- Do not chase per-thread noise under about 15 ms of total CPU time. It is below the timer resolution.
