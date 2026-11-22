import { useEffect, useRef, useState, type RefObject } from "react";
import { AnimatePresence, motion, useMotionValueEvent, useReducedMotion, useScroll } from "motion/react";
import { ClipboardList, Inbox, ListTodo, Music, Phone, Timer, Volume2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../lib/T";
import { clamp } from "../lib/spring";
import { CallCompact, FocusCompact, HudPill, MusicCompact, PrivacyDot, SecondaryFocus } from "./notch/Compact";
import { FocusPanel, MusicPanel } from "./notch/Panels";
import { ClipPanel, PocketPanel, TasksPanel } from "./PocketClip";
import {
  CALL_START,
  COMPACT_SHAPE,
  CONTENT_FADE,
  FOCUS_START,
  FOCUS_TOTAL,
  HUD_EVENTS,
  INSTANT,
  ISLAND,
  LYRICS,
  RAIL,
  SCENES,
  SECONDARY_SHAPE,
  SHAPE,
  TRACK_LENGTH,
  TRACK_START,
  lyricAt,
  type Module,
  type Scene,
} from "./notch/spec";

const RAIL_ICON: Record<Module, LucideIcon> = {
  focus: Timer,
  music: Music,
  pocket: Inbox,
  clip: ClipboardList,
  tasks: ListTodo,
  call: Phone,
  hud: Volume2,
};

const STEP_VH = 55;
const CLIP_COUNT = 4;
const HUD_HOLD_MS = 1900;
const CLIP_HOLD_MS = 2200;

function stepAt(progress: number) {
  return clamp(Math.floor(progress * SCENES.length), 0, SCENES.length - 1);
}

function shapeOf(scene: Scene) {
  if (scene.view === "panel") {
    return { width: ISLAND.panelWidth, height: scene.lyrics ? ISLAND.lyricsHeight : ISLAND.panelHeight, radius: ISLAND.panelRadius };
  }
  if (scene.view === "hud") return { width: ISLAND.hudWidth, height: ISLAND.compactHeight, radius: ISLAND.compactRadius };
  if (scene.view === "call") return { width: ISLAND.callWidth, height: ISLAND.compactHeight, radius: ISLAND.compactRadius };
  return {
    width: scene.module === "music" ? ISLAND.musicWidth : ISLAND.focusWidth,
    height: ISLAND.compactHeight,
    radius: ISLAND.compactRadius,
  };
}

function useFitScale(ref: RefObject<HTMLDivElement | null>) {
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const update = () => setScale(Math.min(1, (node.clientWidth - 24) / ISLAND.panelWidth));
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, [ref]);
  return scale;
}

export function IslandStage() {
  const reduced = useReducedMotion() ?? false;
  const wrapperRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const scale = useFitScale(stageRef);
  const [index, setIndex] = useState(0);
  const [remaining, setRemaining] = useState(FOCUS_START);
  const [running, setRunning] = useState(true);
  const [elapsed, setElapsed] = useState(TRACK_START);
  const [playing, setPlaying] = useState(true);
  const [callSeconds, setCallSeconds] = useState(CALL_START);
  const [hudIndex, setHudIndex] = useState(0);
  const [copied, setCopied] = useState(0);
  const { scrollYProgress } = useScroll({ target: wrapperRef, offset: ["start start", "end end"] });

  useMotionValueEvent(scrollYProgress, "change", (progress) => setIndex(stepAt(progress)));

  useEffect(() => {
    setIndex(stepAt(scrollYProgress.get()));
  }, [scrollYProgress]);

  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => setRemaining((value) => Math.max(0, value - 1)), 1000);
    return () => window.clearInterval(id);
  }, [running]);

  useEffect(() => {
    if (remaining === 0) setRunning(false);
  }, [remaining]);

  useEffect(() => {
    if (!playing) return;
    const id = window.setInterval(
      () => setElapsed((value) => (value + 0.25 >= TRACK_LENGTH ? 0 : value + 0.25)),
      250,
    );
    return () => window.clearInterval(id);
  }, [playing]);

  useEffect(() => {
    const id = window.setInterval(() => setCallSeconds((value) => value + 1), 1000);
    return () => window.clearInterval(id);
  }, []);

  const scene = SCENES[index];

  useEffect(() => {
    if (scene.view !== "hud") return;
    setHudIndex(0);
    const id = window.setInterval(() => setHudIndex((value) => (value + 1) % HUD_EVENTS.length), HUD_HOLD_MS);
    return () => window.clearInterval(id);
  }, [scene.view]);

  useEffect(() => {
    if (scene.module !== "clip") return;
    setCopied(0);
    const id = window.setInterval(() => setCopied((value) => (value + 1) % CLIP_COUNT), CLIP_HOLD_MS);
    return () => window.clearInterval(id);
  }, [scene.module]);

  function toggleFocus() {
    if (remaining === 0) {
      setRemaining(FOCUS_TOTAL);
      setRunning(true);
      return;
    }
    setRunning((value) => !value);
  }

  function resetFocus() {
    setRemaining(FOCUS_TOTAL);
    setRunning(true);
  }

  function goTo(step: number) {
    const node = wrapperRef.current;
    if (!node) return;
    const range = Math.max(0, node.offsetHeight - window.innerHeight);
    const top = node.getBoundingClientRect().top + window.scrollY;
    window.scrollTo({ top: top + ((step + 0.5) / SCENES.length) * range, behavior: reduced ? "auto" : "smooth" });
  }

  const shape = shapeOf(scene);
  const lyric = lyricAt(elapsed);
  const contentKey = `${scene.view}-${scene.module}-${scene.lyrics ? "lyrics" : "plain"}`;

  function renderContent() {
    if (scene.view === "compact") {
      return scene.module === "music" ? (
        <MusicCompact lyric={LYRICS[lyric.index]} playing={playing} reduced={reduced} />
      ) : (
        <FocusCompact remaining={remaining} />
      );
    }
    if (scene.view === "call") return <CallCompact seconds={callSeconds} reduced={reduced} />;
    if (scene.view === "hud") return <HudPill event={HUD_EVENTS[hudIndex]} />;
    switch (scene.module) {
      case "focus":
        return (
          <FocusPanel
            remaining={remaining}
            running={running}
            onToggle={toggleFocus}
            onReset={resetFocus}
            onSkip={resetFocus}
          />
        );
      case "music":
        return (
          <MusicPanel
            playing={playing}
            elapsed={elapsed}
            lyricsOn={Boolean(scene.lyrics)}
            reduced={reduced}
            onToggle={() => setPlaying((value) => !value)}
            onPrev={() => setElapsed(0)}
            onNext={() => setElapsed(0)}
          />
        );
      case "pocket":
        return <PocketPanel reduced={reduced} />;
      case "clip":
        return <ClipPanel copied={copied} />;
      case "tasks":
        return <TasksPanel reduced={reduced} />;
      default:
        return null;
    }
  }

  return (
    <div ref={wrapperRef} className="relative" style={{ height: `${SCENES.length * STEP_VH}svh` }}>
      <div className="sticky top-0 flex h-svh flex-col items-center justify-center gap-6 overflow-hidden px-5 sm:px-8">
        <div
          ref={stageRef}
          className="relative h-[300px] w-full max-w-[1040px] overflow-hidden rounded-[32px] border border-stage-line bg-[radial-gradient(120%_90%_at_50%_0%,#1d2b47_0%,#0e1017_55%,#08090c_100%)]"
        >
          <div aria-hidden="true" className="absolute inset-x-0 top-0 flex h-10 items-center justify-between border-b border-white/5 bg-black/40 px-5 text-[12px]">
            <span className="font-semibold text-stage-ink/85">Explorer</span>
            <span className="hidden text-stage-muted tabular-nums sm:inline">Min 22 Nov 09:41</span>
          </div>
          <div
            className="absolute top-0 left-1/2 h-[300px] w-[380px]"
            style={{ transform: `translateX(-50%) scale(${scale})`, transformOrigin: "50% 0" }}
          >
            <motion.div
              className="absolute left-1/2 overflow-hidden bg-black ring-1 ring-inset ring-white/[0.14]"
              style={{ top: ISLAND.top, x: "-50%" }}
              initial={false}
              animate={{ width: shape.width, height: shape.height, borderRadius: shape.radius }}
              transition={reduced ? INSTANT : scene.view === "panel" ? SHAPE : COMPACT_SHAPE}
            >
              <AnimatePresence mode="wait" initial={false}>
                <motion.div
                  key={contentKey}
                  className="h-full w-full"
                  initial={reduced ? false : { opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={reduced ? { opacity: 0 } : { opacity: 0, y: -6 }}
                  transition={reduced ? INSTANT : CONTENT_FADE}
                >
                  {renderContent()}
                </motion.div>
              </AnimatePresence>
            </motion.div>
            <AnimatePresence initial={false}>
              {scene.secondary ? (
                <motion.div
                  key="secondary"
                  className="absolute left-1/2 grid size-9 place-items-center overflow-hidden rounded-full bg-black ring-1 ring-inset ring-white/[0.14]"
                  style={{ top: ISLAND.top, x: ISLAND.musicWidth / 2 + ISLAND.secondaryGap }}
                  initial={reduced ? false : { opacity: 0, scale: 0.6 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.6 }}
                  transition={reduced ? INSTANT : SECONDARY_SHAPE}
                >
                  <SecondaryFocus remaining={remaining} />
                </motion.div>
              ) : null}
            </AnimatePresence>
            {scene.view === "call" ? <PrivacyDot kind="mic" /> : null}
          </div>
        </div>

        <p className="min-h-6 text-center text-[15px] leading-6 text-stage-muted" aria-live="polite">
          <T id={scene.caption[0]} en={scene.caption[1]} />
        </p>

        <nav aria-label="Modul notch">
          <ol className="flex flex-wrap justify-center gap-2">
            {RAIL.map((item) => {
              const Icon = RAIL_ICON[item.module];
              const active = scene.module === item.module;
              return (
                <li key={item.module}>
                  <button
                    type="button"
                    onClick={() => goTo(item.step)}
                    aria-current={active ? "step" : undefined}
                    className={`inline-flex h-9 items-center gap-2 rounded-full px-3.5 text-[13px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                      active ? "bg-stage-ink text-stage" : "text-stage-muted ring-1 ring-inset ring-stage-line hover:text-stage-ink"
                    }`}
                  >
                    <Icon size={14} strokeWidth={1.9} aria-hidden="true" />
                    <T id={item.label[0]} en={item.label[1]} />
                  </button>
                </li>
              );
            })}
          </ol>
        </nav>
      </div>
    </div>
  );
}
