---
colors: {primary: "#F04438", surface: "#202124", text: "#F5F5F7", muted: "#A8ABB2"}
typography: {display: "SF Pro Display", body: "SF Pro Text"}
spacing: {base: "4px", control: "8px", section: "16px"}
radius: {control: "6px", shell: "18px", dock: "18px"}
motion: {window: "Genie snapshot warp", dockPresence: "160-220ms", iconBounce: "440ms", reduced: "settle immediately"}
components: {trafficLights: "per-window-cover-and-own-native", dockItem: "presence-progress", genie: "snapshot-overlay"}
---

# Otto OS — Native Design Contract

> Active live visual & interaction guardrails (`<= 8 KB`). Full per-surface pixel specs live in [archive/design-v1.md](archive/design-v1.md). Web landing visual rules live in [landing/DESIGN.md](../landing/DESIGN.md).

## 1. Visual Direction & External Reference

Otto is a Windows 11 shell surface borrowing macOS's spatial clarity without pretending to own external application state: quiet, precise, responsive, and grounded in native Win32 truth.

- **Apple Human Interface Guidelines**: Consult live [Apple HIG (Designing for macOS)](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos) for affected patterns (`Layout`, `Materials`, `Motion`, `Accessibility`, `Sidebars`, `Charting data`). Windows conventions and accessibility settings take precedence when they conflict; record intentional departures in the PR and here.
- **Avoid**: Large web-style slides, scale-from-zero windows, decorative frame bounce, permanent control pills, competing animation engines, or text characters standing in for vector icons.

## 2. Materials & Surface Hierarchy

- **Transient shell surfaces (Top Bar, Dock, Control Center, Spotlight, Popup Menu)**: Support user-selectable appearance (`LiquidGlass`, `Frosted`, `Tinted`, `Solid`; `Auto`/`Light`/`Dark`; 7 Accent presets defaulting to `LiquidGlass + Dark + Blue`). Backdrop capture drives adaptive white/graphite ink hysteresis (`0.44`/`0.56`) with fixed-direction specular rim lighting (never pointer-tracking). Top Bar bleeds `22 pt` past top/left/right edges so only its bottom meniscus refracts.
- **Fallback priority (strict)**: Windows High Contrast > `AppearanceStyle::Solid` (zero backdrop capture) > Windows Reduce Transparency / Battery Saver / unavailable blur (opaque fallback) > selected glass material.
- **Center Notch / Dynamic Island**: Stays Obsidian Black (`#000000`) outside High Contrast with a directional PROLED specular rim.
- **Otto Settings (`OttoSettings`) & Otto Finder (`OttoFinder`)**: Standalone resizable windows using native Windows 11 Mica (`DWMSBT_MAINWINDOW = 2`) with neutral hairline-bordered surfaces following resolved Light/Dark theme and Accent color (falling back to solid surfaces under `Solid`, Transparency Off, High Contrast, or Battery Saver). Never double-stacks Acrylic or captured-backdrop refraction. `OttoFinder` adopts a `52 pt` unified toolbar with native traffic lights, collapsible `204 pt` sidebar, auto-appearing `28 pt` tab bar, 4 Finder view stages (`Icon`, `List` with `▸/▾` disclosure tree, `Column` Miller Columns + preview, `Gallery` stage + filmstrip), `24 pt` Path/Status bars, and in-window `Quick Look` (`Space`), `Get Info` (`Ctrl+I`), and `Go to Folder` (`Ctrl+Shift+G`) sheets.
- **Contrast invariants**: Calibrated accent `on_accent` ink guarantees WCAG AA (`>= 4.5:1`); Control Center card text sits on strengthened fills maintaining `>= 4.5:1` across adaptive ink states.

## 3. Geometry, Typography & Motion Guardrails

- **Geometry**: Strict `4 pt` grid, concentric radii (`R_inner = R_outer - d`), G2-continuous squircle corners (`1.528665 R` shoulder), and physical pixels scaled once from `otto-core::Metrics`. Layout rects and hit targets share one source of truth in `otto-core`; render never computes independent hitboxes.
- **Typography & Icons**: SF Pro typography via DirectWrite with character ellipsis trimming + clipping; OpenType tabular figures (`tnum`) mandatory for live clocks, timers, HUD percentages, seekbar timestamps, calendar numerals, Dock badges, and Screen Time KPI/chart readouts. All icons are drawn vector glyphs on unified stroke scales (cached in `otto-render::kit` / `appearance::tokens`).
- **Motion**: All timing, spring, and easing parameters live in `otto-core::motion`. Transitions are time-based, reversible mid-flight, and settle immediately under Windows Reduced Motion. Genie locks one Dock tile target, shares strip boundaries on one bounded overlay, and restores via the exact inverse curve.

## 4. Core Surface Rules

- **Top Bar**: `Brand -> App Menus` on left; Notch centered; `Tray Overflow -> Wi-Fi -> Volume -> Battery -> Search -> Control Center -> Notifications -> Date/Time` on right. Width pressure collapses hidden actions into explicit overflow; only Volume accepts wheel input; only one popup/Control Center stays open at once.
- **Window Controls**: Per-window non-topmost `OttoWindowControls` cover overlays sit directly above each external window's native `- □ ×` caption buttons with sampled row-band fills (`[Yellow, Green, Red]` on top-right, forwarding caption drag/double-click/context-menu/resize borders); `OttoSettings` and `OttoFinder` draw native top-left traffic lights inside their `WM_NCCALCSIZE` headers and are excluded from external overlays. Hover reveals internal glyphs (`×`, `−`, `+`/`⤢`), press-and-release cancels on pointer exit, and 600 ms Green hover / right-click opens Window Tiling.
- **Notch & Panels**: Expanded Notch (`416 pt` wide) hosts `Focus`, `Time` (`196 pt` height), `Shelf`, `Clip`, `Tasks` (`176 pt` inline editor), and `Music` (`254 pt` with centered 3-line lyrics drawer) tabs with `>= 24 px` hit targets and `>= 4 px` separation. Dual compact activities detach the secondary module into an Obsidian Black circle `8 pt` right of the Main Notch with click-to-swap and right-click dismiss.
- **Dock**: Always visible over windows by default unless Auto-Hide or Reserve Screen Space is chosen; hides for true fullscreen apps in all modes. Clicking the active app asserts focus (no minimize toggle).

## 5. Accessibility & Input Parity

- Read Windows animation, transparency, and high-contrast preferences dynamically.
- Full keyboard traversal with visible concentric focus rings and Windows UI Automation (`WM_GETOBJECT` / `IInvokeProvider`) across Bar, expanded Notch controls, Dock, Control Center, and Spotlight.
- Dimmed or unsupported media/window controls communicate their reason on both hover and keyboard focus.
