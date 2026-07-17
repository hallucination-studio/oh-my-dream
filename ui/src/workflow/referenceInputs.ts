import type { Edge, Node } from "@xyflow/react";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { arePortTypesCompatible, type NodeTypeSpec } from "../nodes/catalog.ts";
import { workflowEdgeId } from "./editor.ts";
import type { PortType } from "./types.ts";

export interface ReferenceTarget {
  nodeId: string;
  inputName: string;
}

export interface ReferenceSource {
  nodeId: string;
  outputName: string;
}

export function compatibleImageOutputs(
  nodes: readonly Node[],
  targetNodeId: string,
  inputName: string,
): ReferenceSource[] {
  const input = referenceInput(nodes, { nodeId: targetNodeId, inputName });
  return nodes.flatMap((node) => {
    if (node.id === targetNodeId) return [];
    const capability = capabilityOf(node);
    if (!capability || capability.status.availability !== "available") return [];
    return capability.outputs
      .filter((output) => arePortTypesCompatible(output.type, input.type))
      .map((output) => ({ nodeId: node.id, outputName: output.name }));
  });
}

export function addReference(
  nodes: readonly Node[],
  edges: readonly Edge[],
  target: ReferenceTarget,
  source: ReferenceSource,
  data: Record<string, unknown>,
): Edge[] {
  const input = referenceInput(nodes, target);
  assertCompatible(nodes, target, source, input.type);
  const references = orderedReferences(edges, target);
  if (references.some((edge) => matchesSource(edge, source))) {
    throw new Error("duplicate reference output");
  }
  const maximum = input.cardinality.many.maximum;
  if (maximum !== null && references.length >= maximum) {
    throw new Error("reference input maximum reached");
  }
  references.push({
    id: workflowEdgeId(source.nodeId, source.outputName, target.nodeId, target.inputName),
    source: source.nodeId,
    sourceHandle: source.outputName,
    target: target.nodeId,
    targetHandle: target.inputName,
    type: "workflow",
    data,
  });
  return mergeReferences(edges, target, references);
}

export function replaceReference(
  nodes: readonly Node[],
  edges: readonly Edge[],
  target: ReferenceTarget,
  index: number,
  source: ReferenceSource,
): Edge[] {
  const input = referenceInput(nodes, target);
  assertCompatible(nodes, target, source, input.type);
  const references = orderedReferences(edges, target);
  assertIndex(references, index);
  if (references.some((edge, edgeIndex) => edgeIndex !== index && matchesSource(edge, source))) {
    throw new Error("duplicate reference output");
  }
  references[index] = {
    ...references[index],
    source: source.nodeId,
    sourceHandle: source.outputName,
  };
  return mergeReferences(edges, target, references);
}

export function removeReference(
  edges: readonly Edge[],
  target: ReferenceTarget,
  index: number,
): Edge[] {
  const references = orderedReferences(edges, target);
  assertIndex(references, index);
  references.splice(index, 1);
  return mergeReferences(edges, target, references);
}

export function moveReference(
  edges: readonly Edge[],
  target: ReferenceTarget,
  fromIndex: number,
  toIndex: number,
): Edge[] {
  const references = orderedReferences(edges, target);
  assertIndex(references, fromIndex);
  assertIndex(references, toIndex);
  const [moved] = references.splice(fromIndex, 1);
  references.splice(toIndex, 0, moved);
  return mergeReferences(edges, target, references);
}

function referenceInput(nodes: readonly Node[], target: ReferenceTarget) {
  const capability = capabilityOf(nodes.find((node) => node.id === target.nodeId));
  const input = capability?.inputs.find((candidate) => candidate.name === target.inputName);
  if (!input || input.type !== "image" || typeof input.cardinality !== "object") {
    throw new Error("target is not an ordered image reference input");
  }
  return input as typeof input & { cardinality: { many: { minimum: number; maximum: number | null } } };
}

function assertCompatible(
  nodes: readonly Node[],
  target: ReferenceTarget,
  source: ReferenceSource,
  expectedType: PortType,
) {
  if (source.nodeId === target.nodeId) throw new Error("incompatible reference output");
  const capability = capabilityOf(nodes.find((node) => node.id === source.nodeId));
  const output = capability?.outputs.find((candidate) => candidate.name === source.outputName);
  if (
    !output ||
    !arePortTypesCompatible(output.type, expectedType) ||
    capability?.status.availability !== "available"
  ) {
    throw new Error("incompatible reference output");
  }
}

function orderedReferences(edges: readonly Edge[], target: ReferenceTarget): Edge[] {
  return edges
    .map((edge, index) => ({ edge, index }))
    .filter(({ edge }) => isReference(edge, target))
    .sort((left, right) => orderOf(left.edge, left.index) - orderOf(right.edge, right.index))
    .map(({ edge }) => edge);
}

function mergeReferences(
  edges: readonly Edge[],
  target: ReferenceTarget,
  references: readonly Edge[],
): Edge[] {
  const unrelated = edges.filter((edge) => !isReference(edge, target));
  return [
    ...unrelated,
    ...references.map((edge, order) => ({ ...edge, data: { ...edge.data, order } })),
  ];
}

function capabilityOf(node: Node | undefined): NodeTypeSpec | undefined {
  return (node?.data as FlowNodeData | undefined)?.capability;
}

function isReference(edge: Edge, target: ReferenceTarget): boolean {
  return edge.target === target.nodeId && edge.targetHandle === target.inputName;
}

function matchesSource(edge: Edge, source: ReferenceSource): boolean {
  return edge.source === source.nodeId && edge.sourceHandle === source.outputName;
}

function orderOf(edge: Edge, fallback: number): number {
  return typeof edge.data?.order === "number" ? edge.data.order : fallback;
}

function assertIndex(edges: readonly Edge[], index: number) {
  if (!Number.isInteger(index) || index < 0 || index >= edges.length) {
    throw new Error("reference index is out of bounds");
  }
}
