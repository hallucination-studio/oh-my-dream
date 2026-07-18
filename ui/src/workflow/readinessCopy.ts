// Translates engine-owned readiness issues into creator-facing guidance, in
// canvas order. The frontend never invents rules — it only renders findings.

import type { JsonValue } from "../api/types.ts";
import type { WorkflowReadinessDto } from "../api/index.ts";

export interface ReadinessIssueView {
  nodeId: string | null;
  copy: string;
}

export function projectReadinessIssues(
  readiness: WorkflowReadinessDto | null,
  inputType: (nodeId: string, inputKey: string) => string | null,
  canvasOrder: readonly string[],
): ReadinessIssueView[] {
  if (!readiness || readiness.state !== "blocked") return [];
  const indexOf = (nodeId: string | null) => {
    const index = nodeId === null ? -1 : canvasOrder.indexOf(nodeId);
    return index === -1 ? canvasOrder.length : index;
  };
  return readiness.issues
    .map((issue) => viewOf(issue, inputType))
    .sort((left, right) => indexOf(left.nodeId) - indexOf(right.nodeId));
}

function viewOf(
  issue: JsonValue,
  inputType: (nodeId: string, inputKey: string) => string | null,
): ReadinessIssueView {
  const record = objectOf(issue);
  const kind = typeof record?.kind === "string" ? record.kind : null;
  const nodeId = typeof record?.node_id === "string" ? record.node_id : null;
  const detail = objectOf(record?.detail);
  if (kind === "required_input_missing" && typeof detail?.input_key === "string") {
    const sourceType =
      (nodeId ? inputType(nodeId, detail.input_key) : null) ?? detail.input_key;
    return {
      nodeId,
      copy: `Connect ${article(sourceType)} ${label(sourceType)} output to ${label(detail.input_key)}.`,
    };
  }
  if (kind === "required_parameter_missing" && typeof detail?.parameter_key === "string") {
    return {
      nodeId,
      copy: PARAMETER_GUIDANCE[detail.parameter_key]
        ?? `Enter a value for ${label(detail.parameter_key)}.`,
    };
  }
  if (kind === "asset_unavailable") {
    return { nodeId, copy: "The selected Asset is not available." };
  }
  return { nodeId, copy: "This step needs attention before it can run." };
}

function objectOf(value: JsonValue | undefined): Record<string, JsonValue> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, JsonValue>)
    : null;
}

/** Frozen creator guidance for required parameters (docs/DESKTOP_UI.md, Readiness). */
const PARAMETER_GUIDANCE: Record<string, string> = {
  generation_profile_ref: "Choose a generation model.",
  asset_id: "Choose an asset.",
  text: "Write the text.",
};

function article(key: string): string {
  return /^[aeiou]/i.test(key) ? "an" : "a";
}

function label(key: string): string {
  const words = key.replaceAll("_", " ").trim();
  return words ? words[0]!.toUpperCase() + words.slice(1) : key;
}
