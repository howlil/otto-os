---
colors:
  page: "#fdfdfd"
  skyTop: "#a9d8f5"
  skyMid: "#d7eefc"
  surface: "#ffffff"
  ink: "#242424"
  inkSoft: "#525252"
  muted: "#686868"
  accent: "#075bef"
  accentHover: "#0441de"
  accentText: "#0441de"
  shell: "#202124"
  shellInk: "#f5f5f7"
typography:
  sans: "-apple-system, BlinkMacSystemFont, Segoe UI, ui-sans-serif, system-ui, sans-serif"
spacing:
  scale: [4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80]
radius:
  control: "8px"
  card: "18px"
  panel: "16px"
  cta: "9999px"
  waitlistDialog: "24px"
motion:
  curve: "cubic-bezier(0.22, 1, 0.36, 1)"
  reduced: "settle immediately"
---

# Otto OS landing — design direction

## Direction

- **Product:** Landing distribusi beta untuk Otto OS, shell desktop Windows 11 berbahasa Rust yang menghadirkan bar atas, Dock, kontrol jendela Genie, Control Center, dan Dynamic Island.
- **Character:** Tenang, presisi, jujur secara teknis.
- **World:** Canvas editorial hampir putih (`#fdfdfd`) dan tipografi sistem yang tenang, dengan warna langit hanya di dalam product demo Windows. Hindari wallpaper halaman penuh, gradient dekoratif, dan floating navbar berlapis kaca.
- **UI rhythm:** Navbar datar dengan tautan Fitur, Beta, dan FAQ yang tetap terlihat pada mobile; heading medium-weight; satu tombol unduh berbentuk pill beserta hitungan unduhan di hero; pratinjau desktop diikuti cerita fitur yang luas sebelum instruksi beta, dengan divider dan teks alih-alih kartu dekoratif.
- **Akses beta:** Section `#cara-kerja` menjelaskan platform, unduhan langsung dari GitHub Releases, dan langkah instalasi dalam dua kolom yang menumpuk pada mobile. Akses tidak memerlukan pendaftaran email.
- **Moments:**
  1. Pratinjau desktop tunggal di hero yang menampilkan jendela aktif tanpa kartu dekoratif, kontrol kanan-atas, bar atas Liquid Glass, Dynamic Island aktif, serta Dock aplikasi Windows.
  2. Lima adegan produk yang fokus pada bar dan Control Center, Dynamic Island, Dock dan Genie, kontrol jendela, lalu Pocket dan Clip. Setiap adegan memakai HTML interaktif dengan data demo lokal; ukuran dan state visual mengikuti kontrak Rust, dengan varian Light Glass sebagai pilihan preview.
- **Signature:** Kapsul Dynamic Island hitam obsidian (`#000000`) dengan titik jingga fokus dan bar equalizer, dibatasi pada visual hero dan preview fitur Dynamic Island.

## Component references

- **Navigation:** Follow Arc UI's `SiteHeader` simple composition: brand at the start and a short destination list. Keep Otto's flat white header, visible mobile links, and existing accent tokens; do not add the sticky glass treatment or mega menu.
- **Hero:** Follow Arc UI's screenshot-led hero hierarchy: concise headline, one primary action, then the product preview. Keep Otto's white editorial canvas and interactive desktop simulation; do not use the full-screen mesh variants.
- **Feature showcase:** Keep five ordered, alternating feature stories with one product illustration and explanation per story. Arc UI's `FeatureBento` and `ScrollStory` are Pro blocks with six-card or pinned-scroll behaviors, so they do not fit this page's explanatory flow.
- **FAQ:** Follow Arc UI's single-list accordion pattern. Keep the current native `details` implementation and Otto's divider, typography, and motion tokens.
- **Theme boundary:** Arc UI is a component reference; do not import its global foundation stylesheet. Map any adopted component styles to the existing Otto tokens in `src/styles/global.css`.

## Palette and contrast

- Semua pasangan teks berukuran biasa memenuhi batas minimal WCAG 2.2 AA (`4.5:1`).
- Biru logo Orbit O menjadi aksen utama: `--color-accent: #075bef` untuk tombol Join Beta dengan teks putih, `--color-accent-hover: #0441de` untuk hover, dan `--color-accent-text: #0441de` untuk tautan.
- Jingga `--color-focus-activity: #ff6a2b` khusus untuk indikator aktivitas fokus pada preview produk, terpisah dari warna merek.
- Muted teks memakai `--color-muted: #686868` di atas canvas putih; pastikan kontras minimal WCAG 2.2 AA.
- Logo menggunakan simbol `Orbit O` (cincin dengan dua mata) dalam squircle gradien biru; nama `Otto OS` mengikuti stack font sistem. Jangan mengimpor aset atau font merek dari website referensi.

## Frontend structure

- src/styles/otto-preview.css mirrors preview metrics from crates/core/src/layout.rs, dock.rs, window_controls.rs, control_center.rs, and crates/core/src/material.rs. src/scripts/showcase-interactions.ts only changes local demo state and never reads files or clipboard contents.

- `src/domain/`: aturan validasi dan allowlist bersama antara UI dan fungsi API (`F2`).
- `functions/api/beta-downloads.ts`: membaca installer beta dan jumlah unduhan dari GitHub Releases, lalu menyajikan metadata yang di-cache selama lima menit.
- `src/components/ui/`: komponen statis tanpa status bisnis atau `motion/react` (`Button`, `Input`, `Select`, `BrandMark`).
- `src/components/`: komponen seksi halaman; `BetaDownload.astro` menghubungkan CTA ke installer beta terbaru dan menampilkan jumlah unduhan installer dari seluruh rilis beta.

## Quality gates

- **UG1 Performance:** tanpa `opacity: 0` scroll reveal pada section dan tanpa ketergantungan `motion` di jalur unduhan.
- **UG2 Accessibility:** WCAG 2.2 AA kontras, skip link `#main-content`, focus ring terlihat, dan kontrol aktif sejak SSR awal.
- **UG3 Task outcome:** CTA mengunduh installer beta terbaru jika tersedia; jika belum ada rilis atau metadata gagal dimuat, status menjelaskan keadaan dan tautan rilis GitHub tetap dapat dibuka.
- **UG4 Build integrity:** `scripts/build.ts` menormalkan casing path Windows dan memverifikasi `<link rel="stylesheet">` pada seluruh halaman HTML yang terbit.
