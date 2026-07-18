import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { AssetMediaPreview } from "./AssetMediaPreview.tsx";
import type { AssetViewModel } from "./model.ts";

it.each([
  ["image", "IMG"],
  ["video", "IMG"],
  ["audio", "AUDIO"],
] as const)("renders a signed %s URI through the matching media element", (kind, tagName) => {
  render(
    <AssetMediaPreview
      asset={{ ...asset, kind, displayName: kind }}
      className="preview"
    />,
  );
  const media = kind === "audio"
    ? screen.getByLabelText(kind)
    : screen.getByRole("img", { name: kind });
  expect(media.tagName).toBe(tagName);
  expect(media.getAttribute("src")).toBe("desktop-asset://v1/signed");
});

const asset: AssetViewModel = {
  id: "asset",
  kind: "image",
  contentState: "available",
  previewUrl: "desktop-asset://v1/signed",
  displayName: "image",
  projectId: "project",
  sourceNodeId: null,
  sourceNodeType: null,
  mimeType: "image/png",
  byteLength: "1",
  facts: null,
  createdAtEpochMs: "1",
};
