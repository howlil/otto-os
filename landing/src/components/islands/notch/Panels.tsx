import type { ReactNode } from "react";
import { motion } from "motion/react";
import { ClipboardList, Inbox, ListTodo, Music, Pause, Play, Quote, RotateCcw, SkipBack, SkipForward, Timer } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../../lib/T";
import { clamp, formatClock } from "../../lib/spring";
import { Cover, Equalizer } from "./Compact";
import { FOCUS_CUT, FOCUS_TOTAL, LYRICS, PALETTE, TRACK_LENGTH, lyricAt, type Module } from "./spec";

type Tab = Extract<Module, "focus" | "music" | "pocket" | "clip" | "tasks">;

const TABS: Record<Tab, { icon: LucideIcon; label: string }> = {
  focus: { icon: Timer, label: "Focus" },
  music: { icon: Music, label: "Music" },
  pocket: { icon: Inbox, label: "Pocket" },
  clip: { icon: ClipboardList, label: "Clip" },
  tasks: { icon: ListTodo, label: "Tasks" },
};

const TAB_KEYS = Object.keys(TABS) as Tab[];

export function NotchTabs({ active }: { active: Tab }) {
  return (
    <div className="flex items-center gap-0.5 rounded-full bg-white/[0.08] p-[3px]" aria-hidden="true">
      {TAB_KEYS.map((tab) => {
        const { icon: Icon, label } = TABS[tab];
        const on = tab === active;
        return (
          <span
            key={tab}
            className={`inline-flex h-[18px] items-center gap-1.5 rounded-full px-2 text-[12px] font-semibold ${
              on ? "bg-white/[0.16] text-[#f5f5f7]" : "text-[#a8abb2]"
            }`}
          >
            <Icon size={12} strokeWidth={2} />
            {on ? label : null}
          </span>
        );
      })}
    </div>
  );
}

export function Pill({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "danger" | "positive" }) {
  const toneClass =
    tone === "danger" ? "bg-[#ff453a]/25 text-[#ff8a82]" : tone === "positive" ? "bg-white/[0.08] text-[#f5f5f7]" : "bg-white/[0.12] text-[#f5f5f7]";
  return (
    <span className={`inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-2 text-[11px] font-semibold ${toneClass}`}>{children}</span>
  );
}

export function PanelFrame({ tab, actions, children }: { tab: Tab; actions?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex h-full flex-col px-4 pt-[10px] pb-3">
      <div className="flex h-6 shrink-0 items-center justify-between gap-2">
        <NotchTabs active={tab} />
        {actions ? <div className="flex items-center gap-1.5">{actions}</div> : null}
      </div>
      <div className="mt-[6px] flex min-h-0 flex-1 flex-col">{children}</div>
    </div>
  );
}

function IconButton({ label, onClick, children }: { label: [string, string]; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="grid size-7 place-items-center rounded-full text-[#a8abb2] transition-colors hover:bg-white/10 hover:text-[#f5f5f7] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
    >
      {children}
      <span className="sr-only">
        <T id={label[0]} en={label[1]} />
      </span>
    </button>
  );
}

function Tomato({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 28 28" aria-hidden="true">
      <path
        d="M14 9.2c-6.1 0-10 4.2-10 9.6 0 4.1 3.1 6.7 7.1 6.7h5.8c4 0 7.1-2.6 7.1-6.7 0-5.4-3.9-9.6-10-9.6Z"
        fill={PALETTE.focus}
      />
      <path
        d="M14 9.2c-.6-2.1-2.1-3.4-4.3-3.6 1.1 1.5 1.6 2.4 2.1 3.7M14 9.2c.6-2.1 2.1-3.4 4.3-3.6-1.1 1.5-1.6 2.4-2.1 3.7"
        fill="none"
        stroke={PALETTE.positive}
        strokeWidth="1.7"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function FocusPanel({
  remaining,
  running,
  onToggle,
  onReset,
  onSkip,
  total = FOCUS_TOTAL,
}: {
  remaining: number;
  running: boolean;
  onToggle: () => void;
  onReset: () => void;
  onSkip: () => void;
  total?: number;
}) {
  const urgent = remaining <= FOCUS_CUT;
  const fraction = (total - remaining) / total;
  return (
    <PanelFrame tab="focus">
      <div className="flex h-full flex-col justify-between">
        <div className="flex items-center gap-3">
          <span className="grid size-12 shrink-0 place-items-center rounded-[14px] bg-[#ff9230]/15">
            <Tomato size={28} />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-[30px] leading-none font-semibold tracking-[-0.02em] tabular-nums" style={{ color: urgent ? PALETTE.urgent : PALETTE.ink }}>
              {formatClock(remaining)}
            </p>
            <p className="mt-1.5 truncate text-[11px] text-[#a8abb2]">
              <T id="Fokus dalam · selesai 12:48" en="Deep focus · ends 12:48" />
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <IconButton label={["Atur ulang", "Reset"]} onClick={onReset}>
              <RotateCcw size={14} strokeWidth={1.9} aria-hidden="true" />
            </IconButton>
            <button
              type="button"
              onClick={onToggle}
              className={`grid size-10 place-items-center rounded-full bg-[#ff9230] text-black transition-opacity focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                running ? "" : "opacity-55"
              }`}
            >
              {running && remaining > 0 ? (
                <Pause size={15} fill="currentColor" aria-hidden="true" />
              ) : (
                <Play size={15} fill="currentColor" aria-hidden="true" />
              )}
              <span className="sr-only">
                {running && remaining > 0 ? <T id="Jeda" en="Pause" /> : <T id="Putar" en="Play" />}
              </span>
            </button>
            <IconButton label={["Lewati", "Skip"]} onClick={onSkip}>
              <SkipForward size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
            </IconButton>
          </div>
        </div>
        <div>
          <div className="flex gap-1">
            {[0, 1, 2, 3].map((index) => (
              <span key={index} className="relative h-1 flex-1 overflow-hidden rounded-full bg-white/[0.14]">
                <span
                  className="absolute inset-y-0 left-0 rounded-full"
                  style={{ width: `${clamp(fraction * 4 - index, 0, 1) * 100}%`, background: PALETTE.focus }}
                />
              </span>
            ))}
          </div>
          <div className="mt-1.5 flex justify-between text-[10px] text-[#a8abb2] tabular-nums">
            <span>
              <T id="Mulai 12:23" en="Started 12:23" />
            </span>
            <span>
              <T id="Istirahat 12:48" en="Next break 12:48" />
            </span>
          </div>
        </div>
      </div>
    </PanelFrame>
  );
}

function LyricsToggle({ on }: { on: boolean }) {
  return (
    <span className={`grid size-6 place-items-center rounded-full ${on ? "bg-white/[0.16] text-[#f5f5f7]" : "text-[#a8abb2]"}`} aria-hidden="true">
      <Quote size={12} fill={on ? "currentColor" : "none"} strokeWidth={2} />
    </span>
  );
}

function LyricsCard({ index, fraction, reduced }: { index: number; fraction: number; reduced: boolean }) {
  const fill = reduced ? 100 : fraction * 100;
  const rows = [index - 1, index, index + 1];
  return (
    <div className="relative h-[102px] shrink-0 overflow-hidden rounded-[12px] bg-white/[0.05] ring-1 ring-inset ring-white/10">
      <div
        className="absolute inset-0 flex flex-col justify-center px-3.5"
        style={{
          maskImage: "linear-gradient(to bottom, transparent, #000 16px, #000 calc(100% - 16px), transparent)",
          WebkitMaskImage: "linear-gradient(to bottom, transparent, #000 16px, #000 calc(100% - 16px), transparent)",
        }}
      >
        {rows.map((row, slot) => {
          const line = LYRICS[row];
          if (!line) return <span key={`empty-${slot}`} className="h-[30px]" />;
          const active = slot === 1;
          return (
            <motion.p
              key={row}
              initial={reduced ? false : { y: 6, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              transition={{ duration: 0.22 }}
              className={`flex h-[30px] items-center truncate ${active ? "text-[16px] font-bold" : "text-[13px] text-[#a8abb2]"}`}
              style={
                active
                  ? {
                      backgroundImage: `linear-gradient(90deg, #f5f5f7 ${fill}%, rgb(245 245 247 / 0.45) ${fill}%)`,
                      WebkitBackgroundClip: "text",
                      backgroundClip: "text",
                      color: "transparent",
                    }
                  : { opacity: 0.38, filter: "blur(0.6px)" }
              }
            >
              <T id={line[0]} en={line[1]} />
            </motion.p>
          );
        })}
      </div>
    </div>
  );
}

export function MusicPanel({
  playing,
  elapsed,
  lyricsOn,
  reduced,
  onToggle,
  onPrev,
  onNext,
}: {
  playing: boolean;
  elapsed: number;
  lyricsOn: boolean;
  reduced: boolean;
  onToggle: () => void;
  onPrev: () => void;
  onNext: () => void;
}) {
  const { index, fraction } = lyricAt(elapsed);
  const position = clamp(elapsed / TRACK_LENGTH, 0, 1);
  const remain = Math.max(0, TRACK_LENGTH - Math.floor(elapsed));
  return (
    <PanelFrame tab="music" actions={<LyricsToggle on={lyricsOn} />}>
      <div className="flex h-full flex-col justify-between gap-2.5">
        <div className="flex items-center gap-3">
          <Cover size={48} dim={!playing} radius={0.25} />
          <div className="min-w-0 flex-1">
            <p className="truncate text-[15px] leading-tight font-semibold">Lampu Kota</p>
            <p className="truncate text-[11px] text-[#a8abb2]">
              <T id={playing ? "Data demo · Browser" : "Dijeda · Data demo"} en={playing ? "Demo data · Browser" : "Paused · Demo data"} />
            </p>
          </div>
          <Equalizer playing={playing} color={PALETTE.cover} reduced={reduced} />
          <div className="flex shrink-0 items-center gap-1">
            <IconButton label={["Sebelumnya", "Previous"]} onClick={onPrev}>
              <SkipBack size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
            </IconButton>
            <button
              type="button"
              onClick={onToggle}
              className="grid size-10 place-items-center rounded-full text-black transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
              style={{ background: PALETTE.cover }}
            >
              {playing ? <Pause size={15} fill="currentColor" aria-hidden="true" /> : <Play size={15} fill="currentColor" aria-hidden="true" />}
              <span className="sr-only">{playing ? <T id="Jeda" en="Pause" /> : <T id="Putar" en="Play" />}</span>
            </button>
            <IconButton label={["Berikutnya", "Next"]} onClick={onNext}>
              <SkipForward size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
            </IconButton>
          </div>
        </div>
        {lyricsOn ? <LyricsCard index={index} fraction={fraction} reduced={reduced} /> : null}
        <div>
          <div className="h-1 w-full overflow-hidden rounded-full bg-white/[0.15]">
            <div className="h-full rounded-full" style={{ width: `${position * 100}%`, background: PALETTE.cover }} />
          </div>
          <div className="mt-1.5 flex justify-between text-[10px] text-[#a8abb2] tabular-nums">
            <span>{formatClock(Math.floor(elapsed))}</span>
            <span>-{formatClock(remain)}</span>
          </div>
        </div>
      </div>
    </PanelFrame>
  );
}
