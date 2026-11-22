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
  notchUrgent: "#ff4245"
  notchMic: "#ff9f0a"
  notchCamera: "#30d158"
  notchDanger: "#ff453a"
  notchWarning: "#ff9f0a"
  wallpaper: "#dcebfb"
  wallpaperEdge: "#b5cde6"
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
typography:
  sans: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, -apple-system, sans-serif"
radius:
  icon: "28%"
  card: "24px"
  panel: "32px"
  control: "9999px"
motion:
  curve: "cubic-bezier(0.16, 1, 0.3, 1)"
  islandShape: "spring stiffness 439, damping 35, mass 1 (T 0.30 s, zeta 0.84, native expand)"
  islandCompact: "spring stiffness 584, damping 42, mass 1 (T 0.26 s, zeta 0.86, native compact)"
  islandSecondary: "spring stiffness 385, damping 31, mass 1 (T 0.32 s, zeta 0.78, native secondary)"
  reduced: "settle immediately, no transform transitions"
---

# Otto OS landing: arah desain

## Arah

- **Produk:** landing unduhan beta untuk Otto OS, shell desktop native Windows 11 dengan bar atas, notch (Dynamic Island), Dock dengan efek Genie, Control Center dengan Otto Settings, dan Spotlight.
- **Mode:** Persuade. Pengunjung memutuskan untuk mengunduh. Desain adalah produknya; teks dibuat sependek mungkin dan visual boleh berulang.
- **Karakter:** tenang, presisi, editorial. Kanvas terang untuk membaca; momen produk tampil di panggung gelap.
- **Visual signature:** diagram isometrik terurai di hero dan "Setiap bagian punya tugasnya", serta satu notch yang berubah modul saat digulir.
- **Satu momen gerak per bagian:** diagram terurai dan terangkai, bar atas mati dan notch tetap, notch berpindah modul, jendela masuk ke Dock lalu kembali, switch dan Otto Settings, Spotlight list/grid.

## Batas klaim

Hanya klaim CHANGELOG (1.0.0 dan 1.1.0) dan README. Overview enam area di dalam Control Center (1.0.0) sudah digantikan jendela Otto Settings terpisah (1.1.0, DEC-81). Otto Settings tidak punya kontrol tema (DEC-9). Quick Capture & Tasks ditulis sebagai opt-in (Ctrl+Alt+Q). Lirik daring hanya jika pengguna mengizinkan. Background Cleanup default mati. Jangan menulis Microsoft Store, Apple ID/iCloud, AirDrop, berkas .ics, sandbox macOS, atau CPU idle 0%. Spotlight hanya mencari aplikasi terpasang. Clip menyimpan 12 salinan terakhir, hanya di RAM. Semua data di demo bersifat sintetis dan diberi label.

## Bahasa

- Dua bahasa: Indonesia (default) dan Inggris. Toggle ID/EN ada di navigasi.
- Setiap teks memakai komponen `T` yang merender dua span (`.l-id`, `.l-en`). CSS pada `html[data-lang]` menampilkan satu dan menyembunyikan yang lain.
- Pilihan bahasa disimpan di `localStorage` (`otto-lang`). Script inline di `<head>` menerapkannya sebelum render.
- Label UI di dalam mock (menu, tab, tombol, Clip "Copied") memakai bahasa Inggris seperti aplikasi native.

## Warna

- **Kanvas terang:** `page`, `paper`, `ink`, `inkSoft`, `muted`. Teks biasa minimal 4.5:1 di atas kanvas.
- **Aksen:** `accent` (biru merek Orbit O) untuk tombol unduh utama, ring fokus, dan switch BarToggle (kontrol halaman). Mock native tidak memakai aksen ini.
- **Panggung gelap:** `stage` (#000) dan `stagePanel`. `stageInk` dan `stageMuted` di atas #000 memenuhi 4.5:1.
- **Notch (1:1 dengan native):** obsidian #000 untuk permukaan. Fokus `notchFocus` #ff9230, istirahat pendek `notchBreak` #30d158, istirahat panjang `notchLongBreak` #0091ff, urgen `notchUrgent` #ff4245, mikrofon `notchMic` #ff9f0a, kamera `notchCamera` #30d158, tombol tolak `notchDanger` #ff453a, peringatan HUD `notchWarning` #ff9f0a. Cover musik memakai `musicActivity`.
- `focusActivity` (#ff6a2b) tetap untuk indikator di luar notch. Di dalam notch dipakai `notchFocus` karena itu warna native. Pilih satu setelah review brand.
- **Diagram:** `wallpaper` dan `wallpaperEdge` untuk alas. Permukaan, tepi, dan detail part memakai token `parts*`. Jangan menambah warna baru tanpa mencatatnya di sini.
- Ikon aplikasi di Dock dan Spotlight memakai warna solid per aplikasi di komponen, dalam squircle.
- Mock Control Center dan Otto Settings memakai palet native (token `settings*` dan `controlCenterAccent` di front matter). Tint sidebar dan ikon modul mengikuti `tint()` di `crates/render/src/settings/palette.rs`. Ikon aplikasi di daftar Focus memakai abu-abu netral (`#3a3a3c`), karena ikon asli tidak ada di repo.

## Tipografi

- Stack font sistem: Segoe UI Variable Display, lalu Segoe UI. Tidak ada font yang diunduh.
- Heading semibold dengan tracking -0.03em sampai -0.035em dan leading 0.98 sampai 1.04.
- Lead 18px, leading 1.6, lebar baca maksimal sekitar 65 karakter.
- Angka jam, timer, dan meter memakai `tabular-nums`.
- Ukuran notch ikut native: timer panel 30px, judul musik 15px, artis 11px, label HUD 13px.
- Tidak ada gradient text, eyebrow, atau nomor bagian.

## Tata letak

- Container maksimal 1200px. Padding samping 20px di mobile, 32px di sm dan lebih besar.
- Section memakai `py-24` dan `sm:py-32`. Bagian unduh memakai `py-28` dan `sm:py-40`.
- Urutan halaman: hero; bagian terurai (sticky 300svh); bar atas (toggle); notch showcase (sticky 9 langkah, panggung gelap); Dock dengan Genie; Focus (panggung gelap, `#fokus`); Control Center dan Otto Settings (terang, `#kontrol`); Spotlight (panggung gelap); unduh; footer.
- Notch showcase: panggung 300px tinggi, lebar maksimal 1040px, setiap langkah 55svh. Di layar sempit panggung diskalakan agar panel 380px muat.
- Tidak ada kartu fitur seragam. Teks dan diagram mengambil peran kartu.

## Notch showcase (IslandStage)

- Dimensi Dynamic Island (default native): bar 40px, island 4px dari atas, compact 36px tinggi radius 16px.
- Focus compact 184px: cincin, empat langkah siklus, waktu 13px tnum. Music compact 292px: cover 20px, lirik tengah, equalizer 16px.
- Lingkaran sekunder 36px berjarak 8px dari notch utama (Focus saat Music aktif).
- Panel 380x144 radius 28. Lirik 254px dengan kartu 102px. HUD 356px. Panggilan 184px. Satu titik privasi 6px per waktu di bar (kamera hijau, mikrofon oranye). Prioritas kamera, lalu mikrofon, lalu lokasi, sesuai `visible_bar_dot` di `privacy.rs`.
- Langkah: Focus compact, Focus panel, Music + Focus, Music + lirik, Pocket, Clip, Tasks, Panggilan + titik privasi, HUD.
- Rail modul di bawah panggung: klik untuk melompat ke langkah pertama modul. Caption satu kalimat per langkah.
- Data sintetis: timer, lagu "Lampu Kota", daftar Pocket, Clip, Tasks, dan panggilan.

## Settings dan Focus

- **Focus (`#fokus`, panggung gelap):** `FocusPanel` dari notch dipakai langsung dan bisa diklik. Preset 15, 25, 45, dan 60 menit hanya berlaku saat sesi fokus idle. Empat titik menghitung sesi, lalu istirahat panjang (`pomodoro.rs`). HUD "Blocked" mengikuti `FocusBlocked` di `hud.rs`.
- **Halaman Focus:** judul, footer, dan label diambil dari `focus_page` di `settings_content.rs`. Preset blocklist dari `PRESET_BLOCKED_APPS` dan `PRESET_BLOCKED_WEBSITES` di `focus_activity.rs`. Daftar panjang disingkat (tiga aplikasi, dua situs), urutannya tetap.
- **Tidak ditampilkan:** "Most Searched Topics", karena isinya kueri pencarian. Angka demo diberi label "Data demo".
- **Kontrol (`#kontrol`, terang):** `SettingsScene` berisi jendela Otto Settings (sidebar 240 px, 13 modul, footer dari `modules_page`) dan Control Center 376 px. Ukurannya dari `layout_types.rs`: kartu 144, 64, dan 74 px; gap 8; padding 12; radius 26 dan 14.
- **Palet native:** terang `#f2f2f7`, sidebar `#e6e6eb`, kartu `#ffffff`, teks `#1d1d1f`, sekunder `#6e6e73`, switch `#34c759` dan `#d1d1d6`. Gelap `#1e1e20`, `#28282b`, `#2c2c2f`, `#f5f5f7`, `#a1a1a6`, switch `#30d158` dan `#5a5a5f`. Sumber: `crates/render/src/settings/palette.rs`.
- Tidak ada grafik, bar, atau angka yang tidak ada di UI native. Mock mengikuti urutan native, bukan dashboard generik.

## Gerak

- Island memakai spring native (lihat front matter). Konten berpindah dengan fade dan naik 8px selama 0.28 detik, dengan `cubic-bezier(0.16, 1, 0.3, 1)`.
- Scroll menentukan langkah notch. Tidak ada scroll reveal dengan `opacity: 0` pada section (UG1).
- Equalizer, gelombang panggilan, dan karet lirik berjalan. HUD berganti setiap 1.9 detik dan Clip menandai "Copied" setiap 2.2 detik.
- Diagram terurai di-spring. Nilai explode berubah dari scroll, dari menahan diagram hero, dan dari geser ke kiri saat ditahan.
- Genie: jendela meminimalkan ke ikon di Dock dan kembali lewat jalur yang sama.
- `prefers-reduced-motion`: `useReducedMotion` mengambil state akhir tanpa animasi. `scroll-behavior` dimatikan.

## Komponen

- **Nav:** sticky, datar, tanpa blur. Tautan Bagian dan Unduh, toggle ID/EN berbentuk segmented dengan `aria-pressed`.
- **BarToggle:** switch "Bar atas" menyembunyikan bar 40px. Notch tetap di tempatnya. Keycap Ctrl+Alt+B.
- **IslandStage (notch showcase):** lihat bagian Notch showcase. Rail memakai `aria-current="step"`, caption memakai `aria-live="polite"`.
- **Pocket, Clip, Tasks:** ada di dalam notch (`PocketClip.tsx`). Pocket 3x2 kartu per halaman, Clip 4 baris per halaman, Tasks 4 baris.
- **BetaDownload:** tombol (bukan tautan ke halaman rilis) yang langsung mengunduh installer terbaru. Sumber utama `/api/beta-downloads` (Cloudflare Pages Function, cache 5 menit). Jika itu gagal, script mengambil `releases/latest` dari GitHub API.
- **Dock dan Genie:** wallpaper dengan jendela Explorer. Tombol kuning meminimalkan jendela ke ikon. Klik ikon untuk memulihkan.
- **Control Center:** mengikuti layout main page native (`crates/core/src/control_center/layout.rs`). Kartu Wi-Fi dan Bluetooth (switch), tile Focus (switch; gear membuka halaman Focus), tile Battery (hanya tampilan, karena di native tile ini membuka Settings), kartu Display dan Sound dengan slider, lalu Now Playing dengan kontrol. Subtitle memakai copy native: "On", "Off", "Focus On", "Standard".
- **Otto Settings (jendela terpisah, 1.1.0):** sidebar 240 px dengan 13 halaman (60 px pada layar sempit). Modul dikelompokkan Surfaces, Notch tools, dan System dengan footer dari `modules_page`. Label dan subtitle diambil dari `ModuleId` di `crates/core/src/settings/settings_window.rs`. Quick Capture dan Background Cleanup mulai mati. Caption memakai gaya Windows. Tidak ada kontrol tema.
- **Spotlight:** pencarian dengan panah atas/bawah, Enter membuka pesan demo, Escape mengosongkan. Tombol list/grid untuk tampilan katalog aplikasi.

## Aksesibilitas

- Skip link ke `#main-content`.
- Focus ring 2px dengan `accent` di terang dan `focusStage` di panggung gelap (`.on-stage :focus-visible`).
- Setiap kontrol ikon punya label `sr-only`. Ikon dekoratif memakai `aria-hidden`.
- Status unduhan memakai `aria-live="polite"`. Toggle bahasa, tab, dan tampilan list/grid memakai `aria-pressed`.
- Target sentuh minimal 44px untuk tautan navigasi dan footer.

## Struktur kode

- `src/styles/global.css`: token di `@theme` dan kelas utilitas (`range-quiet`, `squircle`).
- `src/layouts/Base.astro`: head, script bahasa, skip link.
- `src/components/lib/T.tsx`, `src/components/lib/spring.ts`: teks dua bahasa, easing, format jam.
- `src/components/ui/BrandMark.astro`: logo Orbit O.
- `src/components/islands/IslandStage.tsx`: notch showcase (scroll, langkah, rail).
- `src/components/islands/notch/spec.ts`: ukuran, spring, warna, dan data demo notch.
- `src/components/islands/notch/Compact.tsx`: tampilan compact, sekunder, panggilan, HUD, titik privasi.
- `src/components/islands/notch/Panels.tsx`: tab, panel Focus, panel Music dan lirik.
- `src/components/islands/PocketClip.tsx`: panel Pocket, Clip, dan Tasks.
- `src/components/islands/BarToggle.tsx`: bar atas dan switch.
- `src/components/islands/Switch.tsx`: switch bersama.
- `src/components/islands/OttoSettings.tsx`: sidebar dan modul Otto Settings (13 modul dalam Surfaces, Notch tools, System).
- `src/components/islands/OttoWindow.tsx`: caption gaya Windows, sidebar, baris, dan palet Otto Settings.
- `src/components/islands/SettingsScene.tsx`: jendela Otto Settings dan Control Center di atas wallpaper.
- `src/components/islands/FocusShowcase.tsx`: panel Focus interaktif, preset, HUD, dan halaman Focus.
- `src/components/islands/ControlCenter.tsx`: Control Center 376 px.
- `src/components/islands/ExplodedDiagram.tsx`, `HeroStack.tsx`, `PartsStory.tsx`, `DockGenie.tsx`, `Spotlight.tsx`.
- `src/components/`: `Nav.astro`, `Footer.astro`, `BetaDownload.astro`.
- `src/sections/Hero.astro`: hero. `src/pages/index.astro`: susunan section. `src/pages/privacy.astro`: kebijakan privasi.
- `functions/api/beta-downloads.ts`: metadata installer beta dari GitHub Releases.

## Gerbang kualitas

- **UG1 Performa:** tanpa `opacity: 0` reveal. Island dengan `client:visible` kecuali HeroStack (`client:load`).
- **UG2 Aksesibilitas:** kontras WCAG 2.2 AA, skip link, focus ring terlihat, kontrol terlihat sejak HTML pertama.
- **UG3 Hasil tugas:** klik CTA langsung mengunduh installer terbaru (`otto-os-setup-<versi>-x64.exe`). Tidak ada pengalihan ke halaman rilis.
- **UG4 Build:** `scripts/build.ts` memverifikasi `<link rel="stylesheet">` di `dist/index.html` dan `dist/privacy/index.html`.
- **UG5 Notch 1:1:** ukuran, radius, dan warna notch mengikuti angka native di bagian Notch showcase. Perubahan native harus diikuti di sini.
- **UG6 Mock native:** judul, footer, label, dan urutan di Otto Settings dan Focus mengikuti `settings_content.rs`, `settings_window.rs`, dan `focus_activity.rs`. Ukuran Control Center mengikuti `layout_types.rs`. Perubahan di sumber itu harus diikuti di sini.
