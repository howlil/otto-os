import { useId, useMemo, useState, type KeyboardEvent } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Code, Disc3, Folder, Globe, NotebookPen, Search, Settings2, Terminal } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../lib/T";

type App = {
  name: string;
  icon: LucideIcon;
  tint: string;
};

const APPS: App[] = [
  { name: "Explorer", icon: Folder, tint: "#5b9cff" },
  { name: "Browser", icon: Globe, tint: "#7fb3ff" },
  { name: "Terminal", icon: Terminal, tint: "#3a3a40" },
  { name: "Code", icon: Code, tint: "#2b8a6e" },
  { name: "Music", icon: Disc3, tint: "#fc3c54" },
  { name: "Notes", icon: NotebookPen, tint: "#ffb36b" },
  { name: "Settings", icon: Settings2, tint: "#8e8e96" },
];

export function Spotlight() {
  const reduced = useReducedMotion() ?? false;
  const inputId = useId();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [launched, setLaunched] = useState<string | null>(null);

  const results = useMemo(
    () => APPS.filter((app) => app.name.toLowerCase().includes(query.trim().toLowerCase())),
    [query],
  );

  function launch(app: App | undefined) {
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
    <div className="mx-auto flex w-full max-w-[600px] flex-col gap-3 text-left">
      <div className="overflow-hidden rounded-[24px] border border-stage-line bg-stage-panel shadow-[0_40px_80px_-40px_rgb(0_0_0/0.9)]">
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
                setQuery(event.target.value);
                setActive(0);
                setLaunched(null);
              }}
              onKeyDown={handleKeyDown}
              className="w-full bg-transparent text-[20px] text-stage-ink caret-[#75b0ff] outline-none"
            />
            {query.length === 0 && (
              <span className="pointer-events-none absolute inset-y-0 left-0 flex items-center text-[20px] text-stage-muted" aria-hidden="true">
                <T id="Cari aplikasi" en="Search apps" />
              </span>
            )}
          </div>
          <span className="hidden items-center gap-1 sm:flex" aria-hidden="true">
            <kbd className="rounded-md border border-stage-line px-1.5 py-0.5 text-[12px] text-stage-muted">Esc</kbd>
          </span>
        </div>

        <ul className="flex flex-col p-2">
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
                  transition={{ duration: 0.18 }}
                  aria-current={selected ? "true" : undefined}
                  onPointerEnter={() => setActive(index)}
                  onClick={() => launch(app)}
                  className={`flex cursor-default items-center gap-3 rounded-[14px] px-3 py-2.5 transition-colors ${
                    selected ? "bg-white/10" : ""
                  }`}
                >
                  <span className="grid size-9 place-items-center squircle text-white" style={{ background: app.tint }} aria-hidden="true">
                    <Icon size={18} strokeWidth={1.8} />
                  </span>
                  <span className="flex-1 text-[15px] text-stage-ink">{app.name}</span>
                  {selected && (
                    <span className="text-[12px] text-stage-muted" aria-hidden="true">
                      <T id="Buka" en="Open" />
                    </span>
                  )}
                </motion.li>
              );
            })}
          </AnimatePresence>
          {results.length === 0 && (
            <li className="px-3 py-6 text-center text-[14px] text-stage-muted">
              <T id="Tidak ada aplikasi yang cocok" en="No matching apps" />
            </li>
          )}
        </ul>

        <div className="flex min-h-11 items-center justify-between border-t border-stage-line px-5 text-[13px] text-stage-muted">
          <span aria-live="polite">
            {launched ? (
              <>
                <T id={`Membuka ${launched} (demo)`} en={`Opening ${launched} (demo)`} />
              </>
            ) : (
              <T id="Demo: tidak membuka aplikasi." en="Demo: no app is opened." />
            )}
          </span>
          <span className="hidden sm:inline">
            <T id="↑↓ pilih · Enter buka" en="↑↓ select · Enter open" />
          </span>
        </div>
      </div>
      <p className="text-center text-[13px] text-stage-muted">
        <T
          id="Spotlight mencari aplikasi terpasang di perangkat, bukan file atau hasil web."
          en="Spotlight searches apps installed on this device, not files or web results."
        />
      </p>
    </div>
  );
}
