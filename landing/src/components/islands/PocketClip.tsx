import type { LucideIcon } from "lucide-react";
import { Check, Copy, FileText, Link, Plus, Type, X } from "lucide-react";
import { motion } from "motion/react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { T } from "../lib/T";
import { PanelFrame, Pill } from "./notch/Panels";
import { CLIP_ITEMS, INITIAL_TASKS, type Kind, type NotchTabKey, POCKET_ITEMS, type TaskItem } from "./notch/spec";

const KIND_ICON: Record<Kind, LucideIcon> = { file: FileText, link: Link, text: Type };

const KIND_TILE: Record<Kind, string> = {
  file: "bg-[#8fa3b8]/25 text-[#c9d6e4]",
  link: "bg-[#5b9cff]/25 text-[#9cc4ff]",
  text: "bg-[#7fd17a]/20 text-[#b6e9b1]",
};

const EASE = [0.16, 1, 0.3, 1] as [number, number, number, number];

export function PocketPanel({ reduced, onSelectTab }: { reduced: boolean; onSelectTab?: (tab: NotchTabKey) => void }) {
  const [items, setItems] = useState(POCKET_ITEMS);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);

  function addSample() {
    setItems((curr) => [
      { kind: "file", label: [`Cuplikan-${curr.length + 1}.png`, `Capture-${curr.length + 1}.png`] },
      ...curr.slice(0, 5),
    ]);
  }

  return (
    <PanelFrame
      tab="pocket"
      onSelectTab={onSelectTab}
      actions={
        <>
          <span className="px-1 text-[11px] font-medium text-[#a8abb2] tabular-nums">{items.length}/12</span>
          <Pill onClick={addSample}>
            <Plus size={10} strokeWidth={2.4} aria-hidden="true" />
            Add
          </Pill>
          {items.length > 0 ? (
            <Pill tone="danger" onClick={() => setItems([])}>
              Clear
            </Pill>
          ) : null}
        </>
      }
    >
      {items.length === 0 ? (
        <button
          type="button"
          onClick={() => setItems(POCKET_ITEMS)}
          className="flex h-full w-full items-center justify-center rounded-[12px] border border-dashed border-white/25 text-[12px] text-[#a8abb2] transition-colors hover:border-[#0a84ff] hover:text-[#f5f5f7] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff]"
        >
          <T id="Jatuhkan file, tautan, atau teks ke Pocket" en="Drop files, links or text into Pocket" />
        </button>
      ) : (
        <div className="grid h-full grid-cols-3 grid-rows-2 gap-1.5">
          {items.slice(0, 6).map((item, index) => {
            const Icon = KIND_ICON[item.kind];
            const isCopied = copiedIdx === index;
            return (
              <motion.div
                key={item.label[1]}
                initial={reduced ? false : { opacity: 0, y: 5, scale: 0.97 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                transition={{ duration: 0.24, delay: reduced ? 0 : index * 0.04, ease: EASE }}
                className="flex min-w-0 items-center gap-1.5 rounded-[10px] bg-white/[0.07] p-1.5 ring-1 ring-inset ring-white/[0.06]"
              >
                <span
                  className={`grid size-7 shrink-0 place-items-center rounded-[8px] ${KIND_TILE[item.kind]}`}
                  aria-hidden="true"
                >
                  <Icon size={13} strokeWidth={1.9} />
                </span>
                <span className="min-w-0 flex-1 truncate text-[11px] font-medium">
                  <T id={item.label[0]} en={item.label[1]} />
                </span>
                <button
                  type="button"
                  onClick={() => {
                    setCopiedIdx(index);
                    window.setTimeout(() => setCopiedIdx((curr) => (curr === index ? null : curr)), 1200);
                  }}
                  className="grid size-4 shrink-0 place-items-center rounded text-[#a8abb2] hover:text-[#f5f5f7]"
                >
                  {isCopied ? <Check size={10} className="text-[#30d158]" /> : <Copy size={10} />}
                  <span className="sr-only">
                    <T id="Salin" en="Copy" />
                  </span>
                </button>
                <button
                  type="button"
                  onClick={() => setItems((curr) => curr.filter((_, i) => i !== index))}
                  className="grid size-4 shrink-0 place-items-center rounded text-[#a8abb2] hover:text-[#ff8a82]"
                >
                  <X size={10} />
                  <span className="sr-only">
                    <T id="Hapus" en="Remove" />
                  </span>
                </button>
              </motion.div>
            );
          })}
        </div>
      )}
    </PanelFrame>
  );
}

export function ClipPanel({ onSelectTab }: { onSelectTab?: (tab: NotchTabKey) => void }) {
  const [live, setLive] = useState(true);
  const [items, setItems] = useState(CLIP_ITEMS);
  const [copied, setCopied] = useState<number | null>(0);

  return (
    <PanelFrame
      tab="clip"
      onSelectTab={onSelectTab}
      actions={
        <>
          <span className="px-1 text-[11px] font-medium text-[#a8abb2] tabular-nums">{items.length}/12</span>
          <Pill tone="positive" onClick={() => setLive((v) => !v)}>
            <span
              className={`block size-1.5 rounded-full ${live ? "bg-[#30d158]" : "bg-[#a8abb2]"}`}
              aria-hidden="true"
            />
            {live ? "Live" : "Paused"}
          </Pill>
          {items.length > 0 ? (
            <Pill tone="danger" onClick={() => setItems([])}>
              Clear
            </Pill>
          ) : null}
        </>
      }
    >
      {items.length === 0 ? (
        <button
          type="button"
          onClick={() => setItems(CLIP_ITEMS)}
          className="flex h-full w-full items-center justify-center rounded-[12px] bg-white/[0.04] text-[12px] text-[#a8abb2] hover:text-[#f5f5f7]"
        >
          <T id="Clipboard kosong · klik untuk isi ulang" en="Clipboard empty · click to restore" />
        </button>
      ) : (
        <ul className="flex flex-col gap-1">
          {items.map((item, index) => {
            const Icon = KIND_ICON[item.kind];
            const isCopied = index === copied;
            return (
              <li
                key={item.label[1]}
                className="flex h-[21px] items-center gap-2 rounded-md px-1 text-[12px] transition-colors hover:bg-white/[0.06]"
              >
                <button
                  type="button"
                  onClick={() => setCopied(index)}
                  className="flex min-w-0 flex-1 items-center gap-2 text-left focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff]"
                >
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
                </button>
                <button
                  type="button"
                  onClick={() => setItems((curr) => curr.filter((_, i) => i !== index))}
                  className="grid size-4 shrink-0 place-items-center text-[#a8abb2] hover:text-[#ff8a82]"
                >
                  <X size={11} />
                  <span className="sr-only">
                    <T id="Hapus" en="Remove" />
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </PanelFrame>
  );
}

export function TasksPanel({
  reduced,
  composing,
  onToggleCompose,
  onSelectTab,
  onActiveTaskChange,
}: {
  reduced: boolean;
  composing: boolean;
  onToggleCompose: (next: boolean) => void;
  onSelectTab?: (tab: NotchTabKey) => void;
  onActiveTaskChange?: (task: [string, string] | null) => void;
}) {
  const [tasks, setTasks] = useState<TaskItem[]>(INITIAL_TASKS);
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (composing) inputRef.current?.focus();
  }, [composing]);
  const hasDone = tasks.some((task) => task.done);

  function toggleDone(id: number) {
    setTasks((curr) =>
      curr.map((task) =>
        task.id === id ? { ...task, done: !task.done, active: !task.done ? false : task.active } : task,
      ),
    );
  }

  function selectActive(id: number) {
    setTasks((curr) => {
      const next = curr.map((task) => (task.done ? task : { ...task, active: task.id === id ? !task.active : false }));
      const found = next.find((t) => t.active);
      onActiveTaskChange?.(found ? found.text : null);
      return next;
    });
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const cleaned = draft.trim();
    if (!cleaned) return;
    setTasks((curr) => [{ id: Date.now(), text: [cleaned, cleaned], done: false }, ...curr.slice(0, 3)]);
    setDraft("");
    onToggleCompose(false);
  }

  return (
    <PanelFrame
      tab="tasks"
      onSelectTab={onSelectTab}
      actions={
        <>
          <Pill onClick={() => onToggleCompose(!composing)}>
            <Plus size={10} strokeWidth={2.4} aria-hidden="true" />
            Add
          </Pill>
          {hasDone ? (
            <Pill tone="danger" onClick={() => setTasks((curr) => curr.filter((t) => !t.done))}>
              Clear
            </Pill>
          ) : null}
        </>
      }
    >
      <div className="flex h-full flex-col gap-1.5">
        {composing ? (
          <form
            onSubmit={handleSubmit}
            className="flex items-center gap-1.5 rounded-[10px] bg-white/[0.1] px-2.5 py-1 ring-1 ring-inset ring-[#0a84ff]/70"
          >
            <input
              ref={inputRef}
              type="text"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              placeholder="Quick capture · Enter to save"
              className="min-w-0 flex-1 bg-transparent text-[12px] text-[#f5f5f7] placeholder:text-[#a8abb2] outline-none"
            />
            <button
              type="submit"
              className="rounded-full bg-[#0a84ff] px-2 py-0.5 text-[10px] font-semibold text-white"
            >
              Save
            </button>
          </form>
        ) : null}
        <ul className="flex flex-col gap-1">
          {tasks.slice(0, 4).map((task, index) => (
            <motion.li
              key={task.id}
              initial={reduced ? false : { opacity: 0, x: -5 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.22, delay: reduced ? 0 : index * 0.04, ease: EASE }}
              className="flex h-[20px] items-center gap-2 text-[12px]"
            >
              <button
                type="button"
                aria-pressed={task.done}
                onClick={() => toggleDone(task.id)}
                className={`grid size-3.5 shrink-0 place-items-center rounded-full transition-colors ${
                  task.done ? "bg-[#f5f5f7] text-black" : "ring-1 ring-inset ring-white/45 hover:ring-white"
                }`}
              >
                {task.done ? <Check size={9} strokeWidth={3} aria-hidden="true" /> : null}
                <span className="sr-only">
                  <T id="Tandai selesai" en="Mark done" />
                </span>
              </button>
              <button
                type="button"
                onClick={() => selectActive(task.id)}
                className={`min-w-0 flex-1 truncate text-left ${
                  task.done
                    ? "text-[#a8abb2] line-through"
                    : task.active
                      ? "font-semibold text-[#ff9230]"
                      : "text-[#f5f5f7]"
                }`}
              >
                <T id={task.text[0]} en={task.text[1]} />
              </button>
              {task.active && !task.done ? (
                <span className="shrink-0 rounded-full bg-[#ff9230]/20 px-1.5 py-px text-[9.5px] font-semibold text-[#ff9230]">
                  Focus
                </span>
              ) : null}
            </motion.li>
          ))}
        </ul>
      </div>
    </PanelFrame>
  );
}
