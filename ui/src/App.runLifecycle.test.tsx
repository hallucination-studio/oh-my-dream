import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api/index.ts";
import type { DurableWorkflowRunEventDto, WorkflowDto } from "./api/types.ts";
import { useRunController } from "./workflow/useRunController.ts";

const PROJECT_ID = "10000000-0000-4000-8000-000000000001";
const WORKFLOW_ID = "20000000-0000-4000-8000-000000000001";
const NODE_ID = "30000000-0000-4000-8000-000000000001";
const RUN_ID = "40000000-0000-4000-8000-000000000001";
const EXECUTION_ID = "50000000-0000-4000-8000-000000000001";

afterEach(() => vi.restoreAllMocks());

describe("canonical Workflow Run event repair", () => {
  it("deduplicates events and repairs a sequence gap through the bounded query", async () => {
    let observe: ((event: DurableWorkflowRunEventDto) => void) | null = null;
    vi.spyOn(api, "observeWorkflowRunEvents").mockImplementation(async (next) => {
      observe = next;
      return () => undefined;
    });
    vi.spyOn(api, "workflowStartRun").mockResolvedValue({
      workflow_run_id: RUN_ID,
      project_id: PROJECT_ID,
      workflow_id: WORKFLOW_ID,
      workflow_revision: "1",
      scope: { kind: "whole_workflow" },
      state: "queued",
      created_at_epoch_ms: "1",
      updated_at_epoch_ms: "1",
      node_executions: [{
        node_id: NODE_ID,
        node_execution_id: EXECUTION_ID,
        state: "pending",
        progress_basis_points: null,
      }],
    });
    const list = vi.spyOn(api, "workflowListRunEvents")
      .mockResolvedValueOnce({ events: [], next_sequence: null })
      .mockResolvedValueOnce({ events: [event("2", "node_started")], next_sequence: null });
    const applyProgress = vi.fn();
    const setStatus = vi.fn();
    const view = renderHook(() => useRunController({
      getWorkflow: () => workflow(),
      setStatus,
      resetProjection: vi.fn(),
      applyProgress,
      settleProjection: vi.fn(),
      onSucceeded: vi.fn(),
    }));

    await act(() => view.result.current.run());
    act(() => {
      observe?.(event("1", "run_queued"));
      observe?.(event("3", "node_progressed", 5000));
      observe?.(event("3", "node_progressed", 5000));
    });

    await waitFor(() => expect(list).toHaveBeenCalledWith(PROJECT_ID, RUN_ID, "1", 500));
    await waitFor(() => expect(applyProgress).toHaveBeenCalledTimes(2));
  });

  it("accumulates node outputs and settles with a steps count", async () => {
    let observe: ((event: DurableWorkflowRunEventDto) => void) | null = null;
    vi.spyOn(api, "observeWorkflowRunEvents").mockImplementation(async (next) => {
      observe = next;
      return () => undefined;
    });
    vi.spyOn(api, "workflowStartRun").mockResolvedValue({
      workflow_run_id: RUN_ID,
      project_id: PROJECT_ID,
      workflow_id: WORKFLOW_ID,
      workflow_revision: "1",
      scope: { kind: "whole_workflow" },
      state: "queued",
      created_at_epoch_ms: "1",
      updated_at_epoch_ms: "1",
      node_executions: [{
        node_id: NODE_ID,
        node_execution_id: EXECUTION_ID,
        state: "pending",
        progress_basis_points: null,
      }],
    });
    vi.spyOn(api, "workflowListRunEvents").mockResolvedValue({ events: [], next_sequence: null });
    const settleProjection = vi.fn();
    const setStatus = vi.fn();
    const view = renderHook(() => useRunController({
      getWorkflow: () => workflow(),
      setStatus,
      resetProjection: vi.fn(),
      applyProgress: vi.fn(),
      settleProjection,
      onSucceeded: vi.fn(),
    }));

    await act(() => view.result.current.run());
    act(() => {
      observe?.({
        workflow_run_id: RUN_ID,
        sequence: "1",
        occurred_at_epoch_ms: "1",
        payload: {
          type: "node_succeeded",
          node_execution_id: EXECUTION_ID,
          outputs: [{
            key: "image",
            value: { type: "image", asset_id: "asset-1", preview_uri: "data:image/svg+xml;utf8,x" },
          }],
        },
      });
      observe?.({
        workflow_run_id: RUN_ID,
        sequence: "2",
        occurred_at_epoch_ms: "2",
        payload: { type: "run_succeeded" },
      });
    });

    await waitFor(() =>
      expect(settleProjection).toHaveBeenCalledWith({
        state: "succeeded",
        steps: 1,
        outputs: {
          [NODE_ID]: { image: { kind: "image", value: "data:image/svg+xml;utf8,x" } },
        },
      }),
    );
  });
});

function event(
  sequence: string,
  type: string,
  progress_basis_points?: number,
): DurableWorkflowRunEventDto {
  return {
    workflow_run_id: RUN_ID,
    sequence,
    occurred_at_epoch_ms: sequence,
    payload: {
      type,
      ...(type.startsWith("node_") ? { node_execution_id: EXECUTION_ID } : {}),
      ...(progress_basis_points === undefined ? {} : { progress_basis_points }),
    },
  };
}

function workflow(): WorkflowDto {
  return {
    schema_version: 1,
    workflow_id: WORKFLOW_ID,
    project_id: PROJECT_ID,
    revision: "1",
    created_at_epoch_ms: "1",
    updated_at_epoch_ms: "1",
    nodes: [],
    input_bindings: [],
  };
}
