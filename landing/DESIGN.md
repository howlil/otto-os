---
colors:
  page: "#fbfbfa"
  paper: "#ffffff"
  ink: "#1b1b1d"
  inkSoft: "#46464c"
  muted: "#66666c"
  accent: "#075bef"
  accentHover: "#0441de"
  accentSoft: "#e8f1ff"
  logoTop: "#126eeb"
  logoBottom: "#0441de"
  logoLight: "#2c8ffc"
  focusStage: "#75b0ff"
  focusActivity: "#ff6a2b"
  musicActivity: "#fc3c54"
  positive: "#15803d"
  stage: "#000000"
  stagePanel: "#121214"
  stageInk: "#f5f5f7"
  stageMuted: "#a8abb2"
  notchFocus: "#ff9230"
  notchBreak: "#30d158"
  notchLongBreak: "#0091ff"
  notchScreenBlue: "#0a84ff"
  notchUrgent: "#ff4245"
  notchMic: "#ff9f0a"
  notchCamera: "#30d158"
  notchLocation: "#0a84ff"
  notchDanger: "#ff453a"
  notchAccept: "#24a148"
  notchWarning: "#ff9f0a"
  wallpaper: "#dcebfb"
  wallpaperMid: "#d4e6fa"
  wallpaperEdge: "#b5cde6"
  wallpaperDuskTop: "#162138"
  wallpaperDuskMid: "#101522"
  wallpaperDuskEdge: "#0b0d14"
  coverLight: "#ffb0b8"
  coverDeep: "#4b1224"
  screenBlueMid: "#3da0ff"
  screenBlueLight: "#6bb7ff"
  partsTeal: "#30b0c7"
  partsPurple: "#af52de"
  partsMint: "#5ec9a7"
  partsDarkCard: "#323238"
  windowMinimize: "#febc2e"
  windowZoom: "#28c840"
  windowClose: "#ff5f57"
  partsEdge: "#cdd5e0"
  partsNotchEdge: "#2a2a2e"
  partsSurface: "#f1f3f7"
  partsSurfaceSoft: "#f6f7f9"
  partsTile: "#e4edff"
  partsTrack: "#e5e8ee"
  partsControl: "#d7dce5"
  partsGlyph: "#8d939c"
  partsSlate: "#8fa3b8"
  partsBlue: "#5b9cff"
  partsGreen: "#7fd17a"
  partsAmber: "#ffb36b"
  partsViolet: "#a78bfa"
  partsRose: "#ff7a8a"
  partsFill: "#ffffff"
  settingsBody: "#f2f2f7"
  settingsSidebar: "#e6e6eb"
  settingsCard: "#ffffff"
  settingsText: "#1d1d1f"
  settingsSecondary: "#6e6e73"
  settingsSwitchOn: "#34c759"
  settingsSwitchOff: "#d1d1d6"
  settingsDarkBody: "#1e1e20"
  settingsDarkSidebar: "#28282b"
  settingsDarkCard: "#2c2c2f"
  settingsDarkText: "#f5f5f7"
  settingsDarkSecondary: "#a1a1a6"
  settingsDarkSwitchOn: "#30d158"
  settingsDarkSwitchOff: "#5a5a5f"
  controlCenterAccent: "#007aff"
  wallpaperDaylightGlow: "#8fb9e8"
  wallpaperDuskShade: "#3a62a8"
  wallpaperDuskDeep: "#1e3056"
typography:
  sans: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, -apple-system, sans-serif"
radius:
  icon: "26%"
  card: "24px"
  panel: "32px"
  control: "9999px"
elevation: "none; 1px hairline border (rgb(27 27 29 / 0.11)) instead of shadow"
motion:
  curve: "cubic-bezier(0.16, 1, 0.3, 1)"
  islandShape: "spring stiffness 439, damping 35, mass 1 (T 0.30 s, zeta 0.84, native expand)"
  islandCompact: "spring stiffness 584, damping 42, mass 1 (T 0.26 s, zeta 0.86, native compact)"
  islandSecondary: "spring stiffness 385, damping 31, mass 1 (T 0.32 s, zeta 0.78, native secondary)"
  reduced: "settle immediately, no transform transitions"
---

# Otto OS landing: arah desain (v1.3.0)

## Arah

- **Produk:** landing unduhan resmi Otto OS v1.3.0, shell desktop native Rust untuk Windows 11 22H2+ (Top Bar, Dynamic Island & Classic Notch, Screen Time, Dock Genie, Top-Right Window Controls & Tiling, Focus Shield, Material Appearance Engine, Control Center, Otto Settings dengan Process Audit & SHA-256 Updates, serta Spotlight).
- **Mode:** Persuade. Pengunjung mencoba mekanisme shell langsung di halaman lalu mengunduh installer dari GitHub Releases.
- **Karakter:** tenang, presisi, arsitektural. Kanvas kertas terang (`#fbfbfa` / `#ffffff`) berganti panggung obsidian (`#000000`) dengan komposisi berbeda di tiap bagian — tanpa pola kartu berulang dan tanpa scroll-jack panjang.
- **Visual signature:** kanvas desktop di hero dengan bingkai rasio layar 16:9 (4:3 di mobile): notch Classic dengan timer dan telinga 6 px, popover notifikasi dan kalender, jendela kode, dan dock yang membesar mengikuti kursor, plus shine di badge unduhan dan tile. Di bawahnya panggung Notch 1:1 dengan pemilih bezel `Dynamic Island | Classic Notch` dan atelier material `Liquid Glass | Frosted | Tinted | Solid`.

## Batas klaim

Hanya klaim dari `CHANGELOG.md` (v1.0.0–v1.3.0), `README.md`, dan implementasi aktual di `crates/`:
- 13 modul di `Otto Settings` (`Surfaces`, `Notch tools`, `System`), dengan `Quick Capture` dan `Background Cleanup` default mati.
- Appearance v1.3.0: 4 gaya permukaan (`Liquid Glass`, `Frosted`, `Tinted`, `Solid`), 3 mode tema (`Auto`, `Light`, `Dark`), 7 aksen terkalibrasi WCAG (`System`, `Blue`, `Purple`, `Rose`, `Amber`, `Emerald`, `Graphite`), serta toggle `Visible in Screenshots / Screen Share`.
- Window Controls menutupi pojok kanan atas (`[Yellow, Green, Red]`) dengan menu Tiling dan sudut membulat Windows 11.
- Focus v1.2/v1.3: Focus Shield dengan tombol `Back` (tidak menutup paksa aplikasi/tab), tab `Block apps` dan `Activity` 7 hari, serta tab `Time` di Notch (`416 × 196 px`).
- Process & Memory Buffer Audit di `Otto Settings → General`: klasifikasi 3 tingkat (`SafeBackground`, `ActiveWindow`, `SystemProtected`), toggle manual `EcoQoS`/`Restore`, dan `End Task` aman.
- Pembaruan in-app di `Otto Settings → About` dengan verifikasi `SHA256SUMS.txt` dan `/RELAUNCH` tanpa polling latar belakang.
- Jangan mengklaim Microsoft Store, Apple ID/iCloud, AirDrop, `.ics`, sandbox macOS, atau CPU idle 0%.

## Bahasa

- Dua bahasa: Indonesia (default) dan Inggris melalui komponen `T` (`.l-id` / `.l-en`) dengan penyimpanan `localStorage("otto-lang")`.
- Label UI di dalam mock native memakai bahasa Inggris sesuai aplikasi Rust.
- Tidak ada label instruksi yang tidak berdampak (hindari caption tutorial panjang, pengulangan subjudul, atau label placeholder generik).

## Tata letak & ritme

- Container maksimal `1200px`, padding `20px` mobile / `32px` sm+. Bagian bergantian antara kanvas terang dan panggung obsidian; tiap bagian menjawab satu pesan dan punya satu momen gerak.
- Sembilan bab, dari atas ke bawah:
  1. **Hero (`Hero.astro` + `HeroDesktop.tsx`):** badge jumlah unduhan dengan shine (`.badge-sheen`, 5,5 detik) berada di atas judul dua baris (`Layar yang diam, kerjamu yang jalan.`); ringkasan dan CTA biru dipusatkan. Kanvas desktop 16:9 yang dibatasi `max-w-[1040px]` punya shine sendiri (`.hero-sheen`, 6,5 detik) dan ukuran UI 1:1 dengan metrik native (UG8).
  2. **Notch (`#notch`, obsidian):** `IslandStage` dengan toggle bezel `Dynamic Island | Classic Notch` dan 9 ruang kerja (`Focus`, `Screen Time`, `Musik & Lirik`, `Dual Split`, `Pocket`, `Clip`, `Tasks`, `Panggilan & Privasi`, `System HUD`) yang berganti otomatis saat terlihat.
  3. **Bar (`#bar`, kanvas terang):** `TopBarStudio` — bar atas menyingkir, muncul saat kursor menyentuh tepi atas, lalu memperlihatkan kalender bulanan dan Notification Center; keycap `Ctrl + Alt + B`.
  4. **Dock (`#dock`, paper):** `DockGenie` dengan pemilih `Selalu tampil | Auto-Hide | Reserve Space`; tombol kuning memicu animasi Genie dan Dock membesar mengikuti kursor.
  5. **Focus (`#fokus`, obsidian):** `FocusShowcase` — timer Focus dan Focus Shield dengan tombol `Back`; tidak ada aplikasi yang ditutup paksa.
  6. **Material (`#tampilan`, kanvas terang):** `AppearanceAtelier` — Control Center di atas kertas dengan cakram aksen sebagai latar yang dibiaskan kaca; empat gaya permukaan bergantian otomatis, sementara mode tema dan 7 aksen tetap bisa dipilih.
  7. **Modul (`#modul`, paper):** `ModuleRack` — 13 modul sebagai toggle; `Quick Capture` dan `Background Cleanup` mulai dalam keadaan mati.
  8. **Spotlight (`#spotlight`, obsidian):** `Spotlight` — `Ctrl+Alt+Space` untuk `shell:AppsFolder`, tampilan daftar atau kisi, dengan demo mengetik lalu meluncurkan aplikasi.
  9. **Unduh (`#unduh`, kanvas terang):** `AgentStack` (ilustrasi CSS dari glyph wajah logo, bukan foto, tanpa klaim fitur AI) sebagai penutup, dengan CTA `BetaDownload` bergradasi logo (`.surface-logo`).
- Wallpaper (`.wallpaper-daylight` dan `.wallpaper-dusk`) hanya dipakai Hero, karena hanya Hero yang berbentuk desktop. Overlay radialnya `rgb(143 185 232 / 0.6)`, `rgb(58 98 168 / 0.45)`, dan `rgb(30 48 86 / 0.65)`, dengan padanan hex di frontmatter.
- Bab lain tidak meniru bingkai desktop (wallpaper, jendela, menu bar palsu). Bar, Dock, dan Material tampil di atas kertas `bg-page`/`bg-paper`; Notch, Focus, dan Spotlight di atas obsidian.

## Gerbang kualitas

- **UG1 Performa:** tanpa `opacity: 0` reveal pada section. Island memakai `client:visible` kecuali `HeroDesktop` (`client:load`). Reveal `data-reveal` hanya aktif saat `js-reveal` terpasang; elemen reveal di dalam island memakai `suppressHydrationWarning` karena IntersectionObserver menambahkan `is-in`.
- **UG2 Aksesibilitas:** WCAG 2.2 AA, skip link, focus ring terlihat di terang & gelap, keyboard navigable, `prefers-reduced-motion` menuntaskan gerak seketika.
- **UG3 Hasil tugas:** CTA `BetaDownload` langsung mengunduh installer `.exe` terbaru dari `/api/beta-downloads` (fallback GitHub API). Di `bun run dev` endpoint itu (Cloudflare Pages Function) tidak aktif, sehingga 404 di konsol dev diharapkan; fallback GitHub API dipakai. Badge jumlah unduhan (`[data-beta-count]` di `Hero.astro`) adalah total unduhan installer `otto-os-setup-*-x64.exe` dari semua rilis non-draft; fallback dev menghitung angka yang sama dari daftar rilis dan menyembunyikan badge bila daftar tidak lengkap.
- **UG4 Build:** `scripts/build.ts` memverifikasi `<link rel="stylesheet">` di `dist/index.html` dan `dist/privacy/index.html`.
- **UG5 Notch 1:1:** dimensi mengikuti `Metrics` di `crates/core/src/layout/metrics.rs` (`416 px` expanded, `196 px` Screen Time, `254 px` lyrics, `356 px` HUD).
- **UG6 Mock native:** urutan, label, dan perilaku mengikuti `settings_window.rs`, `settings_content.rs`, `appearance_settings.rs`, `focus_settings.rs`, `general_settings.rs`, `notification_center.rs`, dan `calendar.rs`.
- **UG7 Gerak:** satu momen gerak per bab. Panggung autoplay hanya berjalan saat terlihat, berhenti begitu pengguna mengendalikannya, dan `prefers-reduced-motion` menampilkan frame diam. Hero membawa dua shine (`.badge-sheen` dan `.hero-sheen`), dan bagian unduh membawa shine di kartu depan `AgentStack`; semuanya disembunyikan saat reduced motion.
- **UG8 Mock hero (1:1):** `HeroDesktop.tsx` memakai metrik native dari `crates/core/src/layout/metrics.rs`: bar 32 px (`MACBOOK_PRO_14`, gaya Classic Notch bawaan), slot bar dari `MACBOOK_PRO_14`, font 12 px, ikon 15 px. Caption jendela 30 px dengan radius 8 dan titik 12 px berjarak 20 px (`crates/core/src/window/controls/cover.rs`, `layout.rs`). Dock dari `DockMetrics::MACOS_SONOMA` (`crates/core/src/dock/layout.rs`), dengan konstanta di `islands/dock/spec.ts`, rumus di `islands/dock/layout.ts`, dan ilustrasi tile di `islands/dock/DockArt.tsx`: slot 48 px berjarak 8 px, pil 68 px dengan padding 8/12/10 px, radius 18 px, dan 6 px dari bawah; divider 14 px; titik 4 px (aktif 5×4 px) 8,6 px di bawah slot; tooltip 26 px berjarak 10 px di atas ikon dengan panah 5 px. Magnify kosinus-kuadrat 1,45 dalam radius 2,6 slot dari pusat dasar dengan reflow. Squircle ikon mengisi 220/256 slot dengan rim putih 30 % (`crates/render/src/dock/renderer.rs`). Bayangan jatuh native dihilangkan karena aturan tanpa shadow. Di bawah 640 px dock diskalakan 0,8 dan dua slot terakhir disembunyikan; itu adaptasi responsif, bukan metrik native. Notifikasi 328 px dengan radius 20 (`crates/core/src/menu/notification_center.rs`). Notch Classic 200×32 dengan sudut bawah 10 px dan telinga 6 px dari `notch/spec.ts` (`classicFocusWidth`, `classicCompactHeight`, `classicCompactRadius`, `classicEarRadius`), setara `notch_ear_radius` di `crates/render/src/notch/shell.rs`. Popup di mock berjarak 8 px di bawah bar (`top-10`). Jika metrik native berubah, ubah konstanta di tempat yang sama.
