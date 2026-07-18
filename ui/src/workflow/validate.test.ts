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

  it("rejects self-loops and edges that would close a cycle", () => {
    const nodes = [
      flowNode("prompt", "TextPrompt"),
      flowNode("image", "TextToImage"),
      flowNode("video", "ImageToVideo"),
    ];
    const edges = [
      { id: "e1", source: "prompt", target: "image" },
      { id: "e2", source: "image", target: "video" },
    ];

    // Self-loop: rejected before any type check.
    expect(isValidConnection(connection("prompt", "text", "prompt", "text"), nodes, edges)).toBe(false);
    // video can already be reached from prompt, so wiring video back closes a cycle.
    expect(isValidConnection(connection("video", "video", "prompt", "text"), nodes, edges)).toBe(false);
    // A fresh downstream edge into the chain stays legal.
    expect(isValidConnection(connection("image", "image", "video", "image"), nodes, edges)).toBe(true);
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
