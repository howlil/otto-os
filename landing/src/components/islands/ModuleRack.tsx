import type { LucideIcon } from "lucide-react";
import {
  ClipboardList,
  Inbox,
  LayoutGrid,
  Leaf,
  Minimize2,
  Music,
  PanelTop,
  PenLine,
  Pill,
  Search,
  SlidersHorizontal,
  Timer,
  WandSparkles,
} from "lucide-react";
import { useInView, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { T } from "../lib/T";
import { Switch } from "./Switch";

type Module = { label: string; icon: LucideIcon; on: boolean };

const MODULES: Module[] = [
  { label: "Top Bar", icon: PanelTop, on: true },
  { label: "Notch", icon: Pill, on: true },
  { label: "Dock", icon: LayoutGrid, on: true },
  { label: "Window Controls", icon: Minimize2, on: true },
  { label: "Genie", icon: WandSparkles, on: true },
  { label: "Spotlight", icon: Search, on: true },
  { label: "Control Center", icon: SlidersHorizontal, on: true },
  { label: "Now Playing", icon: Music, on: true },
  { label: "Focus", icon: Timer, on: true },
  { label: "Clipboard", icon: ClipboardList, on: true },
  { label: "Pocket", icon: Inbox, on: true },
  { label: "Quick Capture", icon: PenLine, on: false },
  { label: "Background Cleanup", icon: Leaf, on: false },
];

const DEMO: { index: number; to: boolean }[] = [
  { index: 11, to: true },
  { index: 4, to: false },
  { index: 4, to: true },
  { index: 11, to: false },
];

const TICK_MS = 1700;

export function ModuleRack() {
  const reduced = useReducedMotion() ?? false;
  const rootRef = useRef<HTMLDivElement>(null);
  const inView = useInView(rootRef, { amount: 0.3 });
  const [states, setStates] = useState<boolean[]>(() => MODULES.map((module) => module.on));
  const [demo, setDemo] = useState(0);
  const [auto, setAuto] = useState(true);

  useEffect(() => {
    if (!auto || !inView || reduced) return;
    const id = window.setTimeout(() => {
      const step = DEMO[demo % DEMO.length];
      setStates((curr) => curr.map((value, index) => (index === step.index ? step.to : value)));
      setDemo((value) => value + 1);
    }, TICK_MS);
    return () => window.clearTimeout(id);
  }, [auto, inView, reduced, demo]);

  function toggle(index: number, next: boolean) {
    setAuto(false);
    setStates((curr) => curr.map((value, idx) => (idx === index ? next : value)));
  }

  const active = states.filter(Boolean).length;

  return (
    <div ref={rootRef} className="mx-auto w-full max-w-[920px]">
      <div className="flex items-baseline gap-3 border-b border-line pb-8 tabular-nums" aria-live="polite">
        <span className="text-[64px] leading-none font-medium tracking-[-0.04em] text-ink sm:text-[80px]">
          {active}
        </span>
        <span className="text-[16px] text-ink-soft">
          / {MODULES.length} <T id="modul aktif" en="modules active" />
        </span>
      </div>

      <ul className="grid sm:grid-cols-2 sm:gap-x-12">
        {MODULES.map((module, index) => {
          const Icon = module.icon;
          const on = states[index];
          return (
            <li key={module.label} className="flex items-center gap-4 border-b border-line py-4">
              <span
                className={`grid size-9 shrink-0 place-items-center rounded-[10px] transition-colors ${
                  on ? "bg-accent-soft text-accent-text" : "bg-black/[0.04] text-muted"
                }`}
                aria-hidden="true"
              >
                <Icon size={17} strokeWidth={1.9} />
              </span>
              <span
                className={`min-w-0 flex-1 truncate text-[15px] font-medium transition-colors ${on ? "text-ink" : "text-muted"}`}
              >
                {module.label}
              </span>
              <Switch checked={on} onChange={(next) => toggle(index, next)} label={[module.label, module.label]} />
            </li>
          );
        })}
      </ul>
    </div>
  );
}
