import { useState } from "react";
import { useReducedMotion } from "motion/react";
import { useEased } from "../lib/spring";

export type PartId = "bar" | "notch" | "window" | "cc" | "dock";

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
  n?: number;
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
const OY = 200;

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
    n: 5,
    rect: [84, 204, 276, 232],
    r: 14,
    lift: 46,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 5,
    details: [
      { rect: [96, 209, 118, 229], r: 6, fill: "#5b9cff" },
      { rect: [126, 209, 148, 229], r: 6, fill: "#7fd17a" },
      { rect: [156, 209, 178, 229], r: 6, fill: "#ffb36b" },
      { rect: [186, 209, 208, 229], r: 6, fill: "#a78bfa" },
      { rect: [216, 209, 238, 229], r: 6, fill: "#ff7a8a" },
      { rect: [246, 209, 268, 229], r: 6, fill: "#8fa3b8" },
    ],
    label: { id: "Dock", en: "Dock" },
  },
  {
    id: "window",
    n: 3,
    rect: [36, 44, 262, 192],
    r: 12,
    lift: 92,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 4,
    details: [
      { rect: [36, 44, 262, 62], r: 0, fill: "#f1f3f7" },
      { rect: [36, 62, 92, 192], r: 0, fill: "#f6f7f9" },
      { rect: [106, 78, 136, 106], r: 6, fill: "#e4edff" },
      { rect: [156, 78, 186, 106], r: 6, fill: "#e4edff" },
      { rect: [206, 78, 236, 106], r: 6, fill: "#e4edff" },
      { rect: [106, 124, 136, 152], r: 6, fill: "#e4edff" },
      { rect: [156, 124, 186, 152], r: 6, fill: "#e4edff" },
      { rect: [206, 124, 236, 152], r: 6, fill: "#e4edff" },
      { rect: [106, 170, 220, 176], r: 3, fill: "#e5e8ee" },
    ],
    dots: [
      { c: [52, 53], r: 3.6, fill: "#ff5f57" },
      { c: [64, 53], r: 3.6, fill: "#febc2e" },
      { c: [76, 53], r: 3.6, fill: "#28c840" },
    ],
    label: { id: "Jendela", en: "Window" },
  },
  {
    id: "bar",
    n: 1,
    rect: [0, 0, 360, 16],
    r: 0,
    lift: 130,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 3,
    details: [
      { rect: [10, 6, 42, 10], r: 2, fill: "#8d939c" },
      { rect: [52, 6, 74, 10], r: 2, fill: "#8d939c" },
      { rect: [84, 6, 108, 10], r: 2, fill: "#8d939c" },
      { rect: [300, 6, 326, 10], r: 2, fill: "#8d939c" },
      { rect: [334, 6, 350, 10], r: 2, fill: "#8d939c" },
    ],
    label: { id: "Bar atas", en: "Top bar" },
  },
  {
    id: "cc",
    n: 4,
    rect: [250, 30, 344, 128],
    r: 14,
    lift: 180,
    fill: "#ffffff",
    edge: "#cdd5e0",
    thick: 4,
    details: [
      { rect: [262, 44, 288, 56], r: 6, fill: "#075bef" },
      { rect: [262, 64, 288, 76], r: 6, fill: "#d7dce5" },
      { rect: [262, 90, 334, 94], r: 2, fill: "#e5e8ee" },
      { rect: [262, 90, 312, 94], r: 2, fill: "#075bef" },
      { rect: [262, 106, 334, 110], r: 2, fill: "#e5e8ee" },
      { rect: [262, 106, 300, 110], r: 2, fill: "#8fa3b8" },
    ],
    label: { id: "Control Center", en: "Control Center" },
  },
  {
    id: "notch",
    n: 2,
    rect: [128, 0, 232, 24],
    r: 12,
    lift: 240,
    fill: "#000000",
    edge: "#2a2a2e",
    thick: 4,
    dots: [{ c: [146, 12], r: 3, fill: "#ff6a2b" }],
    label: { id: "Notch", en: "Notch" },
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
  return (
    points
      .map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`)
      .join(" ") + " Z"
  );
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
  labels?: boolean;
  className?: string;
};

export function ExplodedDiagram({ explode, initial, active, labels = false, className = "" }: DiagramProps) {
  const reduced = useReducedMotion() ?? false;
  const e = useEased(explode, initial ?? explode, reduced);
  const [hover, setHover] = useState<PartId | null>(null);
  const shown = active !== undefined ? active : hover;

  return (
    <svg
      viewBox="80 0 544 520"
      className={className}
      aria-hidden="true"
      focusable="false"
      role="presentation"
    >
      {PARTS.map((part) => {
        const z = part.lift * e;
        const [cu, cv] = centerOf(part.rect);
        const base = project(cu, cv, 0);
        const tip = project(cu, cv, z);
        const isActive = part.id !== "base" && shown === part.id;
        const interactive = part.id !== "base";

        return (
          <g key={part.id}>
            {part.lift > 0 && (
              <line
                x1={base[0]}
                y1={base[1]}
                x2={tip[0]}
                y2={tip[1]}
                stroke="rgb(27 27 29 / 0.35)"
                strokeWidth="1"
                strokeDasharray="3 4"
              />
            )}
            <path d={faceAt(part.rect, part.r, z - part.thick)} fill={part.edge} />
            <path
              d={faceAt(part.rect, part.r, z)}
              fill={part.fill}
              stroke={isActive ? "#075bef" : "rgb(27 27 29 / 0.12)"}
              strokeWidth={isActive ? 2 : 0.8}
              style={{ cursor: interactive ? "pointer" : "default", transition: "stroke 200ms" }}
              onPointerEnter={interactive ? () => setHover(part.id as PartId) : undefined}
              onPointerLeave={interactive ? () => setHover(null) : undefined}
            />
            {part.details?.map((detail, index) => (
              <path key={index} d={faceAt(detail.rect, detail.r ?? 0, z)} fill={detail.fill} pointerEvents="none" />
            ))}
            {part.dots?.map((dot, index) => (
              <path key={index} d={dotAt(dot, z)} fill={dot.fill} pointerEvents="none" />
            ))}
            {part.n !== undefined && (
              <g pointerEvents="none">
                <circle cx={tip[0]} cy={tip[1]} r="11" fill={isActive ? "#075bef" : "#1b1b1d"} />
                <text
                  x={tip[0]}
                  y={tip[1] + 0.5}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fontSize="12"
                  fontWeight="600"
                  fill="#ffffff"
                >
                  {part.n}
                </text>
              </g>
            )}
            {labels && isActive && part.label && (
              <g pointerEvents="none">
                <text
                  className="l-id"
                  x={tip[0] + 18}
                  y={tip[1] + 4}
                  fontSize="14"
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
                  x={tip[0] + 18}
                  y={tip[1] + 4}
                  fontSize="14"
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

export const PART_ORDER: PartId[] = ["bar", "notch", "window", "cc", "dock"];
