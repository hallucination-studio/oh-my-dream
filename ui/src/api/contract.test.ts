import { describe, expect, it } from "vitest";
import assetFixture from "../__fixtures__/asset.json";
import progressFixture from "../__fixtures__/node_progress_event.json";
import projectFixture from "../__fixtures__/project.json";
import runWorkflowFixture from "../__fixtures__/run_workflow_result.json";
import type { Asset, Project } from "./types.ts";
import type { NodeProgressEvent, RunOutput, RunOutputs } from "../workflow/types.ts";

describe("backend DTO fixtures", () => {
  it("match the frontend DTO contracts", () => {
    expect(isRunOutputs(runWorkflowFixture.outputs)).toBe(true);
    expect(isAsset(assetFixture)).toBe(true);
    expect(isProject(projectFixture)).toBe(true);
    expect(isNodeProgressEvent(progressFixture)).toBe(true);
  });
});

function isRunOutputs(value: unknown): value is RunOutputs {
  return isRecord(value) && Object.values(value).every(isNodeOutputs);
}

function isNodeOutputs(value: unknown): value is Record<string, RunOutput> {
  return isRecord(value) && Object.values(value).every(isRunOutput);
}

function isRunOutput(value: unknown): value is RunOutput {
  return (
    isRecord(value) &&
    isRunOutputKind(value.kind) &&
    typeof value.value === "string"
  );
}

function isAsset(value: unknown): value is Asset {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    (value.kind === "image" || value.kind === "video" || value.kind === "audio") &&
    typeof value.file_path === "string" &&
    (value.thumbnail_path === null || typeof value.thumbnail_path === "string") &&
    typeof value.workflow_snapshot !== "undefined" &&
    (value.prompt === null || typeof value.prompt === "string") &&
    (value.project_id === null || typeof value.project_id === "string") &&
    (value.project_name === null || typeof value.project_name === "string") &&
    (value.source_node_id === null || typeof value.source_node_id === "string") &&
    (value.source_node_type === null || typeof value.source_node_type === "string") &&
    (value.model === null || typeof value.model === "string") &&
    (value.seed === null || typeof value.seed === "number") &&
    (value.cost === null || typeof value.cost === "number") &&
    Array.isArray(value.tags) &&
    value.tags.every((tag) => typeof tag === "string") &&
    typeof value.created_at === "number"
  );
}

function isProject(value: unknown): value is Project {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.name === "string" &&
    typeof value.created_at === "number"
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

function isRunOutputKind(value: unknown): value is RunOutput["kind"] {
  return (
    value === "image" ||
    value === "video" ||
    value === "audio" ||
    value === "string" ||
    value === "model" ||
    value === "int" ||
    value === "float"
  );
}

function isNodeState(value: unknown): value is NodeProgressEvent["state"] {
  return (
    value === "idle" ||
    value === "running" ||
    value === "done" ||
    value === "cached" ||
    value === "error"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
