import type { Connection, Node } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import catalogFixture from "../__fixtures__/capability_catalog.json";
import type { CapabilityCatalog } from "../api/types.ts";
import { nodeSpecFromBundle } from "../nodes/catalog.ts";
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
    expect(isValidConnection(connection("prompt", "text", "video", "image"), nodes)).toBe(false);
  });
});

function flowNode(id: string, type: string): Node {
  return {
    id,
    type: "workflow",
    position: { x: 0, y: 0 },
    data: {
      type,
      params: {},
      capability: nodeSpec(type),
      onParamChange: () => {},
    } satisfies FlowNodeData,
  };
}

function nodeSpec(type: string) {
  const catalog = catalogFixture as unknown as CapabilityCatalog;
  const entry = catalog.capabilities.find((candidate) => candidate.contract.reference.id === type);
  if (!entry) throw new Error(`missing fixture capability ${type}`);
  return nodeSpecFromBundle({
    selector: entry.selector,
    reference: entry.contract.reference,
    contract: entry.contract,
    presentation: entry.presentation,
    status: entry.status,
  });
}

function connection(
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): Connection {
  return { source, sourceHandle, target, targetHandle };
}
