/** Creator-language parameter labels (docs/DESKTOP_UI.md, Labels and copy). */

const PARAMETER_LABELS: Record<string, string> = {
  text: "Text",
  aspect_ratio: "Aspect ratio",
  duration_seconds: "Duration (seconds)",
  asset_id: "Asset",
  generation_profile_ref: "Generation model",
};

/**
 * The label for a parameter key: the frozen label when one exists, otherwise a
 * sentence-case humanization of the key. Parameter keys never appear as-is.
 */
export function parameterLabel(key: string): string {
  const known = PARAMETER_LABELS[key];
  if (known) return known;
  const words = key.replaceAll("_", " ").trim();
  if (words.length === 0) return key;
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** Human option labels for enum parameters: `square` → `Square 1:1`. */
const OPTION_LABELS: Record<string, string> = {
  square: "Square 1:1",
  landscape_16_9: "Landscape 16:9",
  landscape_4_3: "Landscape 4:3",
  portrait_3_4: "Portrait 3:4",
  portrait_9_16: "Portrait 9:16",
};

/**
 * The label for an enum option value: the frozen label when one exists,
 * otherwise a sentence-case humanization. Raw enum keys never appear as-is.
 */
export function optionLabel(value: string): string {
  const known = OPTION_LABELS[value];
  if (known) return known;
  return parameterLabel(value);
}
