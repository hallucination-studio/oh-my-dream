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
