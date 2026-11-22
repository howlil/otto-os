import { motion } from "motion/react";
import { Check, Copy, FileText, Link, Pencil, Plus, Type, X } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../lib/T";
import { PanelFrame, Pill } from "./notch/Panels";
import { CLIP_ITEMS, POCKET_ITEMS, TASKS, type Kind } from "./notch/spec";

const KIND_ICON: Record<Kind, LucideIcon> = { file: FileText, link: Link, text: Type };

const KIND_TILE: Record<Kind, string> = {
  file: "bg-[#8fa3b8]/25 text-[#c9d6e4]",
  link: "bg-[#5b9cff]/25 text-[#9cc4ff]",
  text: "bg-[#7fd17a]/20 text-[#b6e9b1]",
};

const EASE = [0.16, 1, 0.3, 1] as [number, number, number, number];

export function PocketPanel({ reduced }: { reduced: boolean }) {
  return (
    <PanelFrame
      tab="pocket"
      actions={
        <>
          <span className="px-1 text-[11px] font-medium text-[#a8abb2] tabular-nums">1/2</span>
          <Pill>
            <Plus size={10} strokeWidth={2.4} aria-hidden="true" />
            Add
          </Pill>
          <Pill tone="danger">Clear</Pill>
        </>
      }
    >
      <div className="grid h-full grid-cols-3 grid-rows-2 gap-1.5">
        {POCKET_ITEMS.slice(0, 6).map((item, index) => {
          const Icon = KIND_ICON[item.kind];
          return (
            <motion.div
              key={item.label[1]}
              initial={reduced ? false : { opacity: 0, y: 6, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              transition={{ duration: 0.3, delay: reduced ? 0 : index * 0.06, ease: EASE }}
              className="flex min-w-0 items-center gap-1.5 rounded-[10px] bg-white/[0.07] p-1.5 ring-1 ring-inset ring-white/[0.06]"
            >
              <span className={`grid size-7 shrink-0 place-items-center rounded-[8px] ${KIND_TILE[item.kind]}`} aria-hidden="true">
                <Icon size={13} strokeWidth={1.9} />
              </span>
              <span className="min-w-0 flex-1 truncate text-[11px] font-medium">
                <T id={item.label[0]} en={item.label[1]} />
              </span>
              <Copy size={10} className="shrink-0 text-[#a8abb2]" aria-hidden="true" />
              <X size={10} className="shrink-0 text-[#a8abb2]" aria-hidden="true" />
            </motion.div>
          );
        })}
      </div>
    </PanelFrame>
  );
}

export function ClipPanel({ copied }: { copied: number }) {
  return (
    <PanelFrame
      tab="clip"
      actions={
        <>
          <span className="px-1 text-[11px] font-medium text-[#a8abb2] tabular-nums">1/3</span>
          <Pill tone="positive">
            <span className="block size-1.5 rounded-full bg-[#30d158]" aria-hidden="true" />
            Live
          </Pill>
          <Pill tone="danger">Clear</Pill>
        </>
      }
    >
      <ul className="flex flex-col gap-1">
        {CLIP_ITEMS.map((item, index) => {
          const Icon = KIND_ICON[item.kind];
          const isCopied = index === copied;
          return (
            <li key={item.label[1]} className="flex h-[19px] items-center gap-2 text-[12px]">
              <Icon size={11} strokeWidth={2} className="shrink-0 text-[#a8abb2]" aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate">
                <T id={item.label[0]} en={item.label[1]} />
              </span>
              {isCopied ? (
                <span className="inline-flex shrink-0 items-center gap-1 text-[11px] font-medium text-[#30d158]">
                  <Check size={11} strokeWidth={2.4} aria-hidden="true" />
                  Copied
                </span>
              ) : (
                <span className="shrink-0 text-[10px] text-[#a8abb2] tabular-nums">{item.size}</span>
              )}
              <X size={11} className="shrink-0 text-[#a8abb2]" aria-hidden="true" />
            </li>
          );
        })}
      </ul>
    </PanelFrame>
  );
}

export function TasksPanel({ reduced }: { reduced: boolean }) {
  const hasDone = TASKS.some((task) => task.done);
  return (
    <PanelFrame tab="tasks" actions={hasDone ? <Pill tone="danger">Clear</Pill> : null}>
      <ul className="flex flex-col gap-1">
        {TASKS.map((task, index) => (
          <motion.li
            key={task.text[1]}
            initial={reduced ? false : { opacity: 0, x: -6 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.3, delay: reduced ? 0 : index * 0.06, ease: EASE }}
            className="flex h-[19px] items-center gap-2 text-[12px]"
          >
            <span
              className={`grid size-3.5 shrink-0 place-items-center rounded-full ${
                task.done ? "bg-[#f5f5f7] text-black" : "ring-1 ring-inset ring-white/40"
              }`}
              aria-hidden="true"
            >
              {task.done ? <Check size={9} strokeWidth={3} /> : null}
            </span>
            <span className={`min-w-0 flex-1 truncate ${task.done ? "text-[#a8abb2] line-through" : ""}`}>
              <T id={task.text[0]} en={task.text[1]} />
            </span>
            <Pencil size={10} className="shrink-0 text-[#a8abb2]" aria-hidden="true" />
          </motion.li>
        ))}
      </ul>
    </PanelFrame>
  );
}
