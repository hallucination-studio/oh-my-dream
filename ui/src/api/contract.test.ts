import { describe, expect, it } from "vitest";
import assetFixture from "../__fixtures__/asset.json";
import assistantOperationsFixture from "../__fixtures__/assistant_operations.json";
import assistantApprovalFixture from "../__fixtures__/assistant_approval.json";
import capabilityCatalogFixture from "../__fixtures__/capability_catalog.json";
import progressFixture from "../__fixtures__/node_progress_event.json";
import openProjectFixture from "../__fixtures__/open_project.json";
import nodeCapabilitiesFixture from "../__fixtures__/node_capabilities.json";
import generationProfilesFixture from "../__fixtures__/generation_profiles.json";
import generationProviderSettingsFixture from "../__fixtures__/generation_provider_settings.json";
import generationTaskFixture from "../__fixtures__/generation_task.json";
import generationTasksFixture from "../__fixtures__/generation_tasks.json";
import projectFixture from "../__fixtures__/project.json";
import workflowFixture from "../__fixtures__/workflow.json";
import workflowRunFixture from "../__fixtures__/workflow_run.json";
import workflowRunEventsFixture from "../__fixtures__/workflow_run_events.json";
import { isCatalogEntry } from "./capabilityContractValidators.ts";
import type {
  AssetDto,
  AssistantApprovalDecisionInput,
  AssistantPendingWorkflowChange,
  CapabilityCatalog,
  OpenProjectResult,
  Project,
  GenerationProviderSettingsDto,
  GenerationTaskDto,
  GenerationTaskListPageDto,
  GenerationTaskSummaryDto,
} from "./types.ts";
import type { NodeProgressEvent } from "../workflow/types.ts";

describe("backend DTO fixtures", () => {
  it("match the frontend DTO contracts", () => {
    expect(workflowFixture.workflow.revision).toBe("1");
    expect(workflowRunFixture.node_executions[0]?.state).toBe("pending");
    expect(workflowRunEventsFixture.events[0]?.sequence).toBe("1");
    expect(isAsset(assetFixture)).toBe(true);
    expect(isProject(projectFixture)).toBe(true);
    expect(isOpenProject(openProjectFixture)).toBe(true);
    expect(isNodeCapabilityList(nodeCapabilitiesFixture)).toBe(true);
    expect(isGenerationProfileList(generationProfilesFixture)).toBe(true);
    expect(isGenerationProviderSettings(generationProviderSettingsFixture)).toBe(true);
    expect(isGenerationTask(generationTaskFixture)).toBe(true);
    expect(isGenerationTaskListPage(generationTasksFixture)).toBe(true);
    expect(isNodeProgressEvent(progressFixture)).toBe(true);
    expect(isCapabilityCatalog(capabilityCatalogFixture)).toBe(true);
    if (!isAssistantApprovalFixture(assistantApprovalFixture)) {
      throw new Error("assistant approval fixture does not match the DTO contract");
    }
    const approval: AssistantPendingWorkflowChange = assistantApprovalFixture.pending;
    const decision: AssistantApprovalDecisionInput = assistantApprovalFixture.decision;
    expect(approval.mutation_digest_hex).toBe("00".repeat(32));
    expect(decision).toEqual({
      project_id: "10000000-0000-4000-8000-000000000001",
      workflow_change_id: "20000000-0000-4000-8000-000000000001",
      approval_scope_id: "30000000-0000-4000-8000-000000000001",
      mutation_digest_hex: "00".repeat(32),
      decision: "approve",
    });
  });
});

function isAssistantApprovalFixture(value: unknown): value is {
  pending: AssistantPendingWorkflowChange;
  decision: AssistantApprovalDecisionInput;
} {
  if (!isRecord(value) || !isRecord(value.pending) || !isRecord(value.decision)) return false;
  const pending = value.pending;
  const decision = value.decision;
  return (
    typeof pending.project_id === "string" &&
    typeof pending.workflow_change_id === "string" &&
    typeof pending.approval_scope_id === "string" &&
    typeof pending.mutation_digest_hex === "string" &&
    pending.state === "awaiting_approval" &&
    isRecord(pending.lineage) &&
    Array.isArray(pending.mutations) &&
    Array.isArray(pending.readiness_issues) &&
    typeof decision.project_id === "string" &&
    typeof decision.workflow_change_id === "string" &&
    typeof decision.approval_scope_id === "string" &&
    typeof decision.mutation_digest_hex === "string" &&
    (decision.decision === "approve" || decision.decision === "reject")
  );
}

function isGenerationTask(value: unknown): value is GenerationTaskDto {
  if (!isRecord(value) || !(value.result === null || isGenerationTaskResult(value.result))) {
    return false;
  }
  return isGenerationTaskSummary(value);
}

function isGenerationTaskListPage(value: unknown): value is GenerationTaskListPageDto {
  return isRecord(value) && Array.isArray(value.tasks) && value.tasks.every(isGenerationTaskSummary)
    && (value.next_cursor === null || typeof value.next_cursor === "string");
}

function isGenerationTaskSummary(value: unknown): value is GenerationTaskSummaryDto {
  if (!isRecord(value)) return false;
  const encoded = JSON.stringify(value);
  if (/route_id|remote_task_id|credential|signed_url|raw_payload|native_model/.test(encoded)) return false;
  return (
    typeof value.id === "string" &&
    typeof value.project_id === "string" &&
    typeof value.workflow_id === "string" &&
    typeof value.workflow_run_id === "string" &&
    typeof value.workflow_node_id === "string" &&
    typeof value.workflow_node_execution_id === "string" &&
    ["text", "image", "video", "voice"].includes(String(value.request_kind)) &&
    ["queued", "running", "cancel_requested", "succeeded", "failed", "cancelled"].includes(String(value.status)) &&
    (value.progress_percent === null || (typeof value.progress_percent === "number" && value.progress_percent >= 0 && value.progress_percent <= 100)) &&
    typeof value.generation_profile_ref === "string" &&
    typeof value.provider_id === "string" &&
    (value.provider_display_name === null || typeof value.provider_display_name === "string") &&
    (value.prompt_preview === null || typeof value.prompt_preview === "string") &&
    (value.preview_asset_id === null || typeof value.preview_asset_id === "string") &&
    typeof value.has_result === "boolean" &&
    (value.failure === null || isGenerationTaskFailure(value.failure)) &&
    typeof value.created_at_epoch_ms === "string" &&
    typeof value.updated_at_epoch_ms === "string" &&
    (value.completed_at_epoch_ms === null || typeof value.completed_at_epoch_ms === "string")
  );
}

function isGenerationTaskFailure(value: unknown): boolean {
  return isRecord(value) && typeof value.kind === "string" && typeof value.code === "string"
    && typeof value.message === "string";
}

function isGenerationTaskResult(value: unknown): boolean {
  if (!isRecord(value) || typeof value.kind !== "string") return false;
  if (value.kind === "text") return typeof value.content === "string";
  return value.kind === "asset" && typeof value.asset_id === "string" && typeof value.media_kind === "string";
}

function isGenerationProviderSettings(value: unknown): value is GenerationProviderSettingsDto {
  if (!isRecord(value) || typeof value.settings_revision !== "string" || !Array.isArray(value.profiles)) {
    return false;
  }
  const encoded = JSON.stringify(value);
  if (/credential|account|endpoint|native_model|remote_task|supports_/.test(encoded)) return false;
  return value.profiles.length === 3 && value.profiles.every((profile) =>
    isRecord(profile) &&
    typeof profile.profile_ref === "string" &&
    ["text", "image", "video", "voice"].includes(String(profile.generation_kind)) &&
    (profile.selected_binding === null ||
      (isRecord(profile.selected_binding) &&
        typeof profile.selected_binding.provider_id === "string" &&
        typeof profile.selected_binding.route_id === "string")) &&
    Array.isArray(profile.provider_choices) &&
    profile.provider_choices.every((provider) =>
      isRecord(provider) &&
      typeof provider.provider_id === "string" &&
      typeof provider.display_name === "string" &&
      Array.isArray(provider.routes) &&
      provider.routes.length > 0 &&
      provider.routes.every((route) =>
        isRecord(route) && typeof route.route_id === "string" && typeof route.display_name === "string"
      )
    )
  );
}

function isNodeCapabilityList(value: unknown): boolean {
  return Array.isArray(value) && value.length === 7 && value.every((contract) =>
    isRecord(contract) &&
    isRecord(contract.capability_ref) &&
    typeof contract.capability_ref.id === "string" &&
    typeof contract.capability_ref.version === "string" &&
    Array.isArray(contract.parameters) &&
    Array.isArray(contract.inputs) &&
    Array.isArray(contract.outputs) &&
    typeof contract.execution_kind === "string"
  );
}

function isGenerationProfileList(value: unknown): boolean {
  return Array.isArray(value) && value.every((profile) =>
    isRecord(profile) &&
    typeof profile.profile_ref === "string" &&
    typeof profile.display_name === "string" &&
    isRecord(profile.availability) &&
    typeof profile.availability.state === "string" &&
    typeof profile.availability.observed_at_epoch_ms === "string" &&
    typeof profile.availability.expires_at_epoch_ms === "string"
  );
}

describe("assistant operation contract fixture", () => {
  it("contains exactly the eleven versioned Rust-owned tools", () => {
    const operationIds = assistantOperationsFixture.operations.map((operation) => operation.id);
    expect(operationIds).toHaveLength(11);
    expect(new Set(operationIds).size).toBe(11);
    expect(operationIds.every((id) => /^assistant\.[a-z_]+(?:\.[a-z_]+)*@1$/.test(id))).toBe(true);
    expect(operationIds).toContain("assistant.workspace.get_snapshot@1");
    expect(operationIds).toContain("assistant.workflow.request_apply@1");
  });

  it("keeps trusted context out of every model input schema", () => {
    const encoded = JSON.stringify(
      assistantOperationsFixture.operations.map((operation) => operation.input_schema),
    );
    expect(encoded).not.toMatch(/project_id|session_id|approval_scope_id|invocation_id/);
  });
});

function isAsset(value: unknown): value is AssetDto {
  return (
    isRecord(value) &&
    typeof value.asset_id === "string" &&
    typeof value.project_id === "string" &&
    (value.media_kind === "image" || value.media_kind === "video" || value.media_kind === "audio") &&
    (value.content_state === "pending" ||
      value.content_state === "available" ||
      value.content_state === "missing") &&
    typeof value.display_name === "string" &&
    typeof value.created_at_epoch_ms === "string" &&
    isRecord(value.content) &&
    typeof value.content.content_fingerprint_hex === "string" &&
    typeof value.content.byte_length === "string" &&
    typeof value.content.mime_type === "string" &&
    isRecord(value.media_facts) &&
    isRecord(value.origin)
  );
}

function isProject(value: unknown): value is Project {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.name === "string" &&
    typeof value.revision === "string" &&
    typeof value.created_at_epoch_ms === "string" &&
    typeof value.updated_at_epoch_ms === "string"
  );
}

function isOpenProject(value: unknown): value is OpenProjectResult {
  if (!isRecord(value) || !isProject(value.project)) {
    return false;
  }
  const summary = value.current_workflow_summary;
  return (
    summary === null ||
    (isRecord(summary) &&
      typeof summary.workflow_id === "string" &&
      typeof summary.workflow_revision === "string" &&
      (summary.readiness === "ready" || summary.readiness === "blocked"))
  );
}

function isCapabilityCatalog(value: unknown): value is CapabilityCatalog {
  return isRecord(value) && Array.isArray(value.capabilities) && value.capabilities.every(isCatalogEntry);
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
