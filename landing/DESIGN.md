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
typography:
  sans: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, -apple-system, sans-serif"
radius:
  icon: "28%"
  card: "24px"
  panel: "32px"
  control: "9999px"
motion:
  curve: "cubic-bezier(0.16, 1, 0.3, 1)"
  island: "spring stiffness 420, damping 36, mass 0.9"
  reduced: "settle immediately, no transform transitions"
---

# Otto OS landing: arah desain

## Arah

- **Produk:** landing unduhan beta untuk Otto OS, shell desktop native Windows 11 dengan bar atas, Dynamic Island, Dock dengan efek Genie, Control Center, Pocket, Clip, dan Spotlight.
- **Mode:** Persuade. Pengunjung memutuskan untuk mengunduh. Desain adalah produknya, dan teks dibuat seminimal mungkin.
- **Karakter:** tenang, presisi, editorial. Kanvas terang untuk membaca; momen produk tampil di panggung gelap.
- **Visual signature:** diagram isometrik teknis yang terurai (bar, jendela, Dock, Control Center, notch) di hero dan bagian "Setiap bagian punya tugasnya". Diagram ini menggantikan mockup kartu dan ilustrasi ikon.
- **Satu momen gerak per bagian:** diagram terurai dan terangkai, island mengembang dan menukar aktivitas, jendela masuk ke Dock lalu kembali, switch dan slider Control Center, drag ke Pocket, navigasi keyboard di Spotlight.

## Batas klaim

Hanya klaim yang didukung CHANGELOG 1.0.0 dan README. Jangan menulis Microsoft Store, Apple ID/iCloud, AirDrop, berkas .ics, sandbox macOS, atau CPU idle 0%. Spotlight hanya mencari aplikasi terpasang, bukan berkas atau hasil web. Clip menyimpan 12 salinan terakhir, hanya di RAM.

## Bahasa

- Dua bahasa: Indonesia (default) dan Inggris. Toggle ID/EN ada di navigasi.
- Setiap teks memakai komponen `T` yang merender dua span (`.l-id`, `.l-en`). CSS pada `html[data-lang]` menampilkan satu dan menyembunyikan yang lain, sehingga screen reader hanya membaca bahasa aktif.
- Pilihan bahasa disimpan di `localStorage` (`otto-lang`). Script inline di `<head>` menerapkannya sebelum render untuk menghindari kedipan.

## Warna

- **Kanvas terang:** `page`, `paper`, `ink`, `inkSoft`, `muted`. Teks biasa minimal 4.5:1 di atas kanvas.
- **Aksen:** `accent` (biru merek Orbit O) untuk tombol unduh dan tautan. `accentHover` untuk hover. `accentSoft` untuk latar lembut.
- **Panggung gelap:** `stage` (#000) dan `stagePanel` untuk bagian gelap. `stageInk` dan `stageMuted` (#a8abb2 di atas #000, di atas 4.5:1). Garis panggung memakai putih 14%.
- **Aktivitas:** `focusActivity` (#ff6a2b) untuk indikator fokus dan cincin timer. `musicActivity` (#fc3c54) untuk ikon musik.
- **Jendela:** `windowMinimize`, `windowZoom`, `windowClose` hanya untuk tiga tombol di sudut jendela.
- **Diagram:** `wallpaper` dan `wallpaperEdge` untuk alas. Permukaan, tepi, dan detail part memakai token `parts*`. Jangan menambah warna baru tanpa mencatatnya di sini.
- Ikon aplikasi di Dock dan Spotlight memakai warna solid per aplikasi yang didefinisikan di komponen, dalam squircle.

## Tipografi

- Stack font sistem: Segoe UI Variable Display, lalu Segoe UI. Tidak ada font yang diunduh.
- Heading semibold dengan tracking -0.03em sampai -0.035em dan leading 0.98 sampai 1.04. Batas tracking -0.04em.
- Lead 18px, leading 1.6, lebar baca maksimal sekitar 65 karakter.
- Angka jam dan timer memakai `tabular-nums`.
- Tidak ada gradient text, eyebrow, atau nomor bagian.

## Tata letak

- Container maksimal 1200px. Padding samping 20px di mobile, 32px di sm dan lebih besar.
- Section memakai `py-24` dan `sm:py-32`. Bagian unduh memakai `py-28` dan `sm:py-40`.
- Urutan halaman: hero, bagian terurai (sticky 300svh), island di panggung gelap, Dock dengan Genie, Control Center split, Pocket dan Clip, Spotlight di panggung gelap, unduh, footer.
- Persyaratan satu baris di bagian unduh, tanpa FAQ dan tanpa daftar panjang.
- Bagian terurai memakai sticky stage dengan lima langkah. Indeks aktif mengikuti progres scroll.
- Tidak ada kartu fitur seragam atau ikon gaya kartu. Teks dan diagram mengambil peran kartu.

## Gerak

- Island memakai spring (stiffness 420, damping 36, mass 0.9). Transisi teks memakai `cubic-bezier(0.16, 1, 0.3, 1)` selama 0.28 detik.
- Diagram terurai di-spring. Nilai explode berubah dari scroll, dari menahan diagram hero (atau Spasi, Enter, panah saat fokus), dan dari geser ke kiri saat ditahan. Lepas untuk merakit kembali. Saat pertama tampil, diagram membuka sekali sebagai petunjuk.
- Genie: jendela meminimalkan ke ikon di Dock dan kembali lewat jalur yang sama.
- `prefers-reduced-motion`: `useReducedMotion` mengambil state akhir tanpa animasi. `scroll-behavior` dimatikan.
- Tidak ada scroll reveal dengan `opacity: 0` pada section (UG1).

## Komponen

- **Nav:** sticky, datar, tanpa blur. Tautan Bagian dan Unduh, toggle ID/EN berbentuk segmented dengan `aria-pressed`.
- **BetaDownload:** tombol (bukan tautan ke halaman rilis) yang langsung mengunduh installer terbaru. Sumber utama `/api/beta-downloads` (Cloudflare Pages Function, cache 5 menit). Jika itu gagal, script mengambil `releases/latest` dari GitHub API. Klik pertama saat data belum siap menampilkan "Menyiapkan unduhan…", lalu unduhan dimulai otomatis. Jika installer belum ada, tombol menjelaskan kondisinya. Jika data tidak bisa diambil, klik berikutnya mencoba lagi.
- **Diagram hero:** diagram itu sendiri adalah kontrolnya (`role="button"`, `aria-pressed`, dinamai oleh caption "Tahan untuk membongkar…"). Tidak ada tombol terpisah dan tidak ada tautan "Lihat bagiannya".
- **Island demo:** tiga keadaan (idle, compact, expanded). Tab Fokus dengan timer 25 menit dan tombol atur ulang, lewati, jeda. Tab Musik dengan progres dan lirik demo. Data sintetis.
- **Dock dan Genie:** wallpaper dengan jendela Explorer. Tombol kuning meminimalkan jendela ke ikon. Klik ikon untuk memulihkan. Dock membesar dengan kosinus saat kursor di atasnya; utilitas disembunyikan di mobile.
- **Control Center:** switch Wi-Fi, Bluetooth, Focus memakai `role="switch"`. Slider Kecerahan dan Suara memakai kelas `range-quiet`. Demo musik dengan play/pause.
- **Pocket dan Clip:** Pocket menerima seret-lepas dengan `motion` drag dan klik untuk menambah. Clip menampilkan Live/Paused, salin contoh, dan hapus item.
- **Spotlight:** pencarian dengan panah atas/bawah, Enter membuka pesan demo, Escape mengosongkan. Hanya aplikasi terpasang yang tampil.

## Aksesibilitas

- Skip link ke `#main-content`.
- Focus ring 2px dengan `accent` di terang dan `focusStage` di panggung gelap (`.on-stage :focus-visible`).
- Setiap kontrol ikon punya label `sr-only`. Ikon dekoratif memakai `aria-hidden`.
- Status unduhan memakai `aria-live="polite"`. Toggle bahasa dan tab memakai `aria-pressed`.
- Target sentuh minimal 44px untuk tautan navigasi dan footer.

## Struktur kode

- `src/styles/global.css`: token di `@theme` dan kelas utilitas (`range-quiet`, `squircle`).
- `src/layouts/Base.astro`: head, script bahasa, skip link.
- `src/components/lib/T.tsx`, `src/components/lib/spring.ts`: teks dua bahasa, easing, format jam.
- `src/components/ui/BrandMark.astro`: logo Orbit O.
- `src/components/islands/`: `ExplodedDiagram.tsx` (proyeksi isometrik), `HeroStack.tsx`, `PartsStory.tsx`, `IslandStage.tsx`, `DockGenie.tsx`, `ControlCenter.tsx`, `PocketClip.tsx`, `Spotlight.tsx`.
- `src/components/`: `Nav.astro`, `Footer.astro`, `BetaDownload.astro`.
- `src/sections/Hero.astro`: hero.
- `src/pages/index.astro`: susunan section. `src/pages/privacy.astro`: kebijakan privasi.
- `functions/api/beta-downloads.ts`: metadata installer beta dari GitHub Releases.

## Gerbang kualitas

- **UG1 Performa:** tanpa `opacity: 0` reveal. Island dengan `client:visible` kecuali HeroStack (`client:load`).
- **UG2 Aksesibilitas:** kontras WCAG 2.2 AA, skip link, focus ring terlihat, kontrol terlihat sejak HTML pertama.
- **UG3 Hasil tugas:** klik CTA langsung mengunduh installer terbaru (`otto-os-setup-<versi>-x64.exe`). Tidak ada pengalihan ke halaman rilis.
- **UG4 Build:** `scripts/build.ts` memverifikasi `<link rel="stylesheet">` di `dist/index.html` dan `dist/privacy/index.html`.
