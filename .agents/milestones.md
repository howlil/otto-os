# Otto OS: fitur yang berdampak

Daftar pendek. Hanya memuat hal yang dirasakan pengguna atau mencegah kerusakan nyata. Detail teknis ada di [engineering.md](engineering.md).

## Sudah berdampak

- **Keamanan dan resource (M01):** mencegah kebocoran dan crash.
- **Pemulihan shell (M02):** taskbar dikembalikan kalau Otto mati mendadak.
- **Kontrol window dengan aman (M03):** perintah hanya dikirim ke window yang masih ada.
- **Keyboard bar (M04):** Ctrl+F2, Tab, Shift+Tab, Enter, dan Escape. Sudah dicek manual.
- **Dock (M07):** overflow "N More Apps", tampilan saat window menutupi Dock.
- **Kontrol window dan Genie (M08):** tombol merah, kuning, dan hijau, serta animasi minimize.
- **Musik dan lirik (M10):** kontrol pemutar, seek, lirik, dan status Dynamic Island.
- **Tampilan konsisten (M05, bagian inti):** warna bar, notch, Dock, dan menu dicek di desktop bersih.
- **Tes render (G2):** mencegah kebocoran handle grafis.

## Belum, dan hanya penting kalau diperlukan

- **Paket MSIX (M13):** perlu hanya kalau Otto mau dibagikan ke orang lain.

## Catatan

- Idle CPU sekitar 0,8% saat diam. Diterima, tidak dikejar lagi.
- Daftar milestone yang panjang, status Undefined, dan tabel prioritas dihapus.
