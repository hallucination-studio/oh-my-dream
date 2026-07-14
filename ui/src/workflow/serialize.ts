// Converts the React Flow node/edge state into the engine's Workflow JSON
// (docs/DESIGN.md §5). Kept separate so the serialization contract lives in one
// place and can be unit-tested independently of the canvas.

import type { Edge, Node } from "@xyflow/react";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import type { OutputRef, Workflow, WorkflowInputBinding } from "./types.ts";

export function toWorkflow(nodes: Node[], edges: Edge[], projectId: string): Workflow {
  return {
    version: "1.0",
    project_id: projectId,
    nodes: nodes.map((node) => {
      const data = node.data as FlowNodeData;
      const inputs: Record<string, WorkflowInputBinding> = {};
      const incoming = new Map<string, Edge[]>();
      for (const edge of edges) {
        if (edge.target !== node.id || !edge.targetHandle || !edge.sourceHandle) continue;
        const group = incoming.get(edge.targetHandle) ?? [];
        group.push(edge);
        incoming.set(edge.targetHandle, group);
      }
      for (const [handle, group] of incoming) {
        const ordered = isManyInput(data, handle);
        const sorted = ordered ? sortOrderedEdges(group) : group;
        if (!ordered && sorted.length > 1) {
          throw new Error(`multiple edges target \`${node.id}.${handle}\``);
        }
        const refs = sorted.map((edge) => [edge.source, edge.sourceHandle!] as OutputRef);
        if (ordered) {
          inputs[handle] = { kind: "ordered_many", sources: refs };
        } else if (refs[0]) {
          inputs[handle] = refs[0];
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

function isManyInput(data: FlowNodeData, handle: string): boolean {
  const cardinality = data.capability?.inputs.find((port) => port.name === handle)?.cardinality;
  return cardinality !== undefined && cardinality !== "one";
}

function sortOrderedEdges(edges: Edge[]): Edge[] {
  return edges
    .map((edge, index) => ({ edge, index }))
    .sort((left, right) => {
      const leftOrder = typeof left.edge.data?.order === "number" ? left.edge.data.order : left.index;
      const rightOrder = typeof right.edge.data?.order === "number" ? right.edge.data.order : right.index;
      return leftOrder - rightOrder || left.index - right.index;
    })
    .map(({ edge }) => edge);
}
