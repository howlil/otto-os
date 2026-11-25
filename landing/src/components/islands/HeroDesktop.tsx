import { BatteryMedium, Bell, Code, Search, SlidersHorizontal, Volume2, Wifi, X } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Fragment, type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import { T } from "../lib/T";
import { DockArt, type DockArtKind } from "./dock/DockArt";
import { DOT_BOTTOM, dockCenters, hoveredSlot, magnifyScale, tooltipWidth } from "./dock/layout";
import { DOCK } from "./dock/spec";
import { FocusCompact } from "./notch/Compact";
import { FOCUS_START, ISLAND } from "./notch/spec";

type PopupKind = null | "notif" | "calendar";

const CLASSIC_EAR_RADIUS = ISLAND.classicEarRadius;
const CLASSIC_EAR_LEFT = `radial-gradient(circle at 0 100%, transparent ${CLASSIC_EAR_RADIUS - 0.5}px, #000 ${CLASSIC_EAR_RADIUS}px)`;
const CLASSIC_EAR_RIGHT = `radial-gradient(circle at 100% 100%, transparent ${CLASSIC_EAR_RADIUS - 0.5}px, #000 ${CLASSIC_EAR_RADIUS}px)`;

type DockSlot = {
  name: string;
  art: DockArtKind;
  running?: boolean;
  active?: boolean;
  utility?: boolean;
  dividerBefore?: boolean;
};

const DOCK_SLOTS: DockSlot[] = [
  { name: "Explorer", art: "finder", running: true },
  { name: "Browser", art: "browser", running: true },
  { name: "Terminal", art: "terminal" },
  { name: "VS Code", art: "code", running: true, active: true },
  { name: "Music", art: "music", running: true },
  { name: "Notes", art: "notes" },
  { name: "Downloads", art: "folder", utility: true, dividerBefore: true },
  { name: "Recycle Bin", art: "trash", utility: true },
];

const DAYS_MINI = [
  { d: 1, m: true },
  { d: 2, m: true },
  { d: 3, m: true },
  { d: 4, m: true },
  { d: 5, m: true },
  { d: 6, m: true },
  { d: 7, m: true },
  { d: 8, m: true },
  { d: 9, m: true },
  { d: 10, m: true },
  { d: 11, m: true },
  { d: 12, m: true },
  { d: 13, m: true },
  { d: 14, m: true },
  { d: 15, m: true },
  { d: 16, m: true },
  { d: 17, m: true },
  { d: 18, m: true },
  { d: 19, m: true },
  { d: 20, m: true },
  { d: 21, m: true },
  { d: 22, m: true },
  { d: 23, m: true, today: true },
  { d: 24, m: true },
  { d: 25, m: true },
  { d: 26, m: true },
  { d: 27, m: true },
  { d: 28, m: true },
  { d: 29, m: true },
  { d: 30, m: true },
  { d: 1, m: false },
  { d: 2, m: false },
  { d: 3, m: false },
  { d: 4, m: false },
  { d: 5, m: false },
];

export function HeroDesktop() {
  const reduced = useReducedMotion() ?? false;
  const [popup, setPopup] = useState<PopupKind>(null);
  const [remaining, setRemaining] = useState(FOCUS_START);
  const [scales, setScales] = useState<number[]>(() => DOCK_SLOTS.map(() => 1));
  const [hovered, setHovered] = useState<number | null>(null);
  const dockRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = window.setInterval(() => setRemaining((v) => (v > 60 ? v - 1 : FOCUS_START)), 1000);
    return () => window.clearInterval(id);
  }, []);

  function handleDockMove(event: ReactPointerEvent<HTMLDivElement>) {
    const dock = dockRef.current;
    if (!dock) return;
    const rect = dock.getBoundingClientRect();
    const x = (event.clientX - (rect.left + rect.width / 2)) / (rect.width / dock.offsetWidth);
    const wide = window.matchMedia("(min-width: 640px)").matches;
    const visible = DOCK_SLOTS.flatMap((slot, idx) => (wide || !slot.utility ? [idx] : []));
    const centers = dockCenters(visible.map((idx) => DOCK_SLOTS[idx]));
    const hit = hoveredSlot(centers, x);
    setHovered(hit === null ? null : visible[hit]);
    setScales(
      DOCK_SLOTS.map((_, idx) => {
        const pos = visible.indexOf(idx);
        return reduced || pos < 0 ? 1 : magnifyScale(Math.abs(centers[pos] - x));
      }),
    );
  }

  function resetDock() {
    setScales(DOCK_SLOTS.map(() => 1));
    setHovered(null);
  }

  return (
    <div className="relative">
      <div className="wallpaper-daylight relative mx-auto aspect-[4/3] w-full max-w-[1040px] overflow-hidden rounded-[24px] border border-line sm:aspect-video">
        <div className="absolute inset-x-0 top-0 z-20 flex h-8 items-center justify-between border-b border-black/[0.06] bg-white/65 px-2 text-[12px] text-ink backdrop-blur-xl">
          <div className="flex min-w-0 items-center gap-[2px]">
            <span className="grid w-[34px] shrink-0 place-items-center">
              <img src="/favicon.svg" alt="" width={15} height={15} className="shrink-0" />
            </span>
            <span className="hidden min-w-[52px] max-w-[132px] truncate px-1 font-semibold sm:block">VS Code</span>
            <span className="hidden items-center gap-[2px] text-ink-soft lg:flex">
              <span className="grid w-[40px] place-items-center">File</span>
              <span className="grid w-[40px] place-items-center">Edit</span>
              <span className="grid w-[46px] place-items-center">View</span>
              <span className="grid w-[64px] place-items-center">Window</span>
              <span className="grid w-[44px] place-items-center">Help</span>
            </span>
          </div>

          <div className="flex items-center gap-1 text-ink-soft">
            <span className="hidden h-7 w-[108px] items-center justify-center gap-2 lg:flex" aria-hidden="true">
              <Wifi size={15} strokeWidth={2} />
              <Volume2 size={15} strokeWidth={2} />
              <span className="inline-flex items-center gap-1 tabular-nums">
                <BatteryMedium size={15} strokeWidth={2} />
                86%
              </span>
            </span>
            <a
              href="#spotlight"
              className="hidden h-7 w-[30px] place-items-center rounded-md text-ink-soft transition-colors hover:bg-black/[0.06] hover:text-ink sm:grid"
            >
              <Search size={15} strokeWidth={2} aria-hidden="true" />
              <span className="sr-only">Spotlight</span>
            </a>
            <a
              href="#tampilan"
              className="hidden h-7 w-[30px] place-items-center rounded-md text-ink-soft transition-colors hover:bg-black/[0.06] hover:text-ink sm:grid"
            >
              <SlidersHorizontal size={15} strokeWidth={2} aria-hidden="true" />
              <span className="sr-only">Control Center</span>
            </a>
            <button
              type="button"
              aria-pressed={popup === "notif"}
              onClick={() => setPopup((curr) => (curr === "notif" ? null : "notif"))}
              className={`relative grid h-7 w-[30px] place-items-center rounded-md transition-colors ${
                popup === "notif" ? "bg-black/10 text-ink" : "text-ink-soft hover:bg-black/[0.06] hover:text-ink"
              }`}
            >
              <Bell size={15} strokeWidth={2} aria-hidden="true" />
              <span className="absolute top-1.5 right-2 size-1.5 rounded-full bg-[#ff453a]" aria-hidden="true" />
              <span className="sr-only">
                <T id="Notifikasi" en="Notifications" />
              </span>
            </button>
            <button
              type="button"
              aria-pressed={popup === "calendar"}
              onClick={() => setPopup((curr) => (curr === "calendar" ? null : "calendar"))}
              className={`h-7 w-11 rounded-md text-center sm:w-16 lg:w-[128px] font-medium tabular-nums transition-colors ${
                popup === "calendar" ? "bg-black/10 text-ink" : "text-ink hover:bg-black/[0.06]"
              }`}
            >
              <span className="hidden lg:inline">Mon 23 Nov · </span>09:41
            </button>
          </div>
        </div>

        <div
          aria-hidden="true"
          className="pointer-events-none absolute top-0 left-1/2 z-30 origin-top -translate-x-1/2 scale-[0.76] sm:scale-95 md:scale-100"
        >
          <div className="relative" style={{ width: ISLAND.classicFocusWidth, height: ISLAND.classicCompactHeight }}>
            <span className="absolute top-0 right-full size-[6px]" style={{ background: CLASSIC_EAR_LEFT }} />
            <span className="absolute top-0 left-full size-[6px]" style={{ background: CLASSIC_EAR_RIGHT }} />
            <div
              className="absolute inset-0 overflow-hidden bg-black ring-1 ring-inset ring-white/[0.16]"
              style={{ borderRadius: `0 0 ${ISLAND.classicCompactRadius}px ${ISLAND.classicCompactRadius}px` }}
            >
              <FocusCompact remaining={remaining} />
            </div>
          </div>
        </div>

        <AnimatePresence>
          {popup === "notif" ? (
            <motion.div
              key="hero-notif"
              initial={reduced ? false : { opacity: 0, y: -8, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={reduced ? { opacity: 0 } : { opacity: 0, y: -6, scale: 0.98 }}
              transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
              className="absolute top-10 right-4 z-40 w-[328px] rounded-[20px] border border-line bg-white/85 p-2 backdrop-blur-2xl"
            >
              <div className="flex items-center justify-between px-1.5 pb-2">
                <span className="text-[12.5px] font-semibold text-ink">Notifications</span>
                <button
                  type="button"
                  onClick={() => setPopup(null)}
                  className="rounded-full px-2 py-0.5 text-[11px] font-medium text-ink-soft hover:bg-black/[0.06]"
                >
                  <X size={12} aria-hidden="true" />
                  <span className="sr-only">Close</span>
                </button>
              </div>
              <div className="relative pb-2">
                <div
                  className="absolute inset-x-3 bottom-0 h-4 rounded-b-[11px] bg-white/65 ring-1 ring-black/[0.06]"
                  aria-hidden="true"
                />
                <div className="relative flex items-start gap-2.5 rounded-[12px] bg-white/95 p-2.5 ring-1 ring-black/[0.07]">
                  <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#5865f2] text-[12px] font-bold text-white">
                    D
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-2">
                      <span className="flex items-center gap-1.5 text-[11px] font-medium text-ink-soft">
                        Discord
                        <span className="rounded-[8px] bg-black/[0.06] px-1.5 py-px text-[10px] font-semibold text-ink">
                          +2
                        </span>
                      </span>
                      <span className="text-[10.5px] text-muted tabular-nums">2m</span>
                    </div>
                    <p className="truncate text-[12.5px] font-semibold text-ink">Release v1.3.0</p>
                    <p className="truncate text-[11.5px] text-ink-soft">Checksums verified on GitHub.</p>
                  </div>
                </div>
              </div>
              <div className="mt-1.5 flex items-start gap-2.5 rounded-[12px] bg-white/95 p-2.5 ring-1 ring-black/[0.07]">
                <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#1b1b1d] text-[12px] font-bold text-white">
                  GH
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-[11px] font-medium text-ink-soft">GitHub</span>
                    <span className="text-[10.5px] text-muted tabular-nums">9m</span>
                  </div>
                  <p className="truncate text-[12.5px] font-semibold text-ink">Build green · x64</p>
                </div>
              </div>
            </motion.div>
          ) : null}

          {popup === "calendar" ? (
            <motion.div
              key="hero-cal"
              initial={reduced ? false : { opacity: 0, y: -8, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={reduced ? { opacity: 0 } : { opacity: 0, y: -6, scale: 0.98 }}
              transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
              className="absolute top-10 right-4 z-40 w-[252px] rounded-[18px] border border-line bg-white/90 p-3 backdrop-blur-2xl"
            >
              <div className="flex items-center justify-between px-1 pb-2">
                <p className="text-[14px] font-semibold text-ink">
                  November <span className="font-normal text-ink-soft">2026</span>
                </p>
                <button
                  type="button"
                  onClick={() => setPopup(null)}
                  className="grid size-6 place-items-center rounded-full text-ink-soft hover:bg-black/[0.06]"
                >
                  <X size={12} aria-hidden="true" />
                  <span className="sr-only">Close</span>
                </button>
              </div>
              <div className="grid grid-cols-7 text-center text-[10.5px] font-semibold text-ink-soft">
                {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].map((day) => (
                  <span key={day} className="py-1">
                    {day[0]}
                  </span>
                ))}
              </div>
              <div className="grid grid-cols-7 text-center text-[12px] tabular-nums">
                {DAYS_MINI.map((cell) => (
                  <span key={`${cell.m ? "m" : "o"}-${cell.d}`} className="grid h-7 place-items-center">
                    <span
                      className={`grid size-6 place-items-center rounded-full ${
                        cell.today ? "bg-[#ff3b30] font-semibold text-white" : cell.m ? "text-ink" : "text-ink/40"
                      }`}
                    >
                      {cell.d}
                    </span>
                  </span>
                ))}
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>

        <div className="absolute top-[15%] left-1/2 flex h-[50%] w-[88%] -translate-x-1/2 flex-col overflow-hidden rounded-[8px] border border-white/12 bg-[#1e1e22] text-[#f5f5f7] sm:h-[60%] sm:w-[74%]">
          <div className="flex h-[30px] shrink-0 items-center justify-between gap-3 border-b border-white/[0.08] bg-[#252529] pr-2 pl-2.5">
            <div className="flex min-w-0 items-center gap-2 text-[12px] text-white/75">
              <Code size={13} strokeWidth={2} className="shrink-0 text-[#5b9cff]" aria-hidden="true" />
              <span className="shrink-0 font-medium">otto-os</span>
              <span className="shrink-0 text-white/35">/</span>
              <span className="truncate text-white/55 sm:hidden">compose.rs</span>
              <span className="hidden truncate text-white/55 sm:inline">
                crates/core/src/appearance/glass/compose.rs
              </span>
            </div>
            <div className="flex shrink-0 items-center gap-2" aria-hidden="true">
              <span className="size-3 rounded-full bg-window-minimize" />
              <span className="size-3 rounded-full bg-window-zoom" />
              <span className="size-3 rounded-full bg-window-close" />
            </div>
          </div>
          <div className="grid min-h-0 flex-1 grid-cols-1 sm:grid-cols-[164px_minmax(0,1fr)]">
            <div className="hidden flex-col gap-1 border-r border-white/[0.06] bg-[#1a1a1e] p-3 text-[11.5px] text-white/60 sm:flex">
              <span className="rounded-md bg-white/[0.08] px-2 py-1 font-medium text-white">crates/core</span>
              <span className="px-2 py-1">crates/platform</span>
              <span className="px-2 py-1">crates/render</span>
              <span className="px-2 py-1">crates/app</span>
              <span className="mt-auto border-t border-white/[0.06] pt-2 text-[10.5px] text-white/45 tabular-nums">
                Rust 2024 · Win32
              </span>
            </div>
            <div className="overflow-hidden p-4 font-mono text-[11.5px] leading-[1.7] text-white/75 sm:p-5 sm:text-[12px]">
              <p>
                <span className="text-[#75b0ff]">pub fn</span> <span className="text-[#ffd18c]">compose_glass_lit</span>
                (backdrop: &amp;Backdrop, params: GlassParams) {"{"}
              </p>
              <p className="pl-4 text-white/55 hidden sm:block">
                let refracted = snell_bezel(backdrop.sharp, params.bezel_pt);
              </p>
              <p className="pl-4 text-white/55 hidden sm:block">
                let frosted = box_blur_3pass(backdrop.soft, params.blur_pt);
              </p>
              <p className="pl-4 text-white/55 hidden sm:block">
                let rim = dual_specular_rim(LIGHT_DIRECTION, params.rim);
              </p>
              <p className="pl-4">
                <span className="text-[#7fd17a]">blend_optical</span>(refracted, frosted, rim, params.accent)
              </p>
              <p>{"}"}</p>
            </div>
          </div>
        </div>

        <div
          ref={dockRef}
          aria-hidden="true"
          onPointerMove={handleDockMove}
          onPointerLeave={resetDock}
          className="absolute left-1/2 z-20 flex origin-bottom -translate-x-1/2 scale-[0.8] items-end border border-black/[0.14] bg-[#f7faff]/[0.48] ring-1 ring-inset ring-white/[0.76] backdrop-blur-2xl sm:scale-100"
          style={{
            bottom: DOCK.bottomMargin,
            borderRadius: DOCK.radius,
            paddingLeft: DOCK.padX - 1,
            paddingRight: DOCK.padX - 1,
            paddingTop: DOCK.padTop - 1,
            paddingBottom: DOCK.padBottom - 1,
          }}
        >
          {DOCK_SLOTS.map((slot, idx) => (
            <Fragment key={slot.name}>
              {slot.dividerBefore ? (
                <div
                  className="relative hidden shrink-0 self-stretch sm:block"
                  style={{
                    width: DOCK.dividerSlot,
                    marginLeft: DOCK.gap,
                    marginTop: -DOCK.padTop,
                    marginBottom: -DOCK.padBottom,
                  }}
                >
                  <span
                    className="absolute inset-y-[20%] w-px bg-black/[0.14]"
                    style={{ left: DOCK.dividerSlot / 2 - DOCK.gap / 2 - 0.5 }}
                  />
                </div>
              ) : null}
              <motion.div
                initial={{ width: DOCK.tile }}
                animate={{ width: DOCK.tile * scales[idx] }}
                transition={{ type: "spring", stiffness: 400, damping: 26, mass: 0.5 }}
                className={`relative shrink-0 self-end ${slot.utility ? "hidden sm:block" : ""}`}
                style={{ height: DOCK.tile, marginLeft: idx === 0 || slot.dividerBefore ? 0 : DOCK.gap }}
              >
                <div className="absolute inset-x-0 bottom-0 aspect-square">
                  <DockArt kind={slot.art} />
                  {hovered === idx ? (
                    <span
                      className="pointer-events-none absolute left-1/2 flex -translate-x-1/2 items-center justify-center whitespace-nowrap border border-white/[0.76] bg-[#f5f7fc]/[0.66] font-semibold text-ink"
                      style={{
                        width: tooltipWidth(slot.name),
                        height: DOCK.tooltipHeight,
                        bottom: `calc(100% + ${DOCK.tooltipGap}px)`,
                        borderRadius: DOCK.tooltipRadius,
                        fontSize: DOCK.tooltipFont,
                      }}
                    >
                      {slot.name}
                      <span
                        className="absolute top-full left-1/2 size-0 -translate-x-1/2 border-x-transparent border-t-[#f5f7fc]/[0.66]"
                        style={{
                          borderLeftWidth: DOCK.tooltipArrow,
                          borderRightWidth: DOCK.tooltipArrow,
                          borderTopWidth: DOCK.tooltipArrow,
                        }}
                      />
                    </span>
                  ) : null}
                </div>
                {slot.running ? (
                  <span
                    className={`absolute left-1/2 -translate-x-1/2 rounded-full ${
                      slot.active ? "h-1 w-[5px] bg-[#0d0d12]/[0.96]" : "size-1 bg-[#1a1a1f]/90"
                    }`}
                    style={{ bottom: -DOT_BOTTOM }}
                  />
                ) : null}
              </motion.div>
            </Fragment>
          ))}
        </div>
        <span aria-hidden="true" className="hero-sheen pointer-events-none absolute inset-y-0 left-0 z-[35] w-2/5" />
      </div>
    </div>
  );
}
