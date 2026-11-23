import { DOCK } from "./spec";

type SlotShape = { dividerBefore?: boolean };

export const DOT_BOTTOM = DOCK.padBottom * (1 - DOCK.dotCenter) + DOCK.dotRadius;

export function dockCenters(slots: SlotShape[]) {
  const raw: number[] = [];
  let cursor = 0;
  slots.forEach((slot, index) => {
    if (slot.dividerBefore && index > 0) cursor += DOCK.dividerSlot;
    raw.push(cursor + DOCK.tile / 2);
    cursor += DOCK.tile + DOCK.gap;
  });
  const width = cursor - DOCK.gap;
  return raw.map((center) => center - width / 2);
}

export function magnifyScale(distance: number) {
  const radius = DOCK.magnifySlots * (DOCK.tile + DOCK.gap);
  if (distance >= radius) return 1;
  const wave = Math.cos((distance / radius) * (Math.PI / 2));
  return 1 + (DOCK.maxScale - 1) * wave * wave;
}

export function hoveredSlot(centers: number[], x: number) {
  const reach = (DOCK.tile + DOCK.gap) * DOCK.hoverReach;
  let found: number | null = null;
  let best = Infinity;
  for (let index = 0; index < centers.length; index += 1) {
    const distance = Math.abs(centers[index] - x);
    if (distance <= reach && distance < best) {
      best = distance;
      found = index;
    }
  }
  return found;
}

export function tooltipWidth(title: string) {
  return Math.max(DOCK.tooltipMinWidth, title.length * DOCK.tooltipFont * DOCK.tooltipCharWidth + DOCK.tooltipPad * 2);
}
