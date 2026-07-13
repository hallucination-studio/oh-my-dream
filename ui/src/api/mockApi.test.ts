import { expect, it, vi } from "vitest";
import { mockApi } from "./mockApi.ts";
import type { RunLifecycleStatus, RunProgress, Workflow } from "../workflow/types.ts";
import type { RunObserver } from "./types.ts";

it("has no persistent asset root outside Tauri", async () => {
  await expect(mockApi.assetsRoot()).resolves.toBeNull();
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
