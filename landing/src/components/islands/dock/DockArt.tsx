import type { ReactNode } from "react";

export type DockArtKind = "finder" | "browser" | "terminal" | "code" | "music" | "notes" | "folder" | "trash";

const at = (f: number) => 18 + 220 * f;
const S = (f: number) => 220 * f;

const line = {
  fill: "none",
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

function Plate({ fill }: { fill: string }) {
  return <rect x={18} y={18} width={220} height={220} rx={49} fill={fill} />;
}

function Finder() {
  const feature = { ...line, stroke: "rgb(20,44,84)", strokeOpacity: 0.96, strokeWidth: S(0.045) };
  return (
    <>
      <Plate fill="rgb(138,214,255)" />
      <path d="M67 18H132.4V238H67A49 49 0 0 1 18 189V67A49 49 0 0 1 67 18Z" fill="rgb(28,120,242)" />
      <rect x={at(0.29) - S(0.035)} y={at(0.29)} width={S(0.07)} height={S(0.11)} rx={S(0.035)} fill="rgb(20,44,84)" fillOpacity={0.96} />
      <rect x={at(0.71) - S(0.035)} y={at(0.29)} width={S(0.07)} height={S(0.11)} rx={S(0.035)} fill="rgb(20,44,84)" fillOpacity={0.96} />
      <path
        {...feature}
        d={`M${at(0.5) + S(0.03)} ${at(0.06)} C${at(0.5) - S(0.04)} ${at(0.3)} ${at(0.5) - S(0.06)} ${at(0.44)} ${at(0.5) + S(0.06)} ${at(0.54)} L${at(0.5) - S(0.01)} ${at(0.94)}`}
      />
      <path
        {...feature}
        d={`M${at(0.24)} ${at(0.65)} C${at(0.38)} ${at(0.82)} ${at(0.62)} ${at(0.82)} ${at(0.76)} ${at(0.65)}`}
      />
    </>
  );
}

function Browser() {
  const c = 128;
  const dial = S(0.41);
  const reach = dial * 0.78;
  const hw = dial * 0.14;
  const k = Math.SQRT1_2;
  const ticks = Array.from({ length: 12 }, (_, i) => {
    const angle = (i * Math.PI) / 6;
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    return (
      <line
        key={i}
        x1={c + cos * dial * 0.78}
        y1={c + sin * dial * 0.78}
        x2={c + cos * dial * 0.91}
        y2={c + sin * dial * 0.91}
        stroke="rgb(255,255,255)"
        strokeOpacity={0.95}
        strokeWidth={S(0.022)}
        strokeLinecap="round"
      />
    );
  });
  const red = `M${c + k * reach} ${c - k * reach} L${c + k * hw} ${c + k * hw} L${c - k * hw} ${c - k * hw} Z`;
  const white = `M${c - k * reach} ${c + k * reach} L${c + k * hw} ${c + k * hw} L${c - k * hw} ${c - k * hw} Z`;
  return (
    <>
      <Plate fill="rgb(250,252,255)" />
      <circle cx={c} cy={c} r={dial} fill="rgb(24,132,247)" />
      {ticks}
      <path d={red} fill="rgb(255,59,48)" fillOpacity={0.98} />
      <path d={white} fill="rgb(255,255,255)" fillOpacity={0.95} />
    </>
  );
}

function Terminal() {
  const pad = S(0.055);
  const glyph = { ...line, stroke: "rgb(245,247,250)", strokeOpacity: 0.96, strokeWidth: S(0.06) };
  return (
    <>
      <Plate fill="rgb(58,62,70)" />
      <rect x={18 + pad} y={18 + pad} width={220 - 2 * pad} height={220 - 2 * pad} rx={49 - pad} fill="rgb(24,26,31)" />
      <path {...glyph} d={`M${at(0.22)} ${at(0.28)} L${at(0.38)} ${at(0.41)} L${at(0.22)} ${at(0.54)}`} />
      <path {...glyph} d={`M${at(0.44)} ${at(0.54)} L${at(0.64)} ${at(0.54)}`} />
    </>
  );
}

function Code() {
  const sw = S(0.075);
  const mid = at(0.5);
  const cyan = { ...line, stroke: "rgb(36,172,242)", strokeWidth: sw };
  const deep = { ...line, stroke: "rgb(0,120,212)", strokeWidth: sw };
  return (
    <>
      <Plate fill="rgb(24,30,42)" />
      <path {...cyan} d={`M${at(0.4)} ${mid - S(0.2)} L${at(0.22)} ${mid} L${at(0.4)} ${mid + S(0.2)}`} />
      <path {...deep} d={`M${at(0.6)} ${mid - S(0.2)} L${at(0.78)} ${mid} L${at(0.6)} ${mid + S(0.2)}`} />
      <line
        x1={at(0.54)}
        y1={mid - S(0.23)}
        x2={at(0.46)}
        y2={mid + S(0.23)}
        stroke="rgb(36,172,242)"
        strokeWidth={Math.max(sw * 0.82, 1.4)}
        strokeLinecap="round"
      />
    </>
  );
}

function Music() {
  const lx = at(0.36);
  const rx = at(0.68);
  const tl = at(0.28);
  const tr = at(0.22);
  const bl = at(0.68);
  const br = at(0.62);
  const stem = S(0.065);
  const nw = S(0.095);
  const nh = S(0.072);
  return (
    <>
      <Plate fill="rgb(250,45,72)" />
      <line x1={lx} y1={tl} x2={lx} y2={bl} stroke="rgb(255,255,255)" strokeWidth={stem} strokeLinecap="round" />
      <line x1={rx} y1={tr} x2={rx} y2={br} stroke="rgb(255,255,255)" strokeWidth={stem} strokeLinecap="round" />
      <line x1={lx} y1={tl} x2={rx} y2={tr} stroke="rgb(255,255,255)" strokeWidth={Math.max(stem * 1.45, 2.2)} strokeLinecap="round" />
      <ellipse cx={lx - nw * 0.55} cy={bl} rx={nw} ry={nh} fill="rgb(255,255,255)" />
      <ellipse cx={rx - nw * 0.55} cy={br} rx={nw} ry={nh} fill="rgb(255,255,255)" />
    </>
  );
}

function Notes() {
  const top = 18;
  const bottom = 18 + S(0.28);
  const header = `M18 ${bottom} V${top + 49} A49 49 0 0 1 67 ${top} H189 A49 49 0 0 1 238 ${top + 49} V${bottom} Z`;
  return (
    <>
      <Plate fill="rgb(252,251,247)" />
      <path d={header} fill="rgb(245,184,41)" />
      {[0.46, 0.62, 0.78].map((f) => (
        <line
          key={f}
          x1={at(0.14)}
          y1={at(f)}
          x2={at(0.86)}
          y2={at(f)}
          stroke="rgb(214,212,206)"
          strokeWidth={S(0.025)}
          strokeLinecap="round"
        />
      ))}
    </>
  );
}

function Folder() {
  const cx = at(0.5);
  const ay0 = at(0.46);
  const ay1 = at(0.67);
  const wing = S(0.09);
  return (
    <>
      <Plate fill="rgb(238,244,252)" />
      <rect x={at(0.14)} y={at(0.22)} width={S(0.34)} height={S(0.2)} rx={S(0.06)} fill="rgb(42,152,240)" />
      <rect x={at(0.14)} y={at(0.28)} width={S(0.72)} height={S(0.54)} rx={S(0.07)} fill="rgb(42,152,240)" />
      <rect x={at(0.14)} y={at(0.36)} width={S(0.72)} height={S(0.46)} rx={S(0.07)} fill="rgb(96,194,255)" />
      <path
        d={`M${cx} ${ay0} L${cx} ${ay1} M${cx - wing} ${ay1 - wing * 0.85} L${cx} ${ay1} L${cx + wing} ${ay1 - wing * 0.85}`}
        fill="none"
        stroke="rgb(24,118,210)"
        strokeOpacity={0.92}
        strokeWidth={S(0.048)}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </>
  );
}

function Trash() {
  const cx = 128;
  const top = at(0.24);
  const bottom = at(0.82);
  const topHw = S(0.28);
  const botHw = S(0.21);
  const bin = { ...line, stroke: "rgb(104,112,124)", strokeOpacity: 0.95, strokeWidth: S(0.025) };
  return (
    <>
      <Plate fill="rgb(232,236,242)" />
      <path
        {...bin}
        d={`M${cx - topHw} ${top} L${cx + topHw} ${top} L${cx + S(0.21)} ${bottom} L${cx - S(0.21)} ${bottom} Z`}
        fill="rgb(180,188,198)"
      />
      <ellipse {...bin} cx={cx} cy={top} rx={topHw} ry={S(0.055)} fill="rgb(248,250,252)" />
      {[-0.6, -0.3, 0, 0.3, 0.6].map((t) => (
        <line
          key={t}
          x1={cx + topHw * 0.82 * t}
          y1={top + S(0.08)}
          x2={cx + botHw * 0.82 * t}
          y2={bottom - S(0.05)}
          stroke="rgb(104,112,124)"
          strokeOpacity={0.95}
          strokeWidth={S(0.022)}
          strokeLinecap="round"
        />
      ))}
    </>
  );
}

const ART = {
  finder: Finder,
  browser: Browser,
  terminal: Terminal,
  code: Code,
  music: Music,
  notes: Notes,
  folder: Folder,
  trash: Trash,
} satisfies Record<DockArtKind, () => ReactNode>;

function Rim() {
  return (
    <rect
      x={18}
      y={18}
      width={220}
      height={220}
      rx={49}
      fill="none"
      stroke="rgb(255,255,255)"
      strokeOpacity={0.3}
      strokeWidth={1.1}
      vectorEffect="non-scaling-stroke"
    />
  );
}

export function DockArt({ kind }: { kind: DockArtKind }) {
  const Art = ART[kind];
  return (
    <svg viewBox="0 0 256 256" className="block size-full" aria-hidden="true" focusable="false">
      <Art />
      <Rim />
    </svg>
  );
}
