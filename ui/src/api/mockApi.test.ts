import { expect, it, vi } from "vitest";
import { mockApi } from "./mockApi.ts";
import type { RunLifecycleStatus, RunProgress, Workflow } from "../workflow/types.ts";
import type { AssistantConfigInput, RunObserver } from "./types.ts";

it("has no persistent asset root outside Tauri", async () => {
  await expect(mockApi.assetsRoot()).resolves.toBeNull();
});

it("persists Project rename for the next deduplicated list refresh", async () => {
  const created = await mockApi.createProject("Before");
  const renamed = await mockApi.renameProject(created, "After");

  await expect(mockApi.getProject(created.id)).resolves.toEqual(renamed);
  await expect(mockApi.listProjects()).resolves.toContainEqual(renamed);
});

it("preserves requested patch outputs and rejects undeclared names", async () => {
  const projectId = "named-output-project";
  const created = await mockApi.applyWorkflowPatch(projectId, "create", {
    expected_revision: null,
    operations: [
      {
        op: "add_node",
        alias: "prompt",
        capability: { id: "TextPrompt", version: "1.0" },
        params: {},
        position: null,
      },
      {
        op: "add_node",
        alias: "image",
        capability: { id: "TextToImage", version: "1.0" },
        params: {},
        position: null,
      },
      {
        op: "set_input",
        node: { kind: "alias", alias: "image" },
        input: "prompt",
        binding: {
          kind: "single",
          source: { node: { kind: "alias", alias: "prompt" }, output: "text" },
        },
      },
    ],
  });

  expect(created.workflow_head?.workflow.nodes[1]?.inputs.prompt).toEqual({
    kind: "single",
    source: { node_id: "n1", output: "text" },
  });
  await expect(mockApi.applyWorkflowPatch(projectId, "invalid", {
    expected_revision: 1,
    operations: [{
      op: "set_input",
      node: { kind: "id", id: "n2" },
      input: "prompt",
      binding: {
        kind: "single",
        source: { node: { kind: "id", id: "n1" }, output: "missing" },
      },
    }],
  })).rejects.toThrow("OUTPUT_NOT_DECLARED");
});

it("persists assistant settings for the in-browser workspace", async () => {
  const original = await mockApi.getAssistantConfig();
  const input: AssistantConfigInput = {
    enabled: true,
    base_url: original.base_url,
    model: original.model,
    api_key: null,
    clear_api_key: false,
  };

  await mockApi.setAssistantConfig(input);

  await expect(mockApi.getAssistantConfig()).resolves.toMatchObject({
    enabled: true,
    base_url: original.base_url,
    model: original.model,
    has_key: false,
  });

  await mockApi.setAssistantConfig({
    ...input,
    enabled: original.enabled,
  });
});

it("emits running then succeeded with nested node outputs", async () => {
  vi.useFakeTimers();
  const progress: RunProgress[] = [];
  const statuses: RunLifecycleStatus[] = [];

  mockApi.runWorkflow(workflow(), observer(progress, statuses));
  await vi.runAllTimersAsync();

  expect(progress.map((event) => event.nodeState)).toEqual([
    "running",
    "done",
    "running",
    "done",
  ]);
  expect(statuses).toEqual([{
    state: "succeeded",
    outputs: {
      prompt: { text: { kind: "string", value: "mock://text/prompt" } },
      image: { image: { kind: "image", value: "mock://image/image" } },
    },
  }]);
  vi.useRealTimers();
});

it("emits cancelling before authoritative cancellation", async () => {
  vi.useFakeTimers();
  const progress: RunProgress[] = [];
  const statuses: RunLifecycleStatus[] = [];

  const handle = mockApi.runWorkflow(workflow(), observer(progress, statuses));
  handle.cancel();

  expect(handle.runId).toMatch(/^run-[a-z0-9-]+$/);
  expect(statuses).toEqual([{ state: "cancelling" }]);
  await vi.runAllTimersAsync();
  expect(progress).toEqual([]);
  expect(statuses).toEqual([{ state: "cancelling" }, { state: "cancelled" }]);
  vi.useRealTimers();
});

it("retains a committed node when cancelled between mock nodes", async () => {
  vi.useFakeTimers();
  const progress: RunProgress[] = [];
  const statuses: RunLifecycleStatus[] = [];
  const handle = mockApi.runWorkflow(workflow(), observer(progress, statuses));

  await vi.advanceTimersToNextTimerAsync();
  await vi.advanceTimersToNextTimerAsync();
  handle.cancel();
  await vi.runAllTimersAsync();

  expect(progress.map(({ nodeId, nodeState }) => [nodeId, nodeState])).toEqual([
    ["prompt", "running"],
    ["prompt", "done"],
  ]);
  expect(statuses).toEqual([{ state: "cancelling" }, { state: "cancelled" }]);
  vi.useRealTimers();
});

function observer(
  progress: RunProgress[],
  statuses: RunLifecycleStatus[],
): RunObserver {
  return {
    onProgress: (event) => progress.push(event),
    onStatus: (status) => statuses.push(status),
  };
}

function workflow(): Workflow {
  return {
    version: "1.0",
    project_id: "default",
    nodes: [
      { id: "prompt", type: "TextPrompt", params: { text: "a red fox" }, inputs: {} },
      {
        id: "image",
        type: "TextToImage",
        params: {},
        inputs: { prompt: ["prompt", "text"] },
      },
    ],
  };
}
