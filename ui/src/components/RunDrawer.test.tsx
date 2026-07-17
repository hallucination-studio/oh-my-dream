import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  GenerationTaskListPageDto,
  GenerationTaskSummaryDto,
  WorkflowRunDto,
} from "../api/types.ts";
import { RunDrawer } from "./RunDrawer.tsx";

const PROJECT_ID = "10000000-0000-0000-0000-000000000001";
const RUN_ID = "20000000-0000-4000-8000-000000000001";
const NODE_ID = "30000000-0000-4000-8000-000000000001";
const EXECUTION_ID = "40000000-0000-4000-8000-000000000001";

describe("RunDrawer", () => {
  it("selects the exact waiting Step and shows normalized Task state", async () => {
    const task = taskSummary("queued");
    const list = vi.fn(async (): Promise<GenerationTaskListPageDto> => ({
      tasks: [task],
      next_cursor: null,
    }));
    const taskApi = {
      generationTaskList: list,
    };
    const view = render(<RunDrawer open onClose={vi.fn()} projectId={PROJECT_ID} run={waitingRun()} taskApi={taskApi} />);

    expect(await screen.findByText("Queued")).toBeTruthy();
    expect(screen.getByText("image.high_quality_general@1")).toBeTruthy();
    expect(list).toHaveBeenCalledWith(PROJECT_ID, null, null, null, 100);

    const running = { ...task, status: "running" as const, progress_percent: 42 };
    view.rerender(
      <RunDrawer
        open
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        run={waitingRun()}
        taskApi={{ generationTaskList: async () => ({ tasks: [running], next_cursor: null }) }}
      />,
    );
    expect(await screen.findByText("Running")).toBeTruthy();
    expect(screen.getByText("42%")).toBeTruthy();

    const failed = {
      ...running,
      status: "failed" as const,
      failure: { kind: "provider_rejected" as const, code: "CONTENT_POLICY", message: "Safe failure" },
    };
    view.rerender(
      <RunDrawer
        open
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        run={waitingRun()}
        taskApi={{ generationTaskList: async () => ({ tasks: [failed], next_cursor: null }) }}
      />,
    );
    expect(await screen.findByText("Safe failure")).toBeTruthy();
  });

  it("does not select a stale Task and preserves output-only preview", async () => {
    const close = vi.fn();
    const stale = { ...taskSummary("succeeded"), workflow_run_id: "other-run" };
    const taskApi = {
      generationTaskList: async () => ({ tasks: [stale], next_cursor: null }),
    };
    render(
      <RunDrawer
        open
        onClose={close}
        projectId={PROJECT_ID}
        run={{ ...waitingRun(), node_executions: waitingRun().node_executions.map((execution) => ({ ...execution, state: "succeeded" as const })) }}
        outputPreview={{ [NODE_ID]: { image: { kind: "image", value: "asset" } } }}
        taskApi={taskApi}
      />,
    );

    expect(await screen.findByText("This Run has no available Task for the selected Step.")).toBeTruthy();
    expect(screen.getByText("Outputs")).toBeTruthy();
    expect(screen.getAllByText("image")).toHaveLength(2);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });

  it("selects the only terminal Task and shows a safe empty state when none exists", async () => {
    const terminalRun = {
      ...waitingRun(),
      state: "succeeded" as const,
      node_executions: waitingRun().node_executions.map((execution) => ({
        ...execution,
        state: "succeeded" as const,
      })),
    };
    const view = render(
      <RunDrawer
        open
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        run={terminalRun}
        taskApi={{ generationTaskList: async () => ({ tasks: [taskSummary("succeeded")], next_cursor: null }) }}
      />,
    );

    expect(await screen.findByText("Succeeded")).toBeTruthy();

    view.rerender(
      <RunDrawer
        open
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        run={terminalRun}
        taskApi={{ generationTaskList: async () => ({ tasks: [], next_cursor: null }) }}
      />,
    );
    expect(await screen.findByText("This Run has no available Task for the selected Step.")).toBeTruthy();
  });
});

function waitingRun(): WorkflowRunDto {
  return {
    workflow_run_id: RUN_ID,
    project_id: PROJECT_ID,
    workflow_id: "50000000-0000-4000-8000-000000000001",
    workflow_revision: "1",
    scope: { kind: "whole_workflow" },
    state: "running",
    created_at_epoch_ms: "1",
    updated_at_epoch_ms: "2",
    node_executions: [{
      node_id: NODE_ID,
      node_execution_id: EXECUTION_ID,
      state: "waiting_for_external_completion",
      progress_basis_points: 4200,
    }],
  };
}

function taskSummary(status: GenerationTaskSummaryDto["status"]): GenerationTaskSummaryDto {
  return {
    id: "60000000-0000-4000-8000-000000000001",
    project_id: PROJECT_ID,
    workflow_id: "50000000-0000-4000-8000-000000000001",
    workflow_run_id: RUN_ID,
    workflow_node_id: NODE_ID,
    workflow_node_execution_id: EXECUTION_ID,
    request_kind: "image",
    status,
    progress_percent: status === "queued" ? null : 100,
    generation_profile_ref: "image.high_quality_general@1",
    provider_id: "mock",
    provider_display_name: "Mock",
    prompt_preview: "A safe preview",
    preview_asset_id: null,
    has_result: false,
    failure: null,
    created_at_epoch_ms: "1",
    updated_at_epoch_ms: "2",
    completed_at_epoch_ms: null,
  };
}
