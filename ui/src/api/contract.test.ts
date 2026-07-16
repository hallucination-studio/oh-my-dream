import { describe, expect, it } from "vitest";
import assetFixture from "../__fixtures__/asset.json";
import assistantConfigFixture from "../__fixtures__/assistant_config.json";
import assistantOperationsFixture from "../__fixtures__/assistant_operations.json";
import assistantApprovalFixture from "../__fixtures__/assistant_approval.json";
import capabilityCatalogFixture from "../__fixtures__/capability_catalog.json";
import progressFixture from "../__fixtures__/node_progress_event.json";
import openProjectFixture from "../__fixtures__/open_project.json";
import nodeCapabilitiesFixture from "../__fixtures__/node_capabilities.json";
import generationProfilesFixture from "../__fixtures__/generation_profiles.json";
import projectFixture from "../__fixtures__/project.json";
import workflowFixture from "../__fixtures__/workflow.json";
import workflowRunFixture from "../__fixtures__/workflow_run.json";
import workflowRunEventsFixture from "../__fixtures__/workflow_run_events.json";
import {
  fixtureFingerprint,
  hasAssistantOperationsShape,
  hasTrustedContextInModelInputs,
  isAssistantOperationsFixture,
} from "./assistantOperationContract.testHelpers.ts";
import { isCatalogEntry } from "./capabilityContractValidators.ts";
import type {
  AssetDto,
  AssistantConfig,
  AssistantApprovalDecisionInput,
  AssistantPendingApproval,
  AssistantOperationsFixture,
  CapabilityCatalog,
  OpenProjectResult,
  Project,
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
    expect(isNodeProgressEvent(progressFixture)).toBe(true);
    expect(isAssistantConfig(assistantConfigFixture)).toBe(true);
    expect(isCapabilityCatalog(capabilityCatalogFixture)).toBe(true);
    if (!isAssistantApprovalFixture(assistantApprovalFixture)) {
      throw new Error("assistant approval fixture does not match the DTO contract");
    }
    const approval: AssistantPendingApproval = assistantApprovalFixture.pending;
    const decision: AssistantApprovalDecisionInput = assistantApprovalFixture.decision;
    expect(approval.candidate_digest).toBe("sha256:candidate");
    expect(decision).toEqual({
      project_id: "project-1",
      approval_scope_id: "scope-1",
      candidate_digest: "sha256:candidate",
      approved: true,
    });
  });
});

function isAssistantApprovalFixture(value: unknown): value is {
  pending: AssistantPendingApproval;
  decision: AssistantApprovalDecisionInput;
} {
  if (!isRecord(value) || !isRecord(value.pending) || !isRecord(value.decision)) return false;
  const pending = value.pending;
  const decision = value.decision;
  return (
    typeof pending.project_id === "string" &&
    typeof pending.approval_scope_id === "string" &&
    typeof pending.user_intent === "string" &&
    typeof pending.candidate_digest === "string" &&
    typeof pending.reviewer_version === "string" &&
    typeof pending.evidence_hash === "string" &&
    typeof pending.review_summary === "string" &&
    isStringArray(pending.review_findings) &&
    pending.effect === "apply_reviewed_workflow_candidate" &&
    isRecord(pending.workflow) &&
    Array.isArray(pending.readiness_blockers) &&
    Array.isArray(pending.assets) && pending.assets.every((asset) =>
      isRecord(asset) && typeof asset.asset_id === "string" &&
      (asset.kind === "image" || asset.kind === "video" || asset.kind === "audio")
    ) &&
    typeof decision.project_id === "string" &&
    typeof decision.approval_scope_id === "string" &&
    typeof decision.candidate_digest === "string" &&
    typeof decision.approved === "boolean"
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
  it("matches the exact generated operation contract", () => {
    expect(hasAssistantOperationsShape(assistantOperationsFixture)).toBe(true);
    expect(isAssistantOperationsFixture(assistantOperationsFixture)).toBe(true);
    expect(isExactAssistantOperationsFixture(assistantOperationsFixture)).toBe(true);
  });

  it("includes bounded capability discovery operations", () => {
    const operationIds = assistantOperationsFixture.operations.map((operation) => operation.id);
    expect(operationIds).toContain("capability_search");
    expect(operationIds).toContain("capability_describe");
    const search = assistantOperationsFixture.operations.find(
      (operation) => operation.id === "capability_search",
    );
    const describe = assistantOperationsFixture.operations.find(
      (operation) => operation.id === "capability_describe",
    );
    expect(search?.input_schema.required).toEqual(["kinds", "query"]);
    expect(describe?.input_schema.properties?.refs).toMatchObject({ maxItems: 3 });
  });

  it("keeps workspace scope out of bounded snapshot model input", () => {
    const snapshot = assistantOperationsFixture.operations.find(
      (operation) => operation.id === "workspace_get_snapshot",
    );
    expect(snapshot?.input_schema.required).toBeUndefined();
    expect(snapshot?.input_schema.properties).toBeUndefined();
    expect(snapshot?.output_schema.properties?.assets).toMatchObject({ maxItems: 8 });
    expect(snapshot?.output_schema.properties?.runs).toMatchObject({ maxItems: 1 });
  });

  it("rejects a canonical model input field rename", () => {
    const fixture = cloneFixture();
    const schema = operationInputSchemaAt(fixture, 1);
    const properties = requiredRecord(schema.properties);
    const [canonicalName] = Object.keys(properties);
    if (canonicalName === undefined) {
      throw new Error("expected a model input property");
    }

    properties.renamed_field = properties[canonicalName];
    delete properties[canonicalName];
    schema.required = requiredStringArray(schema.required).map((name) =>
      name === canonicalName ? "renamed_field" : name,
    );

    expect(isAssistantOperationsFixture(fixture)).toBe(true);
    expect(fixtureFingerprint(fixture)).not.toBe(FROZEN_ASSISTANT_OPERATIONS_FINGERPRINT);
    expect(isExactAssistantOperationsFixture(fixture)).toBe(false);
  });

  it("rejects a removed required model input field", () => {
    const fixture = cloneFixture();
    const schema = operationInputSchemaAt(fixture, 1);
    const required = requiredStringArray(schema.required);

    schema.required = required.slice(0, -1);

    expect(isAssistantOperationsFixture(fixture)).toBe(true);
    expect(fixtureFingerprint(fixture)).not.toBe(FROZEN_ASSISTANT_OPERATIONS_FINGERPRINT);
    expect(isExactAssistantOperationsFixture(fixture)).toBe(false);
  });

  it("rejects operation metadata drift", () => {
    const fixture = cloneFixture();
    const [operation] = fixture.operations;
    if (operation === undefined) {
      throw new Error("expected an operation");
    }
    operation.description = "changed";

    expect(isAssistantOperationsFixture(fixture)).toBe(true);
    expect(fixtureFingerprint(fixture)).not.toBe(FROZEN_ASSISTANT_OPERATIONS_FINGERPRINT);
    expect(isExactAssistantOperationsFixture(fixture)).toBe(false);
  });

  it("rejects trusted context through the recursive safety validator", () => {
    const fixture = cloneFixture();
    const schema = operationInputSchemaAt(fixture, 2);
    const properties = requiredRecord(schema.properties);
    const [propertyName] = Object.keys(properties);
    if (propertyName === undefined) {
      throw new Error("expected a model input property");
    }
    const propertySchema = requiredRecord(properties[propertyName]);

    propertySchema.description = "project_id";
    expect(hasTrustedContextInModelInputs(fixture)).toBe(false);

    propertySchema.type = "object";
    propertySchema.properties = { project_id: { type: "string" } };
    propertySchema.required = ["project_id"];
    propertySchema.additionalProperties = false;

    expect(hasAssistantOperationsShape(fixture)).toBe(true);
    expect(hasTrustedContextInModelInputs(fixture)).toBe(true);
    expect(isAssistantOperationsFixture(fixture)).toBe(false);
  });

  it("ignores trusted names inside annotation instance data", () => {
    const fixture = cloneFixture();
    const schema = operationInputSchemaAt(fixture, 1);
    const properties = requiredRecord(schema.properties);
    const propertySchema = requiredRecord(properties[Object.keys(properties)[0] ?? ""]);

    propertySchema.default = {
      properties: { project_id: "instance value" },
      required: ["project_id"],
    };
    propertySchema.examples = [{ properties: { project_id: "example value" } }];

    expect(hasAssistantOperationsShape(fixture)).toBe(true);
    expect(hasTrustedContextInModelInputs(fixture)).toBe(false);
  });

  it("detects trusted fields beneath contains", () => {
    const fixture = cloneFixture();
    const schema = operationInputSchemaAt(fixture, 1);
    const properties = requiredRecord(schema.properties);
    const propertySchema = requiredRecord(properties[Object.keys(properties)[0] ?? ""]);

    propertySchema.contains = {
      type: "object",
      properties: { project_id: { type: "string" } },
      required: ["project_id"],
      additionalProperties: false,
    };

    expect(hasAssistantOperationsShape(fixture)).toBe(true);
    expect(hasTrustedContextInModelInputs(fixture)).toBe(true);
  });
});

// Freezes the generated fixture as an opaque artifact, not a semantic source.
const FROZEN_ASSISTANT_OPERATIONS_FINGERPRINT = "fnv1a64:a81c70af3a11db64";

function isExactAssistantOperationsFixture(value: unknown): value is AssistantOperationsFixture {
  return (
    isAssistantOperationsFixture(value) &&
    fixtureFingerprint(value) === FROZEN_ASSISTANT_OPERATIONS_FINGERPRINT
  );
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function cloneFixture(): AssistantOperationsFixture {
  return JSON.parse(JSON.stringify(assistantOperationsFixture)) as AssistantOperationsFixture;
}

function operationInputSchemaAt(value: unknown, index: number): Record<string, unknown> {
  const fixture = requiredRecord(value);
  const operations = fixture.operations;
  if (!Array.isArray(operations)) {
    throw new Error("fixture operations must be an array");
  }
  const operation = operations[index];
  return requiredRecord(requiredRecord(operation).input_schema);
}

function requiredStringArray(value: unknown): string[] {
  if (!isStringArray(value)) {
    throw new Error("expected a string array");
  }
  return value;
}

function requiredRecord(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error("expected an object");
  }
  return value;
}

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

function isAssistantConfig(value: unknown): value is AssistantConfig {
  return (
    isRecord(value) &&
    typeof value.enabled === "boolean" &&
    typeof value.base_url === "string" &&
    typeof value.model === "string" &&
    typeof value.has_key === "boolean"
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
