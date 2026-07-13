import { describe, expect, it } from "vitest";
import assetFixture from "../__fixtures__/asset.json";
import assistantConfigFixture from "../__fixtures__/assistant_config.json";
import assistantSessionFixture from "../__fixtures__/assistant_session.json";
import capabilityManifestFixture from "../__fixtures__/capability_manifest.json";
import progressFixture from "../__fixtures__/node_progress_event.json";
import projectFixture from "../__fixtures__/project.json";
import runWorkflowFixture from "../__fixtures__/run_workflow_result.json";
import skillFixture from "../__fixtures__/skill.json";
import type {
  AssetDto,
  AssistantConfig,
  AssistantSession,
  CapabilityManifest,
  Project,
  Skill,
} from "./types.ts";
import type { NodeProgressEvent, RunOutput, RunOutputs } from "../workflow/types.ts";

describe("backend DTO fixtures", () => {
  it("match the frontend DTO contracts", () => {
    expect(isRunOutputs(runWorkflowFixture.outputs)).toBe(true);
    expect(isAsset(assetFixture)).toBe(true);
    expect(isProject(projectFixture)).toBe(true);
    expect(isNodeProgressEvent(progressFixture)).toBe(true);
    expect(isAssistantConfig(assistantConfigFixture)).toBe(true);
    expect(isAssistantSession(assistantSessionFixture)).toBe(true);
    expect(isCapabilityManifest(capabilityManifestFixture)).toBe(true);
    expect(isSkill(skillFixture)).toBe(true);
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

function isAsset(value: unknown): value is AssetDto {
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

function isAssistantConfig(value: unknown): value is AssistantConfig {
  return (
    isRecord(value) &&
    typeof value.enabled === "boolean" &&
    typeof value.base_url === "string" &&
    typeof value.model === "string" &&
    typeof value.has_key === "boolean" &&
    typeof value.temperature === "number" &&
    typeof value.max_tool_iters === "number" &&
    (value.system_prompt_extra === null || typeof value.system_prompt_extra === "string") &&
    typeof value.developer_mode === "boolean" &&
    isRecord(value.skills) &&
    Array.isArray(value.skills.installed) &&
    Array.isArray(value.skills.enabled)
  );
}

function isAssistantSession(value: unknown): value is AssistantSession {
  return isRecord(value) && typeof value.port === "number" && typeof value.token === "string";
}

function isCapabilityManifest(value: unknown): value is CapabilityManifest {
  return isRecord(value) && Array.isArray(value.capabilities);
}

function isSkill(value: unknown): value is Skill {
  return (
    isRecord(value) &&
    typeof value.name === "string" &&
    typeof value.version === "string" &&
    typeof value.description === "string" &&
    typeof value.enabled === "boolean" &&
    typeof value.developer_mode_required === "boolean" &&
    typeof value.status === "string"
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
