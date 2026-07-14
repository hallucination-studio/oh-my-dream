import { render, screen } from "@testing-library/react";
import { ReactFlowProvider, type NodeProps } from "@xyflow/react";
import { describe, expect, it, vi } from "vitest";
import catalogFixture from "../__fixtures__/capability_catalog.json";
import type { CapabilityCatalog } from "../api/types.ts";
import { nodeSpecFromBundle, recoveryNodeSpec } from "./catalog.ts";
import { WorkflowFlowNode, type FlowNodeData } from "./WorkflowFlowNode.tsx";

const catalog = catalogFixture as unknown as CapabilityCatalog;

describe("WorkflowFlowNode", () => {
  it("renders handles from the exact contract projection", () => {
    const entry = catalog.capabilities.find((candidate) => candidate.contract.reference.id === "VideoConcat");
    if (!entry) throw new Error("missing VideoConcat fixture");

    renderNode({
      type: "VideoConcat",
      contractVersion: "1.0",
      capability: nodeSpecFromBundle({
        selector: entry.selector,
        reference: entry.contract.reference,
        contract: entry.contract,
        presentation: entry.presentation,
        status: entry.status,
      }),
      params: {},
    });

    expect(screen.getByText("Video Concat")).toBeTruthy();
    expect(document.querySelectorAll(".wf-port")).toHaveLength(2);
  });

  it("keeps a missing exact version visible as a recovery placeholder", () => {
    renderNode({
      type: "TextPrompt",
      contractVersion: "9.9",
      capability: recoveryNodeSpec({ id: "TextPrompt", version: "9.9" }, "migrate or remove this node"),
      params: {},
    });

    expect(screen.getByText("Unavailable TextPrompt")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("migrate or remove this node");
    expect(document.querySelectorAll(".wf-port")).toHaveLength(0);
  });
});

function renderNode(data: Omit<FlowNodeData, "onParamChange">): void {
  const props = {
    id: "node",
    selected: false,
    data: { ...data, onParamChange: vi.fn() },
  } as unknown as NodeProps;
  render(
    <ReactFlowProvider>
      <WorkflowFlowNode {...props} />
    </ReactFlowProvider>,
  );
}
