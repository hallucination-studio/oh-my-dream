/**
 * MiniMap colors. React Flow needs color literals, so these mirror
 * `theme/tokens.css` layer 1 by hand — keep them in sync:
 * bg = --p-panel, mask = --p-floor at 55%, node = --p-hair-2.
 */
export const MINIMAP_THEME = {
  bg: "#ffffff",
  mask: "rgba(237, 240, 243, 0.55)",
  node: "#d5dbe1",
} as const;
