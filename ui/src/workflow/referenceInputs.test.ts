import type { Edge, Node } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import type { NodeTypeSpec } from "../nodes/catalog.ts";
import { fromWorkflow } from "./editor.ts";
import {
  addReference,
  compatibleImageOutputs,
  moveReference,
  removeReference,
  replaceReference,
} from "./referenceInputs.ts";
import { toWorkflow } from "./serialize.ts";

describe("compatibleImageOutputs", () => {
  it("enumerates only exact image outputs outside the target node", () => {
    const nodes = [
      node("first", [], [["image", "image"]]),
      node("video", [], [["video", "video"]]),
      node("target", [["images", "image"]], [["preview", "image"]]),
    ];

    expect(compatibleImageOutputs(nodes, "target", "images")).toEqual([
      { nodeId: "first", outputName: "image" },
    ]);
  });
});

describe("ordered reference mutations", () => {
  const nodes = [
    node("first", [], [["image", "image"]]),
    node("second", [], [["image", "image"]]),
    node("third", [], [["image", "image"]]),
    node("video", [], [["video", "video"]]),
    node("target", [["images", "image"]], [], 2),
  ];

  it("adds compatible references with contiguous order and rejects duplicates and maximum overflow", () => {
    const first = addReference(nodes, [], target, source("first"), { color: "image" });
    const second = addReference(nodes, first, target, source("second"), { color: "image" });

    expect(second.map(summary)).toEqual([
      ["first", "workflow-edge:[\"first\",\"image\",\"target\",\"images\"]", 0],
      ["second", "workflow-edge:[\"second\",\"image\",\"target\",\"images\"]", 1],
    ]);
    expect(() => addReference(nodes, second, target, source("first"), {})).toThrow("duplicate");
    expect(() => addReference(nodes, second, target, source("third"), {})).toThrow("maximum");
    expect(() => addReference(nodes, [], target, { nodeId: "video", outputName: "video" }, {}))
      .toThrow("incompatible");
  });

  it("replaces one reference while preserving its edge identity", () => {
    const initial = [edge("stable-first", "first", 0), edge("stable-second", "second", 1)];

    const replaced = replaceReference(nodes, initial, target, 0, source("third"));

    expect(replaced.map(summary)).toEqual([
      ["third", "stable-first", 0],
      ["second", "stable-second", 1],
    ]);
  });

  it("removes and moves references while normalizing order and preserving unrelated edges", () => {
    const unrelated: Edge = {
      id: "prompt-edge", source: "prompt", sourceHandle: "text",
      target: "target", targetHandle: "prompt", data: { order: 99 },
    };
    const initial = [edge("second", "second", 8), unrelated, edge("first", "first", 3)];

    const moved = moveReference(initial, target, 1, 0);
    const removed = removeReference(moved, target, 1);

    expect(moved.filter(isReference).map(summary)).toEqual([
      ["second", "second", 0],
      ["first", "first", 1],
    ]);
    expect(removed.filter(isReference).map(summary)).toEqual([["second", "second", 0]]);
    expect(removed.find((candidate) => candidate.id === "prompt-edge")).toBe(unrelated);
  });

  it.each([1, 9])("serializes and reopens %i references in exact order", (count) => {
    const sources = Array.from({ length: count }, (_, index) =>
      node(`source-${index}`, [], [["image", "image"]]));
    const graphNodes = [...sources, node("target", [["images", "image"]], [], 16)];
    const graphEdges = sources.reduce<Edge[]>(
      (edges, sourceNode) => addReference(
        graphNodes,
        edges,
        target,
        source(sourceNode.id),
        { color: "image" },
      ),
      [],
    );

    const workflow = toWorkflow(graphNodes, graphEdges, "project-a");
    const reopened = fromWorkflow(
      workflow,
      () => {},
      { bundles: new Map(), summaries: [] },
    );

    expect(reopened.edges.map((edge) => [edge.source, edge.data?.order])).toEqual(
      sources.map((sourceNode, order) => [sourceNode.id, order]),
    );
  });
});

const target = { nodeId: "target", inputName: "images" };

function source(nodeId: string) {
  return { nodeId, outputName: "image" };
}

function edge(id: string, sourceNode: string, order: number): Edge {
  return {
    id, source: sourceNode, sourceHandle: "image",
    target: "target", targetHandle: "images", data: { order },
  };
}

function isReference(edge: Edge) {
  return edge.target === "target" && edge.targetHandle === "images";
}

function summary(edge: Edge) {
  return [edge.source, edge.id, edge.data?.order];
}

function node(
  id: string,
  inputs: [string, "image" | "video"][],
  outputs: [string, "image" | "video"][],
  maximum = 16,
): Node {
  const capability = {
    selector: null,
    ref: { id, version: "1.0" },
    type: id,
    contractVersion: "1.0",
    label: id,
    description: id,
    category: "test",
    inputs: inputs.map(([name, type]) => ({
      name, type, cardinality: { many: { minimum: 1, maximum } }, required: true,
    })),
    outputs: outputs.map(([name, type]) => ({
      name, type, cardinality: "one" as const, required: false,
    })),
    params: [],
    status: { availability: "available", reason: null, provider_health: null, status_revision: 1 },
    contract: null,
    presentation: null,
    contextualCreationRoute: null,
  } satisfies NodeTypeSpec;
  return {
    id, type: "workflow", position: { x: 0, y: 0 },
    data: { type: id, params: {}, capability, onParamChange: () => {} } satisfies FlowNodeData,
  };
}
