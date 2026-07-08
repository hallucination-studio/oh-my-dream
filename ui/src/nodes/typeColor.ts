// Maps a port data type to its channel color. This single source drives node
// port gems, edge colors, and any type-coded UI, keeping the "typed wiring"
// signature consistent everywhere.

import type { PortType } from "../workflow/types.ts";

export const TYPE_COLOR: Record<PortType, string> = {
  string: "var(--type-text)",
  image: "var(--type-image)",
  video: "var(--type-video)",
  model: "var(--type-model)",
  int: "var(--type-number)",
  float: "var(--type-number)",
};

export function typeColor(type: PortType | undefined): string {
  return type ? TYPE_COLOR[type] : "var(--line-strong)";
}
