// Projects an admitted WorkflowRunDto into the step timeline the run surface
// renders. No second execution model is created — the DTO stays the authority.

import type { WorkflowRunDto } from "../api/index.ts";

export type RunStepState = "waiting" | "running" | "complete" | "attention" | "cancelled";

export interface RunTimelineStep {
  executionId: string;
  nodeId: string;
  state: RunStepState;
  progressBasisPoints: number | null;
}

/** One row per admitted Node Execution, in deterministic plan order. */
export function projectRunTimeline(run: WorkflowRunDto): RunTimelineStep[] {
  return run.node_executions.map((execution) => ({
    executionId: execution.node_execution_id,
    nodeId: execution.node_id,
    state: stepState(execution.state),
    progressBasisPoints: execution.progress_basis_points,
  }));
}

export function stepStateLabel(state: RunStepState): string {
  switch (state) {
    case "waiting":
      return "Waiting";
    case "running":
      return "Running";
    case "complete":
      return "Complete";
    case "attention":
      return "Needs attention";
    case "cancelled":
      return "Cancelled";
  }
}

export function runHeadline(run: WorkflowRunDto): string {
  switch (run.state) {
    case "queued":
      return "Run queued";
    case "running": {
      const done = run.node_executions.filter(
        (execution) => execution.state === "succeeded",
      ).length;
      return `Running ${done} of ${run.node_executions.length} steps`;
    }
    case "succeeded":
      return "Run complete";
    case "failed":
      return "Run failed";
    case "cancelled":
      return "Run cancelled";
  }
}

/** Elapsed milliseconds from admission until now (active) or the terminal update. */
export function elapsedMs(run: WorkflowRunDto, nowMs: number): number {
  const from = Number(run.created_at_epoch_ms);
  const to = run.state === "queued" || run.state === "running"
    ? nowMs
    : Number(run.updated_at_epoch_ms);
  return Math.max(0, to - from);
}

export function formatElapsed(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(rest).padStart(2, "0");
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${Number(mm)}:${ss}`;
}

function stepState(state: WorkflowRunDto["node_executions"][number]["state"]): RunStepState {
  switch (state) {
    case "pending":
      return "waiting";
    case "running":
    case "waiting_for_external_completion":
      return "running";
    case "succeeded":
      return "complete";
    case "failed":
    case "blocked":
      return "attention";
    case "cancelled":
      return "cancelled";
  }
}
