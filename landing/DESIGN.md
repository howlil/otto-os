---
colors:
  page: "#fdfdfd"
  skyTop: "#a9d8f5"
  skyMid: "#d7eefc"
  surface: "#ffffff"
  ink: "#242424"
  inkSoft: "#525252"
  muted: "#686868"
  accent: "#ff6a2b"
  accentHover: "#e9571c"
  accentText: "#b83b0a"
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
  waitlistDialog: "28px"
motion:
  curve: "cubic-bezier(0.22, 1, 0.36, 1)"
  reduced: "settle immediately"
---

# Otto OS landing — design direction

## Direction

- **Product:** Landing pre-release dan waitlist beta untuk Otto OS, shell desktop Windows 11 berbahasa Rust yang menghadirkan bar atas, Dock, kontrol jendela Genie, Control Center, dan Dynamic Island.
- **Character:** Tenang, presisi, jujur secara teknis.
- **World:** Canvas editorial hampir putih (`#fdfdfd`) dan tipografi sistem yang tenang, dengan warna langit hanya di dalam product demo Windows. Hindari wallpaper halaman penuh, gradient dekoratif, dan floating navbar berlapis kaca.
- **UI rhythm:** Navbar datar dan ringan; heading medium-weight; satu tombol Join Beta berbentuk pill; satu preview desktop berbingkai tipis; section bawah memakai divider dan teks alih-alih kartu dekoratif.
- **Moments:**
  1. Pratinjau desktop tunggal di hero yang menampilkan jendela aktif tanpa kartu dekoratif, kontrol kanan-atas, bar atas Liquid Glass, Dynamic Island aktif, serta Dock aplikasi Windows.
  2. Empat pratinjau mikro sejajar pada daftar fitur utama (`Bar atas & Control Center`, `Dock & efek Genie`, `Kontrol jendela`, `Dynamic Island, Pocket & Clip`).
- **Signature:** Kapsul Dynamic Island hitam obsidian (`#000000`) dengan titik jingga fokus dan bar equalizer, dibatasi pada visual hero dan preview fitur Dynamic Island.

## Palette and contrast

- Semua pasangan teks berukuran biasa memenuhi batas minimal WCAG 2.2 AA (`4.5:1`).
- CTA pembuka waitlist memakai kapsul obsidian dengan teks putih; jingga `--color-accent: #ff6a2b` dipakai untuk submit (`Daftar beta`, `Simpan profil`) dengan teks gelap `--color-ink: #0f1115` (`6.61:1`).
- Tautan dan aksen teks di atas permukaan terang menggunakan `--color-accent-text: #b83b0a` (`> 6.3:1`).
- Muted teks memakai `--color-muted: #686868` di atas canvas putih; pastikan kontras minimal WCAG 2.2 AA.
- Logo menggunakan simbol geometris `Twin-T`; nama `Otto OS` mengikuti stack font sistem. Jangan mengimpor aset atau font merek dari website referensi.

## Frontend structure

- `src/domain/`: aturan validasi dan allowlist bersama antara UI dan fungsi API (`F2`).
- `src/data/`: klien `POST /api/waitlist` tanpa ketergantungan komponen UI (`F1`).
- `src/components/ui/`: komponen statis tanpa status bisnis atau `motion/react` (`Button`, `Input`, `Select`, `BrandMark`).
- `src/components/`: komponen seksi halaman dan state waitlist lokal (`F3`, `F4`). Formulir beta muncul dalam native `<dialog>` yang fokusnya dapat dikembalikan ke CTA; non-JS memakai form POST langsung.

## Quality gates

- **UG1 Performance:** tanpa `opacity: 0` scroll reveal pada section dan tanpa ketergantungan `motion` di jalur waitlist.
- **UG2 Accessibility:** WCAG 2.2 AA kontras, skip link `#main-content`, focus ring terlihat, dan kontrol aktif sejak SSR awal.
- **UG3 Task outcome:** CTA rounded-full memicu dialog; Escape, close, dan backdrop dapat menutupnya; pendaftaran baru, email duplikat, fallback non-JS form submit, dan error validasi tetap pulih tanpa menghapus input.
- **UG4 Build integrity:** `scripts/build.ts` menormalkan casing path Windows dan memverifikasi `<link rel="stylesheet">` pada seluruh halaman HTML yang terbit.
