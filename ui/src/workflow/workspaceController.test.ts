import { describe, expect, it, vi } from "vitest";
import type {
  WorkflowApplyPatchOutput,
  WorkflowHead,
  WorkflowPatchOperation,
} from "../api/index.ts";
import { deferred } from "../test/appFixtures.ts";
import {
  WorkspaceController,
  type WorkspaceBarrierReason,
  workflowPatchOperations,
} from "./workspaceController.ts";

const REASONS: WorkspaceBarrierReason[] = [
  "assistant_turn",
  "prepare_run",
  "undo",
  "redo",
  "project_switch",
  "close",
];

describe("WorkspaceController", () => {
  it("serializes patches and projects every acknowledgement from its canonical head", async () => {
    const first = deferred<WorkflowApplyPatchOutput>();
    const applyPatch = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(output(head(3, "second")));
    const projectHead = vi.fn();
    const controller = new WorkspaceController({ applyPatch, projectHead });
    controller.activate("project", head(1, "initial"));

    const firstPatch = controller.enqueue([replaceText("first")]);
    const secondPatch = controller.enqueue([replaceText("second")]);

    await Promise.resolve();
    expect(applyPatch).toHaveBeenCalledTimes(1);
    expect(applyPatch.mock.calls[0]?.[2].expected_revision).toBe(1);
    first.resolve(output(head(2, "normalized-first")));
    await firstPatch;
    await secondPatch;

    expect(applyPatch).toHaveBeenCalledTimes(2);
    expect(applyPatch.mock.calls[1]?.[2].expected_revision).toBe(2);
    expect(projectHead.mock.calls.map(([value]) => value.revision)).toEqual([2, 3]);
    expect(controller.currentHead()?.workflow.nodes[0]?.params.text).toBe("second");
  });

  it("does not let a stale optimistic response replace a newer authoritative head", async () => {
    const pending = deferred<WorkflowApplyPatchOutput>();
    const projectHead = vi.fn();
    const controller = new WorkspaceController({
      applyPatch: vi.fn().mockReturnValue(pending.promise),
      projectHead,
    });
    controller.activate("project", head(4, "initial"));

    const patch = controller.enqueue([replaceText("optimistic")]);
    controller.adoptHead(head(6, "external"));
    pending.resolve(output(head(5, "stale")));
    await patch;

    expect(controller.currentHead()?.revision).toBe(6);
    expect(controller.currentHead()?.workflow.nodes[0]?.params.text).toBe("external");
    expect(projectHead).toHaveBeenCalledTimes(1);
  });

  it("keeps a newer local draft visible while an older draft acknowledgement settles", async () => {
    const first = deferred<WorkflowApplyPatchOutput>();
    const applyPatch = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(output(head(3, "newer")));
    const projectHead = vi.fn();
    const controller = new WorkspaceController({ applyPatch, projectHead });
    controller.activate("project", head(1, "initial"));
    const older = head(1, "older").workflow;
    const newer = head(1, "newer").workflow;

    const olderPatch = controller.enqueueDraft(older);
    await Promise.resolve();
    controller.noteDraft(newer);
    first.resolve(output(head(2, "older")));
    await olderPatch;

    expect(controller.currentHead()?.revision).toBe(2);
    expect(projectHead).not.toHaveBeenCalled();

    await controller.enqueueDraft(newer);
    expect(applyPatch.mock.calls[1]?.[2].expected_revision).toBe(2);
    expect(projectHead).toHaveBeenCalledWith(head(3, "newer"));
  });

  it.each(REASONS)("uses the shared queue for the %s barrier", async (reason) => {
    const pending = deferred<WorkflowApplyPatchOutput>();
    const controller = new WorkspaceController({
      applyPatch: vi.fn().mockReturnValue(pending.promise),
      projectHead: vi.fn(),
    });
    controller.activate("project", head(1, "initial"));
    void controller.enqueue([replaceText("queued")]);
    const action = vi.fn().mockResolvedValue("done");

    const barrier = controller.runAfterBarrier(reason, action);
    await Promise.resolve();
    expect(action).not.toHaveBeenCalled();
    pending.resolve(output(head(2, "queued")));

    await expect(barrier).resolves.toBe("done");
    expect(action).toHaveBeenCalledTimes(1);
  });

  it("exposes undo and redo through the same barrier", async () => {
    const pending = deferred<WorkflowApplyPatchOutput>();
    const controller = new WorkspaceController({
      applyPatch: vi.fn().mockReturnValue(pending.promise),
      projectHead: vi.fn(),
    });
    controller.activate("project", head(1, "initial"));
    void controller.enqueue([replaceText("queued")]);
    const undo = vi.fn();
    const redo = vi.fn();

    const undoResult = controller.runUndo(undo);
    const redoResult = controller.runRedo(redo);
    expect(undo).not.toHaveBeenCalled();
    expect(redo).not.toHaveBeenCalled();
    pending.resolve(output(head(2, "queued")));

    await expect(undoResult).resolves.toBeUndefined();
    await expect(redoResult).resolves.toBeUndefined();
    expect(undo).toHaveBeenCalledOnce();
    expect(redo).toHaveBeenCalledOnce();
  });

  it("aborts a guarded action and restores focus when a patch fails", async () => {
    const failure = new Error("WORKFLOW_REVISION_CONFLICT");
    const controller = new WorkspaceController({
      applyPatch: vi.fn().mockRejectedValue(failure),
      projectHead: vi.fn(),
    });
    controller.activate("project", head(1, "initial"));
    void controller.enqueue([replaceText("conflict")]).catch(() => undefined);
    const action = vi.fn();
    const restoreFocus = vi.fn();

    await expect(
      controller.runAfterBarrier("assistant_turn", action, restoreFocus),
    ).rejects.toBe(failure);

    expect(action).not.toHaveBeenCalled();
    expect(restoreFocus).toHaveBeenCalledTimes(1);
    expect(controller.failure()).toBe(failure);
  });

  it("derives one typed patch for a complete local graph draft", () => {
    const base = head(1, "before").workflow;
    const draft = structuredClone(base);
    draft.nodes[0]!.params = { text: "after" };
    draft.nodes.push({
      id: "n1",
      type: "TextToImage",
      contract_version: "1.0",
      params: {},
      inputs: {
        prompt: { kind: "single", source: { node_id: "prompt", output: "text" } },
      },
      position: [40, 50],
    });

    expect(workflowPatchOperations(base, draft)).toEqual([
      {
        op: "add_node",
        alias: "draft-n1",
        capability: { id: "TextToImage", version: "1.0" },
        params: {},
        position: [40, 50],
      },
      {
        op: "replace_params",
        node: { kind: "id", id: "prompt" },
        params: { text: "after" },
      },
      {
        op: "set_input",
        node: { kind: "alias", alias: "draft-n1" },
        input: "prompt",
        binding: { kind: "single", source: { kind: "id", id: "prompt" } },
      },
    ]);
  });

  it("rebinds unchanged inputs when their source node is recreated", () => {
    const base = head(1, "before").workflow;
    base.nodes.push({
      id: "image",
      type: "TextToImage",
      contract_version: "1.0",
      params: {},
      inputs: { prompt: ["prompt", "text"] },
      position: [40, 50],
    });
    const draft = structuredClone(base);
    draft.nodes[0]!.contract_version = "2.0";

    expect(workflowPatchOperations(base, draft)).toContainEqual({
      op: "set_input",
      node: { kind: "id", id: "image" },
      input: "prompt",
      binding: { kind: "single", source: { kind: "alias", alias: "draft-prompt" } },
    });
  });
});

function replaceText(text: string): WorkflowPatchOperation {
  return {
    op: "replace_params",
    node: { kind: "id", id: "prompt" },
    params: { text },
  };
}

function head(revision: number, text: string): WorkflowHead {
  return {
    project_id: "project",
    revision,
    workflow: {
      version: "1.0",
      project_id: "project",
      nodes: [
        {
          id: "prompt",
          type: "TextPrompt",
          contract_version: "1.0",
          params: { text },
          inputs: {},
          position: [0, 0],
        },
      ],
    },
  };
}

function output(workflowHead: WorkflowHead): WorkflowApplyPatchOutput {
  return {
    workflow_head: workflowHead,
    aliases: [],
    readiness_blockers: [],
    changed: true,
    deduplicated: false,
    undo_id: `undo-${workflowHead.revision}`,
  };
}
