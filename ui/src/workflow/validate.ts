// UI-side connection validation: reject a wire whose source output type does
// not equal the target input type. This mirrors the engine's build-time
// type-checked wiring so users get immediate feedback at connect time.

import type { Connection, Node } from "@xyflow/react";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { findNodeType } from "../nodes/catalog.ts";
import type { PortType } from "./types.ts";

function outputType(node: Node, handle: string | null | undefined): PortType | undefined {
  const spec = findNodeType((node.data as FlowNodeData).type);
  return spec?.outputs.find((p) => p.name === handle)?.type;
}

function inputType(node: Node, handle: string | null | undefined): PortType | undefined {
  const spec = findNodeType((node.data as FlowNodeData).type);
  return spec?.inputs.find((p) => p.name === handle)?.type;
}

export function isValidConnection(connection: Connection, nodes: Node[]): boolean {
  const source = nodes.find((n) => n.id === connection.source);
  const target = nodes.find((n) => n.id === connection.target);
  if (!source || !target) {
    return false;
  }
  const from = outputType(source, connection.sourceHandle);
  const to = inputType(target, connection.targetHandle);
  // Exact-match only, matching engine PortType::is_compatible_with.
  return from !== undefined && from === to;
}
