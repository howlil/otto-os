---
colors: {primary: "#F04438", surface: "#202124", text: "#F5F5F7", muted: "#A8ABB2"}
typography: {display: "SF Pro Display", body: "SF Pro Text"}
spacing: {base: "4px", control: "8px", section: "16px"}
radius: {control: "6px", shell: "18px", dock: "18px"}
motion: {window: "Genie snapshot warp", dockPresence: "160-220ms", iconBounce: "440ms", reduced: "settle immediately"}
components: {trafficLights: "native-window-follow", dockItem: "presence-progress", genie: "snapshot-overlay"}
---
## Direction

Otto is a Windows shell surface borrowing the spatial confidence of macOS without pretending to own external application state. Its character is quiet, precise, and responsive. Its world is a layered desktop with one native source of truth for each window and a temporary captured visual layer for Genie motion. Its signature moment is the application being pulled into its exact Dock tile instead of merely shrinking in place.

Avoid large web-style slides, scale-from-zero windows, decorative bounce on window frames, and competing animation engines.

## Guardrails

External HWND lifecycle state remains native Win32. Otto may mask minimize and restore with one temporary captured snapshot overlay; close remains native and does not route through the Dock.

Motion values live in `otto-core::motion`; app and render layers do not duplicate timing policy.

Every transition is time-based, reversible from its current value, and safe to settle immediately when reduced motion is enabled. Genie locks one Dock target, uses shared strip boundaries on one bounded overlay, and restores through the exact inverse path.

## State

Platform observation owns the truth about external windows. App state owns local animation progress. Core owns interpolation and resolved geometry.

Closing feedback must recover if the target window remains visible, for example because the application opened an unsaved-changes dialog.

## Accessibility

Read Windows client-area animation and UI-effects preferences. Reduced motion removes bounce and interpolation while preserving the same actions and final states.
