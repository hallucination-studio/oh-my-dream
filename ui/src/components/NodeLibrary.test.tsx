import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import contracts from "../__fixtures__/node_capabilities.json";
import { nodeSpecFromExactContract } from "../nodes/exactCapability.ts";
import type { NodeCapabilityContractDto } from "../api/types.ts";
import { NodeLibrary } from "./NodeLibrary.tsx";

it("renders the exact capability list without legacy search or bundles", () => {
  const exact = contracts as NodeCapabilityContractDto[];
  const onOpenAssets = vi.fn();
  render(
    <NodeLibrary
      contracts={exact}
      loadedSpecs={exact.map(nodeSpecFromExactContract)}
      onAdd={vi.fn()}
      onOpenAssets={onOpenAssets}
    />,
  );

  expect(screen.getByRole("button", { name: "Text" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Text to Image" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Image Asset" })).toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "Search nodes" }), {
    target: { value: "asset" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Use an existing asset" }));
  expect(onOpenAssets).toHaveBeenCalledOnce();
});
