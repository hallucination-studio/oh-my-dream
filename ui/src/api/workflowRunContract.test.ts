import { describe, expect, it } from "vitest";
import cancelNotActiveFixture from "../__fixtures__/cancel_workflow_run_not_active.json";
import cancelRequestedFixture from "../__fixtures__/cancel_workflow_run_requested.json";
import cancelledFixture from "../__fixtures__/workflow_run_cancelled.json";
import failedFixture from "../__fixtures__/workflow_run_failed.json";
import progressFixture from "../__fixtures__/workflow_run_progress.json";
import startedFixture from "../__fixtures__/workflow_run_started.json";
import succeededFixture from "../__fixtures__/workflow_run_succeeded.json";
import type {
  CancelWorkflowRunResult,
  WorkflowRunEvent,
  WorkflowRunResult,
} from "./types.ts";
import type { NodeProgressEvent, RunOutput } from "../workflow/types.ts";

describe("scoped workflow run DTO fixtures", () => {
  it("match the frontend discriminated unions", () => {
    expect(isWorkflowRunEvent(startedFixture)).toBe(true);
    expect(isWorkflowRunEvent(progressFixture)).toBe(true);
    expect(isWorkflowRunResult(succeededFixture)).toBe(true);
    expect(isWorkflowRunResult(cancelledFixture)).toBe(true);
    expect(isWorkflowRunResult(failedFixture)).toBe(true);
    expect(isCancelWorkflowRunResult(cancelRequestedFixture)).toBe(true);
    expect(isCancelWorkflowRunResult(cancelNotActiveFixture)).toBe(true);
  });
});

function isWorkflowRunEvent(value: unknown): value is WorkflowRunEvent {
  if (!isRecord(value) || typeof value.run_id !== "string") return false;
  if (value.event === "started") return typeof value.project_id === "string";
  return value.event === "progress" && isNodeProgressEvent(value.node);
}

function isWorkflowRunResult(value: unknown): value is WorkflowRunResult {
  if (!isRecord(value) || typeof value.run_id !== "string") return false;
  if (value.status === "succeeded") return isRunOutputs(value.outputs);
  if (value.status === "cancelled") return true;
  return value.status === "failed" && typeof value.reason === "string";
}

function isCancelWorkflowRunResult(value: unknown): value is CancelWorkflowRunResult {
  return (
    isRecord(value) &&
    typeof value.run_id === "string" &&
    (value.status === "requested" || value.status === "not_active")
  );
}

function isNodeProgressEvent(value: unknown): value is NodeProgressEvent {
  return (
    isRecord(value) &&
    typeof value.node_id === "string" &&
    isNodeState(value.state) &&
    (value.progress === null || typeof value.progress === "number") &&
    (value.cost === null || typeof value.cost === "number")
  );
}

function isRunOutputs(value: unknown): boolean {
  return (
    isRecord(value) &&
    Object.values(value).every(
      (outputs) =>
        isRecord(outputs) &&
        Object.values(outputs).every(
          (output) =>
            isRecord(output) &&
            isRunOutputKind(output.kind) &&
            typeof output.value === "string",
        ),
    )
  );
}

function isNodeState(value: unknown): value is NodeProgressEvent["state"] {
  return ["idle", "running", "done", "cached", "error"].includes(
    value as NodeProgressEvent["state"],
  );
}

function isRunOutputKind(value: unknown): value is RunOutput["kind"] {
  return ["image", "video", "audio", "string", "model", "int", "float"].includes(
    value as RunOutput["kind"],
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
