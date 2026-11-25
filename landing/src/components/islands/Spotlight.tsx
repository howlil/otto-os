import type { LucideIcon } from "lucide-react";
import { Code, Disc3, Folder, Globe, LayoutGrid, List, NotebookPen, Search, Settings2, Terminal } from "lucide-react";
import { AnimatePresence, motion, useInView, useReducedMotion } from "motion/react";
import { type KeyboardEvent, useEffect, useId, useMemo, useRef, useState } from "react";
import { T } from "../lib/T";

type AppEntry = {
  name: string;
  category: string;
  icon: LucideIcon;
  tint: string;
};

const APPS: AppEntry[] = [
  { name: "Explorer", category: "System", icon: Folder, tint: "#5b9cff" },
  { name: "Browser", category: "Internet", icon: Globe, tint: "#7fb3ff" },
  { name: "Terminal", category: "Developer", icon: Terminal, tint: "#3a3a40" },
  { name: "VS Code", category: "Developer", icon: Code, tint: "#2b8a6e" },
  { name: "Spotify", category: "Media", icon: Disc3, tint: "#fc3c54" },
  { name: "Notes", category: "Productivity", icon: NotebookPen, tint: "#ffb36b" },
  { name: "Otto Settings", category: "System", icon: Settings2, tint: "#8e8e96" },
];

const DEMO: { wait: number; query: string; launch: string | null }[] = [
  { wait: 900, query: "v", launch: null },
  { wait: 650, query: "vs", launch: null },
  { wait: 1300, query: "vs", launch: "VS Code" },
  { wait: 2600, query: "", launch: null },
];

export function Spotlight() {
  const reduced = useReducedMotion() ?? false;
  const inputId = useId();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [launched, setLaunched] = useState<string | null>(null);
  const [grid, setGrid] = useState(false);
  const stageRef = useRef<HTMLDivElement>(null);
  const inView = useInView(stageRef, { amount: 0.4 });
  const [auto, setAuto] = useState(true);
  const [beat, setBeat] = useState(0);

  useEffect(() => {
    if (!auto || !inView || reduced) return;
    const step = DEMO[beat % DEMO.length];
    const id = window.setTimeout(() => {
      setQuery(step.query);
      setActive(0);
      setLaunched(step.launch);
      setBeat((value) => value + 1);
    }, step.wait);
    return () => window.clearTimeout(id);
  }, [auto, inView, reduced, beat]);

  const results = useMemo(
    () => APPS.filter((app) => app.name.toLowerCase().includes(query.trim().toLowerCase())),
    [query],
  );

  function launch(app: AppEntry | undefined) {
    if (!app) return;
    setLaunched(app.name);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((value) => (results.length ? (value + 1) % results.length : 0));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((value) => (results.length ? (value - 1 + results.length) % results.length : 0));
    } else if (event.key === "Tab") {
      event.preventDefault();
      setGrid((value) => !value);
    } else if (event.key === "Enter") {
      event.preventDefault();
      launch(results[active]);
    } else if (event.key === "Escape") {
      setQuery("");
      setLaunched(null);
      setActive(0);
    }
  }

  return (
    <div ref={stageRef} className="mx-auto w-full max-w-[620px] text-left">
      <div>
        <div className="relative flex items-center gap-3 border-b border-stage-line px-5 py-4">
          <Search size={20} strokeWidth={1.8} className="shrink-0 text-stage-muted" aria-hidden="true" />
          <label htmlFor={inputId} className="sr-only">
            <T id="Cari aplikasi lokal" en="Search local apps" />
          </label>
          <div className="relative min-w-0 flex-1">
            <input
              id={inputId}
              type="text"
              value={query}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => {
                setAuto(false);
                setQuery(event.target.value);
                setActive(0);
                setLaunched(null);
              }}
              onKeyDown={handleKeyDown}
              className="w-full bg-transparent text-[19px] text-stage-ink caret-[#75b0ff] outline-none"
            />
            {query.length === 0 && (
              <span
                className="pointer-events-none absolute inset-y-0 left-0 flex items-center text-[19px] text-stage-muted"
                aria-hidden="true"
              >
                <T id="Cari aplikasi terpasang" en="Search installed apps" />
              </span>
            )}
          </div>
          <div
            className="flex items-center gap-1 rounded-full bg-white/[0.07] p-0.5"
            role="group"
            aria-label="Spotlight view"
          >
            <button
              type="button"
              aria-pressed={!grid}
              onClick={() => setGrid(false)}
              className={`grid size-7 place-items-center rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                grid ? "text-stage-muted hover:text-stage-ink" : "bg-white/16 text-stage-ink"
              }`}
            >
              <List size={15} strokeWidth={1.9} aria-hidden="true" />
              <span className="sr-only">
                <T id="Tampilan daftar" en="List view" />
              </span>
            </button>
            <button
              type="button"
              aria-pressed={grid}
              onClick={() => setGrid(true)}
              className={`grid size-7 place-items-center rounded-full transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#75b0ff] ${
                grid ? "bg-white/16 text-stage-ink" : "text-stage-muted hover:text-stage-ink"
              }`}
            >
              <LayoutGrid size={15} strokeWidth={1.9} aria-hidden="true" />
              <span className="sr-only">
                <T id="Tampilan kisi" en="Grid view" />
              </span>
            </button>
          </div>
        </div>

        <ul className={grid ? "grid grid-cols-4 gap-1.5 p-3.5" : "flex flex-col p-2"}>
          <AnimatePresence initial={false}>
            {results.map((app, index) => {
              const Icon = app.icon;
              const selected = index === active;
              return (
                <motion.li
                  key={app.name}
                  layout={!reduced}
                  initial={reduced ? false : { y: -4 }}
                  animate={{ y: 0 }}
                  exit={{ y: -4 }}
                  transition={{ duration: 0.16 }}
                  aria-current={selected ? "true" : undefined}
                  onPointerEnter={() => setActive(index)}
                  onClick={() => launch(app)}
                  className={
                    grid
                      ? `flex cursor-pointer flex-col items-center gap-2 rounded-[14px] px-1 py-3 transition-colors ${selected ? "bg-white/10" : ""}`
                      : `flex cursor-pointer items-center gap-3 rounded-[14px] px-3 py-2.5 transition-colors ${selected ? "bg-white/10" : ""}`
                  }
                >
                  <span
                    className="grid size-9 place-items-center squircle text-white"
                    style={{ background: app.tint }}
                    aria-hidden="true"
                  >
                    <Icon size={18} strokeWidth={1.8} />
                  </span>
                  <span
                    className={
                      grid
                        ? "w-full truncate text-center text-[12px] text-stage-ink"
                        : "flex-1 text-[15px] font-medium text-stage-ink"
                    }
                  >
                    {app.name}
                  </span>
                  {!grid ? (
                    <span className="text-[12px] text-stage-muted">
                      {launched === app.name ? <T id="Diluncurkan" en="Launched" /> : app.category}
                    </span>
                  ) : null}
                </motion.li>
              );
            })}
          </AnimatePresence>
          {results.length === 0 && (
            <li className="col-span-full px-3 py-6 text-center text-[14px] text-stage-muted">
              <T id="Tidak ada aplikasi yang cocok" en="No matching apps" />
            </li>
          )}
        </ul>

        <div className="flex min-h-10 items-center justify-between border-t border-stage-line px-5 text-[12px] text-stage-muted">
          <span className="font-mono">shell:AppsFolder</span>
          <span>↑↓ · Tab · Enter · Esc</span>
        </div>
      </div>
    </div>
  );
}
