import type { Edge, Node } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import type { NodeTypeSpec } from "../nodes/catalog.ts";
import { toWorkflow } from "./serialize.ts";

describe("toWorkflow", () => {
  it("serializes named input ports from React Flow edges", () => {
    const nodes: Node[] = [
      flowNode("prompt", "TextPrompt", { text: "a red fox" }, [10, 20]),
      flowNode("image", "TextToImage", { model: "mock-image" }, [30, 40]),
      flowNode("video", "ImageToVideo", { model: "mock-video" }, [50, 60]),
    ];
    const edges: Edge[] = [
      edge("prompt-to-image", "prompt", "text", "image", "prompt"),
      edge("image-to-video", "image", "image", "video", "image"),
    ];

    const workflow = toWorkflow(nodes, edges, "project-1");

    expect(workflow).toEqual({
      version: "1.0",
      project_id: "project-1",
      nodes: [
        {
          id: "prompt",
          type: "TextPrompt",
          contract_version: "1.0",
          params: { text: "a red fox" },
          inputs: {},
          position: [10, 20],
        },
        {
          id: "image",
          type: "TextToImage",
          contract_version: "1.0",
          params: { model: "mock-image" },
          inputs: { prompt: ["prompt", "text"] },
          position: [30, 40],
        },
        {
          id: "video",
          type: "ImageToVideo",
          contract_version: "1.0",
          params: { model: "mock-video" },
          inputs: { image: ["image", "image"] },
          position: [50, 60],
        },
      ],
    });
  });

  it("rejects multiple edges targeting the same input", () => {
    const nodes: Node[] = [
      flowNode("prompt-a", "TextPrompt", { text: "first" }, [0, 0]),
      flowNode("prompt-b", "TextPrompt", { text: "second" }, [0, 100]),
      flowNode("image", "TextToImage", {}, [200, 0]),
    ];
    const edges: Edge[] = [
      edge("a-to-image", "prompt-a", "text", "image", "prompt"),
      edge("b-to-image", "prompt-b", "text", "image", "prompt"),
    ];

    expect(() => toWorkflow(nodes, edges, "project-1")).toThrow(
      "multiple edges target `image.prompt`",
    );
  });

  it("serializes many inputs as explicit ordered bindings", () => {
    const nodes: Node[] = [
      flowNode("first", "ImageToVideo", {}, [0, 0]),
      flowNode("second", "ImageToVideo", {}, [0, 100]),
      flowNode("concat", "VideoConcat", {}, [200, 0], ["clips"]),
    ];
    const edges: Edge[] = [
      edge("second", "second", "video", "concat", "clips", 1),
      edge("first", "first", "video", "concat", "clips", 0),
    ];

    expect(toWorkflow(nodes, edges, "project-1").nodes[2]?.inputs).toEqual({
      clips: {
        kind: "ordered_many",
        sources: [["first", "video"], ["second", "video"]],
      },
    });
  });

  it("does not invent a binding for an unconnected many input", () => {
    const nodes: Node[] = [flowNode("concat", "VideoConcat", {}, [0, 0], ["clips"] )];

    expect(toWorkflow(nodes, [], "project-1").nodes[0]?.inputs).toEqual({});
  });
});

function flowNode(
  id: string,
  type: string,
  params: Record<string, unknown>,
  position: [number, number],
  manyInputs: string[] = [],
): Node {
  return {
    id,
    type: "workflow",
    position: { x: position[0], y: position[1] },
    data: {
      type,
      params,
      onParamChange: () => {},
      capability: {
        selector: null,
        ref: { id: type, version: "1.0" },
        type,
        contractVersion: "1.0",
        label: type,
        description: type,
        category: "test",
        inputs: manyInputs.map((name) => ({ name, type: "video", cardinality: { many: { minimum: 2, maximum: null } }, required: true })),
        outputs: [],
        params: [],
        status: { availability: "available", reason: null, provider_health: null, status_revision: 0 },
        contract: null,
        presentation: null,
        contextualCreationRoute: null,
      } as NodeTypeSpec,
    } satisfies FlowNodeData,
  };
}

function edge(
  id: string,
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
  order?: number,
): Edge {
  return { id, source, sourceHandle, target, targetHandle, data: order === undefined ? undefined : { order } };
}
