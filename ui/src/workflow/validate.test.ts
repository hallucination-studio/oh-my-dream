import type { Connection, Node } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";
import { isValidConnection } from "./validate.ts";

describe("isValidConnection", () => {
  it("accepts matching port types and rejects mismatches", () => {
    const nodes = [
      flowNode("prompt", "TextPrompt"),
      flowNode("image", "TextToImage"),
      flowNode("video", "ImageToVideo"),
    ];

    expect(isValidConnection(connection("prompt", "text", "image", "prompt"), nodes)).toBe(true);
    expect(isValidConnection(connection("image", "image", "video", "image"), nodes)).toBe(true);
    expect(isValidConnection(connection("image", "image", "image", "prompt"), nodes)).toBe(false);
  });
});

function flowNode(id: string, type: string): Node {
  return {
    id,
    type: "workflow",
    position: { x: 0, y: 0 },
    data: { type, params: {}, onParamChange: () => {} } satisfies FlowNodeData,
  };
}

function connection(
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): Connection {
  return { source, sourceHandle, target, targetHandle };
}
