import { useRef, useState } from "react";
import { useMotionValueEvent, useScroll } from "motion/react";
import { ExplodedDiagram, PART_ORDER } from "./ExplodedDiagram";
import { T } from "../lib/T";
import { clamp } from "../lib/spring";

const STEPS: { name: [string, string]; body: [string, string] }[] = [
  {
    name: ["Bar atas", "Top bar"],
    body: ["Nama aplikasi, menu, status, dan jam dalam satu baris.", "App name, menus, status and clock in one line."],
  },
  {
    name: ["Notch", "Notch"],
    body: ["Dynamic Island untuk Focus, musik, Pocket, Clip, Tasks, dan HUD.", "Dynamic Island for Focus, music, Pocket, Clip, Tasks and HUD."],
  },
  {
    name: ["Jendela", "Window"],
    body: [
      "Minimize, zoom, dan tutup di pojok kanan atas. Jendela sistem tetap memakai kontrolnya.",
      "Minimize, zoom and close in the top-right corner. System windows keep their own.",
    ],
  },
  {
    name: ["Control Center", "Control Center"],
    body: ["Wi-Fi, Bluetooth, suara, dan kecerahan dalam satu panel.", "Wi-Fi, Bluetooth, sound and brightness in one panel."],
  },
  {
    name: ["Dock", "Dock"],
    body: ["Ikon squircle dengan efek Genie saat jendela diperkecil.", "Squircle icons, with a Genie effect when windows minimize."],
  },
];

export function PartsStory() {
  const ref = useRef<HTMLDivElement>(null);
  const [index, setIndex] = useState(0);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start start", "end end"] });

  useMotionValueEvent(scrollYProgress, "change", (progress) => {
    setIndex(clamp(Math.floor(progress * STEPS.length), 0, STEPS.length - 1));
  });

  return (
    <div ref={ref} className="relative h-[300svh]">
      <div className="sticky top-0 flex h-svh flex-col justify-center overflow-hidden">
        <div className="mx-auto grid w-full max-w-[1200px] items-center gap-6 px-5 sm:px-8 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)] lg:gap-16">
          <ExplodedDiagram explode={1} active={PART_ORDER[index]} className="mx-auto h-auto w-full max-w-[500px]" />
          <ol className="flex flex-col gap-1">
            {STEPS.map((step, stepIndex) => {
              const current = stepIndex === index;
              return (
                <li
                  key={step.name[1]}
                  className="border-t border-line py-5 last:border-b"
                  aria-current={current ? "step" : undefined}
                >
                  <h3
                    className={`text-[clamp(1.5rem,2.6vw,2.25rem)] leading-tight font-semibold tracking-[-0.03em] transition-colors duration-500 ${
                      current ? "text-ink" : "text-muted"
                    }`}
                  >
                    <T id={step.name[0]} en={step.name[1]} />
                  </h3>
                  <p className={`mt-2 max-w-[460px] text-base leading-relaxed transition-colors duration-500 ${current ? "text-ink-soft" : "text-muted"}`}>
                    <T id={step.body[0]} en={step.body[1]} />
                  </p>
                </li>
              );
            })}
          </ol>
        </div>
      </div>
    </div>
  );
}
