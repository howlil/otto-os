# Media assets for the README showcase

Otto OS does not currently ship real product screenshots or recordings in this directory. Capture them on Windows 11 before embedding them into the [main README](../../README.md); do not use concept renders as evidence of the application.

## Recommended captures

| Filename | What to capture | Suggested output |
| --- | --- | --- |
| `hero-desktop.webp` | Full desktop showing the Otto Top Bar, Dynamic Island, Dock, and native Windows context | 1920×1080 or 2560×1440 WebP |
| `dynamic-island.gif` | A short, cropped loop showing actual expand/collapse, Focus and Media | GIF under 5 MB, or link to a hosted video |
| `control-center.webp` | Real Control Center popover, with readable system controls | 1280×800 WebP |
| `focus-screen-time.webp` | Focus mode or Screen Time interface | 1280×800 WebP |
| `otto-settings.webp` | Standalone settings window with Modules or Appearance | 1280×800 WebP |

## Capture and publishing requirements

- In **Otto Settings → General**, enable **Visible in Screenshots / Screen Share** so `WDA_EXCLUDEFROMCAPTURE` does not hide Otto surfaces.
- Use a clean desktop at 100% or 150% scaling. Remove personal notifications, account names, secrets and unrelated applications.
- Prefer a sharp still screenshot over a large GIF. Use short motion previews only when behavior needs movement to be understood.
- Inspect the captured media at GitHub README width, optimize file sizes, and add descriptive `alt` text.
- Once the real files exist, add a hero image directly below the README header and provide a compact, optional demo gallery. Do not add image links to files that have not been committed.
- Preserve the accessible text-based links to the [official interactive product demo](https://otto-os-landing.pages.dev) even after adding images.
