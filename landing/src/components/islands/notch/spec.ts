import type { Transition } from "motion/react";

export type NotchStyleMode = "island" | "classic";

export const ISLAND = {
  top: 4,
  compactHeight: 36,
  classicCompactHeight: 32,
  compactRadius: 16,
  classicCompactRadius: 10,
  classicEarRadius: 6,
  focusWidth: 184,
  classicFocusWidth: 200,
  musicWidth: 292,
  callWidth: 184,
  hudWidth: 356,
  ringingWidth: 380,
  ringingHeight: 96,
  panelWidth: 416,
  panelHeight: 144,
  statsHeight: 196,
  lyricsHeight: 254,
  tasksComposeHeight: 182,
  panelRadius: 28,
  classicPanelRadius: 22,
  secondaryGap: 8,
};

export const PALETTE = {
  ink: "#f5f5f7",
  muted: "#a8abb2",
  focus: "#ff9230",
  shortBreak: "#30d158",
  longBreak: "#0091ff",
  screenBlue: "#0a84ff",
  urgent: "#ff4245",
  danger: "#ff453a",
  accept: "#24a148",
  positive: "#30d158",
  warning: "#ff9f0a",
  mic: "#ff9f0a",
  camera: "#30d158",
  location: "#0a84ff",
  cover: "#fc3c54",
};

export const SHAPE: Transition = { type: "spring", stiffness: 439, damping: 35, mass: 1 };
export const COMPACT_SHAPE: Transition = { type: "spring", stiffness: 584, damping: 42, mass: 1 };
export const SECONDARY_SHAPE: Transition = { type: "spring", stiffness: 385, damping: 31, mass: 1 };
export const CONTENT_FADE: Transition = { duration: 0.24, ease: [0.16, 1, 0.3, 1] as [number, number, number, number] };
export const INSTANT: Transition = { duration: 0 };

export const FOCUS_TOTAL = 1500;
export const FOCUS_START = 1122;
export const FOCUS_CUT = 60;
export const TRACK_LENGTH = 214;
export const TRACK_START = 81;
export const CALL_START = 134;

export type NotchWorkspace = "focus" | "time" | "music" | "split" | "pocket" | "clip" | "tasks" | "call" | "hud";
export type NotchTabKey = "focus" | "time" | "pocket" | "clip" | "tasks" | "music";

export type WorkspaceMeta = {
  id: NotchWorkspace;
  label: [string, string];
  summary: [string, string];
};

export const WORKSPACES: WorkspaceMeta[] = [
  {
    id: "focus",
    label: ["Focus", "Focus"],
    summary: ["Pomodoro empat siklus langsung dari notch.", "Four-cycle Pomodoro directly inside the notch."],
  },
  {
    id: "time",
    label: ["Screen Time", "Screen Time"],
    summary: [
      "Tab Time baru di v1.3: porsi fokus hari ini, 3 aplikasi teratas, dan grafik 7 hari interaktif.",
      "New Time tab in v1.3: today's focus share, top 3 apps and an interactive 7-day stacked chart.",
    ],
  },
  {
    id: "music",
    label: ["Musik & Lirik", "Music & Lyrics"],
    summary: [
      "Kontrol media GSMTC, warna adaptif dari sampul album, dan laci lirik karaoke 3 baris.",
      "GSMTC playback, cover-adaptive accents and a 3-line synced karaoke lyrics drawer.",
    ],
  },
  {
    id: "split",
    label: ["Dual Split", "Dual Split"],
    summary: [
      "Dua aktivitas hidup berdampingan. Klik lingkaran sekunder untuk menukar posisi seketika.",
      "Two live activities side by side. Click the secondary circle to swap slots instantly.",
    ],
  },
  {
    id: "pocket",
    label: ["Pocket", "Pocket"],
    summary: [
      "Rak sementara 12 slot untuk menyeret file, tautan, atau cuplikan lintas jendela.",
      "A 12-slot holding shelf to drag files, links or snippets across windows.",
    ],
  },
  {
    id: "clip",
    label: ["Clip", "Clip"],
    summary: [
      "Riwayat 12 salinan terakhir di RAM saja — tanpa menulis ke disk, melewati format kata sandi.",
      "Last 12 copies held strictly in RAM — never written to disk, skipping sensitive formats.",
    ],
  },
  {
    id: "tasks",
    label: ["Tasks", "Tasks"],
    summary: [
      "Quick Capture opt-in (Ctrl+Alt+Q) dengan input inline; tugas aktif tampil di subjudul Focus.",
      "Opt-in Quick Capture (Ctrl+Alt+Q) with inline entry; active task syncs to the Focus subtitle.",
    ],
  },
  {
    id: "call",
    label: ["Panggilan & Privasi", "Calls & Privacy"],
    summary: [
      "Kartu panggilan masuk dengan terima/tolak, gelombang suara aktif, dan titik kamera/mikrofon/lokasi.",
      "Ringing call banner, live voice waveform and single-priority camera/mic/location dots.",
    ],
  },
  {
    id: "hud",
    label: ["System HUD", "System HUD"],
    summary: [
      "Menggantikan OSD bawaan Windows untuk volume, kecerahan, Bluetooth, baterai, DND, dan Caps Lock.",
      "Replaces the native Windows OSD for volume, brightness, Bluetooth, battery, DND and Caps Lock.",
    ],
  },
];

export const LYRICS: [string, string][] = [
  ["Lampu kota menyala pelan di kaca", "City lights glow softly across the glass"],
  ["Setiap detik kembali pada ritmenya", "Every second returns to its rhythm"],
  ["Jendela tenang di atas meja kerja", "Quiet windows resting above the desk"],
  ["Satu tarikan napas sebelum mulai lagi", "One quiet breath before starting again"],
];

export type Kind = "file" | "link" | "text";

export const POCKET_ITEMS: { kind: Kind; label: [string, string] }[] = [
  { kind: "file", label: ["Rancangan-v1.3.pdf", "Spec-v1.3.pdf"] },
  { kind: "link", label: ["github.com/howlil/otto-os", "github.com/howlil/otto-os"] },
  { kind: "text", label: ["Catatan rilis", "Release notes"] },
  { kind: "file", label: ["orbit-o.svg", "orbit-o.svg"] },
  { kind: "text", label: ["SHA256SUMS.txt", "SHA256SUMS.txt"] },
  { kind: "file", label: ["Notulen.docx", "Brief.docx"] },
];

export const CLIP_ITEMS: { kind: Kind; label: [string, string]; size: string }[] = [
  {
    kind: "text",
    label: ["cargo clippy --workspace --all-targets", "cargo clippy --workspace --all-targets"],
    size: "38 B",
  },
  { kind: "link", label: ["github.com/howlil/otto-os/releases", "github.com/howlil/otto-os/releases"], size: "34 B" },
  { kind: "file", label: ["otto-os-setup-1.3.1-x64.exe", "otto-os-setup-1.3.1-x64.exe"], size: "1 item" },
  { kind: "text", label: ["Ctrl+Alt+Space · Spotlight", "Ctrl+Alt+Space · Spotlight"], size: "26 B" },
];

export type TaskItem = {
  id: number;
  text: [string, string];
  done: boolean;
  active?: boolean;
};

export const INITIAL_TASKS: TaskItem[] = [
  { id: 1, text: ["Finalisasi shader Liquid Glass", "Finalize Liquid Glass shader"], done: false, active: true },
  { id: 2, text: ["Tinjau grafik Screen Time 7 hari", "Review 7-day Screen Time chart"], done: false },
  { id: 3, text: ["Verifikasi checksum rilis v1.3", "Verify v1.3 release checksums"], done: false },
  { id: 4, text: ["Rapikan migrasi config", "Clean up config migration"], done: true },
];

export type HudGlyph = "volume" | "volumeMuted" | "sun" | "caps" | "bolt" | "battery" | "headphones" | "dnd" | "lang";
export type HudTone = "neutral" | "positive" | "warning" | "muted";

export type HudEvent = {
  id: string;
  glyph: HudGlyph;
  label: [string, string];
  tone: HudTone;
  value?: [string, string];
  level?: number;
  pill?: [string, string];
};

export const HUD_EVENTS: HudEvent[] = [
  { id: "vol", glyph: "volume", label: ["Volume", "Volume"], tone: "neutral", value: ["64%", "64%"], level: 0.64 },
  {
    id: "bright",
    glyph: "sun",
    label: ["Brightness", "Brightness"],
    tone: "neutral",
    value: ["78%", "78%"],
    level: 0.78,
  },
  {
    id: "bt",
    glyph: "headphones",
    label: ["WH-1000XM5", "WH-1000XM5"],
    tone: "positive",
    value: ["84%", "84%"],
    level: 0.84,
  },
  { id: "caps", glyph: "caps", label: ["Caps Lock", "Caps Lock"], tone: "positive", pill: ["On", "On"] },
  {
    id: "charge",
    glyph: "bolt",
    label: ["Charging", "Charging"],
    tone: "positive",
    value: ["86%", "86%"],
    level: 0.86,
  },
  { id: "dnd", glyph: "dnd", label: ["Do Not Disturb", "Do Not Disturb"], tone: "neutral", pill: ["On", "On"] },
  { id: "lang", glyph: "lang", label: ["Keyboard", "Keyboard"], tone: "neutral", pill: ["EN", "EN"] },
  {
    id: "low",
    glyph: "battery",
    label: ["Low battery", "Low battery"],
    tone: "warning",
    value: ["16%", "16%"],
    level: 0.16,
  },
  { id: "mute", glyph: "volumeMuted", label: ["Volume", "Volume"], tone: "muted", value: ["Muted", "Muted"], level: 0 },
];

export type WeeklyDay = {
  day: string;
  totalMinutes: number;
  focusMinutes: number;
  today?: boolean;
};

export const WEEKLY_SCREEN_TIME: WeeklyDay[] = [
  { day: "Mon", totalMinutes: 255, focusMinutes: 162 },
  { day: "Tue", totalMinutes: 312, focusMinutes: 205 },
  { day: "Wed", totalMinutes: 278, focusMinutes: 184 },
  { day: "Thu", totalMinutes: 342, focusMinutes: 238 },
  { day: "Fri", totalMinutes: 294, focusMinutes: 196 },
  { day: "Sat", totalMinutes: 168, focusMinutes: 82 },
  { day: "Sun", totalMinutes: 228, focusMinutes: 155, today: true },
];

export const TOP_SCREEN_APPS = [
  { key: "code", name: "VS Code", duration: "1h 48m", share: 100, badge: "VS", color: "#0a84ff" },
  { key: "chrome", name: "Chrome · github.com", duration: "1h 12m", share: 67, badge: "Ch", color: "#30b0c7" },
  { key: "figma", name: "Figma", duration: "48m", share: 44, badge: "Fg", color: "#af52de" },
];

export function lyricAt(elapsed: number) {
  const position = Math.min(1, Math.max(0, elapsed / TRACK_LENGTH)) * LYRICS.length;
  const index = Math.min(LYRICS.length - 1, Math.floor(position));
  return { index, fraction: Math.min(1, Math.max(0, position - index)) };
}

export function formatMinutesBrief(minutes: number) {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h > 0 && m > 0) return `${h}h ${m}m`;
  if (h > 0) return `${h}h`;
  return `${m}m`;
}
