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

## Patterns

Notch media controls (Now Playing tab) hit at 24 px minimum; the visual track may be smaller and is centered in its hit rect. Controls in the expanded notch keep at least 4 px between hit rects and never cross the notch curve. Dimmed or unsupported controls must name the reason.

Liquid glass (bar and Dock, Dark and Light): the surface shows a blurred, saturation-boosted copy of what is behind it, tinted toward the theme, with a brighter rim on the upper-left edge, a faint sheen on the upper half, a soft inner shadow on the lower-right, and a one-pixel dark outline. Near the edge the surface bends toward sharp backdrop content (the lens band, 20 pt) while the interior stays soft so icons and text read on it. The bar is a rectangle and uses no lens. Dark and Light share one pipeline and differ only in `GlassParams`; primary text must keep 4.5:1 over the brightest and darkest possible backdrop. Battery Saver, disabled transparency, and high contrast replace glass with the solid `dock_fill` and `bar_background` colours. The notch stays obsidian in both themes.

## State

Platform observation owns the truth about external windows. App state owns local animation progress. Core owns interpolation and resolved geometry.

Closing feedback must recover if the target window remains visible, for example because the application opened an unsaved-changes dialog.

## Accessibility

Read Windows client-area animation and UI-effects preferences. Reduced motion removes bounce and interpolation while preserving the same actions and final states.

Notch controls join the bar keyboard order only while the expanded panel is open: the Pomodoro tab always, the Media tab and media controls only when media is present. Pomodoro play, reset, and skip are reachable while the Pomodoro tab shows. Tabs, lyrics toggle, previous, play/pause, next, and the seekbar are reachable on the Media tab; Left and Right seek by 5 s while the seekbar is focused. Each focused control draws a focus ring and activates through the same path as a click.
