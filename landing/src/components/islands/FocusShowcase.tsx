import type { LucideIcon } from "lucide-react";
import { ArrowLeft, Code2, Gamepad2, Globe, MessageCircle, Moon, Music, Play, Search, ShieldAlert } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useReducer, useState } from "react";
import { T } from "../lib/T";
import { FocusPanel } from "./notch/Panels";
import { formatMinutesBrief, PALETTE, WEEKLY_SCREEN_TIME } from "./notch/spec";
import { Switch } from "./Switch";

type Phase = "focus" | "break" | "rest";
type FocusTab = "block" | "activity";
type AppFilter = "blocked" | "recent" | "all";
type PreviewTarget = "work" | "youtube.com" | "discord";

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
const INITIAL: Cycle = { phase: "focus", completed: 1, minutes: 25, remaining: 25 * 60, running: true };

const BLOCK_CATALOG: { id: string; name: string; detail: string; icon: LucideIcon; tint: string; recent?: boolean }[] =
  [
    { id: "discord", name: "Discord", detail: "38m today", icon: MessageCircle, tint: "#5865f2", recent: true },
    { id: "youtube.com", name: "YouTube", detail: "youtube.com", icon: Play, tint: "#ff375f", recent: true },
    { id: "steam", name: "Steam", detail: "Launcher", icon: Gamepad2, tint: "#2c3e55", recent: true },
    { id: "x.com", name: "X / Twitter", detail: "x.com", icon: Globe, tint: "#3a3a3c", recent: true },
    { id: "riot", name: "Riot Client", detail: "Game client", icon: Gamepad2, tint: "#d13639" },
    { id: "spotify", name: "Spotify", detail: "1h 12m today", icon: Music, tint: "#1db954", recent: true },
    { id: "vscode", name: "VS Code", detail: "2h 14m today", icon: Code2, tint: "#0a84ff", recent: true },
  ];

const PREVIEW_TARGETS: [PreviewTarget, string][] = [
  ["work", "VS Code"],
  ["youtube.com", "YouTube"],
  ["discord", "Discord"],
];

const APP_FILTERS: [AppFilter, string][] = [
  ["blocked", "Blocked"],
  ["recent", "Recent"],
  ["all", "All apps"],
];

const ACTIVITY_STATS: [string, string, string][] = [
  ["Screen Time", "3h 48m", "Today"],
  ["Focus Share", "68%", "2h 35m"],
  ["Distractions", "4", "Shielded"],
];

const PEAK_MIN = Math.max(...WEEKLY_SCREEN_TIME.map((d) => d.totalMinutes));

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

export function FocusShowcase() {
  const reduced = useReducedMotion() ?? false;
  const [cycle, dispatch] = useReducer(reduce, INITIAL);
  const [tab, setTab] = useState<FocusTab>("block");
  const [filter, setFilter] = useState<AppFilter>("blocked");
  const [search, setSearch] = useState("");
  const [selectedDay, setSelectedDay] = useState<number>(6);
  const [previewTarget, setPreviewTarget] = useState<PreviewTarget>("youtube.com");

  const [blocked, setBlocked] = useState<Record<string, boolean>>({
    discord: true,
    "youtube.com": true,
    steam: true,
    "x.com": true,
    riot: true,
    spotify: false,
    vscode: false,
  });

  useEffect(() => {
    if (!cycle.running) return;
    const id = window.setInterval(() => dispatch({ type: "tick" }), 1000);
    return () => window.clearInterval(id);
  }, [cycle.running]);

  const focusing = cycle.running && cycle.phase === "focus";
  const shieldActive = focusing && previewTarget !== "work" && Boolean(blocked[previewTarget]);

  const filteredCatalog = BLOCK_CATALOG.filter((item) => {
    const q = search.trim().toLowerCase();
    if (q && !item.name.toLowerCase().includes(q) && !item.detail.toLowerCase().includes(q)) return false;
    if (filter === "blocked") return Boolean(blocked[item.id]);
    if (filter === "recent") return Boolean(item.recent);
    return true;
  });

  const activeDay = WEEKLY_SCREEN_TIME[selectedDay] ?? WEEKLY_SCREEN_TIME[6];

  return (
    <div className="grid gap-12 lg:grid-cols-[minmax(0,416px)_minmax(0,1fr)] lg:items-start lg:gap-16">
      <div className="flex min-w-0 flex-col gap-6">
        <div className="relative h-36 w-full max-w-[416px] overflow-hidden rounded-[28px] bg-black ring-1 ring-inset ring-white/[0.15]">
          <FocusPanel
            total={phaseSeconds(cycle)}
            remaining={cycle.remaining}
            running={cycle.running}
            onToggle={() => dispatch({ type: "toggle" })}
            onReset={() => dispatch({ type: "reset" })}
            onSkip={() => dispatch({ type: "skip" })}
          />
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {PRESETS.map((minutes) => {
            const on = cycle.minutes === minutes;
            return (
              <button
                key={minutes}
                type="button"
                aria-pressed={on}
                disabled={cycle.running || cycle.phase !== "focus"}
                onClick={() => dispatch({ type: "preset", minutes })}
                className={`h-8 rounded-full px-3.5 text-[12.5px] font-medium ring-1 ring-inset transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] disabled:opacity-40 ${
                  on
                    ? "bg-stage-ink text-stage ring-stage-ink"
                    : "text-stage-muted ring-stage-line hover:text-stage-ink"
                }`}
              >
                {minutes}m
              </button>
            );
          })}
          <span className="ml-auto flex gap-1.5" aria-hidden="true">
            {[0, 1, 2, 3].map((index) => (
              <span
                key={index}
                className={`size-2 rounded-full ${index < cycle.completed ? "bg-[#ff9230]" : "bg-stage-line"}`}
              />
            ))}
          </span>
        </div>

        <div className="flex flex-col gap-4 border-t border-stage-line pt-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="text-[13px] font-medium text-stage-muted">
              <T id="Uji Focus Shield" en="Test Focus Shield" />
            </p>
            <div className="flex items-center gap-1.5" role="group" aria-label="Test Focus Shield">
              {PREVIEW_TARGETS.map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  aria-pressed={previewTarget === id}
                  onClick={() => setPreviewTarget(id)}
                  className={`h-8 rounded-full px-3 text-[12.5px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                    previewTarget === id ? "bg-stage-ink text-stage" : "text-stage-muted hover:text-stage-ink"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {shieldActive ? (
            <motion.div
              key="focus-shield"
              initial={reduced ? false : { opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
              className="flex flex-wrap items-center gap-x-4 gap-y-3 rounded-[16px] bg-[#ff9230]/[0.1] p-4"
            >
              <span className="grid size-10 shrink-0 place-items-center rounded-full bg-[#ff9230]/[0.18] text-[#ff9230]">
                <ShieldAlert size={19} strokeWidth={2} aria-hidden="true" />
              </span>
              <div className="min-w-0 flex-1">
                <p className="text-[14.5px] font-semibold text-stage-ink">
                  {previewTarget === "youtube.com" ? "YouTube" : "Discord"}{" "}
                  <T id="diblokir saat Focus" en="paused during Focus" />
                </p>
                <p className="mt-0.5 text-[12.5px] text-stage-muted">
                  <T id="Tidak ada tab atau kerja yang ditutup." en="Nothing was closed. Step back cleanly." />
                </p>
              </div>
              <button
                type="button"
                onClick={() => setPreviewTarget("work")}
                className="inline-flex h-9 items-center gap-1.5 rounded-full bg-white px-5 text-[13px] font-semibold text-black transition-transform hover:bg-white/90 active:scale-95 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff]"
              >
                <ArrowLeft size={14} strokeWidth={2.2} aria-hidden="true" />
                Back
              </button>
            </motion.div>
          ) : (
            <div className="flex items-center gap-3 rounded-[16px] border border-stage-line p-4">
              <span
                className="size-2 shrink-0 rounded-full"
                style={{ background: focusing ? PALETTE.focus : PALETTE.muted }}
                aria-hidden="true"
              />
              <p className="min-w-0 flex-1 text-[13.5px] text-stage-ink">
                {previewTarget === "work" ? (
                  <T id="Ruang kerja tetap terbuka dan tenang." en="Your workspace stays open and undisturbed." />
                ) : (
                  <T
                    id="Situs dan aplikasi pengalih tidak ditutup paksa."
                    en="Blocked apps and tabs are never force-closed."
                  />
                )}
              </p>
              <span
                className="flex shrink-0 items-center gap-1.5 text-[12px] font-medium"
                style={{ color: focusing ? PALETTE.focus : PALETTE.muted }}
              >
                <Moon size={12} strokeWidth={2.2} aria-hidden="true" />
                {focusing ? "Focus On" : "Paused"}
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="min-w-0">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <p className="text-[22px] font-medium tracking-[-0.02em]">Focus</p>
            <p className="mt-1 text-[12.5px] tabular-nums text-stage-muted">3h 48m today · 2h 35m focus · 4 blocked</p>
          </div>
          <div
            className="inline-flex rounded-full border border-stage-line p-1"
            role="tablist"
            aria-label="Focus settings tabs"
          >
            {(
              [
                ["block", "Block apps"],
                ["activity", "Activity"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={tab === id}
                onClick={() => setTab(id)}
                className={`h-8 rounded-full px-3.5 text-[12.5px] font-medium transition-colors ${
                  tab === id ? "bg-stage-ink text-stage" : "text-stage-muted hover:text-stage-ink"
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        {tab === "block" ? (
          <div className="mt-6 flex flex-col gap-5">
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex min-w-[180px] flex-1 items-center gap-2 border-b border-stage-line pb-2">
                <Search size={14} className="shrink-0 text-stage-muted" aria-hidden="true" />
                <input
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  placeholder="Filter apps or sites"
                  aria-label="Filter apps or sites"
                  className="h-8 w-full bg-transparent text-[13.5px] text-stage-ink placeholder:text-stage-muted outline-none"
                />
              </div>
              <div className="flex items-center gap-1.5" role="group" aria-label="App filter">
                {APP_FILTERS.map(([id, label]) => (
                  <button
                    key={id}
                    type="button"
                    aria-pressed={filter === id}
                    onClick={() => setFilter(id)}
                    className={`h-8 rounded-full px-3 text-[12px] font-medium transition-colors ${
                      filter === id ? "bg-stage-ink text-stage" : "text-stage-muted hover:text-stage-ink"
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            <ul className="border-t border-stage-line">
              {filteredCatalog.map((item) => {
                const Icon = item.icon;
                return (
                  <li key={item.id} className="flex items-center gap-3.5 border-b border-stage-line py-3.5">
                    <span
                      className="grid size-9 shrink-0 place-items-center rounded-[10px] text-white"
                      style={{ background: item.tint }}
                      aria-hidden="true"
                    >
                      <Icon size={17} strokeWidth={1.9} />
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[14px] font-medium text-stage-ink">{item.name}</p>
                      <p className="truncate text-[12px] text-stage-muted">{item.detail}</p>
                    </div>
                    <Switch
                      tone="system-dark"
                      checked={Boolean(blocked[item.id])}
                      onChange={(next) => setBlocked((curr) => ({ ...curr, [item.id]: next }))}
                      label={[item.name, item.name]}
                    />
                  </li>
                );
              })}
            </ul>
            {filteredCatalog.length === 0 ? (
              <p className="py-6 text-center text-[13px] text-stage-muted">
                <T id="Tidak ada aplikasi yang cocok" en="No matching apps" />
              </p>
            ) : null}
            <p className="text-[12px] leading-[1.5] text-stage-muted">
              <T
                id="Aplikasi dan tab yang diblokir menampilkan Focus Shield dengan tombol Back selama Focus berjalan."
                en="Blocked apps and browser tabs show the Focus Shield overlay with a Back button while Focus runs."
              />
            </p>
          </div>
        ) : (
          <div className="mt-6 flex flex-col gap-6">
            <div className="grid grid-cols-3 border-y border-stage-line">
              {ACTIVITY_STATS.map(([label, val, sub], index) => (
                <div key={label} className={`py-4 ${index > 0 ? "border-l border-stage-line pl-4" : "pr-4"}`}>
                  <p className="text-[11.5px] text-stage-muted">{label}</p>
                  <p className="mt-1.5 text-[22px] font-medium tracking-[-0.02em] tabular-nums">{val}</p>
                  <p className="mt-0.5 text-[11.5px] text-stage-muted">{sub}</p>
                </div>
              ))}
            </div>

            <div>
              <div className="flex flex-wrap items-center justify-between gap-3 text-[12.5px]">
                <span className="font-medium tabular-nums text-stage-ink">
                  {activeDay.today ? `Today (${activeDay.day})` : activeDay.day} · Total{" "}
                  {formatMinutesBrief(activeDay.totalMinutes)} · Focus {formatMinutesBrief(activeDay.focusMinutes)}
                </span>
                <span className="flex items-center gap-3 text-[11.5px] text-stage-muted">
                  <span className="flex items-center gap-1.5">
                    <span className="size-2 rounded-full bg-[#0a84ff]" />
                    Screen
                  </span>
                  <span className="flex items-center gap-1.5">
                    <span className="size-2 rounded-full bg-[#ff9230]" />
                    Focus
                  </span>
                </span>
              </div>
              <div className="mt-5 flex h-[132px] items-end gap-2.5">
                {WEEKLY_SCREEN_TIME.map((day, idx) => {
                  const h = Math.round((day.totalMinutes / PEAK_MIN) * 104);
                  const fh = Math.round((day.focusMinutes / day.totalMinutes) * h);
                  const active = idx === selectedDay;
                  return (
                    <button
                      key={day.day}
                      type="button"
                      aria-pressed={active}
                      onClick={() => setSelectedDay(idx)}
                      className="flex flex-1 flex-col items-center gap-1.5 rounded-md py-1 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff]"
                    >
                      <div className="flex h-[106px] w-full items-end justify-center">
                        <div
                          className={`relative w-6 overflow-hidden rounded-[6px] transition-opacity ${
                            active ? "opacity-100" : "opacity-60 hover:opacity-100"
                          }`}
                          style={{ height: h, background: "#0a84ff" }}
                        >
                          <span
                            className="absolute inset-x-0 bottom-0 rounded-t-[3px]"
                            style={{ height: fh, background: "#ff9230" }}
                          />
                        </div>
                      </div>
                      <span className={`text-[11px] font-medium ${active ? "text-stage-ink" : "text-stage-muted"}`}>
                        {day.day}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>

            <p className="text-[12px] leading-[1.5] text-stage-muted">
              Stored locally in %LOCALAPPDATA%\otto-os\focus_activity.txt. Never leaves your PC.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
