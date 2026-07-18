// Deterministic canvas placement: a fixed slot grid scanned in order, so a
// newly added node never lands beneath another node — even after deletions.

import type { Node } from "@xyflow/react";

const NODE_WIDTH = 336;
const NODE_HEIGHT = 220;
const GAP_X = 44;
const GAP_Y = 40;
const COLUMNS = 4;
const ORIGIN = { x: 140, y: 100 };
const SCAN_LIMIT = 512;

export function firstFreeSlot(nodes: readonly Node[]): { x: number; y: number } {
  for (let index = 0; index < SCAN_LIMIT; index += 1) {
    const candidate = slot(index);
    if (!nodes.some((node) => overlaps(candidate, node))) return candidate;
  }
  return slot(0);
}

function slot(index: number): { x: number; y: number } {
  const column = index % COLUMNS;
  const row = Math.floor(index / COLUMNS);
  return {
    x: ORIGIN.x + column * (NODE_WIDTH + GAP_X),
    y: ORIGIN.y + row * (NODE_HEIGHT + GAP_Y),
  };
}

function overlaps(candidate: { x: number; y: number }, node: Node): boolean {
  const width = node.measured?.width ?? node.width ?? NODE_WIDTH;
  const height = node.measured?.height ?? node.height ?? NODE_HEIGHT;
  return (
    candidate.x < node.position.x + width &&
    candidate.x + NODE_WIDTH > node.position.x &&
    candidate.y < node.position.y + height &&
    candidate.y + NODE_HEIGHT > node.position.y
  );
}
