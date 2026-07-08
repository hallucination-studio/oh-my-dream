import { describe, expect, it, vi } from "vitest";
import { mockApi } from "./mockApi.ts";
import type { RunStatus, Workflow } from "../workflow/types.ts";

describe("mockApi", () => {
  it("has no persistent asset root outside Tauri", async () => {
    await expect(mockApi.assetsRoot()).resolves.toBeNull();
  });

  it("emits running then succeeded with nested node outputs", () => {
    vi.useFakeTimers();
    const observed: RunStatus[] = [];

    mockApi.runWorkflow(workflow(), (status) => observed.push(status));
    vi.runAllTimers();

    expect(observed.map((status) => status.state)).toEqual(["running", "running", "succeeded"]);
    expect(observed.at(-1)).toEqual({
      state: "succeeded",
      outputs: {
        prompt: { text: { kind: "string", value: "mock://text/prompt" } },
        image: { image: { kind: "image", value: "mock://image/image" } },
      },
    });
    vi.useRealTimers();
  });
});

function workflow(): Workflow {
  return {
    version: "1.0",
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
