export const WAITLIST_PLATFORMS = [
  "Windows 11 22H2 atau lebih baru",
  "Windows 11 24H2 atau lebih baru",
  "Windows 11 versi lebih lama",
  "Windows 10 atau lainnya",
] as const;

export const WAITLIST_USAGES = [
  "Jarang",
  "Kadang-kadang",
  "Hampir setiap hari",
] as const;

export const WAITLIST_SOURCES = ["hero"] as const;

export const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
export const MAX_EMAIL_LENGTH = 254;

export const WAITLIST_ERROR_MESSAGES: Record<string, string> = {
  invalid_email: "Alamat email belum valid. Periksa formatnya, misalnya nama@email.com.",
  invalid_body: "Permintaan tidak terbaca. Coba kirim ulang.",
  rate_limited: "Terlalu banyak percobaan dalam waktu singkat. Tunggu beberapa menit lalu coba lagi.",
  forbidden: "Sesi pendaftaran ini tidak cocok untuk mengubah profil.",
  invalid_profile: "Pilih versi Windows dan frekuensi pemakaian yang tersedia.",
  unsupported_media_type: "Format pengiriman tidak didukung. Muat ulang halaman lalu coba lagi.",
};

export function isValidWaitlistEmail(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return normalized.length > 0 && normalized.length <= MAX_EMAIL_LENGTH && EMAIL_PATTERN.test(normalized);
}
