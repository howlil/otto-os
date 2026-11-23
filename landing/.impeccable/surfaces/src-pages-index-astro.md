---
version: 1
slug: "src-pages-index-astro"
primary_target: "src/pages/index.astro"
related_targets:
  - "src/sections/Hero.astro"
  - "src/components/islands/HeroDesktop.tsx"
  - "src/components/islands/IslandStage.tsx"
  - "src/components/islands/TopBarStudio.tsx"
  - "src/components/islands/DockGenie.tsx"
  - "src/components/islands/FocusShowcase.tsx"
  - "src/components/islands/AppearanceAtelier.tsx"
  - "src/components/islands/ModuleRack.tsx"
  - "src/components/ui/AgentStack.astro"
  - "src/components/islands/Spotlight.tsx"
---

# Landing page surface brief

Scope: landing/src/pages/index.astro (v1.3.0 showcase update inside established DESIGN.md system). Mode: Persuade.

Audience: pengguna Windows 11 yang menyukai ritme desktop macOS dan menghargai rekayasa native Rust. Job: mengalami semua 13 modul dan fitur v1.3.0 secara interaktif tanpa scroll-jack repetitif, lalu mengunduh installer dari GitHub Releases.

Constraints: tetap di dalam design system `landing/DESIGN.md` (`#fbfbfa`, `#ffffff`, `#1b1b1d`, `#075bef`, `#000000`, palet native); dua bahasa ID/EN; hapus label tanpa dampak; tanpa shadow (elevasi memakai garis 1px); tidak ada AI slop atau klaim di luar CHANGELOG v1.0–v1.3.

## Direction contract

THESIS: Otto OS ditampilkan sebagai satu cerita produk dari atas ke bawah: sembilan bab, masing-masing menjawab satu pesan dengan satu panggung hidup (notch, bar, dock, focus, material, modul, spotlight, lalu unduhan). Menolak pola section berulang, bingkai desktop yang diulang di tiap bab, dinding kartu fitur seragam, dan teks panjang di tempat ilustrasi. Hanya hero yang berbentuk desktop.

OWN-WORLD: kertas editorial hangat (#fbfbfa) berpadu panggung obsidian murni (#000000), garis ink 1px, aksen Orbit (#075bef), dan permukaan native Windows Mica / Liquid Glass. Tanpa gradien teks, kicker eyebrow, nomor section, atau caption instruksi filler.

STORY: hero memperlihatkan shell hidup dalam lima detik; lalu sembilan bab berurutan memperkenalkan kemampuan v1.3.0 (Notch Screen Time dan Dynamic/Classic Island, Bar yang menyingkir dengan kalender dan Notification Center, Dock dan Genie, Focus Shield, 4 material × 7 aksen, 13 modul, Spotlight), dan penutup mengarahkan ke unduhan.

FIRST VIEWPORT: masthead terpusat ala Apple (badge jumlah unduhan dengan shine di atas, judul dua baris, ringkasan satu kalimat, tombol unduh biru), lalu kanvas desktop 16:9 Windows 11 + Otto OS yang hidup dengan lintasan shine.

FORM: narasi vertikal sembilan bab di dalam sistem desain Otto OS yang sudah ditetapkan (seed key a1606bad); kanvas terang dan panggung obsidian bergantian, satu momen gerak per bab.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.
