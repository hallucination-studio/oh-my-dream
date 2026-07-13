import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AssetCard } from "./AssetCard.tsx";
import type { AssetViewModel } from "./model.ts";

const asset: AssetViewModel = {
  id: "asset-1",
  kind: "video",
  fileUrl: "/tmp/a.mp4",
  thumbnailUrl: "/tmp/a.png",
  prompt: "a red fox",
  projectName: "Default",
  sourceNodeType: "ImageToVideo",
  model: "mock-video",
  seed: null,
  cost: 900,
  createdAt: 0,
};

describe("AssetCard", () => {
  it("renders the kind badge and preview image", () => {
    render(
      <AssetCard asset={asset} selected={false} onSelect={() => {}} onJump={() => {}} />,
    );
    expect(screen.getByText("video")).toBeTruthy();
    const img = screen.getByRole("img") as HTMLImageElement;
    expect(img.getAttribute("src")).toBe("/tmp/a.png");
  });
});
