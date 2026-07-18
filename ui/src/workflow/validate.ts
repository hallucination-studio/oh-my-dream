// UI-side connection validation: reject a wire whose source output type does
// not equal the target input type, a self-loop, or an edge that would close a
// cycle. This mirrors the engine's build-time graph checks so users get
// immediate feedback at connect time instead of a later save failure.

import type { Connection, Edge, Node } from "@xyflow/react";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { canConnectPorts } from "../nodes/catalog.ts";

export function isValidConnection(
  connection: Connection,
  nodes: Node[],
  edges: Edge[] = [],
): boolean {
  if (connection.source === connection.target) {
    return false;
  }
  if (closesCycle(connection, edges)) {
    return false;
  }
  const source = nodes.find((n) => n.id === connection.source);
  const target = nodes.find((n) => n.id === connection.target);
  if (!source || !target) {
    return false;
  }
  return canConnectPorts(
    (source.data as FlowNodeData).capability,
    connection.sourceHandle,
    (target.data as FlowNodeData).capability,
    connection.targetHandle,
  );
}

/** Adding source→target closes a cycle when the target can already reach the source. */
function closesCycle(connection: Connection, edges: Edge[]): boolean {
  const adjacency = new Map<string, string[]>();
  for (const edge of edges) {
    adjacency.set(edge.source, [...(adjacency.get(edge.source) ?? []), edge.target]);
  }
  const stack = [connection.target];
  const seen = new Set<string>();
  while (stack.length > 0) {
    const current = stack.pop()!;
    if (current === connection.source) return true;
    if (seen.has(current)) continue;
    seen.add(current);
    for (const next of adjacency.get(current) ?? []) stack.push(next);
  }
  return false;
}
