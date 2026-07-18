import { expect, it } from "vitest";
import type { WorkflowRunDto } from "../api/index.ts";
import {
  elapsedMs,
  formatElapsed,
  projectRunTimeline,
  runHeadline,
  stepStateLabel,
} from "./runTimeline.ts";

it("projects one row per admitted execution in plan order", () => {
  const steps = projectRunTimeline(run("running", [
    { state: "succeeded", progress_basis_points: 10_000 },
    { state: "waiting_for_external_completion", progress_basis_points: 4200 },
    { state: "pending", progress_basis_points: null },
  ]));
  expect(steps.map((step) => step.state)).toEqual(["complete", "running", "waiting"]);
  expect(steps.map((step) => step.nodeId)).toEqual(["node-0", "node-1", "node-2"]);
  expect(stepStateLabel("attention")).toBe("Needs attention");
});

it("headlines active and terminal runs", () => {
  expect(runHeadline(run("running", [
    { state: "succeeded", progress_basis_points: 10_000 },
    { state: "running", progress_basis_points: 1000 },
    { state: "pending", progress_basis_points: null },
  ]))).toBe("Running 1 of 3 steps");
  expect(runHeadline(run("queued", []))).toBe("Run queued");
  expect(runHeadline(run("succeeded", []))).toBe("Run complete");
  expect(runHeadline(run("cancelled", []))).toBe("Run cancelled");
});

it("measures elapsed from admission until now or the terminal update", () => {
  expect(elapsedMs(run("running", []), 61_000)).toBe(60_000);
  expect(elapsedMs({ ...run("succeeded", []), updated_at_epoch_ms: "41000" }, 99_000)).toBe(40_000);
  expect(formatElapsed(0)).toBe("0:00");
  expect(formatElapsed(65_000)).toBe("1:05");
  expect(formatElapsed(3_725_000)).toBe("1:02:05");
});

function run(
  state: WorkflowRunDto["state"],
  executions: Array<{
    state: WorkflowRunDto["node_executions"][number]["state"];
    progress_basis_points: number | null;
  }>,
): WorkflowRunDto {
  return {
    workflow_run_id: "run-1",
    project_id: "project-1",
    workflow_id: "workflow-1",
    workflow_revision: "1",
    scope: { kind: "whole_workflow" },
    state,
    created_at_epoch_ms: "1000",
    updated_at_epoch_ms: "2000",
    node_executions: executions.map((execution, index) => ({
      node_id: `node-${index}`,
      node_execution_id: `execution-${index}`,
      state: execution.state,
      progress_basis_points: execution.progress_basis_points,
    })),
  };
}
