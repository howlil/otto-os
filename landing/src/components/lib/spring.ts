import { useEffect, useRef, useState } from "react";
import { animate } from "motion/react";

export function useEased(target: number, initial: number, reduced: boolean, duration = 0.95) {
  const [value, setValue] = useState(initial);
  const current = useRef(initial);

  useEffect(() => {
    if (reduced) {
      current.current = target;
      setValue(target);
      return;
    }
    const controls = animate(current.current, target, {
      duration,
      ease: [0.16, 1, 0.3, 1],
      onUpdate: (next) => {
        current.current = next;
        setValue(next);
      },
    });
    return () => controls.stop();
  }, [target, reduced, duration]);

  return value;
}

export function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function formatClock(totalSeconds: number) {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}
