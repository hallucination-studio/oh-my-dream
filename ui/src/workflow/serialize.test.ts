import type { Edge, Node } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
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

    const workflow = toWorkflow(nodes, edges);

    expect(workflow).toEqual({
      version: "1.0",
      project_id: "default",
      nodes: [
        {
          id: "prompt",
          type: "TextPrompt",
          params: { text: "a red fox" },
          inputs: {},
          position: [10, 20],
        },
        {
          id: "image",
          type: "TextToImage",
          params: { model: "mock-image" },
          inputs: { prompt: ["prompt", "text"] },
          position: [30, 40],
        },
        {
          id: "video",
          type: "ImageToVideo",
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

    expect(() => toWorkflow(nodes, edges)).toThrow(
      "multiple edges target `image.prompt`",
    );
  });
});

function flowNode(
  id: string,
  type: string,
  params: Record<string, unknown>,
  position: [number, number],
): Node {
  return {
    id,
    type: "workflow",
    position: { x: position[0], y: position[1] },
    data: { type, params, onParamChange: () => {} } satisfies FlowNodeData,
  };
}

function edge(
  id: string,
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): Edge {
  return { id, source, sourceHandle, target, targetHandle };
}
