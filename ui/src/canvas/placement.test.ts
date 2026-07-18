import { expect, it } from "vitest";
import type { Node } from "@xyflow/react";
import { firstFreeSlot } from "./placement.ts";

it("places the first nodes on the deterministic slot grid", () => {
  expect(firstFreeSlot([])).toEqual({ x: 140, y: 100 });
});

it("never overlaps existing nodes, even after deletions leave gaps", () => {
  const nodes = [
    nodeAt("a", 140, 100),
    nodeAt("c", 140 + 2 * 380, 100),
  ];
  // Slot 1 (520, 100) is free even though slot 2 is taken.
  expect(firstFreeSlot(nodes)).toEqual({ x: 520, y: 100 });
  // Once every slot on the first row is taken, placement moves to the next row.
  const fullRow = [0, 1, 2, 3].map((i) => nodeAt(`n${i}`, 140 + i * 380, 100));
  expect(firstFreeSlot(fullRow)).toEqual({ x: 140, y: 360 });
});

function nodeAt(id: string, x: number, y: number): Node {
  return {
    id,
    type: "workflow",
    position: { x, y },
    measured: { width: 336, height: 220 },
    data: {},
  } as unknown as Node;
}
