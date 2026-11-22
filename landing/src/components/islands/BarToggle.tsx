import { useEffect, useRef, useState, type RefObject } from "react";
import { motion, useReducedMotion } from "motion/react";
import { BatteryMedium, Volume2, Wifi } from "lucide-react";
import { T } from "../lib/T";
import { FocusCompact, IslandShell } from "./notch/Compact";
import { ISLAND } from "./notch/spec";
import { Switch } from "./Switch";

const BAR_DESIGN_WIDTH = 640;
const BAR_DESIGN_HEIGHT = 240;
const MENUS = ["File", "Edit", "View"];

function useFrameWidth(ref: RefObject<HTMLDivElement | null>) {
  const [width, setWidth] = useState(BAR_DESIGN_WIDTH);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const update = () => setWidth(node.clientWidth);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

export function BarToggle() {
  const reduced = useReducedMotion() ?? false;
  const [on, setOn] = useState(true);
  const frameRef = useRef<HTMLDivElement>(null);
  const frameWidth = useFrameWidth(frameRef);
  const designWidth = Math.max(BAR_DESIGN_WIDTH, frameWidth);
  const scale = frameWidth / designWidth;

  return (
    <div className="grid items-center gap-10 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] lg:gap-14">
      <div className="flex flex-col gap-5">
        <div className="flex items-center justify-between gap-6 rounded-[18px] bg-paper p-5 ring-1 ring-line">
          <div className="flex flex-col gap-1">
            <p className="text-[16px] font-semibold">
              <T id="Bar atas" en="Top bar" />
            </p>
            <p className="text-[14px] text-muted">
              <T id={on ? "Aktif" : "Mati, notch tetap jalan"} en={on ? "On" : "Off, the notch keeps running"} />
            </p>
          </div>
          <Switch checked={on} onChange={setOn} label={["Bar atas", "Top bar"]} />
        </div>
        <div className="flex flex-wrap items-center gap-1.5 text-[13px] text-muted">
          {["Ctrl", "Alt", "B"].map((key, index) => (
            <span key={key} className="flex items-center gap-1.5">
              {index > 0 ? <span aria-hidden="true">+</span> : null}
              <kbd className="rounded-md border border-line-strong bg-paper px-2 py-0.5 font-sans text-[12px] text-ink-soft">{key}</kbd>
            </span>
          ))}
          <span className="ml-1">
            <T id="dari mana saja" en="from anywhere" />
          </span>
        </div>
      </div>

      <div
        ref={frameRef}
        className="relative overflow-hidden rounded-[24px] bg-[linear-gradient(160deg,#dcebfb,#b5cde6)] ring-1 ring-line"
        style={{ height: BAR_DESIGN_HEIGHT * scale }}
      >
        <div
          className="absolute top-0 left-0 origin-top-left"
          style={{ width: designWidth, height: BAR_DESIGN_HEIGHT, transform: `scale(${scale})` }}
        >
          <motion.div
            className="absolute inset-x-0 top-0 flex items-center justify-between overflow-hidden border-b border-black/5 bg-paper/90 px-4 text-[13px] text-ink"
            initial={false}
            animate={{ height: on ? ISLAND.barHeight : 0, opacity: on ? 1 : 0 }}
            transition={reduced ? { duration: 0 } : { duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
          >
            <div className="flex min-w-0 items-center gap-3">
              <img src="/favicon.svg" alt="" width={16} height={16} />
              <span className="font-semibold">Explorer</span>
              <span className="flex gap-3 text-ink-soft">
                {MENUS.map((menu) => (
                  <span key={menu}>{menu}</span>
                ))}
              </span>
            </div>
            <div className="flex items-center gap-2 text-ink-soft">
              <Wifi size={14} strokeWidth={2} aria-hidden="true" />
              <Volume2 size={14} strokeWidth={2} aria-hidden="true" />
              <span className="flex items-center gap-1 tabular-nums">
                <BatteryMedium size={14} strokeWidth={2} aria-hidden="true" />
                86%
              </span>
              <span className="ml-1 text-ink tabular-nums">Min 22 Nov 09:41</span>
            </div>
          </motion.div>

          <div className="absolute top-1 left-1/2 -translate-x-1/2">
            <IslandShell width={ISLAND.focusWidth}>
              <FocusCompact remaining={1122} />
            </IslandShell>
          </div>

          <div className="absolute inset-x-5 bottom-5 h-[118px] rounded-[14px] bg-paper p-3 ring-1 ring-line">
            <div className="flex gap-1.5" aria-hidden="true">
              <span className="size-2.5 rounded-full bg-window-close" />
              <span className="size-2.5 rounded-full bg-window-minimize" />
              <span className="size-2.5 rounded-full bg-window-zoom" />
            </div>
            <div className="mt-4 flex flex-col gap-2" aria-hidden="true">
              <span className="h-2 w-2/3 rounded-full bg-ink/10" />
              <span className="h-2 w-1/2 rounded-full bg-ink/10" />
              <span className="h-2 w-3/4 rounded-full bg-ink/10" />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
