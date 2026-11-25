import { useReducedMotion } from "motion/react";
import { useState } from "react";
import { useEased } from "../lib/spring";

export type PartId = "bar" | "notch" | "window" | "cc" | "spotlight" | "dock";

type Pt = [number, number];
type Rect = [number, number, number, number];

type Detail = {
  rect: Rect;
  r?: number;
  fill: string;
};

type Dot = {
  c: Pt;
  r: number;
  fill: string;
};

type Part = {
  id: PartId | "base";
  rect: Rect;
  r: number;
  lift: number;
  fill: string;
  edge: string;
  thick: number;
  details?: Detail[];
  dots?: Dot[];
  label?: { id: string; en: string };
};

const COS = Math.cos(Math.PI / 6);
const OX = 300;
const OY = 216;

const PARTS: Part[] = [
  {
    id: "base",
    rect: [0, 0, 360, 240],
    r: 14,
    lift: 0,
    fill: "#dcebfb",
    edge: "#b5cde6",
    thick: 10,
  },
  {
    id: "dock",
    rect: [78, 204, 282, 232],
    r: 14,
    lift: 42,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 5,
    details: [
      { rect: [90, 209, 112, 228], r: 6, fill: "#5b9cff" },
      { rect: [118, 209, 140, 228], r: 6, fill: "#7fd17a" },
      { rect: [146, 209, 168, 228], r: 6, fill: "#ffb36b" },
      { rect: [174, 209, 196, 228], r: 6, fill: "#a78bfa" },
      { rect: [202, 209, 224, 228], r: 6, fill: "#ff7a8a" },
      { rect: [236, 209, 254, 228], r: 5, fill: "#5ec9a7" },
      { rect: [260, 209, 276, 228], r: 5, fill: "#8fa3b8" },
    ],
    label: { id: "Dock & Genie", en: "Dock & Genie" },
  },
  {
    id: "window",
    rect: [34, 42, 260, 190],
    r: 12,
    lift: 84,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 4,
    details: [
      { rect: [34, 42, 260, 60], r: 0, fill: "#f1f3f7" },
      { rect: [34, 60, 90, 190], r: 0, fill: "#f6f7f9" },
      { rect: [104, 76, 136, 104], r: 6, fill: "#e4edff" },
      { rect: [152, 76, 184, 104], r: 6, fill: "#e4edff" },
      { rect: [200, 76, 232, 104], r: 6, fill: "#e4edff" },
      { rect: [104, 122, 136, 150], r: 6, fill: "#e4edff" },
      { rect: [152, 122, 184, 150], r: 6, fill: "#e4edff" },
      { rect: [200, 122, 232, 150], r: 6, fill: "#e4edff" },
    ],
    dots: [
      { c: [222, 51], r: 3.6, fill: "#febc2e" },
      { c: [234, 51], r: 3.6, fill: "#28c840" },
      { c: [246, 51], r: 3.6, fill: "#ff5f57" },
    ],
    label: { id: "Kontrol Jendela", en: "Window Controls" },
  },
  {
    id: "spotlight",
    rect: [104, 88, 252, 146],
    r: 14,
    lift: 128,
    fill: "#1b1b1d",
    edge: "#3a3a40",
    thick: 4,
    details: [
      { rect: [116, 100, 240, 114], r: 6, fill: "#2c2c2f" },
      { rect: [116, 122, 166, 132], r: 4, fill: "#46464c" },
      { rect: [176, 122, 230, 132], r: 4, fill: "#323238" },
    ],
    label: { id: "Spotlight", en: "Spotlight" },
  },
  {
    id: "bar",
    rect: [0, 0, 360, 16],
    r: 0,
    lift: 168,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 3,
    details: [
      { rect: [10, 6, 42, 10], r: 2, fill: "#8d939c" },
      { rect: [52, 6, 74, 10], r: 2, fill: "#8d939c" },
      { rect: [84, 6, 108, 10], r: 2, fill: "#8d939c" },
      { rect: [298, 6, 326, 10], r: 2, fill: "#8d939c" },
      { rect: [334, 6, 350, 10], r: 2, fill: "#8d939c" },
    ],
    label: { id: "Bar Atas", en: "Top Bar" },
  },
  {
    id: "cc",
    rect: [252, 26, 348, 126],
    r: 14,
    lift: 208,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 4,
    details: [
      { rect: [264, 38, 294, 52], r: 6, fill: "#075bef" },
      { rect: [264, 58, 294, 72], r: 6, fill: "#d7dce5" },
      { rect: [302, 38, 336, 52], r: 6, fill: "#e8f1ff" },
      { rect: [302, 58, 336, 72], r: 6, fill: "#f1f3f7" },
      { rect: [264, 86, 336, 91], r: 2.5, fill: "#e5e8ee" },
      { rect: [264, 86, 314, 91], r: 2.5, fill: "#075bef" },
      { rect: [264, 104, 336, 109], r: 2.5, fill: "#e5e8ee" },
      { rect: [264, 104, 302, 109], r: 2.5, fill: "#8fa3b8" },
    ],
    label: { id: "Control Center", en: "Control Center" },
  },
  {
    id: "notch",
    rect: [122, 0, 238, 24],
    r: 12,
    lift: 254,
    fill: "#000000",
    edge: "#2a2a2e",
    thick: 4,
    dots: [
      { c: [142, 12], r: 3.2, fill: "#ff9230" },
      { c: [218, 12], r: 3.2, fill: "#fc3c54" },
    ],
    label: { id: "Dynamic Island", en: "Dynamic Island" },
  },
];

function project(u: number, v: number, z: number): Pt {
  return [OX + (u - v) * COS, OY + (u + v) * 0.5 - z];
}

function outline(rect: Rect, r: number): Pt[] {
  const [u0, v0, u1, v1] = rect;
  if (r <= 0) {
    return [
      [u0, v0],
      [u1, v0],
      [u1, v1],
      [u0, v1],
    ];
  }
  const k = Math.min(r, (u1 - u0) / 2, (v1 - v0) / 2);
  const corners: [number, number, number, number][] = [
    [u1 - k, v0 + k, -90, 0],
    [u1 - k, v1 - k, 0, 90],
    [u0 + k, v1 - k, 90, 180],
    [u0 + k, v0 + k, 180, 270],
  ];
  const points: Pt[] = [];
  for (const [cx, cy, a0, a1] of corners) {
    for (let i = 0; i <= 6; i++) {
      const angle = ((a0 + ((a1 - a0) * i) / 6) * Math.PI) / 180;
      points.push([cx + k * Math.cos(angle), cy + k * Math.sin(angle)]);
    }
  }
  return points;
}

function circle(c: Pt, r: number): Pt[] {
  const points: Pt[] = [];
  for (let i = 0; i < 16; i++) {
    const angle = (i / 16) * Math.PI * 2;
    points.push([c[0] + r * Math.cos(angle), c[1] + r * Math.sin(angle)]);
  }
  return points;
}

function toPath(points: Pt[]) {
  return `${points.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`).join(" ")} Z`;
}

function faceAt(rect: Rect, r: number, z: number) {
  return toPath(outline(rect, r).map(([u, v]) => project(u, v, z)));
}

function dotAt(dot: Dot, z: number) {
  return toPath(circle(dot.c, dot.r).map(([u, v]) => project(u, v, z)));
}

function centerOf(rect: Rect): Pt {
  return [(rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2];
}

type DiagramProps = {
  explode: number;
  initial?: number;
  active?: PartId | null;
  onSelect?: (id: PartId) => void;
  className?: string;
};

export function ExplodedDiagram({ explode, initial, active, onSelect, className = "" }: DiagramProps) {
  const reduced = useReducedMotion() ?? false;
  const e = useEased(explode, initial ?? explode, reduced, 0.72);
  const [hover, setHover] = useState<PartId | null>(null);
  const shown = hover ?? active ?? null;

  return (
    <svg viewBox="70 -26 560 560" className={className} role="img" aria-label="Otto OS architecture layers">
      {PARTS.map((part) => {
        const z = part.lift * e;
        const [cu, cv] = centerOf(part.rect);
        const base = project(cu, cv, 0);
        const tip = project(cu, cv, z);
        const isActive = part.id !== "base" && shown === part.id;
        const interactive = part.id !== "base";

        return (
          <g key={part.id}>
            {part.lift > 0 && e > 0.05 && (
              <line
                x1={base[0]}
                y1={base[1]}
                x2={tip[0]}
                y2={tip[1]}
                stroke={isActive ? "#075bef" : "rgb(27 27 29 / 0.28)"}
                strokeWidth={isActive ? "1.4" : "1"}
                strokeDasharray="3 4"
              />
            )}
            <path d={faceAt(part.rect, part.r, z - part.thick)} fill={part.edge} />
            {/* biome-ignore lint/a11y/noStaticElementInteractions: interactive isometric architecture layer */}
            <path
              d={faceAt(part.rect, part.r, z)}
              fill={part.fill}
              stroke={
                isActive
                  ? "#075bef"
                  : part.id === "notch" || part.id === "spotlight"
                    ? "rgb(255 255 255 / 0.16)"
                    : "rgb(27 27 29 / 0.14)"
              }
              strokeWidth={isActive ? 2 : 0.9}
              style={{ cursor: interactive ? "pointer" : "default", transition: "stroke 180ms" }}
              onPointerEnter={interactive ? () => setHover(part.id as PartId) : undefined}
              onPointerLeave={interactive ? () => setHover(null) : undefined}
              onClick={interactive && onSelect ? () => onSelect(part.id as PartId) : undefined}
            />
            {part.details?.map((detail) => (
              <path
                key={`${detail.rect.join(",")}-${detail.fill}`}
                d={faceAt(detail.rect, detail.r ?? 0, z)}
                fill={detail.fill}
                pointerEvents="none"
              />
            ))}
            {part.dots?.map((dot) => (
              <path
                key={`${dot.c[0]}-${dot.c[1]}-${dot.fill}`}
                d={dotAt(dot, z)}
                fill={dot.fill}
                pointerEvents="none"
              />
            ))}
            {e > 0.65 && isActive && part.label && (
              <g pointerEvents="none">
                <circle cx={tip[0]} cy={tip[1]} r="4.5" fill="#075bef" />
                <text
                  className="l-id"
                  x={tip[0] + 12}
                  y={tip[1] + 4}
                  fontSize="13.5"
                  fontWeight="600"
                  fill="#1b1b1d"
                  stroke="#fbfbfa"
                  strokeWidth="4"
                  paintOrder="stroke"
                >
                  {part.label.id}
                </text>
                <text
                  className="l-en"
                  x={tip[0] + 12}
                  y={tip[1] + 4}
                  fontSize="13.5"
                  fontWeight="600"
                  fill="#1b1b1d"
                  stroke="#fbfbfa"
                  strokeWidth="4"
                  paintOrder="stroke"
                >
                  {part.label.en}
                </text>
              </g>
            )}
          </g>
        );
      })}
    </svg>
  );
}

export const PART_ORDER: PartId[] = ["bar", "notch", "cc", "spotlight", "window", "dock"];
