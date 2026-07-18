import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AssetLibrary } from "./AssetLibrary.tsx";
import type { AssetViewModel } from "./model.ts";

const image: AssetViewModel = {
  id: "asset-1",
  kind: "image",
  contentState: "available",
  previewUrl: null,
  displayName: "a sunset city",
  projectId: "project-1",
  sourceNodeId: "node-1",
  sourceNodeType: "Workflow node",
  mimeType: "image/png",
  byteLength: "100",
  facts: "1024×1024",
  createdAtEpochMs: "0",
};

const video: AssetViewModel = {
  ...image,
  id: "asset-2",
  kind: "video",
  displayName: "city in motion",
  facts: "4.0s · 24fps",
};

function renderLibrary(overrides: Partial<Parameters<typeof AssetLibrary>[0]> = {}) {
  return render(
    <AssetLibrary
      assets={[image, video]}
      error={null}
      onAddToCanvas={vi.fn()}
      onJumpToNode={vi.fn()}
      onImport={vi.fn()}
      {...overrides}
    />,
  );
}

describe("AssetLibrary", () => {
  it("shows per-kind counts and switches between grid and list", () => {
    renderLibrary();
    expect(screen.getByRole("button", { name: "All (2)" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Image (1)" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Video (1)" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Switch to list view" }));
    expect(screen.getByText("1024×1024")).toBeTruthy();
    expect(screen.getByText("4.0s · 24fps")).toBeTruthy();
  });

  it("guides each empty context honestly", () => {
    const view = renderLibrary({ hasProject: false, assets: [] });
    expect(screen.getByText("Open a Project to see its Assets.")).toBeTruthy();

    view.rerender(
      <AssetLibrary
        assets={[]}
        error={null}
        hasProject
        onAddToCanvas={vi.fn()}
        onJumpToNode={vi.fn()}
        onImport={vi.fn()}
      />,
    );
    expect(screen.getByText("Run a workflow or import media to fill your library.")).toBeTruthy();
  });

  it("renders bounded skeletons while loading", () => {
    renderLibrary({ loading: true, assets: [] });
    expect(screen.getByRole("status")).toBeTruthy();
    expect(document.querySelectorAll(".lib__skel").length).toBeGreaterThan(0);
  });
});
