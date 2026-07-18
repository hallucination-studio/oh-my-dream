import { expect, it } from "vitest";
import {
  mockAssetGet,
  mockAssetImport,
  mockAssetIssuePreview,
  mockAssetList,
  mockAssetPublish,
  mockPreviewFixture,
} from "./mockAssets.ts";

const PROJECT = "10000000-0000-4000-8000-0000000000a1";

it("imports, lists in insertion order, gets, and issues labelled previews", async () => {
  const image = await mockAssetImport(PROJECT, "image");
  const video = await mockAssetImport(PROJECT, "video");
  expect(image?.content_state).toBe("available");
  expect(video?.display_name).toBe("Imported video sample");

  const listed = await mockAssetList(PROJECT, null, null, 100);
  expect(listed.assets.map((asset) => asset.asset_id)).toEqual([
    image?.asset_id,
    video?.asset_id,
  ]);
  expect(listed.next_cursor).toBeNull();

  const fetched = await mockAssetGet(PROJECT, image!.asset_id);
  expect(fetched.media_kind).toBe("image");

  const preview = await mockAssetIssuePreview(PROJECT, video!.asset_id);
  expect(preview.preview_uri.startsWith("data:image/svg+xml")).toBe(true);
  expect(decodeURIComponent(preview.preview_uri)).toContain("DETERMINISTIC SAMPLE");
  expect(decodeURIComponent(preview.preview_uri)).toContain("mock video");
});

it("filters by kind, paginates by cursor, and rejects unknown assets", async () => {
  const project = "10000000-0000-4000-8000-0000000000b2";
  mockAssetPublish(project, "image", "First", { kind: "user_import" });
  mockAssetPublish(project, "image", "Second", { kind: "user_import" });
  mockAssetPublish(project, "audio", "Third", { kind: "user_import" });

  const images = await mockAssetList(project, "image", null, 1);
  expect(images.assets.map((asset) => asset.display_name)).toEqual(["First"]);
  const rest = await mockAssetList(project, "image", images.next_cursor, 10);
  expect(rest.assets.map((asset) => asset.display_name)).toEqual(["Second"]);

  await expect(mockAssetGet(project, "missing")).rejects.toThrow("asset.not_found");
});

it("keeps preview fixtures visibly labelled for every kind", () => {
  for (const kind of ["image", "video", "audio"] as const) {
    const fixture = decodeURIComponent(mockPreviewFixture(kind, "label"));
    expect(fixture).toContain("DETERMINISTIC SAMPLE");
    expect(fixture).toContain(`mock ${kind}`);
  }
});
