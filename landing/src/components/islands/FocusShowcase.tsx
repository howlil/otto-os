import { useEffect, useReducer, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Code2, MessageCircle, Moon, Music } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../lib/T";
import { FocusPanel } from "./notch/Panels";
import { PALETTE } from "./notch/spec";
import { Badge, COLORS, Row, Section, WindowFrame } from "./OttoWindow";
import { SettingsSidebar } from "./OttoSettings";
import { Switch } from "./Switch";

type Phase = "focus" | "break" | "rest";

type Cycle = {
  phase: Phase;
  completed: number;
  minutes: number;
  remaining: number;
  running: boolean;
};

type Action =
  | { type: "toggle" }
  | { type: "reset" }
  | { type: "skip" }
  | { type: "preset"; minutes: number }
  | { type: "tick" };

const PRESETS = [15, 25, 45, 60];
const BREAK_SECONDS = 5 * 60;
const REST_SECONDS = 15 * 60;
const HUD_EVERY_MS = 6000;
const HUD_HOLD_MS = 2200;
const PRESET_APPS = ["Discord", "Steam", "Spotify"];
const PRESET_SITES = [
  { domain: "youtube.com", label: "YouTube" },
  { domain: "x.com", label: "X / Twitter" },
];
const NEUTRAL_TILE = "#3a3a3c";

const INITIAL: Cycle = { phase: "focus", completed: 0, minutes: 25, remaining: 25 * 60, running: false };

const APPS: { name: string; detail: string; icon: LucideIcon }[] = [
  { name: "Code", detail: "1h 12m open · 9x opened · 218 clicks · 2.4k keys (~480w)", icon: Code2 },
  { name: "Discord", detail: "38m open · 4x opened · 96 clicks · 740 keys (~148w)", icon: MessageCircle },
  { name: "Spotify", detail: "2h 03m open · 6x opened · 12 clicks · 88 keys (~18w)", icon: Music },
];

const SITES = [
  { domain: "youtube.com", detail: "42m active · 14 visits" },
  { domain: "github.com", detail: "1h 05m active · 31 visits" },
];

function phaseSeconds(cycle: Pick<Cycle, "phase" | "minutes">) {
  if (cycle.phase === "focus") return cycle.minutes * 60;
  return cycle.phase === "break" ? BREAK_SECONDS : REST_SECONDS;
}

function advance(cycle: Cycle): Cycle {
  if (cycle.phase === "focus") {
    const next = (cycle.completed % 4) + 1;
    if (next >= 4) return { ...cycle, phase: "rest", completed: 4, remaining: REST_SECONDS, running: false };
    return { ...cycle, phase: "break", completed: next, remaining: BREAK_SECONDS, running: false };
  }
  if (cycle.phase === "break") return { ...cycle, phase: "focus", remaining: cycle.minutes * 60, running: false };
  return { ...cycle, phase: "focus", completed: 0, remaining: cycle.minutes * 60, running: false };
}

function reduce(cycle: Cycle, action: Action): Cycle {
  switch (action.type) {
    case "toggle":
      return { ...cycle, running: !cycle.running };
    case "reset":
      return { ...cycle, running: false, remaining: phaseSeconds(cycle) };
    case "skip":
      return advance(cycle);
    case "preset":
      if (cycle.running || cycle.phase !== "focus") return cycle;
      return { ...cycle, minutes: action.minutes, remaining: action.minutes * 60 };
    case "tick":
      if (!cycle.running) return cycle;
      return cycle.remaining > 1 ? { ...cycle, remaining: cycle.remaining - 1 } : advance(cycle);
  }
}

function Value({ children }: { children: string }) {
  return <span className="shrink-0 text-[13px] font-semibold tabular-nums">{children}</span>;
}

function BlockControl({
  name,
  blocked,
  onToggle,
  badge = false,
}: {
  name: string;
  blocked: boolean;
  onToggle: (next: boolean) => void;
  badge?: boolean;
}) {
  return (
    <div className="flex shrink-0 items-center gap-3">
      {badge ? (
        <Badge theme="dark" tone={blocked ? "warning" : "good"}>
          {blocked ? "Blocked" : "Allowed"}
        </Badge>
      ) : null}
      <Switch tone="system-dark" checked={blocked} onChange={onToggle} label={[name, name]} />
    </div>
  );
}

export function FocusShowcase() {
  const [cycle, dispatch] = useReducer(reduce, INITIAL);
  const [focusModule, setFocusModule] = useState(true);
  const [notify, setNotify] = useState(true);
  const [calls, setCalls] = useState(true);
  const [blocked, setBlocked] = useState<Record<string, boolean>>({
    Discord: true,
    Code: false,
    Spotify: false,
    Steam: false,
    "youtube.com": true,
    "github.com": false,
    "x.com": false,
  });
  const [hud, setHud] = useState<{ id: number; app: string } | null>(null);

  const focusing = cycle.running && cycle.phase === "focus";
  const targets = PRESET_APPS.filter((app) => blocked[app]).join("|");

  useEffect(() => {
    if (!cycle.running) return;
    const id = window.setInterval(() => dispatch({ type: "tick" }), 1000);
    return () => window.clearInterval(id);
  }, [cycle.running]);

  useEffect(() => {
    if (!focusing || targets === "") {
      setHud(null);
      return;
    }
    const list = targets.split("|");
    let step = 0;
    const show = () => {
      setHud({ id: step, app: list[step % list.length] });
      step += 1;
    };
    show();
    const id = window.setInterval(show, HUD_EVERY_MS);
    return () => window.clearInterval(id);
  }, [focusing, targets]);

  useEffect(() => {
    if (!hud) return;
    const id = window.setTimeout(() => setHud(null), HUD_HOLD_MS);
    return () => window.clearTimeout(id);
  }, [hud]);

  const c = COLORS.dark;

  function setBlockedFor(key: string) {
    return (next: boolean) => setBlocked((current) => ({ ...current, [key]: next }));
  }

  return (
    <div className="grid gap-12 lg:grid-cols-[minmax(0,380px)_minmax(0,1fr)] lg:items-center lg:gap-16">
      <div className="flex flex-col gap-5">
        <div className="relative h-36 w-full max-w-[380px] overflow-hidden rounded-[28px] bg-black ring-1 ring-inset ring-white/[0.14]">
          <FocusPanel
            total={phaseSeconds(cycle)}
            remaining={cycle.remaining}
            running={cycle.running}
            onToggle={() => dispatch({ type: "toggle" })}
            onReset={() => dispatch({ type: "reset" })}
            onSkip={() => dispatch({ type: "skip" })}
          />
        </div>

        <div className="flex flex-wrap gap-2">
          {PRESETS.map((minutes) => {
            const on = cycle.minutes === minutes;
            return (
              <button
                key={minutes}
                type="button"
                aria-pressed={on}
                disabled={cycle.running || cycle.phase !== "focus"}
                onClick={() => dispatch({ type: "preset", minutes })}
                className={`h-8 rounded-full px-3.5 text-[13px] font-medium ring-1 ring-inset transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] disabled:opacity-40 ${
                  on ? "bg-stage-ink text-stage ring-stage-ink" : "text-stage-muted ring-stage-line hover:text-stage-ink"
                }`}
              >
                <T id={`${minutes} menit`} en={`${minutes} min`} />
              </button>
            );
          })}
        </div>

        <div className="flex items-center gap-3 text-[13px] text-stage-muted">
          <span className="flex gap-1.5" aria-hidden="true">
            {[0, 1, 2, 3].map((index) => (
              <span key={index} className={`size-2 rounded-full ${index < cycle.completed ? "bg-[#ff9230]" : "bg-stage-line"}`} />
            ))}
          </span>
          <T id="Empat sesi, lalu istirahat panjang." en="Four sessions, then a long break." />
        </div>

        <div className="flex h-9 items-center">
          <AnimatePresence mode="wait">
            {hud ? (
              <motion.div
                key={hud.id}
                initial={{ opacity: 0, y: -6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
                className="flex h-9 items-center gap-2 rounded-[16px] bg-black pr-1.5 pl-3 text-[13px] font-medium text-stage-ink ring-1 ring-inset ring-white/[0.14]"
              >
                <Moon size={14} strokeWidth={2} style={{ color: PALETTE.warning }} aria-hidden="true" />
                {hud.app}
                <span
                  className="rounded-full px-2 py-0.5 text-[11px] font-semibold"
                  style={{ color: PALETTE.warning, background: "rgb(255 159 10 / 0.18)" }}
                >
                  Blocked
                </span>
              </motion.div>
            ) : null}
          </AnimatePresence>
        </div>
      </div>

      <div className="flex min-w-0 flex-col gap-3">
        <WindowFrame
          theme="dark"
          title="Otto Settings"
          bodyClassName="h-[520px] lg:h-[560px]"
          sidebar={<SettingsSidebar theme="dark" selected="Focus" />}
        >
          <div className="flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-4 [scrollbar-color:rgb(255_255_255/0.2)_transparent] [scrollbar-width:thin]">
            <div className="px-1">
              <p className="text-[20px] font-semibold tracking-[-0.01em]">Focus & Screen Time</p>
              <p className="mt-1 text-[12px]" style={{ color: c.secondary }}>
                2h 14m active today · 1h 36m in Focus · 412 clicks · 3.1k keys
              </p>
            </div>

            <Section
              theme="dark"
              footer="Blocking applies while a Focus session is active. Screen time pauses after 3 minutes of inactivity."
            >
              <Row
                theme="dark"
                title="Focus & Pomodoro"
                subtitle="Pomodoro, timer & stopwatch tab"
                trailing={
                  <Switch
                    tone="system-dark"
                    checked={focusModule}
                    onChange={setFocusModule}
                    label={["Focus & Pomodoro", "Focus & Pomodoro"]}
                  />
                }
              />
              <Row
                theme="dark"
                title="Block Notifications in Focus"
                subtitle="Suppress Notch notification banners and Dock badges during Focus"
                trailing={
                  <Switch
                    tone="system-dark"
                    checked={notify}
                    onChange={setNotify}
                    label={["Block Notifications in Focus", "Block Notifications in Focus"]}
                  />
                }
              />
              <Row
                theme="dark"
                title="Allow Incoming Calls in Focus"
                subtitle="Let ringing calls through unless the calling app is blocked"
                trailing={
                  <Switch
                    tone="system-dark"
                    checked={calls}
                    onChange={setCalls}
                    label={["Allow Incoming Calls in Focus", "Allow Incoming Calls in Focus"]}
                  />
                }
              />
            </Section>

            <Section
              theme="dark"
              title="Today & 7-Day Activity"
              footer="Clicks and keystrokes count totals only. Typed characters and passwords are never recorded."
            >
              <Row
                theme="dark"
                title="Screen Time Today"
                subtitle="7-day total: 11h 02m across 5 day(s)"
                trailing={<Value>2h 14m</Value>}
              />
              <Row
                theme="dark"
                title="Focus Session Time"
                subtitle="7-day Focus: 6h 40m · 3 blocked today (9 in 7d)"
                trailing={<Value>1h 36m</Value>}
              />
              <Row theme="dark" title="Mouse Clicks" subtitle="7-day total: 3.8k clicks" trailing={<Value>412</Value>} />
              <Row
                theme="dark"
                title="Typing Activity"
                subtitle="7-day total: 21.4k keys (~4.3k words)"
                trailing={<Value>3.1k keys (~620w)</Value>}
              />
            </Section>

            <Section
              theme="dark"
              title="Most Used & Most Opened Apps"
              footer="Toggle any app switch to block and auto-minimize it during Focus."
            >
              {APPS.map((app) => (
                <Row
                  key={app.name}
                  theme="dark"
                  icon={app.icon}
                  tint={NEUTRAL_TILE}
                  title={app.name}
                  subtitle={app.detail}
                  trailing={
                    <BlockControl
                      name={app.name}
                      blocked={blocked[app.name] ?? false}
                      onToggle={setBlockedFor(app.name)}
                      badge
                    />
                  }
                />
              ))}
            </Section>

            <Section theme="dark" title="Top Websites" footer="Detected natively from browser tabs and address bars with zero extensions.">
              {SITES.map((site) => (
                <Row
                  key={site.domain}
                  theme="dark"
                  title={site.domain}
                  subtitle={site.detail}
                  trailing={
                    <BlockControl
                      name={site.domain}
                      blocked={blocked[site.domain] ?? false}
                      onToggle={setBlockedFor(site.domain)}
                      badge
                    />
                  }
                />
              ))}
            </Section>

            <Section
              theme="dark"
              title="Focus Blocklist & Distraction Presets"
              footer="Blocked apps and websites minimize automatically without closing unsaved work."
            >
              <Row
                theme="dark"
                title="+ Block App from File..."
                subtitle="Choose an .exe application to auto-minimize during Focus"
              />
              <Row
                theme="dark"
                title="+ Block Website from Clipboard"
                subtitle="Paste a copied URL or domain into the Focus blocklist"
              />
              {PRESET_APPS.map((name) => (
                <Row
                  key={name}
                  theme="dark"
                  title={`App: ${name}`}
                  subtitle="Auto-minimize this app while Focus Mode runs"
                  trailing={
                    <BlockControl name={`App: ${name}`} blocked={blocked[name] ?? false} onToggle={setBlockedFor(name)} />
                  }
                />
              ))}
              {PRESET_SITES.map((site) => (
                <Row
                  key={site.domain}
                  theme="dark"
                  title={`${site.label} (${site.domain})`}
                  subtitle="Block and minimize browser tabs on this domain during Focus"
                  trailing={
                    <BlockControl
                      name={site.label}
                      blocked={blocked[site.domain] ?? false}
                      onToggle={setBlockedFor(site.domain)}
                    />
                  }
                />
              ))}
            </Section>

            <Section theme="dark" title="Activity Data" footer={"Saved locally in %LOCALAPPDATA%\\otto-os\\focus_activity.txt."}>
              <Row
                theme="dark"
                title="Reset Activity & Screen Time Stats"
                subtitle="Clear Today and 7-day history while keeping your blocklist"
                trailing={<Value>Clear</Value>}
              />
            </Section>
          </div>
        </WindowFrame>
        <p className="px-1 text-[12px]" style={{ color: c.secondary }}>
          <T id="Data demo · tidak dibaca dari perangkat" en="Demo data · not read from your device" />
        </p>
      </div>
    </div>
  );
}
