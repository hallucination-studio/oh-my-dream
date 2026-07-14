import { describe, expect, it } from "vitest";
import assetFixture from "../__fixtures__/asset.json";
import assistantConfigFixture from "../__fixtures__/assistant_config.json";
import assistantOperationsFixture from "../__fixtures__/assistant_operations.json";
import capabilityCatalogFixture from "../__fixtures__/capability_catalog.json";
import progressFixture from "../__fixtures__/node_progress_event.json";
import openProjectFixture from "../__fixtures__/open_project.json";
import projectFixture from "../__fixtures__/project.json";
import runWorkflowFixture from "../__fixtures__/run_workflow_result.json";
import {
  fixtureFingerprint,
  hasAssistantOperationsShape,
  hasTrustedContextInModelInputs,
  isAssistantOperationsFixture,
} from "./assistantOperationContract.testHelpers.ts";
import type {
  AssetDto,
  AssistantConfig,
  AssistantOperationsFixture,
  CapabilityCatalog,
  OpenProjectResult,
  Project,
} from "./types.ts";
import type { NodeProgressEvent, RunOutput, RunOutputs } from "../workflow/types.ts";

describe("backend DTO fixtures", () => {
  it("match the frontend DTO contracts", () => {
    expect(isRunOutputs(runWorkflowFixture.outputs)).toBe(true);
    expect(isAsset(assetFixture)).toBe(true);
    expect(isProject(projectFixture)).toBe(true);
    expect(isOpenProject(openProjectFixture)).toBe(true);
    expect(isNodeProgressEvent(progressFixture)).toBe(true);
    expect(isAssistantConfig(assistantConfigFixture)).toBe(true);
    expect(isCapabilityCatalog(capabilityCatalogFixture)).toBe(true);
  });
});

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
const FROZEN_ASSISTANT_OPERATIONS_FINGERPRINT = "fnv1a64:fdd3be6289822566";

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
    (value.seed === null || typeof value.seed === "string") &&
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

function isOpenProject(value: unknown): value is OpenProjectResult {
  if (!isRecord(value) || !isProject(value.project)) {
    return false;
  }
  return value.workflow_head === null || isWorkflowHead(value.workflow_head);
}

function isWorkflowHead(value: unknown): boolean {
  if (
    !isRecord(value) ||
    typeof value.project_id !== "string" ||
    typeof value.revision !== "number"
  ) {
    return false;
  }
  const workflow = value.workflow;
  return (
    isRecord(workflow) &&
    typeof workflow.version === "string" &&
    typeof workflow.project_id === "string" &&
    Array.isArray(workflow.nodes)
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

function isCatalogEntry(value: unknown): boolean {
  if (!isRecord(value) || !isCapabilityContract(value.contract)) return false;
  return isPresentation(value.presentation) && isCapabilityStatus(value.status);
}

function isCapabilityContract(value: unknown): boolean {
  return (
    isRecord(value) &&
    isCapabilityRef(value.reference) &&
    Array.isArray(value.inputs) &&
    value.inputs.every(isCapabilityPort) &&
    Array.isArray(value.outputs) &&
    value.outputs.every(isCapabilityPort) &&
    isRecord(value.params_schema) &&
    isRecord(value.default_params) &&
    Array.isArray(value.effects) &&
    value.effects.every((effect) => effect === "pure" || effect === "external")
  );
}

function isCapabilityRef(value: unknown): boolean {
  return isRecord(value) && typeof value.id === "string" && typeof value.version === "string";
}

function isCapabilityPort(value: unknown): boolean {
  return (
    isRecord(value) &&
    typeof value.name === "string" &&
    typeof value.port_type === "string" &&
    (value.cardinality === "one" || isRecord(value.cardinality)) &&
    typeof value.required === "boolean"
  );
}

function isPresentation(value: unknown): boolean {
  return (
    isRecord(value) &&
    typeof value.label === "string" &&
    typeof value.description === "string" &&
    typeof value.category === "string" &&
    isStringArray(value.search_terms)
  );
}

function isCapabilityStatus(value: unknown): boolean {
  return (
    isRecord(value) &&
    (value.availability === "available" ||
      value.availability === "unavailable" ||
      value.availability === "degraded") &&
    (value.reason === null || typeof value.reason === "string") &&
    (value.provider_health === null || typeof value.provider_health === "string") &&
    typeof value.status_revision === "number"
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
