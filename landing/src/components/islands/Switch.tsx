import { motion } from "motion/react";
import { T } from "../lib/T";

const SPRING = { type: "spring" as const, stiffness: 500, damping: 34, mass: 0.7 };

const TRACK = {
  accent: { on: "bg-accent", off: "bg-[#c9ccd3]" },
  "system-light": { on: "bg-[#34c759]", off: "bg-[#d1d1d6]" },
  "system-dark": { on: "bg-[#30d158]", off: "bg-[#5a5a5f]" },
};

export type SwitchTone = keyof typeof TRACK;

export function Switch({
  checked,
  onChange,
  label,
  tone = "accent",
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: [string, string];
  tone?: SwitchTone;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-[2px] transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
        checked ? TRACK[tone].on : TRACK[tone].off
      }`}
    >
      <motion.span
        className="block size-[18px] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.25)]"
        animate={{ x: checked ? 16 : 0 }}
        transition={SPRING}
      />
      <span className="sr-only">
        <T id={label[0]} en={label[1]} />
      </span>
    </button>
  );
}
