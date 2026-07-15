import { addEdge, type Connection, type Edge, type Node } from "@xyflow/react";
import { findNodeType, recoveryNodeSpec } from "../nodes/catalog.ts";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import type { CapabilityCacheSnapshot } from "./contractCache.ts";
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
  snapshot: CapabilityCacheSnapshot,
): EditorGraph {
  const nodes = workflow.nodes.map((workflowNode, index) => {
    const spec =
      findNodeType(workflowNode.type, workflowNode.contract_version, workflowNode.params, snapshot) ??
      recoveryNodeSpec(
        { id: workflowNode.type, version: workflowNode.contract_version ?? "1.0" },
        "exact capability version is unavailable",
      );
    return {
      id: workflowNode.id,
      type: "workflow",
      position: positionFor(workflowNode.position, index),
      data: {
        type: workflowNode.type,
        contractVersion: workflowNode.contract_version ?? "1.0",
        capability: spec,
        params: { ...workflowNode.params },
        onParamChange: (name: string, value: unknown) =>
          onParamChange(workflowNode.id, name, value),
      } satisfies FlowNodeData,
    };
  });
  const edges = workflow.nodes.flatMap((target) =>
    Object.entries(target.inputs).flatMap(([targetHandle, binding]) =>
      outputRefsOf(binding).map(([source, sourceHandle], order) => ({
        id: workflowEdgeId(source, sourceHandle, target.id, targetHandle),
        source,
        sourceHandle,
        target: target.id,
        targetHandle,
        type: "workflow",
        data: {
          color: sourceColor(workflow, source, sourceHandle, snapshot),
          order,
        },
      })),
    ),
  );
  return { nodes, edges };
}

function outputRefsOf(binding: import("./types.ts").WorkflowInputBinding): [string, string][] {
  if ("kind" in binding) {
    if (binding.kind === "single") {
      return outputRefsOf(binding.source);
    }
    if (binding.sources.length === 0) return [];
    return binding.sources.flatMap(outputRefsOf);
  }
  if (Array.isArray(binding)) {
    return [binding];
  }
  return [[binding.node_id, binding.output]];
}

export function upsertIncomingEdge(
  edges: Edge[],
  connection: Connection,
  data: Record<string, unknown>,
  many = false,
): Edge[] {
  if (!connection.sourceHandle || !connection.targetHandle) {
    throw new Error("workflow connections require named source and target handles");
  }
  const incoming = edges.filter(
    (edge) => edge.target === connection.target && edge.targetHandle === connection.targetHandle,
  );
  const remaining = many
    ? edges
    : edges.filter(
        (edge) =>
          edge.target !== connection.target || edge.targetHandle !== connection.targetHandle,
      );
  if (!many && incoming.length > 0) {
    return addEdge({ ...connection, type: "workflow", data }, remaining);
  }
  const order = incoming.length;
  return addEdge(
    { ...connection, type: "workflow", data: { ...data, order } },
    remaining,
  );
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

function sourceColor(
  workflow: Workflow,
  nodeId: string,
  outputName: string,
  snapshot: CapabilityCacheSnapshot,
): string {
  const source = workflow.nodes.find((node) => node.id === nodeId);
  const spec = source
    ? findNodeType(source.type, source.contract_version, source.params, snapshot)
    : undefined;
  const portType = spec?.outputs.find((output) => output.name === outputName)?.type;
  return typeColor(portType);
}

export function workflowEdgeId(
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): string {
  return `workflow-edge:${JSON.stringify([source, sourceHandle, target, targetHandle])}`;
}
