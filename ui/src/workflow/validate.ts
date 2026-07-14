// UI-side connection validation: reject a wire whose source output type does
// not equal the target input type. This mirrors the engine's build-time
// type-checked wiring so users get immediate feedback at connect time.

import type { Connection, Node } from "@xyflow/react";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { canConnectPorts } from "../nodes/catalog.ts";

export function isValidConnection(connection: Connection, nodes: Node[]): boolean {
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
