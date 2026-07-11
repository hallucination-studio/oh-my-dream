import type { Connection, Edge } from "@xyflow/react";
import { describe, expect, it, vi } from "vitest";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { fromWorkflow, nextNodeId, upsertIncomingEdge } from "./editor.ts";
import type { Workflow } from "./types.ts";

describe("fromWorkflow", () => {
  it("hydrates React Flow nodes, positions, params, and named-port edges", () => {
    const onParamChange = vi.fn();
    const workflow: Workflow = {
      version: "1.0",
      project_id: "project-a",
      nodes: [
        {
          id: "prompt",
          type: "TextPrompt",
          params: { text: "loaded prompt" },
          inputs: {},
          position: [25, 50],
        },
        {
          id: "image",
          type: "TextToImage",
          params: { model: "mock-image", steps: 32 },
          inputs: { prompt: ["prompt", "text"] },
          position: [250, 50],
        },
      ],
    };

    const graph = fromWorkflow(workflow, onParamChange);

    expect(graph.nodes.map(({ id, position }) => ({ id, position }))).toEqual([
      { id: "prompt", position: { x: 25, y: 50 } },
      { id: "image", position: { x: 250, y: 50 } },
    ]);
    expect((graph.nodes[1].data as FlowNodeData).params).toEqual({
      model: "mock-image",
      steps: 32,
    });
    expect(graph.edges).toEqual([
      expect.objectContaining({
        source: "prompt",
        sourceHandle: "text",
        target: "image",
        targetHandle: "prompt",
        type: "workflow",
      }),
    ]);

    (graph.nodes[1].data as FlowNodeData).onParamChange("steps", 36);
    expect(onParamChange).toHaveBeenCalledWith("image", "steps", 36);
  });
});

describe("upsertIncomingEdge", () => {
  it("replaces the existing edge for the same target input", () => {
    const existing: Edge[] = [edge("first", "prompt-a", "text", "image", "prompt")];
    const replacement: Connection = {
      source: "prompt-b",
      sourceHandle: "text",
      target: "image",
      targetHandle: "prompt",
    };

    const result = upsertIncomingEdge(existing, replacement, { color: "red" });

    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({
      source: "prompt-b",
      target: "image",
      targetHandle: "prompt",
      type: "workflow",
      data: { color: "red" },
    });
  });
});

describe("nextNodeId", () => {
  it("does not collide with ids restored from a workspace", () => {
    const nodes = [
      { id: "n1", position: { x: 0, y: 0 }, data: {} },
      { id: "n7", position: { x: 0, y: 0 }, data: {} },
      { id: "custom", position: { x: 0, y: 0 }, data: {} },
    ];

    expect(nextNodeId(nodes)).toBe("n8");
  });
});

function edge(
  id: string,
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): Edge {
  return { id, source, sourceHandle, target, targetHandle };
}
