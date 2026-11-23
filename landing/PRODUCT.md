# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Pengguna Windows 11 yang menginginkan presisi spasial macOS (bar atas, Dynamic Island, Dock Genie, Focus Shield, Liquid Glass) secara native di Windows 11 tanpa beban Electron.

## Product Purpose

Otto OS v1.3.0 adalah shell desktop native Rust untuk Windows 11 22H2+ yang menyatukan Top Bar, Dynamic Island & Classic Notch (Focus, Screen Time, Music & Synced Lyrics, Pocket, Clip, Tasks, Calls, System HUD), Dock dengan efek Genie dan Downloads Stack, Top-Right Window Controls dengan Tiling, Focus Shield & 7-day Screen Time, Material Appearance Engine (4 gaya permukaan × 3 tema × 7 aksen WCAG), Control Center dengan Audio Output Switcher & DDC/CI, Otto Settings 13 modul dengan Process & Memory Buffer Audit serta pembaruan SHA-256 in-app, dan Spotlight lokal.

## Positioning

Ditulis dengan Rust 2024, Win32, Direct2D, DirectWrite, D3D11, dan GDI dalam satu proses modular. Semua berjalan lokal, tanpa akun, tanpa telemetri.

## Operating Context

- Unduhan: installer per pengguna `otto-os-setup-<versi>-x64.exe` (tanpa UAC), ZIP portabel, checksum `SHA256SUMS.txt`, dan manifest WinGet.
- Pintasan global: `Ctrl+Alt+,` (Otto Settings), `Ctrl+Alt+C` (Control Center), `Ctrl+Alt+Space` (Spotlight), `Ctrl+Alt+B` (Top Bar), `Ctrl+Alt+D` (Dock), `Ctrl+Alt+Q` (Quick Capture), `Alt+N` (Notch), `Alt+P` (Focus).
- Pemulihan: `--quit` dan `--restore-shell` mengembalikan taskbar dan area kerja Windows.

## Capabilities and Constraints

- Windows 11 22H2 atau lebih baru (build 22621+), x64. Windows 10 ditolak saat startup.
- Tanpa telemetri atau akun. Lirik daring opt-in. Quick Capture dan Background Cleanup default mati.
- Pocket menampung 12 item; Clip menyimpan 12 salinan hanya di RAM dan melewati format sensitif.
- Tidak diklaim: Microsoft Store (v1.1/mendatang), Apple ID/iCloud, AirDrop/AirPlay, kalender `.ics`, sandbox macOS, klaim CPU idle 0%.

## Brand Commitments

- Nama: Otto OS. Tanda: Orbit O (`assets/otto-os-orbit.svg`).
- Aksen biru `#075bef` (hover `#0441de`), obsidian `#000000` untuk notch & stage gelap, kanvas kertas `#fbfbfa` / `#ffffff` untuk bacaan editorial.
- Bahasa: Indonesia (default) dan Inggris dengan toggle ID/EN.
- Tanpa accordion FAQ, tanpa eyebrow/kicker, tanpa nomor section dekoratif, tanpa kartu fitur seragam.
