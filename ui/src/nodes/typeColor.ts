// Maps a port data type to its channel color. This single source drives node
// port gems, edge colors, category dots, and any type-coded UI — keeping the
// "typed wiring" signature consistent everywhere.

import type { PortType } from "../workflow/types.ts";

export const TYPE_COLOR: Record<PortType, string> = {
  string: "var(--t-text)",
  image: "var(--t-image)",
  video: "var(--t-video)",
  audio: "var(--t-audio)",
  model: "var(--t-model)",
  int: "var(--t-number)",
  float: "var(--t-number)",
};

export function typeColor(type: PortType | undefined): string {
  return type ? TYPE_COLOR[type] : "var(--ink-3)";
}

/** Color for a node, taken from its primary output (or input) type. */
export function nodeAccent(
  outputs: { type: PortType }[],
  inputs: { type: PortType }[],
): string {
  return typeColor(outputs[0]?.type ?? inputs[0]?.type);
}

/** Creator-facing label for a port's media type. */
export function portTypeLabel(type: PortType): string {
  return type === "string" ? "text" : type;
}
