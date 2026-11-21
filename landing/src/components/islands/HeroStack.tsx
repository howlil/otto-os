import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useReducedMotion } from "motion/react";
import { Hand } from "lucide-react";
import { ExplodedDiagram } from "./ExplodedDiagram";
import { T } from "../lib/T";
import { clamp } from "../lib/spring";

const SCRUB_PX = 240;
const TEASER_OPEN_MS = 900;
const TEASER_CLOSE_MS = 2200;
const HOLD_KEYS = new Set([" ", "Enter", "ArrowUp", "ArrowRight"]);

export function HeroStack() {
  const reduced = useReducedMotion() ?? false;
  const [explode, setExplode] = useState(0);
  const [pressed, setPressed] = useState(false);
  const drag = useRef<{ x: number; id: number } | null>(null);
  const teaser = useRef<number[]>([]);

  useEffect(() => {
    if (reduced) return;
    teaser.current = [
      window.setTimeout(() => setExplode(1), TEASER_OPEN_MS),
      window.setTimeout(() => setExplode(0), TEASER_CLOSE_MS),
    ];
    return () => teaser.current.forEach((id) => window.clearTimeout(id));
  }, [reduced]);

  function stopTeaser() {
    teaser.current.forEach((id) => window.clearTimeout(id));
    teaser.current = [];
  }

  function hold() {
    stopTeaser();
    setPressed(true);
    setExplode(1);
  }

  function release() {
    drag.current = null;
    setPressed(false);
    setExplode(0);
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, id: event.pointerId };
    hold();
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>) {
    const current = drag.current;
    if (!current || current.id !== event.pointerId) return;
    setExplode(clamp(1 + (event.clientX - current.x) / SCRUB_PX, 0, 1));
  }

  function handlePointerUp(event: PointerEvent<HTMLDivElement>) {
    if (drag.current?.id !== event.pointerId) return;
    release();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!HOLD_KEYS.has(event.key)) return;
    event.preventDefault();
    if (!event.repeat) hold();
  }

  function handleKeyUp(event: KeyboardEvent<HTMLDivElement>) {
    if (!HOLD_KEYS.has(event.key)) return;
    release();
  }

  return (
    <figure className="flex w-full flex-col items-end gap-3">
      <div
        role="button"
        tabIndex={0}
        aria-pressed={pressed}
        aria-labelledby="hero-stack-hint"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
        onContextMenu={(event) => event.preventDefault()}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        onBlur={release}
        className="w-full touch-pan-y select-none rounded-[28px] outline-none transition-transform duration-300 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-accent"
        style={{ transform: pressed ? "scale(0.985)" : "scale(1)" }}
      >
        <ExplodedDiagram explode={explode} initial={0} labels className="h-auto w-full max-w-[560px]" />
      </div>
      <figcaption id="hero-stack-hint" className="flex items-center gap-2 text-[13px] text-muted">
        <Hand size={15} strokeWidth={1.8} aria-hidden="true" />
        <T id="Tahan untuk membongkar, geser ke kiri untuk merakit" en="Hold to explode, drag left to reassemble" />
      </figcaption>
    </figure>
  );
}
