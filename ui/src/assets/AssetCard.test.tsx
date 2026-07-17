import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
  createdAtEpochMs: "0",
};

describe("AssetCard", () => {
  it("renders the kind badge and exact video preview element", () => {
    render(
      <AssetCard asset={asset} selected={false} onSelect={() => {}} onJump={() => {}} />,
    );
    expect(screen.getByText("video")).toBeTruthy();
    const video = screen.getByLabelText("A red fox") as HTMLVideoElement;
    expect(video.tagName).toBe("VIDEO");
    expect(video.getAttribute("src")).toBe("desktop-asset://v1/preview");
  });
});
