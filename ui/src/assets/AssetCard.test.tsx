import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AssetCard } from "./AssetCard.tsx";
import type { Asset } from "../api/index.ts";

const asset: Asset = {
  id: "asset-1",
  kind: "video",
  file_path: "/tmp/a.mp4",
  thumbnail_path: "/tmp/a.png",
  workflow_snapshot: {},
  prompt: "a red fox",
  project_id: "project-1",
  project_name: "Default",
  source_node_id: "video",
  source_node_type: "ImageToVideo",
  model: "mock-video",
  seed: null,
  cost: 900,
  tags: [],
  created_at: 0,
};

describe("AssetCard", () => {
  it("renders the kind badge and preview image", () => {
    render(<AssetCard asset={asset} />);
    expect(screen.getByText("video")).toBeTruthy();
    const img = screen.getByRole("img") as HTMLImageElement;
    expect(img.getAttribute("src")).toBe("/tmp/a.png");
  });
});
