import { describe, expect, it } from "vitest";
import type { AssetDto } from "../api/index.ts";
import { assetFromDto } from "./model.ts";

describe("assetFromDto", () => {
  it("mechanically translates the API boundary shape", () => {
    const dto: AssetDto = {
      asset_id: "asset-1",
      project_id: "project-1",
      media_kind: "image",
      content_state: "available",
      display_name: "Sunrise",
      created_at_epoch_ms: "123",
      content: {
        content_fingerprint_hex: "00".repeat(32),
        byte_length: "250",
        mime_type: "image/png",
      },
      media_facts: { kind: "image", width: 1, height: 1 },
      origin: {
        kind: "workflow_node_output",
        workflow_node_id: "node-1",
      },
    };

    expect(assetFromDto(dto, {
      asset_id: "asset-1",
      preview_uri: "desktop-asset://v1/preview",
      expires_at_epoch_ms: "300123",
    })).toEqual({
      id: "asset-1",
      kind: "image",
      contentState: "available",
      previewUrl: "desktop-asset://v1/preview",
      displayName: "Sunrise",
      projectId: "project-1",
      sourceNodeId: "node-1",
      sourceNodeType: "Workflow node",
      mimeType: "image/png",
      byteLength: "250",
      facts: "1×1",
      createdAtEpochMs: "123",
    });
  });

  it.each([
    [{ kind: "image", width: 1024, height: 768 }, "1024×768"],
    [{ kind: "video", duration_seconds: 4, fps: 24 }, "4.0s · 24fps"],
    [{ kind: "audio", duration_seconds: 6 }, "6.0s"],
    [{ kind: "image" }, null],
    [null, null],
  ])("labels media facts from %s", (mediaFacts, expected) => {
    const dto: AssetDto = {
      asset_id: "asset-facts",
      project_id: "project-1",
      media_kind: "image",
      content_state: "available",
      display_name: "Facts",
      created_at_epoch_ms: "1",
      content: { content_fingerprint_hex: "0", byte_length: "1", mime_type: "image/png" },
      media_facts: mediaFacts as AssetDto["media_facts"],
      origin: null,
    };
    expect(assetFromDto(dto, null).facts).toBe(expected);
  });

  it.each(["image", "video", "audio"] as const)(
    "keeps the signed %s preview projection opaque",
    (mediaKind) => {
      const dto: AssetDto = {
        asset_id: `asset-${mediaKind}`,
        project_id: "project-1",
        media_kind: mediaKind,
        content_state: "available",
        display_name: mediaKind,
        created_at_epoch_ms: "1",
        content: {
          content_fingerprint_hex: "00".repeat(32),
          byte_length: "1",
          mime_type: `${mediaKind}/test`,
        },
        media_facts: { kind: mediaKind },
        origin: { kind: "imported", original_file_name: `${mediaKind}.bin` },
      };
      const projected = assetFromDto(dto, {
        asset_id: dto.asset_id,
        preview_uri: `desktop-asset://v1/${mediaKind}`,
        expires_at_epoch_ms: "300001",
      });

      expect(projected.kind).toBe(mediaKind);
      expect(projected.previewUrl).toBe(`desktop-asset://v1/${mediaKind}`);
    },
  );
});
