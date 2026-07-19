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

/* The node header is the frozen pastel tint + deep text pair of the node's
 * media type (docs/DESKTOP_UI.md, Nodes) — never derived on the fly. */
const HEADER_TINT: Partial<Record<PortType, { bg: string; fg: string }>> = {
  string: { bg: "var(--p-h-text-bg)", fg: "var(--p-h-text-fg)" },
  image: { bg: "var(--p-h-image-bg)", fg: "var(--p-h-image-fg)" },
  video: { bg: "var(--p-h-video-bg)", fg: "var(--p-h-video-fg)" },
  audio: { bg: "var(--p-h-audio-bg)", fg: "var(--p-h-audio-fg)" },
};

/** Header tint pair for a node; types without a frozen pair fall back to a mix. */
export function nodeHeaderTint(
  outputs: { type: PortType }[],
  inputs: { type: PortType }[],
): { bg: string; fg: string } {
  const type = outputs[0]?.type ?? inputs[0]?.type;
  const fixed = type ? HEADER_TINT[type] : undefined;
  if (fixed) return fixed;
  const color = typeColor(type);
  return {
    bg: `color-mix(in srgb, ${color} 14%, #ffffff)`,
    fg: `color-mix(in srgb, ${color} 58%, var(--ink))`,
  };
}

/** Creator-facing label for a port's media type. */
export function portTypeLabel(type: PortType): string {
  return type === "string" ? "text" : type;
}
