import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import contracts from "../__fixtures__/node_capabilities.json";
import { capabilityKey, nodeSpecFromExactContract } from "../nodes/exactCapability.ts";
import type { NodeCapabilityContractDto } from "../api/types.ts";
import { NodeLibrary } from "./NodeLibrary.tsx";

it("renders the exact capability list in creator language and stable group order", () => {
  const exact = contracts as NodeCapabilityContractDto[];
  const onOpenAssets = vi.fn();
  render(
    <NodeLibrary
      contracts={exact}
      loadedSpecs={exact.map((contract) => nodeSpecFromExactContract(contract))}
      onAdd={vi.fn()}
      onOpenAssets={onOpenAssets}
    />,
  );

  expect(screen.getByRole("button", { name: "Text" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Generate image" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Create video" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Image asset" })).toBeTruthy();
  const categories = screen
    .getAllByRole("button")
    .map((button) => button.textContent ?? "")
    .filter((text) => /^(Inputs|Generate|Assets)\s*\d+$/.test(text));
  expect(categories[0]).toMatch(/^Inputs/);
  expect(categories[1]).toMatch(/^Generate/);
  expect(categories[2]).toMatch(/^Assets/);
});

it("matches creator-language search aliases", () => {
  const exact = contracts as NodeCapabilityContractDto[];
  render(
    <NodeLibrary
      contracts={exact}
      loadedSpecs={exact.map((contract) => nodeSpecFromExactContract(contract))}
      onAdd={vi.fn()}
      onOpenAssets={vi.fn()}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "Search nodes" }), {
    target: { value: "t2i" },
  });
  expect(screen.getByRole("button", { name: "Generate image" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Create video" })).toBeNull();
});

it("renders bounded skeletons while contracts load", () => {
  render(
    <NodeLibrary
      contracts={[]}
      loadedSpecs={[]}
      loading
      onAdd={vi.fn()}
      onOpenAssets={vi.fn()}
    />,
  );
  expect(screen.getByRole("status", { name: "Loading node types" })).toBeTruthy();
  expect(document.querySelectorAll(".nlib__skel")).toHaveLength(6);
});

it("routes asset searches to the asset library", () => {
  const exact = contracts as NodeCapabilityContractDto[];
  const onOpenAssets = vi.fn();
  render(
    <NodeLibrary
      contracts={exact}
      loadedSpecs={exact.map((contract) => nodeSpecFromExactContract(contract))}
      onAdd={vi.fn()}
      onOpenAssets={onOpenAssets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "Search nodes" }), {
    target: { value: "asset" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Use an existing asset" }));
  expect(onOpenAssets).toHaveBeenCalledOnce();
});

it("disables unavailable model nodes, hides empty compatibility, and keeps saved nodes", () => {
  const image = (contracts as NodeCapabilityContractDto[]).find(
    (contract) => contract.capability_ref.id === "image.generate_from_text",
  )!;
  const unavailable = nodeSpecFromExactContract(image, {
    availability: "unavailable",
    reason: "authentication_required",
    provider_health: null,
    status_revision: 1,
  });
  const key = capabilityKey(image.capability_ref);
  const view = render(
    <NodeLibrary
      contracts={[image]}
      loadedSpecs={[unavailable]}
      onAdd={vi.fn()}
      onOpenAssets={vi.fn()}
    />,
  );

  const disabled = screen.getByRole("button", { name: "Generate image — authentication_required" });
  expect((disabled as HTMLButtonElement).disabled).toBe(true);

  view.rerender(
    <NodeLibrary
      contracts={[image]}
      loadedSpecs={[unavailable]}
      hiddenCapabilityKeys={new Set([key])}
      onAdd={vi.fn()}
      onOpenAssets={vi.fn()}
    />,
  );
  expect(screen.queryByRole("button", { name: /Generate image/ })).toBeNull();

  view.rerender(
    <NodeLibrary
      contracts={[image]}
      loadedSpecs={[unavailable]}
      hiddenCapabilityKeys={new Set([key])}
      savedCapabilityKeys={new Set([key])}
      onAdd={vi.fn()}
      onOpenAssets={vi.fn()}
    />,
  );
  expect(
    (screen.getByRole("button", { name: "Generate image — authentication_required" }) as HTMLButtonElement).disabled,
  ).toBe(true);
});
