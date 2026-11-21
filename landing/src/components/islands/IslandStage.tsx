import { useEffect, useState, type ReactNode } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Disc3, Pause, Play, RotateCcw, SkipBack, SkipForward, X } from "lucide-react";
import { T } from "../lib/T";
import { formatClock } from "../lib/spring";

type Tab = "focus" | "music";

const FOCUS_TOTAL = 1500;
const TRACK_SECONDS = 180;
const LYRICS: [string, string][] = [
  ["Lirik tersinkron, baris demi baris", "Synced lyrics, line by line"],
  ["Baris aktif mengikuti waktu", "The active line follows the time"],
  ["Semua berjalan lokal di perangkatmu", "Everything runs locally on your device"],
];

const COMPACT_WIDTH: Record<Tab, number> = { focus: 184, music: 292 };
const EXPANDED = { width: 400, height: 176, radius: 28 };
const IDLE = { width: 120, height: 32, radius: 16 };
const SPRING = { type: "spring" as const, stiffness: 420, damping: 36, mass: 0.9 };
const INSTANT = { duration: 0 };
const SOFT = { duration: 0.28, ease: [0.16, 1, 0.3, 1] as [number, number, number, number] };

function Equalizer({ playing }: { playing: boolean }) {
  return (
    <span className="flex h-4 items-end gap-[3px]" aria-hidden="true">
      {[0.5, 0.9, 0.35, 0.7].map((base, index) => (
        <motion.span
          key={index}
          className="block h-full w-[2px] origin-bottom rounded-full bg-stage-ink"
          animate={playing ? { scaleY: [base, 1, base * 0.45, base] } : { scaleY: 0.35 }}
          transition={playing ? { duration: 0.9 + index * 0.12, repeat: Infinity, ease: "easeInOut" } : { duration: 0.3 }}
        />
      ))}
    </span>
  );
}

function Ring({ fraction }: { fraction: number }) {
  const radius = 7;
  const length = 2 * Math.PI * radius;
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true" className="shrink-0 -rotate-90">
      <circle cx="9" cy="9" r={radius} fill="none" stroke="rgb(245 245 247 / 0.2)" strokeWidth="2" />
      <circle
        cx="9"
        cy="9"
        r={radius}
        fill="none"
        stroke="#ff6a2b"
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray={length}
        strokeDashoffset={length * (1 - fraction)}
      />
    </svg>
  );
}

function IconButton({
  label,
  onClick,
  children,
  primary = false,
}: {
  label: [string, string];
  onClick: () => void;
  children: ReactNode;
  primary?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center justify-center rounded-full text-stage-ink transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
        primary ? "size-9 bg-accent hover:bg-accent-hover" : "size-8 hover:bg-white/10"
      }`}
    >
      {children}
      <span className="sr-only">
        <T id={label[0]} en={label[1]} />
      </span>
    </button>
  );
}

export function IslandStage() {
  const reduced = useReducedMotion() ?? false;
  const [expanded, setExpanded] = useState(false);
  const [tab, setTab] = useState<Tab>("focus");
  const [focusRunning, setFocusRunning] = useState(true);
  const [remaining, setRemaining] = useState(1452);
  const [musicPlaying, setMusicPlaying] = useState(true);
  const [progress, setProgress] = useState(0.38);

  useEffect(() => {
    if (!focusRunning) return;
    const id = window.setInterval(() => setRemaining((value) => Math.max(0, value - 1)), 1000);
    return () => window.clearInterval(id);
  }, [focusRunning]);

  useEffect(() => {
    if (remaining === 0) setFocusRunning(false);
  }, [remaining]);

  useEffect(() => {
    if (!musicPlaying) return;
    const id = window.setInterval(() => setProgress((value) => Math.min(1, value + 0.25 / TRACK_SECONDS)), 250);
    return () => window.clearInterval(id);
  }, [musicPlaying]);

  useEffect(() => {
    if (progress >= 1) setMusicPlaying(false);
  }, [progress]);

  const focusActive = focusRunning && remaining > 0;
  const musicActive = musicPlaying;
  const idle = !expanded && !focusActive && !musicActive;

  const compactTab: Tab =
    tab === "focus"
      ? focusActive || !musicActive
        ? "focus"
        : "music"
      : musicActive || !focusActive
        ? "music"
        : "focus";
  const secondaryTab: Tab | null = focusActive && musicActive ? (compactTab === "focus" ? "music" : "focus") : null;

  const lyricIndex = Math.min(LYRICS.length - 1, Math.floor(progress * LYRICS.length));
  const lyric = LYRICS[lyricIndex];

  const shape = expanded
    ? { width: EXPANDED.width, height: EXPANDED.height, borderBottomLeftRadius: EXPANDED.radius, borderBottomRightRadius: EXPANDED.radius }
    : idle
      ? { width: IDLE.width, height: IDLE.height, borderBottomLeftRadius: IDLE.radius, borderBottomRightRadius: IDLE.radius }
      : { width: COMPACT_WIDTH[compactTab], height: 36, borderBottomLeftRadius: 18, borderBottomRightRadius: 18 };

  function toggleFocus() {
    if (remaining === 0) {
      setRemaining(FOCUS_TOTAL);
      setFocusRunning(true);
      return;
    }
    setFocusRunning((value) => !value);
  }

  function restartFocus() {
    setRemaining(FOCUS_TOTAL);
    setFocusRunning(true);
  }

  return (
    <div className="flex flex-col items-center gap-8">
      <div className="relative flex h-[260px] w-full items-start justify-center overflow-hidden rounded-[32px] border border-stage-line bg-[radial-gradient(120%_90%_at_50%_0%,#1d2b47_0%,#0e1017_55%,#08090c_100%)] pt-0 sm:h-[260px]">
        <div aria-hidden="true" className="absolute inset-x-0 top-0 h-9 border-b border-white/5 bg-black/40" />
        <div className="flex origin-top scale-[0.8] items-start gap-2 sm:scale-100">
          <motion.div
            className="relative overflow-hidden bg-stage ring-1 ring-white/10"
            style={{ borderTopLeftRadius: 0, borderTopRightRadius: 0 }}
            animate={shape}
            transition={reduced ? INSTANT : SPRING}
          >
            {expanded ? (
              <div className="flex h-full flex-col gap-3 p-4">
                <div className="flex items-center justify-between">
                  <div className="flex gap-1 rounded-full bg-white/10 p-1">
                    {(["focus", "music"] as Tab[]).map((item) => (
                      <button
                        key={item}
                        type="button"
                        aria-pressed={tab === item}
                        onClick={() => setTab(item)}
                        className={`h-7 rounded-full px-3 text-[13px] font-medium transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                          tab === item ? "bg-white text-black" : "text-stage-muted hover:text-stage-ink"
                        }`}
                      >
                        {item === "focus" ? <T id="Fokus" en="Focus" /> : <T id="Musik" en="Music" />}
                      </button>
                    ))}
                  </div>
                  <button
                    type="button"
                    onClick={() => setExpanded(false)}
                    className="inline-flex size-7 items-center justify-center rounded-full text-stage-muted transition-colors hover:bg-white/10 hover:text-stage-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
                  >
                    <X size={15} strokeWidth={1.8} aria-hidden="true" />
                    <span className="sr-only">
                      <T id="Tutup panel" en="Close panel" />
                    </span>
                  </button>
                </div>

                <AnimatePresence mode="wait" initial={false}>
                  {tab === "focus" ? (
                    <motion.div
                      key="focus"
                      initial={reduced ? false : { y: 8 }}
                      animate={{ y: 0 }}
                      exit={reduced ? { y: 0 } : { y: -8 }}
                      transition={SOFT}
                      className="flex flex-1 items-center justify-between"
                    >
                      <div className="flex flex-col">
                        <span className="text-[34px] leading-none font-semibold tracking-[-0.02em] tabular-nums">
                          {formatClock(remaining)}
                        </span>
                        <span className="mt-2 text-[13px] text-stage-muted">
                          <T id="Sesi 1 dari 4" en="Session 1 of 4" />
                        </span>
                      </div>
                      <div className="flex items-center gap-1">
                        <IconButton label={["Atur ulang", "Reset"]} onClick={restartFocus}>
                          <RotateCcw size={16} strokeWidth={1.8} aria-hidden="true" />
                        </IconButton>
                        <IconButton label={focusRunning && remaining > 0 ? ["Jeda", "Pause"] : ["Putar", "Play"]} primary onClick={toggleFocus}>
                          {focusRunning && remaining > 0 ? (
                            <Pause size={16} fill="currentColor" aria-hidden="true" />
                          ) : (
                            <Play size={16} fill="currentColor" aria-hidden="true" />
                          )}
                        </IconButton>
                        <IconButton label={["Lewati", "Skip"]} onClick={restartFocus}>
                          <SkipForward size={16} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
                        </IconButton>
                      </div>
                    </motion.div>
                  ) : (
                    <motion.div
                      key="music"
                      initial={reduced ? false : { y: 8 }}
                      animate={{ y: 0 }}
                      exit={reduced ? { y: 0 } : { y: -8 }}
                      transition={SOFT}
                      className="flex flex-1 flex-col justify-between"
                    >
                      <div className="flex items-center gap-3">
                        <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-music-activity text-white">
                          <Disc3 size={20} strokeWidth={1.8} aria-hidden="true" />
                        </span>
                        <div className="min-w-0 flex-1">
                          <p className="text-[14px] font-semibold">
                            <T id="Demo lagu" en="Demo track" />
                          </p>
                          <p className="truncate text-[13px] text-stage-muted">
                            <AnimatePresence mode="wait" initial={false}>
                              <motion.span
                                key={lyricIndex}
                                initial={reduced ? false : { y: 6 }}
                                animate={{ y: 0 }}
                                exit={reduced ? { y: 0 } : { y: -6 }}
                                transition={{ duration: 0.22 }}
                                className="inline-block"
                              >
                                <T id={lyric[0]} en={lyric[1]} />
                              </motion.span>
                            </AnimatePresence>
                          </p>
                        </div>
                        <Equalizer playing={musicPlaying} />
                      </div>
                      <div className="flex flex-col gap-2">
                        <div className="h-1 w-full overflow-hidden rounded-full bg-white/15">
                          <div className="h-full rounded-full bg-stage-ink" style={{ width: `${progress * 100}%` }} />
                        </div>
                        <div className="flex items-center justify-center gap-1">
                          <IconButton label={["Sebelumnya", "Previous"]} onClick={() => setProgress(0)}>
                            <SkipBack size={16} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
                          </IconButton>
                          <IconButton
                            label={musicPlaying ? ["Jeda", "Pause"] : ["Putar", "Play"]}
                            primary
                            onClick={() => setMusicPlaying((value) => !value)}
                          >
                            {musicPlaying ? (
                              <Pause size={16} fill="currentColor" aria-hidden="true" />
                            ) : (
                              <Play size={16} fill="currentColor" aria-hidden="true" />
                            )}
                          </IconButton>
                          <IconButton label={["Berikutnya", "Next"]} onClick={() => setProgress(0)}>
                            <SkipForward size={16} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
                          </IconButton>
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setExpanded(true)}
                aria-expanded={false}
                className="flex size-full items-center justify-between px-3.5 text-stage-ink transition-colors hover:bg-white/5 focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[#75b0ff]"
              >
                {idle ? (
                  <span className="sr-only">
                    <T id="Buka aktivitas" en="Open activity" />
                  </span>
                ) : compactTab === "focus" ? (
                  <>
                    <span className="flex items-center gap-2">
                      <Ring fraction={remaining / FOCUS_TOTAL} />
                      <span className="text-[13px] font-medium tabular-nums">{formatClock(remaining)}</span>
                    </span>
                    {musicActive ? <Equalizer playing={musicPlaying} /> : <span className="w-4" />}
                  </>
                ) : (
                  <>
                    <span className="flex min-w-0 items-center gap-2">
                      <span className="grid size-5 shrink-0 place-items-center rounded-md bg-music-activity">
                        <Disc3 size={12} strokeWidth={2} aria-hidden="true" />
                      </span>
                      <span className="truncate text-[13px] font-medium">
                        <T id="Demo lagu" en="Demo track" />
                      </span>
                    </span>
                    <Equalizer playing={musicPlaying} />
                  </>
                )}
              </button>
            )}
          </motion.div>

          <AnimatePresence initial={false}>
            {secondaryTab && !expanded && (
              <motion.button
                key={secondaryTab}
                type="button"
                onClick={() => setTab(secondaryTab)}
                initial={reduced ? false : { scale: 0.6 }}
                animate={{ scale: 1 }}
                exit={reduced ? { scale: 1 } : { scale: 0.6 }}
                transition={reduced ? INSTANT : SPRING}
                className="grid size-9 shrink-0 place-items-center rounded-full bg-stage text-stage-ink ring-1 ring-white/10 transition-colors hover:bg-white/10 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
              >
                {secondaryTab === "focus" ? (
                  <Ring fraction={remaining / FOCUS_TOTAL} />
                ) : (
                  <Disc3 size={16} strokeWidth={1.8} aria-hidden="true" />
                )}
                <span className="sr-only">
                  {secondaryTab === "focus" ? <T id="Tampilkan Focus" en="Show Focus" /> : <T id="Tampilkan Musik" en="Show Music" />}
                </span>
              </motion.button>
            )}
          </AnimatePresence>
        </div>
      </div>

      <div className="flex w-full flex-wrap items-center justify-center gap-x-6 gap-y-2 text-[13px] text-stage-muted">
        <span>
          <T id="Klik notch untuk membuka. Lingkaran samping menukar aktivitas." en="Click the notch to open. The side circle swaps activities." />
        </span>
        <span>
          <T id="Demo interaktif, data sintetis." en="Interactive demo, synthetic data." />
        </span>
      </div>
    </div>
  );
}
