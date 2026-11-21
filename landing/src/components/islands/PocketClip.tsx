import { useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, FileText, Link, Plus, Type, X } from "lucide-react";
import { T } from "../lib/T";

type Item = {
  id: number;
  kind: "file" | "link" | "text";
  label: string;
  en?: string;
};

const TRAY: Item[] = [
  { id: 1, kind: "file", label: "Rancangan.pdf" },
  { id: 2, kind: "link", label: "github.com/howlil/otto-os" },
  { id: 3, kind: "text", label: "Catatan rilis", en: "Release notes" },
  { id: 4, kind: "file", label: "orbit-o.svg" },
];

const CLIP_SAMPLES: Item[] = [
  { id: 101, kind: "text", label: "Ringkasan konsep Otto OS", en: "Otto OS concept summary" },
  { id: 102, kind: "link", label: "github.com/howlil/otto-os" },
  { id: 103, kind: "file", label: "Rancangan.pdf" },
  { id: 104, kind: "text", label: "Tautan unduhan beta", en: "Beta download link" },
];

const KIND_ICON: Record<Item["kind"], ReactNode> = {
  file: <FileText size={15} strokeWidth={1.8} aria-hidden="true" />,
  link: <Link size={15} strokeWidth={1.8} aria-hidden="true" />,
  text: <Type size={15} strokeWidth={1.8} aria-hidden="true" />,
};

function ItemLabel({ item }: { item: Item }) {
  return item.en ? <T id={item.label} en={item.en} /> : <>{item.label}</>;
}

export function PocketClip() {
  const shelfRef = useRef<HTMLDivElement>(null);
  const [shelf, setShelf] = useState<Item[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const [clip, setClip] = useState<Item[]>(CLIP_SAMPLES.slice(0, 3));
  const [live, setLive] = useState(true);
  const [copied, setCopied] = useState<number | null>(null);
  const nextSample = useRef(0);

  function addToShelf(item: Item) {
    setShelf((current) => (current.some((entry) => entry.id === item.id) || current.length >= 12 ? current : [...current, item]));
  }

  function removeFromShelf(id: number) {
    setShelf((current) => current.filter((entry) => entry.id !== id));
  }

  function copyItem(item: Item) {
    setCopied(item.id);
    window.setTimeout(() => setCopied((value) => (value === item.id ? null : value)), 1100);
  }

  function captureSample() {
    const sample = CLIP_SAMPLES[nextSample.current % CLIP_SAMPLES.length];
    nextSample.current += 1;
    setClip((current) => [{ ...sample, id: sample.id + nextSample.current * 1000 }, ...current].slice(0, 4));
  }

  function handleDragEnd(item: Item, point: { x: number; y: number }) {
    const rect = shelfRef.current?.getBoundingClientRect();
    if (rect && point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom) {
      addToShelf(item);
    }
    setDragOver(false);
  }

  return (
    <div className="grid w-full gap-5 lg:grid-cols-2">
      <section className="flex flex-col gap-4 rounded-[24px] border border-line bg-paper p-5 sm:p-6">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold tracking-[-0.02em]">Pocket</h3>
          <span className="text-[13px] text-muted tabular-nums">{shelf.length} / 12</span>
        </div>

        <div
          ref={shelfRef}
          className={`flex min-h-[148px] flex-wrap content-start gap-2 rounded-[18px] border-2 border-dashed p-3 transition-colors duration-200 ${
            dragOver ? "border-accent bg-accent-soft" : "border-line-strong bg-page"
          }`}
        >
          <AnimatePresence initial={false}>
            {shelf.map((item) => (
              <motion.span
                key={item.id}
                layout
                initial={{ scale: 0.92, y: 6 }}
                animate={{ scale: 1, y: 0 }}
                exit={{ scale: 0.92 }}
                transition={{ type: "spring", stiffness: 420, damping: 30 }}
                className="inline-flex max-w-full items-center gap-2 rounded-xl bg-paper py-1.5 pr-1 pl-2.5 text-[13px] text-ink shadow-[0_1px_2px_rgb(0_0_0/0.12)] ring-1 ring-black/10"
              >
                <span className="text-ink-soft">{KIND_ICON[item.kind]}</span>
                <span className="max-w-[180px] truncate">
                  <ItemLabel item={item} />
                </span>
                <button
                  type="button"
                  onClick={() => removeFromShelf(item.id)}
                  className="grid size-6 place-items-center rounded-md text-muted hover:bg-black/5 hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                >
                  <X size={13} strokeWidth={2} aria-hidden="true" />
                  <span className="sr-only">
                    <T id="Hapus dari Pocket" en="Remove from Pocket" />
                  </span>
                </button>
              </motion.span>
            ))}
          </AnimatePresence>
          {shelf.length === 0 && (
            <span className="m-auto text-center text-[14px] text-muted">
              <T id="Seret item ke sini" en="Drag items here" />
            </span>
          )}
        </div>

        <div className="flex flex-wrap gap-2">
          {TRAY.map((item) => (
            <motion.button
              key={item.id}
              type="button"
              drag
              dragSnapToOrigin
              dragElastic={0.35}
              whileDrag={{ scale: 1.04, zIndex: 10 }}
              onDragEnd={(_, info) => handleDragEnd(item, info.point)}
              onDrag={(_, info) => {
                const rect = shelfRef.current?.getBoundingClientRect();
                setDragOver(!!rect && info.point.x >= rect.left && info.point.x <= rect.right && info.point.y >= rect.top && info.point.y <= rect.bottom);
              }}
              onClick={() => addToShelf(item)}
              className="inline-flex cursor-grab items-center gap-2 rounded-xl bg-page px-3 py-2 text-[13px] text-ink ring-1 ring-line-strong transition-colors hover:bg-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent active:cursor-grabbing"
            >
              <span className="text-ink-soft" aria-hidden="true">
                {KIND_ICON[item.kind]}
              </span>
              <ItemLabel item={item} />
              <Plus size={13} strokeWidth={2} className="text-muted" aria-hidden="true" />
            </motion.button>
          ))}
        </div>
      </section>

      <section className="flex flex-col gap-4 rounded-[24px] border border-line bg-paper p-5 sm:p-6">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-lg font-semibold tracking-[-0.02em]">Clip</h3>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setLive((value) => !value)}
              className="inline-flex h-8 items-center gap-1.5 rounded-full bg-page px-3 text-[13px] font-medium text-ink ring-1 ring-line-strong transition-colors hover:bg-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
            >
              <span className={`size-2 rounded-full ${live ? "bg-positive" : "bg-muted"}`} aria-hidden="true" />
              {live ? <T id="Live" en="Live" /> : <T id="Dijeda" en="Paused" />}
            </button>
            <button
              type="button"
              onClick={captureSample}
              disabled={!live}
              className="inline-flex h-8 items-center rounded-full bg-ink px-3 text-[13px] font-medium text-page transition-colors hover:bg-ink-soft disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
            >
              <T id="Salin contoh" en="Copy sample" />
            </button>
          </div>
        </div>

        <ul className="flex flex-col gap-2">
          <AnimatePresence initial={false}>
            {clip.map((item) => (
              <motion.li
                key={item.id}
                layout
                initial={{ y: -8 }}
                animate={{ y: 0 }}
                exit={{ y: -8 }}
                transition={{ type: "spring", stiffness: 420, damping: 32 }}
                className="flex items-center gap-3 rounded-[14px] bg-page px-3 py-2.5"
              >
                <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-paper text-ink-soft ring-1 ring-line" aria-hidden="true">
                  {KIND_ICON[item.kind]}
                </span>
                <button
                  type="button"
                  onClick={() => copyItem(item)}
                  className="min-w-0 flex-1 truncate rounded-md text-left text-[14px] text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                >
                  <ItemLabel item={item} />
                </button>
                {copied === item.id ? (
                  <span className="inline-flex items-center gap-1 text-[12px] font-medium text-positive">
                    <Check size={13} strokeWidth={2.2} aria-hidden="true" />
                    <T id="Disalin" en="Copied" />
                  </span>
                ) : null}
                <button
                  type="button"
                  onClick={() => setClip((current) => current.filter((entry) => entry.id !== item.id))}
                  className="grid size-7 shrink-0 place-items-center rounded-md text-muted hover:bg-black/5 hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                >
                  <X size={14} strokeWidth={1.8} aria-hidden="true" />
                  <span className="sr-only">
                    <T id="Hapus dari Clip" en="Remove from Clip" />
                  </span>
                </button>
              </motion.li>
            ))}
          </AnimatePresence>
        </ul>
        <p className="mt-auto text-[13px] text-muted">
          <T id="Riwayat hanya di RAM. Format sensitif dilewati." en="History stays in RAM. Sensitive formats are skipped." />
        </p>
      </section>
    </div>
  );
}
