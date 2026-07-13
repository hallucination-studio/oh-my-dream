// Converts the React Flow node/edge state into the engine's Workflow JSON
// (docs/DESIGN.md §5). Kept separate so the serialization contract lives in one
// place and can be unit-tested independently of the canvas.

import type { Edge, Node } from "@xyflow/react";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import type { OutputRef, Workflow } from "./types.ts";

export function toWorkflow(nodes: Node[], edges: Edge[], projectId = "default"): Workflow {
  return {
    version: "1.0",
    project_id: projectId,
    nodes: nodes.map((node) => {
      const data = node.data as FlowNodeData;
      const inputs: Record<string, OutputRef> = {};
      for (const edge of edges) {
        // An edge into this node maps target handle -> [source node, source handle].
        if (edge.target === node.id && edge.targetHandle && edge.sourceHandle) {
          if (Object.hasOwn(inputs, edge.targetHandle)) {
            throw new Error(`multiple edges target \`${node.id}.${edge.targetHandle}\``);
          }
          inputs[edge.targetHandle] = [edge.source, edge.sourceHandle];
        }
      }
      return {
        id: node.id,
        type: data.type,
        contract_version: data.contractVersion ?? "1.0",
        params: data.params,
        inputs,
        position: [node.position.x, node.position.y] as [number, number],
      };
    }),
  };
}
