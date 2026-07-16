export function isCatalogEntry(value: unknown): boolean {
  if (!isRecord(value) || !isCapabilityContract(value.contract)) return false;
  return isCapabilitySelector(value.selector) &&
    isPresentation(value.presentation) &&
    isCapabilityStatus(value.status);
}

function isCapabilitySelector(value: unknown): boolean {
  return isRecord(value) && typeof value.type_id === "string" && typeof value.mode === "string";
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
    (value.default_params === null || isRecord(value.default_params)) &&
    (value.contextual_creation === null || isContextualCreation(value.contextual_creation)) &&
    Array.isArray(value.effects) &&
    value.effects.every(
      (effect) => effect === "pure" || effect === "local_read" || effect === "external",
    )
  );
}

function isContextualCreation(value: unknown): boolean {
  return isRecord(value) && typeof value.route === "string";
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
    Array.isArray(value.search_terms) &&
    value.search_terms.every((item) => typeof item === "string")
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
