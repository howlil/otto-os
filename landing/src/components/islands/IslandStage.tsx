import type { LucideIcon } from "lucide-react";
import {
  BarChart3,
  ClipboardList,
  Columns2,
  Inbox,
  ListTodo,
  Mic,
  MicOff,
  Music,
  Phone,
  Timer,
  Volume2,
} from "lucide-react";
import { AnimatePresence, motion, useInView, useReducedMotion } from "motion/react";
import { type RefObject, useEffect, useRef, useState } from "react";
import { T } from "../lib/T";
import {
  CallCompact,
  FocusCompact,
  HudPill,
  MusicCompact,
  PrivacyDot,
  RingingCallCard,
  SecondaryFocus,
  SecondaryMusic,
} from "./notch/Compact";
import { FocusPanel, MusicPanel } from "./notch/Panels";
import { ScreenTimePanel } from "./notch/ScreenTimePanel";
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
  lyricAt,
  type NotchStyleMode,
  type NotchTabKey,
  type NotchWorkspace,
  SECONDARY_SHAPE,
  SHAPE,
  TRACK_LENGTH,
  TRACK_START,
  WORKSPACES,
} from "./notch/spec";
import { ClipPanel, PocketPanel, TasksPanel } from "./PocketClip";

const WORKSPACE_ICON: Record<NotchWorkspace, LucideIcon> = {
  focus: Timer,
  time: BarChart3,
  music: Music,
  split: Columns2,
  pocket: Inbox,
  clip: ClipboardList,
  tasks: ListTodo,
  call: Phone,
  hud: Volume2,
};

function useFitScale(ref: RefObject<HTMLDivElement | null>, width: number) {
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const update = () => setScale(Math.min(1, (node.clientWidth - 28) / width));
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, [ref, width]);
  return scale;
}

export function IslandStage() {
  const reduced = useReducedMotion() ?? false;
  const stageRef = useRef<HTMLDivElement>(null);
  const scale = useFitScale(stageRef, ISLAND.panelWidth + 24);

  const [workspace, setWorkspace] = useState<NotchWorkspace>("time");
  const [styleMode, setStyleMode] = useState<NotchStyleMode>("island");

  const [remaining, setRemaining] = useState(FOCUS_START);
  const [running, setRunning] = useState(true);
  const [activeTask, setActiveTask] = useState<[string, string] | null>([
    "Finalisasi shader Liquid Glass",
    "Finalize Liquid Glass shader",
  ]);

  const [elapsed, setElapsed] = useState(TRACK_START);
  const [playing, setPlaying] = useState(true);
  const [lyricsOn, setLyricsOn] = useState(true);

  const [splitPrimary, setSplitPrimary] = useState<"music" | "focus">("music");
  const [tasksComposing, setTasksComposing] = useState(false);

  const [callStage, setCallStage] = useState<"ringing" | "active">("ringing");
  const [callSeconds, setCallSeconds] = useState(CALL_START);
  const [micMuted, setMicMuted] = useState(false);
  const [privacyKind, setPrivacyKind] = useState<"camera" | "mic" | "location">("camera");

  const [hudIndex, setHudIndex] = useState(0);

  const inView = useInView(stageRef, { amount: 0.3 });
  const [auto, setAuto] = useState(true);

  useEffect(() => {
    if (!auto || !inView || reduced) return;
    const id = window.setInterval(() => {
      setWorkspace((curr) => {
        const index = WORKSPACES.findIndex((item) => item.id === curr);
        return WORKSPACES[(index + 1) % WORKSPACES.length].id;
      });
    }, 3800);
    return () => window.clearInterval(id);
  }, [auto, inView, reduced]);

  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => {
      setRemaining((value) => (value > 0 ? value - 1 : FOCUS_TOTAL));
    }, 1000);
    return () => window.clearInterval(id);
  }, [running]);

  useEffect(() => {
    if (!playing) return;
    const id = window.setInterval(() => setElapsed((value) => (value + 0.25 >= TRACK_LENGTH ? 0 : value + 0.25)), 250);
    return () => window.clearInterval(id);
  }, [playing]);

  useEffect(() => {
    if (callStage !== "active") return;
    const id = window.setInterval(() => setCallSeconds((value) => value + 1), 1000);
    return () => window.clearInterval(id);
  }, [callStage]);

  function handleSelectTab(tab: NotchTabKey) {
    setAuto(false);
    setTasksComposing(false);
    setWorkspace(tab);
  }

  const isClassic = styleMode === "classic";
  const topOffset = isClassic ? 0 : ISLAND.top;
  const compactHeight = isClassic ? ISLAND.classicCompactHeight : ISLAND.compactHeight;

  function shapeOf() {
    const expandedRadius = isClassic ? ISLAND.classicPanelRadius : ISLAND.panelRadius;
    const compactRadius = isClassic ? ISLAND.classicCompactRadius : ISLAND.compactRadius;

    if (workspace === "split") {
      return {
        width: splitPrimary === "music" ? ISLAND.musicWidth : isClassic ? ISLAND.classicFocusWidth : ISLAND.focusWidth,
        height: compactHeight,
        radius: compactRadius,
        expanded: false,
      };
    }
    if (workspace === "hud") {
      return { width: ISLAND.hudWidth, height: compactHeight, radius: compactRadius, expanded: false };
    }
    if (workspace === "call") {
      if (callStage === "ringing") {
        return { width: ISLAND.ringingWidth, height: ISLAND.ringingHeight, radius: expandedRadius, expanded: true };
      }
      return { width: ISLAND.callWidth, height: compactHeight, radius: compactRadius, expanded: false };
    }
    if (workspace === "time") {
      return { width: ISLAND.panelWidth, height: ISLAND.statsHeight, radius: expandedRadius, expanded: true };
    }
    if (workspace === "music") {
      return {
        width: ISLAND.panelWidth,
        height: lyricsOn ? ISLAND.lyricsHeight : ISLAND.panelHeight,
        radius: expandedRadius,
        expanded: true,
      };
    }
    if (workspace === "tasks") {
      return {
        width: ISLAND.panelWidth,
        height: tasksComposing ? ISLAND.tasksComposeHeight : ISLAND.panelHeight,
        radius: expandedRadius,
        expanded: true,
      };
    }
    return { width: ISLAND.panelWidth, height: ISLAND.panelHeight, radius: expandedRadius, expanded: true };
  }

  const shape = shapeOf();
  const lyric = lyricAt(elapsed);
  const currentMeta = WORKSPACES.find((w) => w.id === workspace) ?? WORKSPACES[0];
  const contentKey = `${workspace}-${workspace === "split" ? splitPrimary : workspace === "call" ? callStage : "main"}`;

  function renderNotchContent() {
    switch (workspace) {
      case "focus":
        return (
          <FocusPanel
            remaining={remaining}
            running={running}
            activeTask={activeTask}
            onToggle={() => setRunning((v) => !v)}
            onReset={() => setRemaining(FOCUS_TOTAL)}
            onSkip={() => setRemaining(FOCUS_TOTAL)}
            onSelectTab={handleSelectTab}
          />
        );
      case "time":
        return <ScreenTimePanel onSelectTab={handleSelectTab} />;
      case "music":
        return (
          <MusicPanel
            playing={playing}
            elapsed={elapsed}
            lyricsOn={lyricsOn}
            reduced={reduced}
            onToggle={() => setPlaying((v) => !v)}
            onPrev={() => setElapsed(0)}
            onNext={() => setElapsed(0)}
            onToggleLyrics={() => setLyricsOn((v) => !v)}
            onSeek={(sec) => setElapsed(sec)}
            onSelectTab={handleSelectTab}
          />
        );
      case "split":
        return splitPrimary === "music" ? (
          <MusicCompact lyric={LYRICS[lyric.index]} playing={playing} reduced={reduced} />
        ) : (
          <FocusCompact remaining={remaining} />
        );
      case "pocket":
        return <PocketPanel reduced={reduced} onSelectTab={handleSelectTab} />;
      case "clip":
        return <ClipPanel onSelectTab={handleSelectTab} />;
      case "tasks":
        return (
          <TasksPanel
            reduced={reduced}
            composing={tasksComposing}
            onToggleCompose={setTasksComposing}
            onSelectTab={handleSelectTab}
            onActiveTaskChange={setActiveTask}
          />
        );
      case "call":
        return callStage === "ringing" ? (
          <RingingCallCard
            video
            reduced={reduced}
            onAccept={() => setCallStage("active")}
            onDecline={() => setCallStage("active")}
          />
        ) : (
          <CallCompact seconds={callSeconds} muted={micMuted} reduced={reduced} />
        );
      case "hud":
        return <HudPill event={HUD_EVENTS[hudIndex]} />;
    }
  }

  return (
    <div className="mx-auto w-full max-w-[1200px] px-5 pt-16 pb-16 sm:px-8 sm:pt-24 sm:pb-20">
      <div data-reveal suppressHydrationWarning className="flex flex-col items-center gap-8 text-center">
        <div className="max-w-[680px]">
          <h2 className="text-[clamp(2.25rem,4.4vw,3.5rem)] leading-[1.04] font-medium tracking-[-0.035em] text-stage-ink text-balance">
            <T id="Satu notch, semua ruang kerjanya." en="One notch, every workspace." />
          </h2>
          <p
            className="mx-auto mt-5 max-w-[560px] text-[19px] leading-[1.5] text-stage-muted text-pretty"
            aria-live="polite"
          >
            <T id={currentMeta.summary[0]} en={currentMeta.summary[1]} />
          </p>
        </div>

        <div
          className="inline-flex rounded-full bg-white/[0.08] p-1 ring-1 ring-inset ring-white/10"
          role="group"
          aria-label="Notch bezel style"
        >
          <button
            type="button"
            aria-pressed={styleMode === "island"}
            onClick={() => setStyleMode("island")}
            className={`h-8 rounded-full px-3.5 text-[13px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
              styleMode === "island" ? "bg-stage-ink text-stage" : "text-stage-muted hover:text-stage-ink"
            }`}
          >
            Dynamic Island
          </button>
          <button
            type="button"
            aria-pressed={styleMode === "classic"}
            onClick={() => setStyleMode("classic")}
            className={`h-8 rounded-full px-3.5 text-[13px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
              styleMode === "classic" ? "bg-stage-ink text-stage" : "text-stage-muted hover:text-stage-ink"
            }`}
          >
            Classic Notch
          </button>
        </div>
      </div>

      <div ref={stageRef} className="relative mt-8 h-[300px] w-full overflow-hidden">
        <div
          className="absolute top-0 left-1/2 h-[310px] w-[440px]"
          style={{ transform: `translateX(-50%) scale(${scale})`, transformOrigin: "50% 0" }}
        >
          <motion.div
            className="absolute left-1/2 overflow-hidden bg-black ring-1 ring-inset ring-white/[0.15]"
            style={{ top: topOffset, x: "-50%" }}
            initial={false}
            animate={{
              width: shape.width,
              height: shape.height,
              borderTopLeftRadius: isClassic ? 0 : shape.radius,
              borderTopRightRadius: isClassic ? 0 : shape.radius,
              borderBottomLeftRadius: shape.radius,
              borderBottomRightRadius: shape.radius,
            }}
            transition={reduced ? INSTANT : shape.expanded ? SHAPE : COMPACT_SHAPE}
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={contentKey}
                className="h-full w-full"
                initial={reduced ? false : { opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={reduced ? { opacity: 0 } : { opacity: 0, y: -5 }}
                transition={reduced ? INSTANT : CONTENT_FADE}
              >
                {renderNotchContent()}
              </motion.div>
            </AnimatePresence>
          </motion.div>

          <AnimatePresence initial={false}>
            {workspace === "split" ? (
              <motion.button
                key="secondary-circle"
                type="button"
                onClick={() => setSplitPrimary((curr) => (curr === "music" ? "focus" : "music"))}
                className="absolute left-1/2 grid place-items-center overflow-hidden rounded-full bg-black ring-1 ring-inset ring-white/[0.16] transition-transform hover:scale-105 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
                style={{
                  top: topOffset + (isClassic ? 2 : 0),
                  width: isClassic ? 28 : 36,
                  height: isClassic ? 28 : 36,
                  x: shape.width / 2 + ISLAND.secondaryGap,
                }}
                initial={reduced ? false : { opacity: 0, scale: 0.6 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.6 }}
                transition={reduced ? INSTANT : SECONDARY_SHAPE}
              >
                {splitPrimary === "music" ? (
                  <SecondaryFocus remaining={remaining} />
                ) : (
                  <SecondaryMusic playing={playing} reduced={reduced} />
                )}
                <span className="sr-only">
                  <T id="Tukar aktivitas utama dan sekunder" en="Swap primary and secondary activity" />
                </span>
              </motion.button>
            ) : null}
          </AnimatePresence>

          {workspace === "call" && callStage === "active" ? (
            <PrivacyDot kind={privacyKind} left={220 + shape.width / 2 + 12} top={topOffset + compactHeight / 2 - 3} />
          ) : null}
        </div>

        <div className="absolute inset-x-0 bottom-4 flex flex-wrap items-center justify-center gap-2 px-4">
          {workspace === "split" ? (
            <button
              type="button"
              onClick={() => setSplitPrimary((curr) => (curr === "music" ? "focus" : "music"))}
              className="inline-flex h-8 items-center gap-2 rounded-full bg-white/[0.1] px-3.5 text-[12px] font-medium text-stage-ink ring-1 ring-inset ring-white/15 transition-colors hover:bg-white/[0.16]"
            >
              <Columns2 size={13} aria-hidden="true" />
              <T
                id={splitPrimary === "music" ? "Tukar ke Focus utama" : "Tukar ke Musik utama"}
                en={splitPrimary === "music" ? "Swap Focus to main" : "Swap Music to main"}
              />
            </button>
          ) : null}

          {workspace === "call" ? (
            <div className="flex flex-wrap items-center justify-center gap-2">
              <div className="inline-flex rounded-full bg-white/[0.08] p-0.5 ring-1 ring-inset ring-white/12">
                <button
                  type="button"
                  aria-pressed={callStage === "ringing"}
                  onClick={() => setCallStage("ringing")}
                  className={`h-7 rounded-full px-3 text-[12px] font-medium ${callStage === "ringing" ? "bg-stage-ink text-stage" : "text-stage-muted"}`}
                >
                  <T id="Panggilan Masuk" en="Incoming" />
                </button>
                <button
                  type="button"
                  aria-pressed={callStage === "active"}
                  onClick={() => setCallStage("active")}
                  className={`h-7 rounded-full px-3 text-[12px] font-medium ${callStage === "active" ? "bg-stage-ink text-stage" : "text-stage-muted"}`}
                >
                  <T id="Aktif" en="In Call" />
                </button>
              </div>
              {callStage === "active" ? (
                <>
                  <button
                    type="button"
                    aria-pressed={micMuted}
                    onClick={() => setMicMuted((v) => !v)}
                    className={`inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-[12px] font-medium ring-1 ring-inset ${
                      micMuted
                        ? "bg-[#ff453a]/25 text-[#ff8a82] ring-[#ff453a]/40"
                        : "bg-white/[0.08] text-stage-ink ring-white/12"
                    }`}
                  >
                    {micMuted ? <MicOff size={12} aria-hidden="true" /> : <Mic size={12} aria-hidden="true" />}
                    {micMuted ? "Muted" : "Mic On"}
                  </button>
                  <div className="inline-flex rounded-full bg-white/[0.08] p-0.5 ring-1 ring-inset ring-white/12">
                    {(["camera", "mic", "location"] as const).map((kind) => (
                      <button
                        key={kind}
                        type="button"
                        aria-pressed={privacyKind === kind}
                        onClick={() => setPrivacyKind(kind)}
                        className={`h-7 rounded-full px-2.5 text-[11.5px] font-medium capitalize ${
                          privacyKind === kind ? "bg-white/18 text-stage-ink" : "text-stage-muted hover:text-stage-ink"
                        }`}
                      >
                        {kind}
                      </button>
                    ))}
                  </div>
                </>
              ) : null}
            </div>
          ) : null}

          {workspace === "hud" ? (
            <div className="flex flex-wrap items-center justify-center gap-1.5">
              {HUD_EVENTS.map((ev, idx) => (
                <button
                  key={ev.id}
                  type="button"
                  aria-pressed={hudIndex === idx}
                  onClick={() => setHudIndex(idx)}
                  className={`h-7 rounded-full px-2.5 text-[11.5px] font-medium transition-colors ${
                    hudIndex === idx
                      ? "bg-stage-ink text-stage"
                      : "bg-white/[0.07] text-stage-muted hover:bg-white/[0.12] hover:text-stage-ink"
                  }`}
                >
                  <T id={ev.label[0]} en={ev.label[1]} />
                </button>
              ))}
            </div>
          ) : null}
        </div>
      </div>

      <nav aria-label="Notch workspaces" className="mt-6">
        <ul className="flex flex-wrap justify-center gap-2">
          {WORKSPACES.map((item) => {
            const Icon = WORKSPACE_ICON[item.id];
            const active = workspace === item.id;
            return (
              <li key={item.id}>
                <button
                  type="button"
                  aria-pressed={active}
                  onClick={() => {
                    setAuto(false);
                    setTasksComposing(false);
                    setWorkspace(item.id);
                  }}
                  className={`inline-flex h-10 items-center gap-2 rounded-full px-4 text-[13.5px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                    active
                      ? "bg-stage-ink text-stage"
                      : "text-stage-muted ring-1 ring-inset ring-stage-line hover:text-stage-ink"
                  }`}
                >
                  <Icon size={14} strokeWidth={1.9} aria-hidden="true" />
                  <T id={item.label[0]} en={item.label[1]} />
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
    </div>
  );
}
