import { addEdge, type Connection, type Edge, type Node } from "@xyflow/react";
import { findNodeType } from "../nodes/catalog.ts";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { typeColor } from "../nodes/typeColor.ts";
import type { Workflow } from "./types.ts";

export interface EditorGraph {
  nodes: Node[];
  edges: Edge[];
}

type ParamChangeHandler = (nodeId: string, name: string, value: unknown) => void;

export function fromWorkflow(
  workflow: Workflow,
  onParamChange: ParamChangeHandler,
): EditorGraph {
  const nodes = workflow.nodes.map((workflowNode, index) => ({
    id: workflowNode.id,
    type: "workflow",
    position: positionFor(workflowNode.position, index),
    data: {
      type: workflowNode.type,
      params: { ...workflowNode.params },
      onParamChange: (name: string, value: unknown) =>
        onParamChange(workflowNode.id, name, value),
    } satisfies FlowNodeData,
  }));
  const edges = workflow.nodes.flatMap((target) =>
    Object.entries(target.inputs).map(([targetHandle, [source, sourceHandle]]) => ({
      id: edgeId(source, sourceHandle, target.id, targetHandle),
      source,
      sourceHandle,
      target: target.id,
      targetHandle,
      type: "workflow",
      data: { color: sourceColor(workflow, source, sourceHandle) },
    })),
  );
  return { nodes, edges };
}

export function upsertIncomingEdge(
  edges: Edge[],
  connection: Connection,
  data: Record<string, unknown>,
): Edge[] {
  if (!connection.sourceHandle || !connection.targetHandle) {
    throw new Error("workflow connections require named source and target handles");
  }
  const remaining = edges.filter(
    (edge) =>
      edge.target !== connection.target || edge.targetHandle !== connection.targetHandle,
  );
  return addEdge({ ...connection, type: "workflow", data }, remaining);
}

export function nextNodeId(nodes: readonly Pick<Node, "id">[]): string {
  const maxId = nodes.reduce((currentMax, node) => {
    const match = /^n(\d+)$/.exec(node.id);
    if (!match) {
      return currentMax;
    }
    const value = Number(match[1]);
    return Number.isSafeInteger(value) ? Math.max(currentMax, value) : currentMax;
  }, 0);
  return `n${maxId + 1}`;
}

function positionFor(position: [number, number] | undefined, index: number) {
  return position
    ? { x: position[0], y: position[1] }
    : { x: 140 + index * 60, y: 100 + index * 40 };
}

function sourceColor(workflow: Workflow, nodeId: string, outputName: string): string {
  const source = workflow.nodes.find((node) => node.id === nodeId);
  const spec = source ? findNodeType(source.type) : undefined;
  const portType = spec?.outputs.find((output) => output.name === outputName)?.type;
  return typeColor(portType);
}

function edgeId(
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): string {
  return `workflow-edge:${JSON.stringify([source, sourceHandle, target, targetHandle])}`;
}
