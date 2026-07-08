import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset } from "./types.ts";

const invokeMock = vi.fn();
const convertFileSrcMock = vi.fn((path: string) => `asset://localhost${path}`);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  convertFileSrc: convertFileSrcMock,
}));

describe("tauriApi", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    convertFileSrcMock.mockClear();
  });

  it("returns the backend asset root", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    invokeMock.mockResolvedValueOnce("/tmp/oh-my-dream/assets");

    await expect(tauriApi.assetsRoot()).resolves.toBe("/tmp/oh-my-dream/assets");
    expect(invokeMock).toHaveBeenCalledWith("assets_root");
  });

  it("converts asset paths under the backend root into asset protocol URLs", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    const asset = assetFixture({
      file_path: "/tmp/oh-my-dream/assets/files/video.mp4",
      thumbnail_path: "/tmp/oh-my-dream/assets/thumbnails/video.png",
    });
    invokeMock
      .mockResolvedValueOnce("/tmp/oh-my-dream/assets")
      .mockResolvedValueOnce([asset]);

    const assets = await tauriApi.listAssets();

    expect(assets[0]).toMatchObject({
      file_path: "asset://localhost/tmp/oh-my-dream/assets/files/video.mp4",
      thumbnail_path: "asset://localhost/tmp/oh-my-dream/assets/thumbnails/video.png",
    });
    expect(convertFileSrcMock).toHaveBeenCalledWith("/tmp/oh-my-dream/assets/files/video.mp4");
    expect(convertFileSrcMock).toHaveBeenCalledWith("/tmp/oh-my-dream/assets/thumbnails/video.png");
  });

  it("leaves null thumbnails and paths outside the asset root unchanged", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    const asset = assetFixture({
      file_path: "/tmp/outside/video.mp4",
      thumbnail_path: null,
    });
    invokeMock.mockResolvedValueOnce("/tmp/oh-my-dream/assets").mockResolvedValueOnce(asset);

    const converted = await tauriApi.getAsset("asset-1");

    expect(converted.file_path).toBe("/tmp/outside/video.mp4");
    expect(converted.thumbnail_path).toBeNull();
    expect(convertFileSrcMock).not.toHaveBeenCalledWith("/tmp/outside/video.mp4");
  });
});

function assetFixture(overrides: Partial<Asset> = {}): Asset {
  return {
    id: "asset-1",
    kind: "video",
    file_path: "/tmp/oh-my-dream/assets/files/video.mp4",
    thumbnail_path: "/tmp/oh-my-dream/assets/thumbnails/video.png",
    workflow_snapshot: {},
    source_node_id: "video",
    tags: [],
    created_at: 0,
    ...overrides,
  };
}
