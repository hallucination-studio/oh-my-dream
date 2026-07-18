import type {
  CapabilityStatus,
  GenerationProfileForCapability,
  NodeCapabilityContractDto,
} from "../api/types.ts";
import type { NodeTypeSpec, ParamSpec, PortSpec } from "./catalog.ts";
import { parameterLabel } from "./parameterLabels.ts";

export function nodeSpecFromExactContract(
  contract: NodeCapabilityContractDto,
  status: CapabilityStatus = availableStatus(),
  defaultProfileRef: string | null = null,
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
  const params = contract.parameters.map(parameterSpec);
  if (defaultProfileRef !== null) {
    const profileParameter = params.find((parameter) => parameter.name === "generation_profile_ref");
    if (profileParameter && profileParameter.default === undefined) {
      profileParameter.default = defaultProfileRef;
    }
  }
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
    params,
    status,
    contract: null,
    presentation,
    contextualCreationRoute:
      contract.capability_ref.id.endsWith(".read_asset") ? "asset_library" : null,
  };
}

export type ProfileQueryResult =
  | { state: "loading" }
  | { state: "ready"; profiles: readonly GenerationProfileForCapability[] }
  | { state: "error" };

export interface CapabilityProfileProjection {
  status: CapabilityStatus;
  hasCompatibleProfile: boolean;
  defaultProfileRef: string | null;
}

export function capabilityKey(reference: { id: string; version: string }): string {
  return `${reference.id}@${reference.version}`;
}

export function isProfileBackedCapability(contract: NodeCapabilityContractDto): boolean {
  return contract.parameters.some(
    (parameter) => parameter.constraint.kind === "generation_profile_ref",
  );
}

export function profileProjectionForCapability(
  contract: NodeCapabilityContractDto,
  result: ProfileQueryResult,
): CapabilityProfileProjection {
  if (!isProfileBackedCapability(contract)) {
    return { status: availableStatus(), hasCompatibleProfile: true, defaultProfileRef: null };
  }
  if (result.state === "loading") {
    return {
      status: degradedStatus("Checking generation model availability"),
      hasCompatibleProfile: true,
      defaultProfileRef: null,
    };
  }
  if (result.state === "error") {
    return {
      status: degradedStatus("Generation model availability is indeterminate"),
      hasCompatibleProfile: true,
      defaultProfileRef: null,
    };
  }
  if (result.profiles.length === 0) {
    return {
      status: unavailableStatus("No generation model supports this node type"),
      hasCompatibleProfile: false,
      defaultProfileRef: null,
    };
  }
  const available = result.profiles.some((profile) => profile.availability.state === "available");
  if (available) {
    return {
      status: availableStatus(),
      hasCompatibleProfile: true,
      defaultProfileRef: result.profiles.length === 1 ? result.profiles[0]!.profile_ref : null,
    };
  }

  const indeterminate = result.profiles.find(
    (profile) => profile.availability.state === "indeterminate",
  );
  return {
    status: indeterminate
      ? degradedStatus(
          indeterminate.availability.reason ?? "Generation model availability is indeterminate",
        )
      : unavailableStatus(reasonForUnavailableProfile(result.profiles)),
    hasCompatibleProfile: true,
    defaultProfileRef: null,
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
    label: parameterLabel(parameter.key),
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
  const values: Record<string, { label: string; category: string; aliases: string[] }> = {
    "text.provide_literal": {
      label: "Text",
      category: "Inputs",
      aliases: ["prompt", "literal", "string"],
    },
    "image.read_asset": {
      label: "Image asset",
      category: "Assets",
      aliases: ["picture", "photo"],
    },
    "video.read_asset": {
      label: "Video asset",
      category: "Assets",
      aliases: ["clip", "movie"],
    },
    "audio.read_asset": {
      label: "Audio asset",
      category: "Assets",
      aliases: ["sound", "music", "voice"],
    },
    "image.generate_from_text": {
      label: "Generate image",
      category: "Generate",
      aliases: ["text to image", "t2i", "picture", "photo"],
    },
    "video.generate_from_image": {
      label: "Generate video",
      category: "Generate",
      aliases: ["image to video", "i2v", "clip", "animate"],
    },
    "audio.synthesize_speech_from_text": {
      label: "Generate speech",
      category: "Generate",
      aliases: ["text to speech", "tts", "voice", "narration"],
    },
  };
  const value = values[id] ?? { label: id, category: "Other", aliases: [] };
  return { label: value.label, category: value.category, description: value.label, search_terms: [id, ...value.aliases] };
}

function availableStatus(): CapabilityStatus {
  return { availability: "available", reason: null, provider_health: null, status_revision: 0 };
}

function unavailableStatus(reason: string): CapabilityStatus {
  return { availability: "unavailable", reason, provider_health: null, status_revision: 0 };
}

function degradedStatus(reason: string): CapabilityStatus {
  return { availability: "degraded", reason, provider_health: null, status_revision: 0 };
}

function reasonForUnavailableProfile(
  profiles: readonly GenerationProfileForCapability[],
): string {
  return profiles.find((profile) => profile.availability.reason)?.availability.reason
    ?? "Generation model is unavailable";
}
