import { useState } from "react";
import { ExplodedDiagram } from "./ExplodedDiagram";
import { T } from "../lib/T";

export function HeroStack() {
  const [open, setOpen] = useState(true);

  return (
    <figure className="flex w-full flex-col items-center gap-5">
      <ExplodedDiagram explode={open ? 1 : 0} initial={0} labels className="h-auto w-full max-w-[560px]" />
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="inline-flex h-10 items-center rounded-full border border-line-strong px-5 text-sm font-medium text-ink transition-colors duration-200 hover:border-ink hover:bg-paper focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-accent"
      >
        {open ? <T id="Rakit" en="Assemble" /> : <T id="Bongkar" en="Explode" />}
      </button>
    </figure>
  );
}
