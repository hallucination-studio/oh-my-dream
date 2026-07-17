import type { NodeCapabilityContractDto } from "../api/types.ts";
import type { NodeTypeSpec, ParamSpec, PortSpec } from "./catalog.ts";

export function nodeSpecFromExactContract(
  contract: NodeCapabilityContractDto,
): NodeTypeSpec {
  const presentation = presentationFor(contract.capability_ref.id);
  const inputs = contract.inputs.map((input) => ({
    name: input.key,
    type: portType(input.binding.data_type),
    cardinality: "one" as const,
    required: input.binding.kind === "required_single_value",
  }));
  const outputs = contract.outputs.map((output) => ({
    name: output.key,
    type: portType(output.data_type),
    cardinality: "one" as const,
    required: true,
  }));
  return {
    selector: null,
    ref: contract.capability_ref,
    type: contract.capability_ref.id,
    contractVersion: contract.capability_ref.version,
    label: presentation.label,
    description: presentation.description,
    category: presentation.category,
    inputs,
    outputs,
    params: contract.parameters.map(parameterSpec),
    status: {
      availability: "available",
      reason: null,
      provider_health: null,
      status_revision: 0,
    },
    contract: null,
    presentation,
    contextualCreationRoute:
      contract.capability_ref.id.endsWith(".read_asset") ? "asset_library" : null,
  };
}

function parameterSpec(
  parameter: NodeCapabilityContractDto["parameters"][number],
): ParamSpec {
  const constraint = parameter.constraint;
  const presence = parameter.presence;
  const defaultValue =
    presence.kind === "optional_with_default" && typeof presence.default === "object"
      ? parameterDefault(presence.default as Record<string, unknown>)
      : undefined;
  const kind = constraint.kind === "unsigned_integer_range"
    || constraint.kind === "unsigned_integer_allowed_values"
    ? "int"
    : constraint.kind === "choice_allowed_keys"
      ? "enum"
      : "text";
  return {
    name: parameter.key,
    label: parameter.key.replaceAll("_", " "),
    kind,
    required: presence.kind === "required",
    default: defaultValue,
    options: Array.isArray(constraint.values)
      ? constraint.values.map((value) =>
          kind === "int" ? Number(value) : value as string
        )
      : undefined,
    constraints: {
      minimum: numeric(constraint.minimum),
      maximum: numeric(constraint.maximum),
    },
  };
}

function parameterDefault(value: Record<string, unknown>) {
  if (value.kind === "unsigned_integer") return Number(value.value);
  if (value.kind === "generation_profile") return `${value.profile_id}@${value.version}`;
  if (value.kind === "managed_asset") return value.asset_id as string;
  return value.value as string | undefined;
}

function numeric(value: unknown): number | undefined {
  if (typeof value === "number") return value;
  if (typeof value === "string" && /^\d+$/.test(value)) return Number(value);
  return undefined;
}

function portType(value: unknown): PortSpec["type"] {
  return value === "text" ? "string" : value as PortSpec["type"];
}

export function presentationFor(id: string) {
  const values: Record<string, { label: string; category: string }> = {
    "text.provide_literal": { label: "Text", category: "Text" },
    "image.read_asset": { label: "Image Asset", category: "Assets" },
    "video.read_asset": { label: "Video Asset", category: "Assets" },
    "audio.read_asset": { label: "Audio Asset", category: "Assets" },
    "image.generate_from_text": { label: "Text to Image", category: "Generation" },
    "video.generate_from_image": { label: "Image to Video", category: "Generation" },
    "audio.synthesize_speech_from_text": { label: "Text to Speech", category: "Generation" },
  };
  const value = values[id] ?? { label: id, category: "Other" };
  return { ...value, description: value.label, search_terms: [id] };
}
