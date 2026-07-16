import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../api/index.ts";
import type { AssetDto } from "../api/types.ts";
import { useAssets } from "./useAssets.ts";

afterEach(() => vi.restoreAllMocks());

it("lists one Project and issues signed previews only for Available Assets", async () => {
  vi.spyOn(api, "assetList").mockResolvedValue({
    assets: [asset("available"), asset("pending")],
    next_cursor: null,
  });
  const preview = vi.spyOn(api, "assetIssuePreview").mockResolvedValue({
    asset_id: "asset-available",
    preview_uri: "desktop-asset://v1/signed",
    expires_at_epoch_ms: "300001",
  });
  const view = renderHook(() => useAssets("project"));

  await waitFor(() => expect(view.result.current.assets).toHaveLength(2));

  expect(preview).toHaveBeenCalledOnce();
  expect(view.result.current.assets[0]?.previewUrl).toBe("desktop-asset://v1/signed");
  expect(view.result.current.assets[1]?.previewUrl).toBeNull();
});

it("opens the canonical import then refreshes the current Project list", async () => {
  const list = vi.spyOn(api, "assetList").mockResolvedValue({ assets: [], next_cursor: null });
  vi.spyOn(api, "assetImport").mockResolvedValue(asset("available"));
  const view = renderHook(() => useAssets("project"));
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1));

  await act(() => view.result.current.importAsset("image"));

  expect(api.assetImport).toHaveBeenCalledWith("project", "image");
  expect(list).toHaveBeenCalledTimes(2);
});

function asset(contentState: AssetDto["content_state"]): AssetDto {
  return {
    asset_id: `asset-${contentState}`,
    project_id: "project",
    media_kind: "image",
    content_state: contentState,
    display_name: contentState,
    created_at_epoch_ms: "1",
    content: {
      content_fingerprint_hex: "00".repeat(32),
      byte_length: "1",
      mime_type: "image/png",
    },
    media_facts: { kind: "image", width: 1, height: 1 },
    origin: { kind: "imported", original_file_name: "image.png" },
  };
}
