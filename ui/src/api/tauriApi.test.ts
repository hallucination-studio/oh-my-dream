import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset } from "./types.ts";

const invokeMock = vi.fn();
const convertFileSrcMock = vi.fn((path: string) => `asset://localhost${path}`);
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  convertFileSrc: convertFileSrcMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("tauriApi", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    convertFileSrcMock.mockClear();
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
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

    const assets = await tauriApi.listAssets({
      kind: "video",
      project_id: "project-1",
      model: "mock-video",
      prompt: "ocean",
      sort: "cost_desc",
    });

    expect(assets[0]).toMatchObject({
      file_path: "asset://localhost/tmp/oh-my-dream/assets/files/video.mp4",
      thumbnail_path: "asset://localhost/tmp/oh-my-dream/assets/thumbnails/video.png",
    });
    expect(convertFileSrcMock).toHaveBeenCalledWith("/tmp/oh-my-dream/assets/files/video.mp4");
    expect(convertFileSrcMock).toHaveBeenCalledWith("/tmp/oh-my-dream/assets/thumbnails/video.png");
    expect(invokeMock).toHaveBeenCalledWith("list_assets", {
      kind: "video",
      project_id: "project-1",
      model: "mock-video",
      prompt: "ocean",
      sort: "cost_desc",
    });
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

  it("maps node_progress events to running statuses", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    const observed: unknown[] = [];
    let progressHandler: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementationOnce((_event, handler) => {
      progressHandler = handler as (event: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });
    invokeMock.mockResolvedValueOnce({ outputs: {} });

    tauriApi.runWorkflow({ version: "1.0", project_id: "default", nodes: [] }, (status) => {
      observed.push(status);
    });
    progressHandler?.({
      payload: { node_id: "image", state: "running", progress: 0.25, cost: null },
    });
    await Promise.resolve();

    expect(observed[0]).toEqual({
      state: "running",
      nodeId: "image",
      progress: 0.25,
      nodeState: "running",
      cost: undefined,
    });
  });

  it("disposes node_progress listener when listen resolves after the run", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    const dispose = vi.fn();
    let resolveListen: ((dispose: () => void) => void) | undefined;
    listenMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveListen = resolve;
        }),
    );
    invokeMock.mockResolvedValueOnce({ outputs: {} });

    tauriApi.runWorkflow({ version: "1.0", project_id: "default", nodes: [] }, () => {});
    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalled();
    resolveListen?.(dispose);
    await flushPromises();

    expect(dispose).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("run_workflow", {
      workflow_json: JSON.stringify({ version: "1.0", project_id: "default", nodes: [] }),
    });
  });
});

function assetFixture(overrides: Partial<Asset> = {}): Asset {
  return {
    id: "asset-1",
    kind: "video",
    file_path: "/tmp/oh-my-dream/assets/files/video.mp4",
    thumbnail_path: "/tmp/oh-my-dream/assets/thumbnails/video.png",
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
    ...overrides,
  };
}

async function flushPromises(): Promise<void> {
  for (let index = 0; index < 5; index += 1) {
    await Promise.resolve();
  }
}
