import { describe, expect, it } from "vitest";
import type { AssetDto } from "../api/index.ts";
import { assetFromDto } from "./model.ts";

describe("assetFromDto", () => {
  it("mechanically translates the API boundary shape", () => {
    const dto: AssetDto = {
      id: "asset-1",
      kind: "image",
      file_path: "asset://image",
      thumbnail_path: "asset://thumbnail",
      workflow_snapshot: { version: "1.0" },
      prompt: "sunrise",
      project_id: "project-1",
      project_name: "Launch",
      source_node_id: "image",
      source_node_type: "TextToImage",
      model: "mock-image",
      seed: "18446744073709551615",
      cost: 250,
      tags: ["saved"],
      created_at: 123,
    };

    expect(assetFromDto(dto)).toEqual({
      id: "asset-1",
      kind: "image",
      fileUrl: "asset://image",
      thumbnailUrl: "asset://thumbnail",
      prompt: "sunrise",
      projectName: "Launch",
      sourceNodeType: "TextToImage",
      model: "mock-image",
      seed: "18446744073709551615",
      cost: 250,
      createdAt: 123,
    });
  });
});
