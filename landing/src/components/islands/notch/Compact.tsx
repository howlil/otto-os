import type { ReactNode } from "react";
import { motion } from "motion/react";
import {
  BatteryLow,
  CaseUpper,
  Globe,
  Headphones,
  MicOff,
  Moon,
  Navigation,
  Phone,
  PhoneOff,
  Sun,
  Video,
  Volume2,
  VolumeX,
  Zap,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../../lib/T";
import { formatClock } from "../../lib/spring";
import { FOCUS_CUT, FOCUS_TOTAL, ISLAND, PALETTE, type HudEvent, type HudGlyph, type HudTone } from "./spec";

const HUD_ICON: Record<HudGlyph, LucideIcon> = {
  volume: Volume2,
  volumeMuted: VolumeX,
  sun: Sun,
  caps: CaseUpper,
  bolt: Zap,
  battery: BatteryLow,
  headphones: Headphones,
  dnd: Moon,
  lang: Globe,
};

const HUD_TONE: Record<HudTone, string> = {
  neutral: PALETTE.ink,
  positive: PALETTE.positive,
  warning: PALETTE.warning,
  muted: PALETTE.muted,
};

export function IslandShell({ width, height = ISLAND.compactHeight, radius = ISLAND.compactRadius, children }: {
  width: number;
  height?: number;
  radius?: number;
  children: ReactNode;
}) {
  return (
    <div
      className="relative overflow-hidden bg-black ring-1 ring-inset ring-white/[0.14]"
      style={{ width, height, borderRadius: radius }}
    >
      {children}
    </div>
  );
}

export function Ring({ fraction, color, size = 18, stroke = 2 }: { fraction: number; color: string; size?: number; stroke?: number }) {
  const radius = 7;
  const length = 2 * Math.PI * radius;
  return (
    <svg width={size} height={size} viewBox="0 0 18 18" aria-hidden="true" className="shrink-0 -rotate-90">
      <circle cx="9" cy="9" r={radius} fill="none" stroke="rgb(245 245 247 / 0.2)" strokeWidth={stroke} />
      <circle
        cx="9"
        cy="9"
        r={radius}
        fill="none"
        stroke={color}
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeDasharray={length}
        strokeDashoffset={length * (1 - Math.min(1, Math.max(0, fraction)))}
      />
    </svg>
  );
}

export function Equalizer({ playing, color, reduced }: { playing: boolean; color: string; reduced: boolean }) {
  if (!playing) {
    return (
      <span className="flex h-4 items-center gap-[3px]" aria-hidden="true">
        {[0, 1, 2, 3].map((index) => (
          <span key={index} className="block size-[3px] rounded-full bg-[#a8abb2]" />
        ))}
      </span>
    );
  }
  return (
    <span className="flex h-4 items-end gap-[3px]" aria-hidden="true">
      {[0.5, 0.9, 0.35, 0.7].map((base, index) => (
        <motion.span
          key={index}
          className="block h-full w-[2px] origin-bottom rounded-full"
          style={{ background: color }}
          animate={reduced ? { scaleY: base } : { scaleY: [base, 1, base * 0.45, base] }}
          transition={reduced ? { duration: 0 } : { duration: 0.9 + index * 0.12, repeat: Infinity, ease: "easeInOut" }}
        />
      ))}
    </span>
  );
}

export function Waveform({ muted = false, reduced }: { muted?: boolean; reduced: boolean }) {
  if (muted) {
    return (
      <span className="flex h-4 items-center gap-[4px]" aria-hidden="true">
        {[0, 1, 2, 3, 4].map((index) => (
          <span key={index} className="block size-[3px] rounded-full bg-[#a8abb2]" />
        ))}
      </span>
    );
  }
  return (
    <span className="flex h-4 items-center gap-[3px]" aria-hidden="true">
      {[0.45, 0.85, 1, 0.75, 0.5].map((base, index) => (
        <motion.span
          key={index}
          className="block h-full w-[2px] origin-center rounded-full"
          style={{ background: PALETTE.positive }}
          animate={reduced ? { scaleY: base } : { scaleY: [base, 0.3, base] }}
          transition={reduced ? { duration: 0 } : { duration: 0.72 + index * 0.1, repeat: Infinity, ease: "easeInOut" }}
        />
      ))}
    </span>
  );
}

export function Cover({ size, dim = false, radius = 0.25 }: { size: number; dim?: boolean; radius?: number }) {
  return (
    <span
      className="relative block shrink-0 overflow-hidden"
      style={{
        width: size,
        height: size,
        borderRadius: size * radius,
        opacity: dim ? 0.55 : 1,
        background: "linear-gradient(140deg, #ffb0b8 0%, #fc3c54 46%, #4b1224 100%)",
      }}
      aria-hidden="true"
    >
      <span className="absolute inset-[22%] rounded-full border border-white/25" />
      <span className="absolute top-1/2 left-1/2 block size-[18%] -translate-x-1/2 -translate-y-1/2 rounded-full bg-white/70" />
    </span>
  );
}

export function FocusCompact({ remaining }: { remaining: number }) {
  const urgent = remaining <= FOCUS_CUT;
  const color = urgent ? PALETTE.urgent : PALETTE.focus;
  const filled = Math.min(3, Math.floor(((FOCUS_TOTAL - remaining) / FOCUS_TOTAL) * 4));
  return (
    <div className="flex h-full w-full items-center justify-between px-3">
      <Ring fraction={remaining / FOCUS_TOTAL} color={color} />
      <span className="flex items-center gap-[5px]" aria-hidden="true">
        {[0, 1, 2, 3].map((index) => (
          <span key={index} className="block size-[6px] rounded-full" style={{ background: index <= filled ? color : "rgb(245 245 247 / 0.22)" }} />
        ))}
      </span>
      <span className="text-[13px] font-medium tabular-nums" style={{ color: urgent ? PALETTE.urgent : PALETTE.ink }}>
        {formatClock(remaining)}
      </span>
    </div>
  );
}

export function MusicCompact({ lyric, playing, reduced }: { lyric: [string, string]; playing: boolean; reduced: boolean }) {
  return (
    <div className="flex h-full w-full items-center justify-between px-2.5">
      <Cover size={20} radius={0.3} dim={!playing} />
      <span className="min-w-0 flex-1 truncate px-2 text-center text-[13px] font-medium text-[#f5f5f7]">
        <T id={lyric[0]} en={lyric[1]} />
      </span>
      <span className="flex w-4 justify-end">
        <Equalizer playing={playing} color={PALETTE.cover} reduced={reduced} />
      </span>
    </div>
  );
}

export function CallCompact({ seconds, muted, reduced }: { seconds: number; muted: boolean; reduced: boolean }) {
  return (
    <div className="flex h-full w-full items-center justify-between px-3">
      {muted ? (
        <MicOff size={14} strokeWidth={2} color={PALETTE.danger} aria-hidden="true" />
      ) : (
        <Phone size={14} strokeWidth={2} color={PALETTE.positive} aria-hidden="true" />
      )}
      <Waveform muted={muted} reduced={reduced} />
      <span className="text-[13px] font-medium text-[#f5f5f7] tabular-nums">{formatClock(seconds)}</span>
    </div>
  );
}

export function RingingCallCard({
  video = true,
  reduced,
  onAccept,
  onDecline,
}: {
  video?: boolean;
  reduced: boolean;
  onAccept: () => void;
  onDecline: () => void;
}) {
  return (
    <div className="flex h-full w-full items-center gap-3 px-4">
      <div className="relative grid size-12 shrink-0 place-items-center">
        <motion.span
          className="absolute inset-0 rounded-full border-2 border-[#30d158]"
          animate={reduced ? { scale: 1, opacity: 0.75 } : { scale: [1, 1.18, 1], opacity: [0.85, 0.15, 0.85] }}
          transition={reduced ? { duration: 0 } : { duration: 1.4, repeat: Infinity, ease: "easeOut" }}
          aria-hidden="true"
        />
        <span className="grid size-11 place-items-center rounded-full bg-[#1e352a] text-[15px] font-semibold text-[#f5f5f7]">
          NA
        </span>
      </div>
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-1.5 text-[11px] font-medium text-white/60">
          {video ? <Video size={12} strokeWidth={2} aria-hidden="true" /> : null}
          <span>{video ? "Teams · Incoming Video Call" : "Discord · Incoming Call"}</span>
        </p>
        <p className="mt-0.5 truncate text-[15px] font-semibold text-white">Nadia Ardiansyah</p>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <button
          type="button"
          onClick={onDecline}
          className="grid size-11 place-items-center rounded-full bg-[#ff453a] text-white transition-transform hover:brightness-110 active:scale-95 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
        >
          <PhoneOff size={18} strokeWidth={2} aria-hidden="true" />
          <span className="sr-only"><T id="Tolak panggilan" en="Decline call" /></span>
        </button>
        <button
          type="button"
          onClick={onAccept}
          className="grid size-11 place-items-center rounded-full bg-[#24a148] text-white transition-transform hover:brightness-110 active:scale-95 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
        >
          {video ? <Video size={18} strokeWidth={2} aria-hidden="true" /> : <Phone size={18} strokeWidth={2} aria-hidden="true" />}
          <span className="sr-only"><T id="Terima panggilan" en="Accept call" /></span>
        </button>
      </div>
    </div>
  );
}

export function HudPill({ event }: { event: HudEvent }) {
  const Icon = HUD_ICON[event.glyph];
  const tone = HUD_TONE[event.tone];
  return (
    <div className="flex h-full w-full items-center gap-2.5 px-4">
      <Icon size={16} strokeWidth={1.9} color={tone} aria-hidden="true" />
      <span className="w-[104px] shrink-0 truncate text-[13px] font-medium text-[#f5f5f7]">
        <T id={event.label[0]} en={event.label[1]} />
      </span>
      {event.pill ? (
        <span className="flex-1" aria-hidden="true" />
      ) : (
        <span className="relative h-1 flex-1 overflow-hidden rounded-full bg-white/[0.16]" aria-hidden="true">
          <motion.span
            className="absolute inset-y-0 left-0 rounded-full"
            style={{ background: tone }}
            initial={{ width: 0 }}
            animate={{ width: `${(event.level ?? 0) * 100}%` }}
            transition={{ duration: 0.36, ease: [0.16, 1, 0.3, 1] }}
          />
        </span>
      )}
      <span className="flex w-11 shrink-0 justify-end text-[13px] font-medium text-[#f5f5f7] tabular-nums">
        {event.pill ? (
          <span
            className="rounded-full px-2 py-px text-[11px] font-semibold"
            style={{
              color: event.tone === "positive" ? PALETTE.positive : PALETTE.ink,
              background: event.tone === "positive" ? "rgb(48 209 88 / 0.2)" : "rgb(255 255 255 / 0.14)",
            }}
          >
            <T id={event.pill[0]} en={event.pill[1]} />
          </span>
        ) : event.value ? (
          <T id={event.value[0]} en={event.value[1]} />
        ) : null}
      </span>
    </div>
  );
}

export function SecondaryFocus({ remaining }: { remaining: number }) {
  return (
    <div className="grid size-full place-items-center">
      <Ring fraction={remaining / FOCUS_TOTAL} color={PALETTE.focus} size={16} />
    </div>
  );
}

export function SecondaryMusic({ playing, reduced }: { playing: boolean; reduced: boolean }) {
  return (
    <div className="grid size-full place-items-center">
      {playing ? (
        <Equalizer playing color={PALETTE.cover} reduced={reduced} />
      ) : (
        <Cover size={18} dim radius={0.3} />
      )}
    </div>
  );
}

export function PrivacyDot({ kind, left = 312, top = 19 }: { kind: "camera" | "mic" | "location"; left?: number; top?: number }) {
  if (kind === "location") {
    return (
      <div className="pointer-events-none absolute" style={{ left, top: top - 2 }} aria-hidden="true">
        <Navigation size={10} strokeWidth={2.2} color={PALETTE.location} fill={PALETTE.location} />
      </div>
    );
  }
  return (
    <div className="pointer-events-none absolute" style={{ left, top }} aria-hidden="true">
      <span className="block size-[6px] rounded-full" style={{ background: kind === "camera" ? PALETTE.camera : PALETTE.mic }} />
    </div>
  );
}
