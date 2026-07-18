import type {
  CapabilityBundle,
  CapabilityCardinality,
  CapabilityContract,
  CapabilityPresentation,
  CapabilityRef,
  CapabilitySelector,
  CapabilityStatus,
  CapabilitySummary,
  JsonValue,
} from "../api/types.ts";
import type { PortType } from "../workflow/types.ts";
import { parameterLabel } from "./parameterLabels.ts";

export interface PortSpec {
  name: string;
  type: PortType;
  cardinality: CapabilityCardinality;
  required: boolean;
}

export interface InputPortSpec extends PortSpec {
  required: boolean;
}

export interface ParamSpec {
  name: string;
  label: string;
  kind: "text" | "int" | "float" | "boolean" | "enum";
  nullable?: boolean;
  required?: boolean;
  default?: JsonValue;
  options?: JsonValue[];
  constraints?: ParamConstraints;
}

export interface ParamConstraints {
  minimum?: number;
  exclusiveMinimum?: number;
  maximum?: number;
  exclusiveMaximum?: number;
}

export interface NodeTypeSpec {
  selector: CapabilitySelector | null;
  ref: CapabilityRef;
  type: string;
  contractVersion: string;
  label: string;
  description: string;
  category: string;
  inputs: InputPortSpec[];
  outputs: PortSpec[];
  params: ParamSpec[];
  status: CapabilityStatus;
  contract: CapabilityContract | null;
  presentation: CapabilityPresentation | null;
  contextualCreationRoute: string | null;
}

/** Projects loaded bundles into the node shape consumed by React Flow. */
export function nodeSpecFromBundle(bundle: CapabilityBundle): NodeTypeSpec {
  const { selector, reference, contract, presentation, status } = bundle;
  return {
    selector,
    ref: reference,
    type: reference.id,
    contractVersion: reference.version,
    label: presentation?.label ?? `Unavailable ${reference.id}`,
    description: presentation?.description ?? status.reason ?? "Capability unavailable",
    category: presentation?.category ?? "Recovery",
    inputs: contract?.inputs.map(portSpec) ?? [],
    outputs: contract?.outputs.map(portSpec) ?? [],
    params: contract ? paramsFromContract(contract) : [],
    status,
    contract,
    presentation,
    contextualCreationRoute: contract?.contextual_creation?.route ?? null,
  };
}

/** Creates a stable recovery spec for an unknown or degraded persisted ref. */
export function recoveryNodeSpec(reference: CapabilityRef, reason: string): NodeTypeSpec {
  return nodeSpecFromBundle({
    selector: null,
    reference,
    contract: null,
    presentation: null,
    status: {
      availability: "degraded",
      reason,
      provider_health: null,
      status_revision: 0,
    },
  });
}

/** Projects whether the ordinary palette path can create this capability. */
export function paletteCreation(summary: CapabilitySummary): {
  canAdd: boolean;
  route: string | null;
} {
  // Every available capability can be added from the palette, including asset
  // nodes (they start empty and bind their asset through the Inspector picker).
  const route = summary.contextual_creation?.route ?? null;
  return { canAdd: summary.status.availability === "available", route };
}

/** Contextual capabilities are reached from their trusted route, not the generic palette. */
/** Builds canonical params for a mode while preserving only shared fields. */
export function paramsForMode(spec: NodeTypeSpec, current: Record<string, unknown>) {
  const params = Object.fromEntries(spec.params.map((param) => [
    param.name,
    Object.hasOwn(current, param.name) ? current[param.name] : param.default,
  ]));
  if (spec.selector) params.mode = spec.selector.mode;
  return params;
}

/** Groups loaded bundles by their non-authoritative presentation category. */
export function nodesByCategory(specs: NodeTypeSpec[]): { category: string; nodes: NodeTypeSpec[] }[] {
  const groups: { category: string; nodes: NodeTypeSpec[] }[] = [];
  for (const spec of specs) {
    let group = groups.find((candidate) => candidate.category === spec.category);
    if (!group) {
      group = { category: spec.category, nodes: [] };
      groups.push(group);
    }
    group.nodes.push(spec);
  }
  return groups;
}

/** Exact-match compatibility is the React projection of the engine rule. */
export function arePortTypesCompatible(from: PortType, to: PortType): boolean {
  return from === to;
}

/** Validates one generated output/input pair before it reaches the patch queue. */
export function canConnectPorts(
  source: NodeTypeSpec | undefined,
  sourceHandle: string | null | undefined,
  target: NodeTypeSpec | undefined,
  targetHandle: string | null | undefined,
): boolean {
  if (!source || !target || !sourceHandle || !targetHandle) return false;
  const output = source.outputs.find((port) => port.name === sourceHandle);
  const input = target.inputs.find((port) => port.name === targetHandle);
  return output !== undefined && input !== undefined && arePortTypesCompatible(output.type, input.type);
}

function portSpec(port: CapabilityContract["inputs"][number]): PortSpec {
  return {
    name: port.name,
    type: port.port_type,
    cardinality: port.cardinality,
    required: port.required,
  };
}

function paramsFromContract(contract: CapabilityContract): ParamSpec[] {
  const properties = contract.params_schema.properties;
  if (!isRecord(properties)) return [];
  return Object.entries(properties).flatMap(([name, rawSchema]) => {
    if (name === "mode") return [];
    const schema = isRecord(rawSchema) ? rawSchema : {};
    const options = enumValues(schema.enum);
    const kind = parameterKind(schema, options);
    if (!kind) return [];
    const required = Array.isArray(contract.params_schema.required) &&
      contract.params_schema.required.includes(name);
    const defaultValue = contract.default_params && Object.hasOwn(contract.default_params, name)
      ? contract.default_params[name]
      : Object.hasOwn(schema, "default") && isJsonValue(schema.default)
        ? schema.default
        : undefined;
    const spec: ParamSpec = {
      name,
      label: labelFor(name),
      kind,
      nullable: isNullable(schema, options),
      required,
      constraints: constraintsFromSchema(schema),
    };
    if (options) spec.options = options;
    if (defaultValue !== undefined) spec.default = defaultValue;
    return [spec];
  });
}

function parameterKind(
  schema: Record<string, unknown>,
  options: JsonValue[] | undefined,
): ParamSpec["kind"] | undefined {
  if (options) return "enum";
  const types = directSchemaTypes(schema);
  if (types.includes("string")) return "text";
  if (types.includes("integer")) return "int";
  if (types.includes("number")) return "float";
  if (types.includes("boolean")) return "boolean";
  return undefined;
}

function enumValues(value: unknown): JsonValue[] | undefined {
  return Array.isArray(value) && value.length > 0 && value.every(isJsonValue)
    ? (value as JsonValue[])
    : undefined;
}

function isNullable(schema: Record<string, unknown>, options: JsonValue[] | undefined): boolean {
  if (options?.some((option) => option === null)) return true;
  return directSchemaTypes(schema).includes("null");
}

function directSchemaTypes(schema: Record<string, unknown>): string[] {
  const types = schema.type;
  return Array.isArray(types)
    ? types.filter((type): type is string => typeof type === "string")
    : typeof types === "string"
      ? [types]
      : [];
}

function constraintsFromSchema(schema: Record<string, unknown>): ParamConstraints {
  const constraints: ParamConstraints = {};
  for (const key of ["minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum"] as const) {
    if (typeof schema[key] === "number" && Number.isFinite(schema[key])) {
      constraints[key] = schema[key];
    }
  }
  return constraints;
}

function labelFor(name: string): string {
  return parameterLabel(name);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isJsonValue(value: unknown): value is JsonValue {
  return value === null ||
    typeof value === "boolean" ||
    typeof value === "number" ||
    typeof value === "string" ||
    (Array.isArray(value) && value.every(isJsonValue)) ||
    (isRecord(value) && Object.values(value).every(isJsonValue));
}
