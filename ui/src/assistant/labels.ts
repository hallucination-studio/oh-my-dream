/** Creator-language label for an assistant tool identifier (`workflow.add_node` → `Add node`). */
export function toolLabel(toolId: string): string {
  const segment = toolId.split(".").at(-1) ?? toolId;
  return humanizeToken(segment);
}

/** Creator-language label for plan item identifiers. */
export function planItemLabel(itemId: string): string {
  return humanizeToken(itemId);
}

function humanizeToken(token: string): string {
  const words = token.replaceAll("_", " ").trim();
  return words ? words[0]!.toUpperCase() + words.slice(1) : token;
}

/** Creator-language state label for assistant plan and run rows. */
export function assistantStateLabel(state: string): string {
  switch (state) {
    case "running":
      return "Running";
    case "succeeded":
      return "Complete";
    case "failed":
      return "Needs attention";
    case "cancelled":
      return "Cancelled";
    default:
      return humanizeToken(state);
  }
}
