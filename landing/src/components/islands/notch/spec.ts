import type { Transition } from "motion/react";

export const ISLAND = {
  top: 4,
  barHeight: 40,
  compactHeight: 36,
  compactRadius: 16,
  focusWidth: 184,
  musicWidth: 292,
  callWidth: 184,
  hudWidth: 356,
  panelWidth: 380,
  panelHeight: 144,
  lyricsHeight: 254,
  panelRadius: 28,
  secondaryGap: 8,
};

export const PALETTE = {
  ink: "#f5f5f7",
  muted: "#a8abb2",
  focus: "#ff9230",
  urgent: "#ff4245",
  positive: "#30d158",
  warning: "#ff9f0a",
  mic: "#ff9f0a",
  camera: "#30d158",
  cover: "#fc3c54",
};

export const SHAPE: Transition = { type: "spring", stiffness: 439, damping: 35, mass: 1 };
export const COMPACT_SHAPE: Transition = { type: "spring", stiffness: 584, damping: 42, mass: 1 };
export const SECONDARY_SHAPE: Transition = { type: "spring", stiffness: 385, damping: 31, mass: 1 };
export const CONTENT_FADE: Transition = { duration: 0.28, ease: [0.16, 1, 0.3, 1] as [number, number, number, number] };
export const INSTANT: Transition = { duration: 0 };

export const FOCUS_TOTAL = 1500;
export const FOCUS_START = 1122;
export const FOCUS_CUT = 60;
export const TRACK_LENGTH = 214;
export const TRACK_START = 81;
export const CALL_START = 134;

export type Module = "focus" | "music" | "pocket" | "clip" | "tasks" | "call" | "hud";
export type View = "compact" | "panel" | "call" | "hud";

export type Scene = {
  id: string;
  view: View;
  module: Module;
  lyrics?: boolean;
  secondary?: boolean;
  caption: [string, string];
};

export const SCENES: Scene[] = [
  {
    id: "focus-compact",
    view: "compact",
    module: "focus",
    caption: ["Timer fokus tampil ringkas di notch.", "A focus timer sits compact in the notch."],
  },
  {
    id: "focus-panel",
    view: "panel",
    module: "focus",
    caption: ["Pomodoro, timer cepat, dan stopwatch dalam satu panel.", "Pomodoro, quick timer and stopwatch in one panel."],
  },
  {
    id: "music-split",
    view: "compact",
    module: "music",
    secondary: true,
    caption: ["Musik dan fokus jalan bersamaan. Lingkaran samping menukar keduanya.", "Music and focus run together. The side circle swaps them."],
  },
  {
    id: "music-lyrics",
    view: "panel",
    module: "music",
    lyrics: true,
    caption: ["Lirik tersinkron, baris demi baris.", "Synced lyrics, line by line."],
  },
  {
    id: "pocket",
    view: "panel",
    module: "pocket",
    caption: ["Pocket menampung hingga 12 file, tautan, atau teks.", "Pocket holds up to 12 files, links or text."],
  },
  {
    id: "clip",
    view: "panel",
    module: "clip",
    caption: ["Clip menyimpan 12 salinan terakhir, hanya di RAM.", "Clip keeps the last 12 copies, in RAM only."],
  },
  {
    id: "tasks",
    view: "panel",
    module: "tasks",
    caption: ["Quick Capture & Tasks, opt-in lewat Ctrl+Alt+Q.", "Quick Capture & Tasks, opt-in with Ctrl+Alt+Q."],
  },
  {
    id: "call",
    view: "call",
    module: "call",
    caption: ["Panggilan aktif. Titik privasi muncul di sisi notch.", "An active call. A privacy dot appears beside the notch."],
  },
  {
    id: "hud",
    view: "hud",
    module: "hud",
    caption: ["HUD sistem untuk volume, kecerahan, baterai, Caps Lock, dan Bluetooth.", "System HUD for volume, brightness, battery, Caps Lock and Bluetooth."],
  },
];

export const RAIL: { module: Module; step: number; label: [string, string] }[] = [
  { module: "focus", step: 0, label: ["Focus", "Focus"] },
  { module: "music", step: 2, label: ["Musik", "Music"] },
  { module: "pocket", step: 4, label: ["Pocket", "Pocket"] },
  { module: "clip", step: 5, label: ["Clip", "Clip"] },
  { module: "tasks", step: 6, label: ["Tasks", "Tasks"] },
  { module: "call", step: 7, label: ["Panggilan", "Call"] },
  { module: "hud", step: 8, label: ["HUD", "HUD"] },
];

export const LYRICS: [string, string][] = [
  ["Lampu kota mulai menyala", "City lights start to glow"],
  ["Baris aktif mengikuti waktu", "The active line follows the time"],
  ["Lirik tampil saat lagu berjalan", "Lyrics follow the song as it plays"],
  ["Pelan-pelan, satu baris lagi", "Slowly, one more line"],
];

export type Kind = "file" | "link" | "text";

export const POCKET_ITEMS: { kind: Kind; label: [string, string] }[] = [
  { kind: "file", label: ["Rancangan.pdf", "Rancangan.pdf"] },
  { kind: "link", label: ["github.com/howlil/otto-os", "github.com/howlil/otto-os"] },
  { kind: "text", label: ["Catatan rilis", "Release notes"] },
  { kind: "file", label: ["orbit-o.svg", "orbit-o.svg"] },
  { kind: "text", label: ["Tautan unduhan beta", "Beta download link"] },
  { kind: "file", label: ["Notulen rapat.docx", "Meeting notes.docx"] },
  { kind: "text", label: ["Ringkasan konsep", "Concept summary"] },
  { kind: "link", label: ["otto-os/releases", "otto-os/releases"] },
];

export const CLIP_ITEMS: { kind: Kind; label: [string, string]; size: string }[] = [
  { kind: "text", label: ["Ringkasan konsep Otto OS", "Otto OS concept summary"], size: "212 B" },
  { kind: "link", label: ["github.com/howlil/otto-os", "github.com/howlil/otto-os"], size: "28 B" },
  { kind: "file", label: ["Rancangan.pdf", "Rancangan.pdf"], size: "1 item" },
  { kind: "text", label: ["Tautan unduhan beta", "Beta download link"], size: "64 B" },
];

export const TASKS: { text: [string, string]; done: boolean }[] = [
  { text: ["Kirim invoice besok", "Send invoice tomorrow"], done: false },
  { text: ["Review rancangan PR", "Review the PR draft"], done: false },
  { text: ["Rapikan catatan rilis", "Tidy up release notes"], done: false },
  { text: ["Balas email tim", "Reply to team email"], done: true },
];

export type HudGlyph = "volume" | "volumeMuted" | "sun" | "caps" | "bolt" | "battery" | "headphones";
export type HudTone = "neutral" | "positive" | "warning" | "muted";

export type HudEvent = {
  glyph: HudGlyph;
  label: [string, string];
  tone: HudTone;
  value?: [string, string];
  level?: number;
  pill?: boolean;
};

export const HUD_EVENTS: HudEvent[] = [
  { glyph: "volume", label: ["Volume", "Volume"], tone: "neutral", value: ["64%", "64%"], level: 0.64 },
  { glyph: "sun", label: ["Brightness", "Brightness"], tone: "neutral", value: ["72%", "72%"], level: 0.72 },
  { glyph: "caps", label: ["Caps Lock", "Caps Lock"], tone: "positive", pill: true },
  { glyph: "bolt", label: ["Charging", "Charging"], tone: "positive", value: ["86%", "86%"], level: 0.86 },
  { glyph: "battery", label: ["Low battery", "Low battery"], tone: "warning", value: ["18%", "18%"], level: 0.18 },
  { glyph: "headphones", label: ["Headset demo", "Demo headset"], tone: "positive", value: ["72%", "72%"] },
  { glyph: "volumeMuted", label: ["Volume", "Volume"], tone: "muted", value: ["Muted", "Muted"], level: 0 },
];

export function lyricAt(elapsed: number) {
  const position = Math.min(1, Math.max(0, elapsed / TRACK_LENGTH)) * LYRICS.length;
  const index = Math.min(LYRICS.length - 1, Math.floor(position));
  return { index, fraction: Math.min(1, Math.max(0, position - index)) };
}
