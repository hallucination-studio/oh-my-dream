import { render, screen } from "@testing-library/react";
import { ReactFlowProvider, type NodeProps } from "@xyflow/react";
import { describe, expect, it, vi } from "vitest";
import catalogFixture from "../__fixtures__/capability_catalog.json";
import type { CapabilityCatalog } from "../api/types.ts";
import { nodeSpecFromBundle, recoveryNodeSpec } from "./catalog.ts";
import { nodeSpecFromExactContract } from "./exactCapability.ts";
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

    expect(screen.getByText("Video")).toBeTruthy();
    expect(screen.getByText(/concat/)).toBeTruthy();
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

  it("presents a contextual source as the selected Asset instead of its internal capability", () => {
    const entry = catalog.capabilities.find(
      (candidate) => candidate.contract.reference.id === "AudioAssetSource",
    );
    if (!entry) throw new Error("missing AudioAssetSource fixture");

    renderNode({
      type: "AudioAssetSource",
      contractVersion: "1.0",
      capability: nodeSpecFromBundle({
        selector: entry.selector,
        reference: entry.contract.reference,
        contract: entry.contract,
        presentation: entry.presentation,
        status: entry.status,
      }),
      params: { mode: "asset", asset_id: "asset-audio-1" },
      assetPresentation: { title: "Evening rain", available: true },
    });

    expect(screen.getByText("Audio Asset")).toBeTruthy();
    expect(screen.getByText("Evening rain")).toBeTruthy();
    expect(screen.queryByText(/AudioAssetSource/)).toBeNull();
  });

  it("maps execution states to creator-language pills", () => {
    const entry = catalog.capabilities.find((candidate) => candidate.contract.reference.id === "VideoConcat");
    if (!entry) throw new Error("missing VideoConcat fixture");
    const capability = nodeSpecFromBundle({
      selector: entry.selector,
      reference: entry.contract.reference,
      contract: entry.contract,
      presentation: entry.presentation,
      status: entry.status,
    });

    const cases: Array<[FlowNodeData["runtime"], string]> = [
      [undefined, "Not run"],
      [{ state: "running", progress: 0.4 }, "Running"],
      [{ state: "done" }, "Complete"],
      [{ state: "cached" }, "Complete"],
      [{ state: "error" }, "Needs attention"],
    ];
    for (const [runtime, label] of cases) {
      const props = {
        id: "node",
        selected: false,
        data: { type: "VideoConcat", contractVersion: "1.0", capability, params: {}, runtime, onParamChange: () => {} },
      } as unknown as NodeProps;
      const view = render(
        <ReactFlowProvider>
          <WorkflowFlowNode {...props} />
        </ReactFlowProvider>,
      );
      expect(screen.getByText(label)).toBeTruthy();
      view.unmount();
    }
  });
  it("labels every port with its name and media type", () => {
    renderNode({
      type: "video.generate_from_image",
      contractVersion: "1.0",
      capability: nodeSpecFromExactContract({
        capability_ref: { id: "video.generate_from_image", version: "1.0" },
        parameters: [{
          key: "generation_profile_ref",
          constraint: { kind: "generation_profile_ref" },
          presence: { kind: "required" },
        }],
        inputs: [
          { key: "image", binding: { kind: "required_single_value", data_type: "image" } },
          { key: "prompt", binding: { kind: "optional_single_value", data_type: "text" } },
        ],
        outputs: [{ key: "video", data_type: "video", is_primary: true }],
        execution_kind: "content_generation",
      }),
      params: {},
    });

    const rows = document.querySelectorAll(".wf-port-row");
    expect(rows).toHaveLength(3);
    expect(screen.getAllByText("image").length).toBeGreaterThan(0);
    expect(screen.getByText("prompt")).toBeTruthy();
    expect(screen.getAllByText("video").length).toBeGreaterThan(0);
    expect(screen.getByText("text")).toBeTruthy();
    expect(document.querySelectorAll(".wf-port")).toHaveLength(3);
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
