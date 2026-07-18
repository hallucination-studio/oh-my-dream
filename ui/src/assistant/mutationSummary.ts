// Summarizes an Assistant Workflow Change's mutations as creator-language
// facts. Raw identifiers live only in the diagnostics details, never here.

import type { JsonValue } from "../api/types.ts";
import { presentationFor } from "../nodes/exactCapability.ts";

export type MutationLabeler = (nodeId: string) => string;

export function summarizeMutation(mutation: JsonValue, label: MutationLabeler): string | null {
  const record = objectOf(mutation);
  const kind = typeof record?.kind === "string" ? record.kind : null;
  switch (kind) {
    case "add_node": {
      const capability = objectOf(record?.capability);
      const id = typeof capability?.id === "string" ? capability.id : null;
      return `Add ${id ? presentationFor(id).label : "a node"}`;
    }
    case "remove_node":
      return `Remove ${node(record, "node_id", label)}`;
    case "bind_single_input": {
      const target = objectOf(record?.target);
      const item = objectOf(record?.item);
      const source = typeof item?.source_node_id === "string" ? label(item.source_node_id) : "a node";
      const targetName =
        typeof target?.node_id === "string" ? label(target.node_id) : "a node";
      const input = typeof target?.input_key === "string" ? ` to ${target.input_key}` : "";
      return `Connect ${source} → ${targetName}${input}`;
    }
    case "remove_input_item":
      return `Disconnect an input on ${node(objectOf(record?.target), "node_id", label)}`;
    case "replace_node_parameters":
      return `Change settings on ${node(record, "node_id", label)}`;
    case "select_node_capability":
      return `Change the type of ${node(record, "node_id", label)}`;
    case "move_node":
      return null;
    default:
      return "Change the workflow";
  }
}

export function summarizeMutations(
  mutations: readonly JsonValue[],
  label: MutationLabeler,
): string[] {
  const seen = new Set<string>();
  const lines: string[] = [];
  for (const mutation of mutations) {
    const line = summarizeMutation(mutation, label);
    if (line !== null && !seen.has(line)) {
      seen.add(line);
      lines.push(line);
    }
  }
  return lines;
}

function node(
  record: Record<string, JsonValue> | null,
  key: string,
  label: MutationLabeler,
): string {
  const id = typeof record?.[key] === "string" ? (record[key] as string) : null;
  return id ? label(id) : "a node";
}

function objectOf(value: JsonValue | undefined): Record<string, JsonValue> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, JsonValue>)
    : null;
}
