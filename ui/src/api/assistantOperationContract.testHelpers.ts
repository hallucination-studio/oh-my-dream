import type {
  AssistantOperationContract,
  AssistantOperationsFixture,
} from "./types.ts";

const OPERATION_KEYS = [
  "description",
  "effect",
  "id",
  "input_schema",
  "needs_approval",
  "output_schema",
  "strict_json_schema",
  "version",
] as const;

const TRUSTED_CONTEXT_NAMES = new Set([
  "project_id",
  "session_id",
  "request_id",
  "tool_version",
  "approved_effect",
]);

export function hasAssistantOperationsShape(
  value: unknown,
): value is AssistantOperationsFixture {
  if (!hasExactKeys(value, ["operations"])) {
    return false;
  }
  return Array.isArray(value.operations) && value.operations.every(isAssistantOperationShape);
}

export function isAssistantOperationsFixture(
  value: unknown,
): value is AssistantOperationsFixture {
  return hasAssistantOperationsShape(value) && !hasTrustedContextInModelInputs(value);
}

export function hasTrustedContextInModelInputs(value: unknown): boolean {
  if (!hasAssistantOperationsShape(value)) {
    return false;
  }
  return value.operations.some((operation) => schemaExposesTrustedContext(operation.input_schema));
}

export function fixtureFingerprint(value: unknown): string {
  const bytes = new TextEncoder().encode(stableSerialize(value));
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return `fnv1a64:${hash.toString(16).padStart(16, "0")}`;
}

function isAssistantOperationShape(value: unknown): value is AssistantOperationContract {
  if (!hasExactKeys(value, OPERATION_KEYS)) {
    return false;
  }
  return (
    typeof value.id === "string" &&
    Number.isInteger(value.version) &&
    typeof value.description === "string" &&
    isAssistantOperationEffect(value.effect) &&
    typeof value.strict_json_schema === "boolean" &&
    typeof value.needs_approval === "boolean" &&
    isJsonSchemaObject(value.input_schema) &&
    isJsonSchemaObject(value.output_schema)
  );
}

function isAssistantOperationEffect(
  value: unknown,
): value is AssistantOperationContract["effect"] {
  return (
    value === "local_read" ||
    value === "visible_reversible_workflow_patch" ||
    value === "prepared_approval_execution"
  );
}

function isJsonSchemaObject(
  value: unknown,
): value is AssistantOperationContract["input_schema"] {
  return (
    isJsonObject(value) &&
    value.type === "object" &&
    isJsonObject(value.properties) &&
    isStringArray(value.required) &&
    typeof value.additionalProperties === "boolean"
  );
}

function schemaExposesTrustedContext(value: unknown): boolean {
  if (Array.isArray(value)) {
    return value.some(schemaExposesTrustedContext);
  }
  if (!isRecord(value)) {
    return false;
  }

  const properties = value.properties;
  if (isRecord(properties) && Object.keys(properties).some(isTrustedContextName)) {
    return true;
  }
  if (isRecord(properties) && Object.values(properties).some(schemaExposesTrustedContext)) {
    return true;
  }
  const required = value.required;
  if (isStringArray(required) && required.some(isTrustedContextName)) {
    return true;
  }

  return (
    schemaExposesTrustedContext(value.items) ||
    schemaExposesTrustedContext(value.contains) ||
    schemaExposesTrustedContext(value.allOf) ||
    schemaExposesTrustedContext(value.anyOf) ||
    schemaExposesTrustedContext(value.oneOf) ||
    schemaMapExposesTrustedContext(value.definitions) ||
    schemaMapExposesTrustedContext(value.$defs)
  );
}

function schemaMapExposesTrustedContext(value: unknown): boolean {
  return isRecord(value) && Object.values(value).some(schemaExposesTrustedContext);
}

function isTrustedContextName(value: string): boolean {
  return TRUSTED_CONTEXT_NAMES.has(value);
}

function stableSerialize(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableSerialize).join(",")}]`;
  }
  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableSerialize(value[key])}`)
      .join(",")}}`;
  }
  throw new Error("fixture fingerprint requires JSON data");
}

function isJsonObject(value: unknown): value is AssistantOperationContract["input_schema"] {
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isJsonValue(value: unknown): boolean {
  return (
    value === null ||
    typeof value === "boolean" ||
    typeof value === "number" ||
    typeof value === "string" ||
    (Array.isArray(value) && value.every(isJsonValue)) ||
    isJsonObject(value)
  );
}

function hasExactKeys(
  value: unknown,
  expectedKeys: readonly string[],
): value is Record<string, unknown> {
  return isRecord(value) && sameNames(Object.keys(value), expectedKeys);
}

function sameNames(actual: readonly string[], expected: readonly string[]): boolean {
  const sortedExpected = [...expected].sort();
  return actual.length === expected.length &&
    [...actual].sort().every((name, index) => name === sortedExpected[index]);
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
