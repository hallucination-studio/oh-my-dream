import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AssetDto,
  AssistantSendInput,
  Project,
  RunObserver,
  WorkflowRunEvent,
  WorkflowRunResult,
} from "./types.ts";

const invokeMock = vi.fn();
const convertFileSrcMock = vi.fn((path: string) => `asset://localhost${path}`);
const channelMocks: Array<{ onmessage: (event: WorkflowRunEvent) => void }> = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  convertFileSrc: convertFileSrcMock,
  Channel: class<T> {
    onmessage = (_event: T) => {};

    constructor() {
      channelMocks.push(this as unknown as { onmessage: (event: WorkflowRunEvent) => void });
    }
  },
}));

beforeEach(() => {
  invokeMock.mockReset();
  convertFileSrcMock.mockClear();
  channelMocks.length = 0;
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

it("uses the frozen Project commands, paginates, and deduplicates by Project ID", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const alpha = projectFixture("alpha", "Alpha", "1");
  const renamedAlpha = projectFixture("alpha", "Renamed", "2");
  const beta = projectFixture("beta", "Beta", "1");
  invokeMock
    .mockResolvedValueOnce({ projects: [alpha, beta], next_cursor: "next" })
    .mockResolvedValueOnce({ projects: [renamedAlpha], next_cursor: null });

  await expect(tauriApi.listProjects()).resolves.toEqual([renamedAlpha, beta]);
  expect(invokeMock).toHaveBeenNthCalledWith(1, "project_list", {
    request: { limit: 100, cursor: null },
  });
  expect(invokeMock).toHaveBeenNthCalledWith(2, "project_list", {
    request: { limit: 100, cursor: "next" },
  });
});

it("sends canonical create, get, rename, and open Project requests", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const project = projectFixture("alpha", "Alpha", "3");
  invokeMock
    .mockResolvedValueOnce(project)
    .mockResolvedValueOnce(project)
    .mockResolvedValueOnce({ ...project, name: "Renamed", revision: "4" })
    .mockResolvedValueOnce({ project, current_workflow_summary: null });

  await tauriApi.createProject("Alpha");
  await tauriApi.getProject("alpha");
  await tauriApi.renameProject(project, "Renamed");
  await expect(tauriApi.openProject("alpha")).resolves.toEqual({
    project,
    current_workflow_summary: null,
    workflow_head: null,
  });

  expect(invokeMock.mock.calls[0]?.[0]).toBe("project_create");
  expect(invokeMock.mock.calls[0]?.[1]).toMatchObject({
    request: { name: "Alpha" },
  });
  expect(invokeMock).toHaveBeenNthCalledWith(2, "project_get", {
    request: { project_id: "alpha" },
  });
  expect(invokeMock.mock.calls[2]?.[0]).toBe("project_rename");
  expect(invokeMock.mock.calls[2]?.[1]).toMatchObject({
    request: { project_id: "alpha", expected_revision: "3", name: "Renamed" },
  });
  expect(invokeMock).toHaveBeenNthCalledWith(4, "project_open", {
    request: { project_id: "alpha" },
  });
});

it("uses the exact Node Capability and Generation Profile commands", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  invokeMock.mockResolvedValueOnce([]).mockResolvedValueOnce([]);

  await tauriApi.nodeCapabilityList();
  await tauriApi.generationProfileListForCapability({
    id: "image.generate_from_text",
    version: "1.0",
  });

  expect(invokeMock).toHaveBeenNthCalledWith(1, "node_capability_list", { request: {} });
  expect(invokeMock).toHaveBeenNthCalledWith(2, "generation_profile_list_for_capability", {
    request: {
      capability_id: "image.generate_from_text",
      capability_version: "1.0",
    },
  });
});

it("applies Workflow patches through the shared Tauri command", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const input = {
    expected_revision: 4,
    operations: [
      {
        op: "replace_params" as const,
        node: { kind: "id" as const, id: "prompt" },
        params: { text: "updated" },
      },
    ],
  };
  invokeMock.mockResolvedValueOnce({ workflow_head: null, aliases: [] });

  await tauriApi.applyWorkflowPatch("project", "request-5", input);

  expect(invokeMock).toHaveBeenCalledWith("workflow_apply_patch", {
    project_id: "project",
    request_id: "request-5",
    input,
  });
});

it("searches paged capability summaries and loads exact bundles", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const request = { query: "video", category: "video", type_id: "Video", cursor: null, limit: 12 };
  const refs = [{ id: "ImageToVideo", version: "1.0" }];
  invokeMock
    .mockResolvedValueOnce({ capabilities: [], next_cursor: "12" })
    .mockResolvedValueOnce({ capabilities: [] });

  await expect(tauriApi.searchCapabilities(request)).resolves.toEqual({
    capabilities: [],
    next_cursor: "12",
  });
  await expect(tauriApi.getCapabilityBundles(refs)).resolves.toEqual({ capabilities: [] });

  expect(invokeMock).toHaveBeenCalledWith("search_capabilities", request);
  expect(invokeMock).toHaveBeenCalledWith("get_capability_bundles", { refs });
});

it("streams scoped progress and completes from the authoritative result", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const observed: unknown[] = [];
  const terminal = deferred<WorkflowRunResult>();
  invokeMock.mockReturnValueOnce(terminal.promise);

  const handle = tauriApi.runWorkflow(
    { version: "1.0", project_id: "default", nodes: [] },
    observer(observed),
  );
  channelMocks[0]?.onmessage({
    event: "started",
    run_id: handle.runId,
    project_id: "default",
  });
  channelMocks[0]?.onmessage({
    event: "progress",
    run_id: handle.runId,
    node: { node_id: "image", state: "running", progress: 0.25, cost: null },
  });
  terminal.resolve({ status: "succeeded", run_id: handle.runId, outputs: {} });
  await flushPromises();

  expect(observed[0]).toEqual({
    type: "progress",
    nodeId: "image",
    progress: 0.25,
    nodeState: "running",
    cost: undefined,
  });
  expect(observed[1]).toEqual({ state: "succeeded", outputs: {} });
  expect(handle.runId).toMatch(/^run-[a-z0-9-]+$/);
  expect(invokeMock).toHaveBeenCalledWith("start_workflow_run", {
    run_id: handle.runId,
    workflow_json: JSON.stringify({ version: "1.0", project_id: "default", nodes: [] }),
    on_event: expect.anything(),
  });
});

it("buffers cancellation until Started and waits for terminal cancellation", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const observed: unknown[] = [];
  const terminal = deferred<WorkflowRunResult>();
  invokeMock.mockImplementation((command: string, args?: { run_id?: string }) => {
    if (command === "start_workflow_run") return terminal.promise;
    return Promise.resolve({ status: "requested", run_id: args?.run_id });
  });

  const handle = tauriApi.runWorkflow(
    { version: "1.0", project_id: "default", nodes: [] },
    observer(observed),
  );
  handle.cancel();
  handle.cancel();

  expect(observed).toEqual([{ state: "cancelling" }]);
  expect(invokeMock).not.toHaveBeenCalledWith("cancel_workflow_run", expect.anything());

  channelMocks[0]?.onmessage({
    event: "started",
    run_id: handle.runId,
    project_id: "default",
  });
  channelMocks[0]?.onmessage({
    event: "progress",
    run_id: handle.runId,
    node: { node_id: "image", state: "running", progress: 0.5, cost: null },
  });
  channelMocks[0]?.onmessage({
    event: "progress",
    run_id: handle.runId,
    node: { node_id: "image", state: "done", progress: 1, cost: 900 },
  });
  await flushPromises();

  expect(invokeMock).toHaveBeenCalledTimes(2);
  expect(invokeMock).toHaveBeenCalledWith("cancel_workflow_run", { run_id: handle.runId });
  expect(observed).toEqual([
    { state: "cancelling" },
    {
      type: "progress",
      nodeId: "image",
      progress: 1,
      nodeState: "done",
      cost: 900,
    },
  ]);

  terminal.resolve({ status: "cancelled", run_id: handle.runId });
  await flushPromises();
  expect(observed.at(-1)).toEqual({ state: "cancelled" });
});

it("allows cancellation retry after a command failure while keeping success authoritative", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const observed: unknown[] = [];
  const terminal = deferred<WorkflowRunResult>();
  let cancelAttempts = 0;
  invokeMock.mockImplementation((command: string, args?: { run_id?: string }) => {
    if (command === "start_workflow_run") return terminal.promise;
    cancelAttempts += 1;
    return cancelAttempts === 1
      ? Promise.reject(new Error("cancel transport failed"))
      : Promise.resolve({ status: "requested", run_id: args?.run_id });
  });
  const handle = tauriApi.runWorkflow(
    { version: "1.0", project_id: "default", nodes: [] },
    observer(observed),
  );
  channelMocks[0]?.onmessage({
    event: "started",
    run_id: handle.runId,
    project_id: "default",
  });

  handle.cancel();
  await flushPromises();
  expect(observed).toEqual([
    { state: "cancelling" },
    { state: "cancel_failed", reason: "Error: cancel transport failed" },
  ]);

  handle.cancel();
  await flushPromises();
  expect(cancelAttempts).toBe(2);
  terminal.resolve({ status: "succeeded", run_id: handle.runId, outputs: {} });
  await flushPromises();

  expect(observed).toEqual([
    { state: "cancelling" },
    { state: "cancel_failed", reason: "Error: cancel transport failed" },
    { state: "cancelling" },
    { state: "succeeded", outputs: {} },
  ]);
});

it("reports the authoritative failed terminal result", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const observed: unknown[] = [];
  const terminal = deferred<WorkflowRunResult>();
  invokeMock.mockReturnValueOnce(terminal.promise);

  const handle = tauriApi.runWorkflow(
    { version: "1.0", project_id: "default", nodes: [] },
    observer(observed),
  );
  terminal.resolve({ status: "failed", run_id: handle.runId, reason: "provider failed" });
  await flushPromises();

  expect(observed).toEqual([{ state: "failed", reason: "provider failed" }]);
});

it("rejects a terminal result for a different run", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const observed: unknown[] = [];
  const terminal = deferred<WorkflowRunResult>();
  invokeMock.mockReturnValueOnce(terminal.promise);

  const handle = tauriApi.runWorkflow(
    { version: "1.0", project_id: "default", nodes: [] },
    observer(observed),
  );
  terminal.resolve({ status: "cancelled", run_id: "another-run" });
  await flushPromises();

  expect(observed).toEqual([{
    state: "failed",
    reason: `Workflow run identity mismatch: expected ${handle.runId}, received another-run`,
  }]);
});

describe("tauriApi assistant commands", () => {
  it("invokes the surviving assistant configuration commands", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    invokeMock
      .mockResolvedValueOnce({ enabled: true, base_url: "https://api.openai.com/v1", model: "gpt-5.4", has_key: false })
      .mockResolvedValueOnce(undefined);

    await expect(tauriApi.getAssistantConfig()).resolves.toMatchObject({ model: "gpt-5.4" });
    await tauriApi.setAssistantConfig({ enabled: true, base_url: "https://api.openai.com/v1", model: "gpt-5.4", api_key: null, clear_api_key: false });

    expect(invokeMock).toHaveBeenCalledWith("get_assistant_config");
    expect(invokeMock).toHaveBeenCalledWith("set_assistant_config", {
      input: { enabled: true, base_url: "https://api.openai.com/v1", model: "gpt-5.4", api_key: null, clear_api_key: false },
    });
  });

  it("returns the canonical Workflow head from an assistant turn", async () => {
    const { tauriApi } = await import("./tauriApi.ts");
    const input: AssistantSendInput = {
      project_id: "project-1",
      workflow_present: false,
      workflow_revision: null,
      selected_node_ids: [],
      selected_asset_ids: [],
      text: "Build the workflow",
    };
    const head = {
      project_id: "project-1",
      revision: 1,
      workflow: { version: "1.0", project_id: "project-1", nodes: [] },
    };
    const onEvent = vi.fn();
    invokeMock.mockResolvedValueOnce(head);

    await expect(tauriApi.sendAssistant(input, onEvent)).resolves.toEqual(head);
    expect(invokeMock).toHaveBeenCalledWith("assistant_send", {
      input,
      on_event: expect.anything(),
    });
  });

});

function assetFixture(overrides: Partial<AssetDto> = {}): AssetDto {
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

function projectFixture(id: string, name: string, revision: string): Project {
  return {
    id,
    name,
    revision,
    created_at_epoch_ms: "0",
    updated_at_epoch_ms: "0",
  };
}

async function flushPromises(): Promise<void> {
  for (let index = 0; index < 5; index += 1) {
    await Promise.resolve();
  }
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
function observer(observed: unknown[]): RunObserver {
  return {
    onProgress: (progress) => observed.push({ type: "progress", ...progress }),
    onStatus: (status) => observed.push(status),
  };
}
