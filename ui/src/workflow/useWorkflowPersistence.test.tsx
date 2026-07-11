import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api/index.ts";
import { deferred } from "../test/appFixtures.ts";
import type { Workflow } from "./types.ts";
import { useWorkflowPersistence } from "./useWorkflowPersistence.ts";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useWorkflowPersistence", () => {
  it("ignores an old A failure after the workflow changes from A to B and back to A", async () => {
    const firstSave = deferred<void>();
    const onError = vi.fn();
    const saveWorkflow = vi
      .spyOn(api, "saveWorkflow")
      .mockImplementationOnce(() => firstSave.promise)
      .mockResolvedValueOnce();
    const initial = workflow("initial");
    const firstA = workflow("A");
    const middleB = workflow("B");
    const latestA = workflow("A");
    const view = renderHook(
      ({ current }: { current: Workflow }) => useWorkflowPersistence(current, onError),
      { initialProps: { current: initial } },
    );
    view.result.current.markPersisted(initial);

    view.rerender({ current: firstA });
    await waitFor(() => expect(saveWorkflow).toHaveBeenCalledTimes(1));
    view.rerender({ current: middleB });
    view.rerender({ current: latestA });
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 250));
    });

    firstSave.reject(new Error("obsolete A failed"));
    await waitFor(() => expect(saveWorkflow).toHaveBeenCalledTimes(2));

    expect(onError).not.toHaveBeenCalled();
  });
});

function workflow(text: string): Workflow {
  return {
    version: "1.0",
    project_id: "project",
    nodes: [
      {
        id: "prompt",
        type: "TextPrompt",
        params: { text },
        inputs: {},
      },
    ],
  };
}
