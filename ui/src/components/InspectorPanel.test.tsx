import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import catalogFixture from "../__fixtures__/capability_catalog.json";
import type { CapabilityCatalog } from "../api/types.ts";
import { nodeSpecFromBundle } from "../nodes/catalog.ts";
import { InspectorPanel } from "./InspectorPanel.tsx";

const catalog = catalogFixture as unknown as CapabilityCatalog;

it("shows product Asset controls without exposing contextual params", () => {
  const entry = catalog.capabilities.find(
    (candidate) => candidate.contract.reference.id === "ImageAssetSource",
  );
  if (!entry) throw new Error("missing ImageAssetSource fixture");
  const onOpenAssets = vi.fn();

  render(
    <InspectorPanel
      node={{
        id: "image-asset",
        type: "ImageAssetSource",
        params: { mode: "asset", asset_id: "asset-image-1" },
        capability: nodeSpecFromBundle({
          selector: entry.selector,
          reference: entry.contract.reference,
          contract: entry.contract,
          presentation: entry.presentation,
          status: entry.status,
        }),
        assetPresentation: { title: "Mountain study", available: true },
      }}
      onParamChange={() => {}}
      onOpenAssets={onOpenAssets}
    />,
  );

  expect(screen.getByText("Mountain study")).toBeTruthy();
  expect(screen.queryByLabelText(/mode|asset id/i)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Open in Assets" }));
  expect(onOpenAssets).toHaveBeenCalledOnce();
});

it("deletes the selected node and shows a connection panel for a selected edge", () => {
  const onDeleteNode = vi.fn();
  const onDeleteEdge = vi.fn();
  const entry = catalog.capabilities.find(
    (candidate) => candidate.contract.reference.id === "ImageAssetSource",
  );
  if (!entry) throw new Error("missing ImageAssetSource fixture");
  const node = {
    id: "image-asset",
    type: "ImageAssetSource",
    params: {},
    capability: nodeSpecFromBundle({
      selector: entry.selector,
      reference: entry.contract.reference,
      contract: entry.contract,
      presentation: entry.presentation,
      status: entry.status,
    }),
    assetPresentation: { title: "Mountain study", available: true },
  };

  const view = render(
    <InspectorPanel node={node} onParamChange={() => {}} onDeleteNode={onDeleteNode} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Delete node" }));
  expect(onDeleteNode).toHaveBeenCalledWith("image-asset");

  view.rerender(
    <InspectorPanel
      node={null}
      onParamChange={() => {}}
      selectedEdge={{ id: "edge-1", sourceLabel: "Text", targetLabel: "Generate image" }}
      onDeleteEdge={onDeleteEdge}
    />,
  );
  expect(screen.getByText("Text → Generate image")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Delete connection" }));
  expect(onDeleteEdge).toHaveBeenCalledWith("edge-1");
});
