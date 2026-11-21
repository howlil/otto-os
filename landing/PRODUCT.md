# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Pengguna Windows 11 yang menyukai ritme desktop macOS (bar atas, Dock, Dynamic Island) tetapi tetap ingin memakai Windows. Mereka datang dari pencarian atau tautan rilis, sudah tahu apa itu macOS, dan ingin tahu dalam beberapa detik apakah Otto OS layak dicoba. Inferred from repository docs; confirm when the audience changes.

## Product Purpose

Otto OS adalah shell desktop native untuk Windows 11 yang menata bar atas, Dynamic Island, Dock, kontrol jendela Genie, Control Center, Spotlight, Pocket, dan Clip. Landing ini berhasil jika pengunjung memahami mekanismenya dalam beberapa detik lalu mengunduh installer v1.0.0 dari GitHub Releases tanpa email.

## Positioning

Otto OS ditulis dengan Rust dan Win32, Direct2D, DirectWrite, serta GDI, bukan Electron. Mekanisme yang tidak mudah ditiru: jendela yang diperkecil terbang masuk ke tepat ikon Dock-nya (efek Genie), dan Dynamic Island menukar dua aktivitas (Focus dan Music) di notch. Semua berjalan lokal.

## Operating Context

- Unduhan: installer per pengguna `otto-os-setup-<versi>-x64.exe` (tanpa UAC), arsip ZIP portabel, dan manifest WinGet.
- Menu Otto dan Control Center membuka Otto Settings; jalan pintas global `Ctrl+Alt+Space` (Spotlight), `Ctrl+Alt+C` (Control Center), `Ctrl+Alt+N` (notch).
- Pemulihan: `--quit` dan `--restore-shell` mengembalikan taskbar serta area kerja Windows.

## Capabilities and Constraints

- Windows 11 22H2 atau lebih baru (build 22621+), arsitektur x64. Windows 10 diblokir saat start.
- Tanpa telemetri, analitik, atau akun. Lirik tersinkron daring hanya aktif bila pengguna mengizinkannya. Quick Capture dan Background Cleanup default mati.
- Pocket menampung 12 item. Clip menyimpan 12 salinan terakhir hanya di RAM dan melewati format clipboard sensitif.
- Belum atau tidak diklaim: Microsoft Store (ditunda ke v1.1), Apple ID/iCloud, AirDrop/AirPlay, sinkronisasi kalender `.ics`, emulasi sandbox macOS.
- Status v1.0.0 diterbitkan 2026-11-21 (CHANGELOG). Jangan menulis klaim CPU idle 0% sebagai fakta.

## Brand Commitments

- Nama: Otto OS. Tanda: Orbit O (`assets/otto-os-orbit.svg`), squircle gradien biru dengan cincin putih dan dua mata.
- Aksen biru `#075bef` (hover `#0441de`). Jingga `#ff6a2b` khusus indikator fokus. Obsidian `#000000` untuk notch.
- Binding dari pengguna: canvas terang editorial dengan stage gelap untuk bagian produk. Hindari wallpaper halaman penuh dan kartu dekoratif.
- Bahasa: Indonesia dan Inggris dengan pemilih bahasa. Bahasa Indonesia default.
- Tanpa accordion FAQ. Persyaratan tampil satu baris di bagian unduh.

## Evidence on Hand

- `CHANGELOG.md` (1.0.0) dan `README.md` sebagai sumber fitur.
- Jumlah unduhan dan versi terbaru dari GitHub Releases melalui `functions/api/beta-downloads.ts`.
- Tidak ada logo pelanggan, testimoni, benchmark, atau harga. Jangan dibuat-buat.
- Screenshot di `artifacts/e2e` adalah hasil uji lokal dan dapat memuat data pribadi, sehingga tidak dipakai di halaman publik. Demo di landing memakai data sintetis yang diberi label.

## Product Principles

1. Tunjukkan mekanismenya bergerak, jangan jelaskan dengan paragraf.
2. Setiap klaim harus sesuai README dan CHANGELOG yang sudah dirilis.
3. Lokal dan native: tanpa akun, tanpa telemetri; satu-satunya panggilan jaringan adalah unduhan metadata dan pemeriksaan pembaruan saat pengguna memintanya.
4. Teks pendek: satu kalimat per gagasan.

## Accessibility & Inclusion

- Kontras teks memenuhi WCAG 2.2 AA; setiap demo dapat dijalankan dengan keyboard dan memiliki fokus yang terlihat.
- `prefers-reduced-motion` menuntaskan animasi seketika tanpa menghilangkan tindakan.
- Tautan lewati ke `#main-content` tetap ada.
