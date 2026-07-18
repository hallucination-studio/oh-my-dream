import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AssetCard } from "./AssetCard.tsx";
import type { AssetViewModel } from "./model.ts";

const asset: AssetViewModel = {
  id: "asset-1",
  kind: "video",
  contentState: "available",
  previewUrl: "desktop-asset://v1/preview",
  displayName: "A red fox",
  projectId: "project-1",
  sourceNodeId: "node-1",
  sourceNodeType: "Workflow node",
  mimeType: "video/mp4",
  byteLength: "100",
  facts: null,
  createdAtEpochMs: "0",
};

describe("AssetCard", () => {
  it("renders the kind badge and exact video preview element", () => {
    render(
      <AssetCard asset={asset} selected={false} onSelect={() => {}} onJump={() => {}} />,
    );
    expect(screen.getByText("video")).toBeTruthy();
    const poster = screen.getByRole("img", { name: "A red fox" });
    expect(poster.tagName).toBe("IMG");
    expect(poster.getAttribute("src")).toBe("desktop-asset://v1/preview");
  });

  it("shows jump only when an origin node exists and no redundant project meta", () => {
    const onJump = vi.fn();
    const view = render(
      <AssetCard asset={asset} selected={false} onSelect={() => {}} onJump={onJump} />,
    );
    expect(screen.queryByText("Current project")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Jump to source node" }));
    expect(onJump).toHaveBeenCalledOnce();

    view.rerender(
      <AssetCard
        asset={{ ...asset, sourceNodeId: null, sourceNodeType: null }}
        selected={false}
        onSelect={() => {}}
        onJump={onJump}
      />,
    );
    expect(screen.queryByRole("button", { name: "Jump to source node" })).toBeNull();
  });
});
