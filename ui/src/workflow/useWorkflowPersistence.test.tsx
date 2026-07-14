import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api/index.ts";
import type { WorkflowApplyPatchOutput } from "../api/types.ts";
import { deferred } from "../test/appFixtures.ts";
import type { Workflow } from "./types.ts";
import { useWorkflowPersistence } from "./useWorkflowPersistence.ts";
import { WorkspaceController } from "./workspaceController.ts";

const tauriWindowMocks = vi.hoisted(() => ({
  destroy: vi.fn(),
  onCloseRequested: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => tauriWindowMocks,
}));

afterEach(() => {
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  tauriWindowMocks.destroy.mockReset();
  tauriWindowMocks.onCloseRequested.mockReset();
  vi.restoreAllMocks();
});

describe("useWorkflowPersistence", () => {
  it("ignores an old A failure after the workflow changes from A to B and back to A", async () => {
    const firstSave = deferred<never>();
    const onError = vi.fn();
    const applyWorkflowPatch = vi
      .spyOn(api, "applyWorkflowPatch")
      .mockImplementationOnce(() => firstSave.promise)
      .mockResolvedValueOnce({
        workflow_head: { project_id: "project", revision: 2, workflow: workflow("A") },
        aliases: [],
        readiness_blockers: [],
        changed: true,
        deduplicated: false,
        undo_id: "undo-2",
      });
    const initial = workflow("initial");
    const firstA = workflow("A");
    const middleB = workflow("B");
    const latestA = workflow("A");
    const controller = new WorkspaceController({
      applyPatch: api.applyWorkflowPatch,
      projectHead: vi.fn(),
    });
    controller.activate("project", {
      project_id: "project",
      revision: 1,
      workflow: initial,
    });
    const view = renderHook(
      ({ current }: { current: Workflow }) =>
        useWorkflowPersistence(current, controller, onError),
      { initialProps: { current: initial } },
    );
    view.result.current.markPersisted(initial);

    view.rerender({ current: firstA });
    await waitFor(() => expect(applyWorkflowPatch).toHaveBeenCalledTimes(1));
    view.rerender({ current: middleB });
    view.rerender({ current: latestA });
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 250));
    });

    firstSave.reject(new Error("obsolete A failed"));
    await waitFor(() => expect(applyWorkflowPatch).toHaveBeenCalledTimes(2));

    expect(onError).not.toHaveBeenCalled();
  });

  it("blocks a desktop close until the shared queue is acknowledged", async () => {
    const pending = deferred<WorkflowApplyPatchOutput>();
    const applyPatch = vi.fn().mockReturnValue(pending.promise);
    const controller = new WorkspaceController({ applyPatch, projectHead: vi.fn() });
    const initial = workflow("initial");
    controller.activate("project", {
      project_id: "project",
      revision: 1,
      workflow: initial,
    });
    tauriWindowMocks.onCloseRequested.mockResolvedValue(() => undefined);
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const view = renderHook(
      ({ current }: { current: Workflow }) =>
        useWorkflowPersistence(current, controller, vi.fn()),
      { initialProps: { current: initial } },
    );
    view.result.current.markPersisted(initial);
    view.rerender({ current: workflow("changed") });
    await waitFor(() => expect(applyPatch).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(tauriWindowMocks.onCloseRequested).toHaveBeenCalledTimes(1));

    const event = { preventDefault: vi.fn() };
    const close = tauriWindowMocks.onCloseRequested.mock.calls[0]?.[0] as
      ((value: typeof event) => Promise<void>);
    const closeAttempt = close(event);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    expect(tauriWindowMocks.destroy).not.toHaveBeenCalled();
    pending.resolve({
      workflow_head: null,
      aliases: [],
      readiness_blockers: [],
      changed: false,
      deduplicated: false,
      undo_id: null,
    });
    await closeAttempt;

    expect(tauriWindowMocks.destroy).toHaveBeenCalledTimes(1);
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
