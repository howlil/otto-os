import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { motion, useAnimationControls, useReducedMotion } from "motion/react";
import type { LucideIcon } from "lucide-react";
import { Code, Disc3, Download, FileText, Folder, Globe, NotebookPen, Search, Terminal, Trash } from "lucide-react";
import { T } from "../lib/T";

type DockApp = {
  name: [string, string];
  icon: LucideIcon;
  tint: string;
};

const APPS: DockApp[] = [
  { name: ["Explorer", "Explorer"], icon: Folder, tint: "#5b9cff" },
  { name: ["Browser", "Browser"], icon: Globe, tint: "#7fb3ff" },
  { name: ["Terminal", "Terminal"], icon: Terminal, tint: "#1b1b1d" },
  { name: ["Kode", "Code"], icon: Code, tint: "#2b8a6e" },
  { name: ["Musik", "Music"], icon: Disc3, tint: "#fc3c54" },
  { name: ["Catatan", "Notes"], icon: NotebookPen, tint: "#ffb36b" },
];

const UTILITIES: DockApp[] = [
  { name: ["Unduhan", "Downloads"], icon: Download, tint: "#5ec9a7" },
  { name: ["Tempat sampah", "Recycle Bin"], icon: Trash, tint: "#8fa3b8" },
];

const TILES = [...APPS, ...UTILITIES];
const MAX_SCALE = 1.5;
const REACH = 120;
const SPRING = { type: "spring" as const, stiffness: 380, damping: 26, mass: 0.6 };
const GENIE = { duration: 0.7, ease: [0.45, 0, 0.2, 1] as [number, number, number, number] };
const RESTORE = { duration: 0.6, ease: [0.45, 0, 0.2, 1] as [number, number, number, number] };

const FOLDERS: [string, string][] = [
  ["Proyek", "Projects"],
  ["Dokumen", "Documents"],
  ["Gambar", "Pictures"],
  ["Unduhan", "Downloads"],
  ["Musik", "Music"],
  ["Desktop", "Desktop"],
];

function focusScale(distance: number) {
  if (distance >= REACH) return 1;
  const falloff = 0.5 * (1 + Math.cos((Math.PI * distance) / REACH));
  return 1 + (MAX_SCALE - 1) * falloff;
}

export function DockGenie() {
  const reduced = useReducedMotion() ?? false;
  const windowRef = useRef<HTMLDivElement>(null);
  const tileRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const controls = useAnimationControls();
  const [minimized, setMinimized] = useState(false);
  const [busy, setBusy] = useState(false);
  const [scales, setScales] = useState<number[]>(() => TILES.map(() => 1));
  const [hovered, setHovered] = useState<number | null>(null);

  function handlePointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    setScales(
      TILES.map((_, index) => {
        const node = tileRefs.current[index];
        if (!node) return 1;
        const rect = node.getBoundingClientRect();
        return focusScale(Math.abs(event.clientX - (rect.left + rect.width / 2)));
      }),
    );
  }

  function resetDock() {
    setScales(TILES.map(() => 1));
    setHovered(null);
  }

  function offsetToExplorer() {
    const windowNode = windowRef.current;
    const tile = tileRefs.current[0];
    if (!windowNode || !tile) return { x: 0, y: 0 };
    const win = windowNode.getBoundingClientRect();
    const dock = tile.getBoundingClientRect();
    return {
      x: dock.left + dock.width / 2 - (win.left + win.width / 2),
      y: dock.top + dock.height / 2 - (win.top + win.height / 2),
    };
  }

  async function minimize() {
    if (busy || minimized) return;
    setBusy(true);
    const { x, y } = offsetToExplorer();
    await controls.start({
      x,
      y,
      scaleX: 0.14,
      scaleY: 0.1,
      borderRadius: "40px",
      transition: reduced ? { duration: 0 } : GENIE,
    });
    setMinimized(true);
    setBusy(false);
  }

  async function restore() {
    if (busy || !minimized) return;
    setBusy(true);
    setMinimized(false);
    await controls.start({
      x: 0,
      y: 0,
      scaleX: 1,
      scaleY: 1,
      borderRadius: "14px",
      transition: reduced ? { duration: 0 } : RESTORE,
    });
    setBusy(false);
  }

  function renderTile(index: number, className = "") {
    const app = TILES[index];
    const Icon = app.icon;
    const isHovered = hovered === index;
    return (
      <div key={app.name[1]} className={`relative flex flex-col items-center ${className}`}>
        {isHovered && (
          <span className="pointer-events-none absolute -top-9 rounded-md bg-ink px-2 py-1 text-[12px] font-medium whitespace-nowrap text-page">
            <T id={app.name[0]} en={app.name[1]} />
          </span>
        )}
        <motion.button
          ref={(node) => {
            tileRefs.current[index] = node;
          }}
          type="button"
          onPointerEnter={() => setHovered(index)}
          onPointerLeave={() => setHovered((value) => (value === index ? null : value))}
          onFocus={() => setHovered(index)}
          onBlur={() => setHovered((value) => (value === index ? null : value))}
          onClick={index === 0 && minimized ? restore : undefined}
          animate={{ scale: scales[index] }}
          transition={SPRING}
          className="grid size-11 origin-bottom place-items-center squircle text-white shadow-[0_1px_2px_rgb(0_0_0/0.15)] focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-accent sm:size-[52px]"
          style={{ background: app.tint }}
        >
          <Icon className="size-[22px] pointer-events-none sm:size-[26px]" strokeWidth={1.8} aria-hidden="true" />
          <span className="sr-only">
            <T id={app.name[0]} en={app.name[1]} />
          </span>
        </motion.button>
        <span
          className={`mt-1 size-1 rounded-full bg-ink transition-opacity duration-200 ${index === 0 ? "opacity-100" : "opacity-0"}`}
          aria-hidden="true"
        />
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-6">
      <div className="relative h-[440px] w-full overflow-hidden rounded-[32px] border border-line bg-wallpaper sm:h-[520px]">
        <div className="absolute inset-x-0 top-0 flex h-7 items-center justify-between border-b border-line bg-paper/70 px-4 text-[12px] font-medium text-ink">
          <span>Explorer</span>
          <span className="tabular-nums text-ink-soft">14:02</span>
        </div>

        <motion.div
          ref={windowRef}
          animate={controls}
          style={{ display: minimized ? "none" : "flex" }}
          className="absolute top-[56px] left-1/2 h-[min(330px,64%)] w-[min(640px,92%)] -translate-x-1/2 flex-col overflow-hidden rounded-[14px] bg-paper shadow-[0_24px_60px_-28px_rgb(20_40_80/0.45)] ring-1 ring-black/10"
        >
          <div className="flex h-10 shrink-0 items-center gap-2 border-b border-line px-4">
            <span className="size-3 rounded-full bg-window-close" aria-hidden="true" />
            <button
              type="button"
              onClick={minimize}
              className="size-3 rounded-full bg-window-minimize focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
            >
              <span className="sr-only">
                <T id="Kecilkan jendela" en="Minimize window" />
              </span>
            </button>
            <span className="size-3 rounded-full bg-window-zoom" aria-hidden="true" />
            <span className="ml-3 text-[13px] font-medium text-ink-soft">
              <T id="Proyek Otto" en="Otto project" />
            </span>
            <span className="ml-auto flex size-7 items-center justify-center text-muted" aria-hidden="true">
              <Search size={15} strokeWidth={1.8} />
            </span>
          </div>
          <div className="grid min-h-0 flex-1 grid-cols-1 sm:grid-cols-[minmax(0,140px)_minmax(0,1fr)]">
            <aside className="hidden flex-col gap-1 border-r border-line bg-[#f6f7f9] p-3 text-[13px] text-ink-soft sm:flex">
              <span className="flex items-center gap-2 rounded-md bg-accent-soft px-2 py-1.5 font-medium text-accent-text">
                <Folder size={15} strokeWidth={1.8} aria-hidden="true" />
                <T id="Akses cepat" en="Quick access" />
              </span>
              <span className="flex items-center gap-2 px-2 py-1.5">
                <Download size={15} strokeWidth={1.8} aria-hidden="true" />
                <T id="Unduhan" en="Downloads" />
              </span>
              <span className="flex items-center gap-2 px-2 py-1.5">
                <FileText size={15} strokeWidth={1.8} aria-hidden="true" />
                <T id="Dokumen" en="Documents" />
              </span>
            </aside>
            <div className="grid grid-cols-3 content-start gap-2 p-3 sm:gap-3 sm:p-4">
              {FOLDERS.map(([id, en]) => (
                <div key={en} className="flex flex-col items-center gap-2 rounded-xl p-2 text-center sm:p-3">
                  <span className="grid size-10 place-items-center rounded-xl bg-[#cfe3ff] sm:size-12" aria-hidden="true">
                    <Folder size={20} strokeWidth={1.6} className="text-accent-text" />
                  </span>
                  <span className="text-[12px] font-medium text-ink">
                    <T id={id} en={en} />
                  </span>
                </div>
              ))}
            </div>
          </div>
        </motion.div>

        <div
          onPointerMove={handlePointerMove}
          onPointerLeave={resetDock}
          className="absolute bottom-3 left-1/2 flex max-w-[calc(100%-24px)] -translate-x-1/2 items-end gap-1.5 rounded-[22px] border border-white/70 bg-paper/80 px-2.5 pt-2 pb-2.5 shadow-[0_12px_30px_-16px_rgb(20_40_80/0.4)] sm:gap-2 sm:px-3"
        >
          {APPS.map((_, index) => renderTile(index))}
          <span className="mx-1 mb-3 hidden h-10 w-px bg-line-strong sm:block" aria-hidden="true" />
          {UTILITIES.map((_, index) => renderTile(APPS.length + index, "hidden sm:flex"))}
        </div>
      </div>
      <p className="text-center text-[14px] text-ink-soft">
        <T
          id="Kecilkan jendela, lalu klik ikon Explorer untuk memulihkannya."
          en="Minimize the window, then click the Explorer icon to bring it back."
        />
      </p>
    </div>
  );
}
