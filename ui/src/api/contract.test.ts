import { describe, expect, it } from "vitest";
import assetFixture from "../__fixtures__/asset.json";
import runWorkflowFixture from "../__fixtures__/run_workflow_result.json";
import type { Asset } from "./types.ts";
import type { RunOutput, RunOutputs } from "../workflow/types.ts";

describe("backend DTO fixtures", () => {
  it("match the frontend RunOutputs and Asset contracts", () => {
    expect(isRunOutputs(runWorkflowFixture.outputs)).toBe(true);
    expect(isAsset(assetFixture)).toBe(true);
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
    (value.kind === "image" || value.kind === "video") &&
    typeof value.file_path === "string" &&
    (value.thumbnail_path === null || typeof value.thumbnail_path === "string") &&
    typeof value.workflow_snapshot !== "undefined" &&
    (value.source_node_id === null || typeof value.source_node_id === "string") &&
    Array.isArray(value.tags) &&
    value.tags.every((tag) => typeof tag === "string") &&
    typeof value.created_at === "number"
  );
}

function isRunOutputKind(value: unknown): value is RunOutput["kind"] {
  return (
    value === "image" ||
    value === "video" ||
    value === "string" ||
    value === "model" ||
    value === "int" ||
    value === "float"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
