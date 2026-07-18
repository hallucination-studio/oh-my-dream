/**
 * MiniMap colors. React Flow needs color literals, so these mirror
 * `theme/tokens.css` layer 1 by hand — keep them in sync:
 * bg = --p-graphite-sunk, mask = --p-canvas-black at 55%, node = --p-steel.
 */
export const MINIMAP_THEME = {
  bg: "#151a1f",
  mask: "rgba(17, 20, 24, 0.55)",
  node: "#343c46",
} as const;
