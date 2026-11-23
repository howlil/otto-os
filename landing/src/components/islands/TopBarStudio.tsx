import { useEffect, useRef, useState } from "react";
import { motion, useInView, useReducedMotion } from "motion/react";
import { BatteryMedium, Bell, ChevronLeft, ChevronRight, Search, SlidersHorizontal, Volume2, Wifi } from "lucide-react";
import { T } from "../lib/T";

type Pop = "none" | "calendar" | "notif";

type Frame = {
  bar: boolean;
  pop: Pop;
  pointer: { right: string | number; top: string | number };
  wait: number;
};

const AWAY = { right: "30%", top: "72%" } as const;

const TIMELINE: Frame[] = [
  { bar: false, pop: "none", pointer: AWAY, wait: 1800 },
  { bar: true, pop: "none", pointer: { right: "50%", top: 12 }, wait: 1400 },
  { bar: true, pop: "calendar", pointer: { right: 56, top: 12 }, wait: 2800 },
  { bar: true, pop: "notif", pointer: { right: 156, top: 12 }, wait: 2800 },
  { bar: false, pop: "none", pointer: AWAY, wait: 1600 },
];

const STILL_FRAME = 2;

const CELLS = Array.from({ length: 35 }, (_, index) =>
  index < 30 ? { day: index + 1, muted: false } : { day: index - 29, muted: true },
);

const SPRING = { type: "spring" as const, stiffness: 320, damping: 34, mass: 0.8 };
const POINTER = { type: "spring" as const, stiffness: 150, damping: 24, mass: 0.9 };
const POP = { duration: 0.28, ease: [0.16, 1, 0.3, 1] as [number, number, number, number] };

export function TopBarStudio() {
  const reduced = useReducedMotion() ?? false;
  const stageRef = useRef<HTMLDivElement>(null);
  const inView = useInView(stageRef, { amount: 0.45 });
  const [step, setStep] = useState(0);

  useEffect(() => {
    if (reduced || !inView) return;
    const id = window.setTimeout(() => setStep((value) => (value + 1) % TIMELINE.length), TIMELINE[step].wait);
    return () => window.clearTimeout(id);
  }, [reduced, inView, step]);

  const frame = reduced ? TIMELINE[STILL_FRAME] : TIMELINE[step];
  const calendarOpen = frame.pop === "calendar" && frame.bar;
  const notifOpen = frame.pop === "notif" && frame.bar;

  return (
    <div
      ref={stageRef}
      aria-hidden="true"
      className="relative h-[300px] w-full overflow-hidden rounded-[20px] bg-[#eceff3] sm:h-[320px]"
    >
      <motion.div
        className="absolute inset-x-0 top-0 z-30 flex h-8 items-center justify-between border-b border-line bg-white/80 px-3.5 text-[12px] text-ink backdrop-blur-xl"
        initial={false}
        animate={{ y: frame.bar ? 0 : -32 }}
        transition={reduced ? { duration: 0 } : SPRING}
      >
        <div className="flex min-w-0 items-center gap-3">
          <span className="font-semibold">Finder</span>
          <span className="hidden items-center gap-3 text-ink-soft md:flex">
            <span>File</span>
            <span>Edit</span>
            <span>View</span>
            <span>Window</span>
            <span>Help</span>
          </span>
        </div>
        <div className="flex items-center gap-2 text-ink-soft">
          <span className="hidden items-center gap-2 sm:flex" aria-hidden="true">
            <Wifi size={13} strokeWidth={2} />
            <Volume2 size={13} strokeWidth={2} />
            <BatteryMedium size={14} strokeWidth={2} />
          </span>
          <Search size={13} strokeWidth={2} className="hidden sm:block" aria-hidden="true" />
          <SlidersHorizontal size={13} strokeWidth={2} className="hidden sm:block" aria-hidden="true" />
          <span className="relative grid size-6 place-items-center">
            <Bell size={13} strokeWidth={2} />
            <span className="absolute top-1 right-1 size-1.5 rounded-full bg-[#ff453a]" />
          </span>
          <span className="px-1 text-[12px] font-medium tabular-nums text-ink">
            <span className="hidden lg:inline">Mon 23 Nov · </span>09:41
          </span>
        </div>
      </motion.div>

      <motion.div
        className="absolute top-[52px] left-1/2 z-20 -translate-x-1/2 rounded-full bg-ink px-3.5 py-1.5 text-[12px] font-medium text-page"
        initial={false}
        animate={{ opacity: frame.bar ? 0 : 1, y: frame.bar ? -6 : 0 }}
        transition={reduced ? { duration: 0 } : { duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
      >
        Ctrl + Alt + B
      </motion.div>

      <motion.div
        className="pointer-events-none absolute top-9 right-3 z-40 w-[252px] origin-top-right rounded-[16px] border border-line bg-paper p-3 sm:top-10"
        initial={false}
        animate={{ opacity: calendarOpen ? 1 : 0, scale: calendarOpen ? 1 : 0.96, y: calendarOpen ? 0 : -6 }}
        transition={reduced ? { duration: 0 } : POP}
      >
        <div className="flex items-center justify-between px-1 pb-2">
          <p className="text-[14px] font-semibold text-ink">
            November <span className="font-normal text-ink-soft">2026</span>
          </p>
          <div className="flex items-center gap-1 text-ink-soft">
            <ChevronLeft size={14} aria-hidden="true" />
            <ChevronRight size={14} aria-hidden="true" />
          </div>
        </div>
        <div className="grid grid-cols-7 text-center text-[10.5px] font-semibold text-ink-soft">
          {["S", "M", "T", "W", "T", "F", "S"].map((letter, index) => (
            <span key={index} className="py-1">{letter}</span>
          ))}
        </div>
        <div className="grid grid-cols-7 text-center text-[12px] tabular-nums">
          {CELLS.map((cell, index) => (
            <span key={index} className="grid h-7 place-items-center">
              <span
                className={`grid size-6 place-items-center rounded-full ${
                  cell.day === 23 && !cell.muted
                    ? "bg-[#ff3b30] font-semibold text-white"
                    : cell.muted
                      ? "text-ink/40"
                      : "text-ink"
                }`}
              >
                {cell.day}
              </span>
            </span>
          ))}
        </div>
      </motion.div>

      <motion.div
        className="pointer-events-none absolute top-9 right-3 z-40 w-[300px] origin-top-right rounded-[16px] border border-line bg-paper p-2.5 sm:top-10"
        initial={false}
        animate={{ opacity: notifOpen ? 1 : 0, scale: notifOpen ? 1 : 0.96, y: notifOpen ? 0 : -6 }}
        transition={reduced ? { duration: 0 } : POP}
      >
        <div className="flex items-center justify-between px-1.5 pb-2">
          <span className="text-[12.5px] font-semibold text-ink">
            <T id="Notifikasi" en="Notifications" />
          </span>
          <span className="text-[11px] font-medium text-ink-soft">Clear All</span>
        </div>
        <div className="flex items-start gap-2.5 rounded-[12px] border border-line bg-page p-2.5">
          <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#5865f2] text-[12px] font-bold text-white">D</span>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-2">
              <span className="text-[11px] font-medium text-ink-soft">Discord</span>
              <span className="text-[10.5px] text-muted tabular-nums">2m</span>
            </div>
            <p className="truncate text-[12.5px] font-semibold text-ink">Release v1.3.0</p>
          </div>
        </div>
        <div className="mt-1.5 flex items-start gap-2.5 rounded-[12px] border border-line bg-page p-2.5">
          <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#1b1b1d] text-[12px] font-bold text-white">GH</span>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-2">
              <span className="text-[11px] font-medium text-ink-soft">GitHub</span>
              <span className="text-[10.5px] text-muted tabular-nums">9m</span>
            </div>
            <p className="truncate text-[12.5px] font-semibold text-ink">Build green · x64</p>
          </div>
        </div>
      </motion.div>

      <motion.div
        className="pointer-events-none absolute z-50"
        initial={false}
        animate={{ right: frame.pointer.right, top: frame.pointer.top }}
        transition={reduced ? { duration: 0 } : POINTER}
      >
        <svg width="18" height="22" viewBox="0 0 18 22" aria-hidden="true">
          <path d="M1 1 L1 17 L5.2 13.2 L8.2 20 L10.9 18.8 L7.9 12.2 L14 12.2 Z" fill="#ffffff" stroke="#000000" strokeWidth="1.2" strokeLinejoin="round" />
        </svg>
      </motion.div>
    </div>
  );
}
