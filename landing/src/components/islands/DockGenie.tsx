import type { LucideIcon } from "lucide-react";
import {
  Code,
  Disc3,
  Download,
  FileArchive,
  FileImage,
  FileText,
  Folder,
  Globe,
  NotebookPen,
  Terminal,
  Trash,
  Trash2,
  X,
} from "lucide-react";
import { AnimatePresence, motion, useAnimationControls, useInView, useReducedMotion } from "motion/react";
import { type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import { T } from "../lib/T";

type DockMode = "always" | "autohide" | "reserve";
type SheetMode = "normal" | "left" | "right" | "maximized";

type DockApp = {
  id: string;
  name: [string, string];
  icon: LucideIcon;
  tint: string;
  running?: boolean;
};

const APPS: DockApp[] = [
  { id: "explorer", name: ["Explorer", "Explorer"], icon: Folder, tint: "#5b9cff", running: true },
  { id: "browser", name: ["Browser", "Browser"], icon: Globe, tint: "#7fb3ff", running: true },
  { id: "terminal", name: ["Terminal", "Terminal"], icon: Terminal, tint: "#1b1b1d" },
  { id: "code", name: ["VS Code", "VS Code"], icon: Code, tint: "#2b8a6e", running: true },
  { id: "music", name: ["Musik", "Music"], icon: Disc3, tint: "#fc3c54" },
  { id: "notes", name: ["Catatan", "Notes"], icon: NotebookPen, tint: "#ffb36b" },
];

const NOTES_INDEX = APPS.findIndex((app) => app.id === "notes");
const DOWNLOADS_INDEX = APPS.length;
const TRASH_INDEX = APPS.length + 1;
const TILE_COUNT = APPS.length + 2;

const DOWNLOAD_FILES: { name: string; icon: LucideIcon; tint: string }[] = [
  { name: "otto-os-1.3.exe", icon: FileArchive, tint: "#075bef" },
  { name: "spec-v1.3.pdf", icon: FileText, tint: "#ff5f57" },
  { name: "glass-lut.png", icon: FileImage, tint: "#30b0c7" },
  { name: "notes.md", icon: FileText, tint: "#5856d6" },
  { name: "screenshots", icon: Folder, tint: "#5b9cff" },
  { name: "SHA256SUMS", icon: FileText, tint: "#636366" },
];

const SHEET_LINES = [0.92, 0.74, 0.86, 0.58, 0.9, 0.68];

const DOCK_MODES: { id: DockMode; label: [string, string] }[] = [
  { id: "always", label: ["Selalu tampil", "Always"] },
  { id: "autohide", label: ["Auto-Hide", "Auto-Hide"] },
  { id: "reserve", label: ["Reserve Space", "Reserve Space"] },
];

const SHEET_MODES: { id: SheetMode; label: [string, string] }[] = [
  { id: "normal", label: ["Restore Center", "Restore Center"] },
  { id: "left", label: ["Tile Window to Left", "Tile Window to Left"] },
  { id: "right", label: ["Tile Window to Right", "Tile Window to Right"] },
  { id: "maximized", label: ["Zoom / Fill Screen", "Zoom / Fill Screen"] },
];

const MAX_SCALE = 1.45;
const REACH = 115;
const SPRING = { type: "spring" as const, stiffness: 380, damping: 26, mass: 0.6 };
const GENIE = { duration: 0.64, ease: [0.42, 0, 0.2, 1] as [number, number, number, number] };
const RESTORE = { duration: 0.56, ease: [0.22, 1, 0.36, 1] as [number, number, number, number] };
const SHEET_MOVE = { duration: 0.3, ease: [0.16, 1, 0.3, 1] as [number, number, number, number] };

function focusScale(distance: number) {
  if (distance >= REACH) return 1;
  const falloff = 0.5 * (1 + Math.cos((Math.PI * distance) / REACH));
  return 1 + (MAX_SCALE - 1) * falloff;
}

function sheetFrameClass(sheetMode: SheetMode, dockMode: DockMode) {
  if (sheetMode === "left") return "top-[18px] bottom-[100px] left-4 w-[calc(50%-22px)]";
  if (sheetMode === "right") return "top-[18px] bottom-[100px] right-4 w-[calc(50%-22px)]";
  if (sheetMode === "maximized") {
    return dockMode === "reserve" ? "top-3 inset-x-4 bottom-[92px]" : "top-3 inset-x-4 bottom-4";
  }
  return "top-[34px] left-1/2 -translate-x-1/2 w-[min(520px,88%)] h-[min(300px,62%)]";
}

export function DockGenie() {
  const reduced = useReducedMotion() ?? false;
  const sheetRef = useRef<HTMLDivElement>(null);
  const tileRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const controls = useAnimationControls();

  const [dockMode, setDockMode] = useState<DockMode>("always");
  const [sheetMode, setSheetMode] = useState<SheetMode>("normal");
  const [downloadsOpen, setDownloadsOpen] = useState(false);
  const [trashFull, setTrashFull] = useState(true);
  const [edgeReveal, setEdgeReveal] = useState(false);

  const [minimized, setMinimized] = useState(false);
  const [busy, setBusy] = useState(false);
  const [scales, setScales] = useState<number[]>(() => Array(TILE_COUNT).fill(1));
  const [hovered, setHovered] = useState<number | null>(null);

  const stageRef = useRef<HTMLDivElement>(null);
  const inView = useInView(stageRef, { amount: 0.4 });
  const [auto, setAuto] = useState(true);
  const autoStepRef = useRef<() => void>(() => {});
  autoStepRef.current = () => {
    if (minimized) void restore();
    else void minimize();
  };

  useEffect(() => {
    if (!auto || !inView || reduced || busy) return;
    const id = window.setTimeout(() => autoStepRef.current(), minimized ? 1800 : 2600);
    return () => window.clearTimeout(id);
  }, [auto, inView, reduced, busy, minimized]);

  useEffect(() => {
    if (dockMode === "reserve" && sheetMode === "normal") setSheetMode("maximized");
  }, [dockMode, sheetMode]);

  const dockHidden =
    dockMode === "autohide" && sheetMode === "maximized" && !minimized && !edgeReveal && !downloadsOpen;

  function selectDockMode(next: DockMode) {
    setDockMode(next);
    if (next === "autohide") setSheetMode("maximized");
    if (next === "always") setSheetMode("normal");
  }

  function handlePointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    if (reduced) return;
    setScales(
      Array.from({ length: TILE_COUNT }, (_, index) => {
        const node = tileRefs.current[index];
        if (!node) return 1;
        const rect = node.getBoundingClientRect();
        return focusScale(Math.abs(event.clientX - (rect.left + rect.width / 2)));
      }),
    );
  }

  function resetDock() {
    setScales(Array(TILE_COUNT).fill(1));
    setHovered(null);
  }

  function offsetToNotes() {
    const sheet = sheetRef.current;
    const tile = tileRefs.current[NOTES_INDEX];
    if (!sheet || !tile) return { x: 0, y: 0 };
    const from = sheet.getBoundingClientRect();
    const to = tile.getBoundingClientRect();
    return {
      x: to.left + to.width / 2 - (from.left + from.width / 2),
      y: to.top + to.height / 2 - (from.top + from.height / 2),
    };
  }

  async function minimize() {
    if (busy || minimized) return;
    setBusy(true);
    const { x, y } = offsetToNotes();
    await controls.start({
      x,
      y,
      scaleX: 0.14,
      scaleY: 0.1,
      skewX: x < 0 ? 14 : -14,
      opacity: 0.25,
      borderRadius: "42px",
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
      skewX: 0,
      opacity: 1,
      borderRadius: "14px",
      transition: reduced ? { duration: 0 } : RESTORE,
    });
    setBusy(false);
  }

  function toggleNotes() {
    setAuto(false);
    if (minimized) void restore();
    else void minimize();
  }

  function renderUtilityTile(
    index: number,
    label: [string, string],
    Icon: LucideIcon,
    tint: string,
    onClick: () => void,
    active = false,
  ) {
    const isHovered = hovered === index;
    return (
      <div key={label[1]} className="relative hidden flex-col items-center sm:flex">
        {isHovered && (
          <span className="pointer-events-none absolute -top-9 rounded-md bg-ink px-2 py-1 text-[11.5px] font-medium whitespace-nowrap text-page">
            <T id={label[0]} en={label[1]} />
          </span>
        )}
        <motion.button
          ref={(node) => {
            tileRefs.current[index] = node;
          }}
          type="button"
          aria-pressed={active}
          onPointerEnter={() => setHovered(index)}
          onPointerLeave={() => setHovered((v) => (v === index ? null : v))}
          onFocus={() => setHovered(index)}
          onBlur={() => setHovered((v) => (v === index ? null : v))}
          onClick={onClick}
          animate={{ scale: scales[index] }}
          transition={SPRING}
          className="grid size-[50px] origin-bottom place-items-center squircle text-white focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-accent"
          style={{
            background: tint,
            outline: active ? "2px solid var(--color-ink)" : undefined,
            outlineOffset: active ? 3 : undefined,
          }}
        >
          <Icon className="pointer-events-none size-[24px]" strokeWidth={1.85} aria-hidden="true" />
          <span className="sr-only">
            <T id={label[0]} en={label[1]} />
          </span>
        </motion.button>
        <span className="mt-1 size-1 rounded-full opacity-0" aria-hidden="true" />
      </div>
    );
  }

  return (
    <div>
      <div data-reveal suppressHydrationWarning className="flex flex-col items-center gap-3">
        <div className="inline-flex rounded-full border border-line p-1" role="group" aria-label="Dock behavior mode">
          {DOCK_MODES.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              aria-pressed={dockMode === id}
              onClick={() => selectDockMode(id)}
              className={`h-8 rounded-full px-3.5 text-[12.5px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
                dockMode === id ? "bg-ink text-page" : "text-muted hover:text-ink"
              }`}
            >
              <T id={label[0]} en={label[1]} />
            </button>
          ))}
        </div>
        <div className="flex flex-wrap justify-center gap-1.5" role="group" aria-label="Window layout">
          {SHEET_MODES.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              aria-pressed={sheetMode === id}
              onClick={() => setSheetMode(id)}
              className={`h-8 rounded-full px-3 text-[12px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
                sheetMode === id ? "bg-accent-soft text-accent-text" : "text-muted hover:text-ink"
              }`}
            >
              <T id={label[0]} en={label[1]} />
            </button>
          ))}
        </div>
      </div>

      {/* biome-ignore lint/a11y/noStaticElementInteractions: pointer-leave cancels edge reveal preview */}
      <div
        ref={stageRef}
        onMouseLeave={() => setEdgeReveal(false)}
        className="relative mt-8 h-[400px] w-full overflow-hidden rounded-[24px] border border-line bg-page sm:mt-10 sm:h-[440px]"
      >
        <motion.div
          ref={sheetRef}
          layout={!busy && !reduced}
          animate={controls}
          style={{ display: minimized ? "none" : "flex" }}
          transition={SHEET_MOVE}
          className={`absolute z-10 flex-col rounded-[14px] border border-line bg-paper p-5 sm:p-6 ${sheetFrameClass(sheetMode, dockMode)}`}
        >
          <div className="flex items-center gap-2.5 text-[13px] font-medium text-ink">
            <FileText size={16} strokeWidth={1.9} className="shrink-0 text-accent-text" aria-hidden="true" />
            <span>
              <T id="Catatan rilis v1.3" en="Release notes v1.3" />
            </span>
          </div>
          <div className="mt-5 flex min-h-0 flex-1 flex-col gap-2.5" aria-hidden="true">
            {SHEET_LINES.map((width) => (
              <span
                key={width}
                className="h-2 shrink-0 rounded-full bg-black/[0.07]"
                style={{ width: `${width * 100}%` }}
              />
            ))}
          </div>
        </motion.div>

        <AnimatePresence>
          {downloadsOpen ? (
            <motion.div
              initial={reduced ? false : { opacity: 0, y: 12, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 8 }}
              transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
              className="absolute right-5 bottom-[84px] z-30 w-[292px] rounded-[16px] border border-line bg-paper p-3"
            >
              <div className="flex items-center justify-between pb-2">
                <span className="text-[12.5px] font-semibold text-ink">Downloads</span>
                <button
                  type="button"
                  onClick={() => setDownloadsOpen(false)}
                  className="grid size-6 place-items-center rounded-full text-ink-soft hover:bg-black/[0.06]"
                >
                  <X size={12} aria-hidden="true" />
                  <span className="sr-only">Close</span>
                </button>
              </div>
              <div className="grid grid-cols-3 gap-2">
                {DOWNLOAD_FILES.map((file) => {
                  const Icon = file.icon;
                  return (
                    <div
                      key={file.name}
                      className="flex flex-col items-center gap-1.5 rounded-[10px] p-2 text-center transition-colors hover:bg-black/[0.05]"
                    >
                      <span
                        className="grid size-11 place-items-center rounded-[10px] text-white"
                        style={{ background: file.tint }}
                        aria-hidden="true"
                      >
                        <Icon size={18} strokeWidth={1.9} />
                      </span>
                      <span className="w-full truncate text-[10.5px] font-medium text-ink">{file.name}</span>
                    </div>
                  );
                })}
              </div>
              <div className="mt-2 border-t border-line pt-2 text-center text-[11.5px] font-medium text-accent-text">
                Open in Explorer
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>

        {dockMode === "autohide" ? (
          <div
            onMouseEnter={() => setEdgeReveal(true)}
            className="absolute inset-x-0 bottom-0 z-20 h-6"
            aria-hidden="true"
          />
        ) : null}

        <motion.div
          onPointerMove={handlePointerMove}
          onPointerLeave={resetDock}
          onMouseEnter={() => setEdgeReveal(true)}
          animate={{ y: dockHidden ? 92 : 0, opacity: dockHidden ? 0 : 1 }}
          transition={reduced ? { duration: 0 } : { duration: 0.24, ease: [0.16, 1, 0.3, 1] }}
          className="absolute bottom-3 left-1/2 z-20 flex max-w-[calc(100%-24px)] -translate-x-1/2 items-end gap-1.5 rounded-[20px] border border-line bg-paper/90 px-2.5 pt-2 pb-2 backdrop-blur-xl sm:gap-2 sm:px-3"
        >
          {APPS.map((app, index) => {
            const Icon = app.icon;
            const isHovered = hovered === index;
            const isNotes = index === NOTES_INDEX;
            const showsRestore = isNotes && minimized && !reduced;
            return (
              <div key={app.id} className="relative flex flex-col items-center">
                {isHovered && (
                  <span className="pointer-events-none absolute -top-9 rounded-md bg-ink px-2 py-1 text-[11.5px] font-medium whitespace-nowrap text-page">
                    <T id={app.name[0]} en={app.name[1]} />
                  </span>
                )}
                <motion.button
                  ref={(node) => {
                    tileRefs.current[index] = node;
                  }}
                  type="button"
                  onPointerEnter={() => setHovered(index)}
                  onPointerLeave={() => setHovered((v) => (v === index ? null : v))}
                  onFocus={() => setHovered(index)}
                  onBlur={() => setHovered((v) => (v === index ? null : v))}
                  onClick={isNotes ? toggleNotes : undefined}
                  animate={{ scale: scales[index], y: showsRestore ? [0, -5, 0] : 0 }}
                  transition={
                    showsRestore ? { y: { duration: 1.3, repeat: Infinity, ease: "easeInOut" }, scale: SPRING } : SPRING
                  }
                  className="grid size-11 origin-bottom place-items-center squircle text-white focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-accent sm:size-[50px]"
                  style={{ background: app.tint }}
                >
                  <Icon
                    className="pointer-events-none size-[22px] sm:size-[24px]"
                    strokeWidth={1.85}
                    aria-hidden="true"
                  />
                  <span className="sr-only">
                    <T id={app.name[0]} en={app.name[1]} />
                  </span>
                </motion.button>
                <span
                  className={`mt-1 size-1 rounded-full bg-ink transition-opacity duration-200 ${
                    app.running || (isNotes && minimized) ? "opacity-90" : "opacity-0"
                  }`}
                  aria-hidden="true"
                />
              </div>
            );
          })}

          <span className="mx-1 mb-3 hidden h-10 w-px bg-line-strong sm:block" aria-hidden="true" />

          {renderUtilityTile(
            DOWNLOADS_INDEX,
            ["Unduhan", "Downloads"],
            Download,
            "#5ec9a7",
            () => setDownloadsOpen((v) => !v),
            downloadsOpen,
          )}
          {renderUtilityTile(
            TRASH_INDEX,
            trashFull
              ? ["Tempat Sampah (Terisi)", "Recycle Bin (Full)"]
              : ["Tempat Sampah (Kosong)", "Recycle Bin (Empty)"],
            trashFull ? Trash2 : Trash,
            trashFull ? "#636366" : "#98a6b5",
            () => setTrashFull((v) => !v),
          )}
        </motion.div>
      </div>
    </div>
  );
}
