import type { CapabilityRef } from "../api/index.ts";

export function parseCapabilityRef(encoded: string): string | CapabilityRef {
  try {
    const value: unknown = JSON.parse(encoded);
    if (
      typeof value === "object" &&
      value !== null &&
      "id" in value &&
      "version" in value &&
      typeof value.id === "string" &&
      typeof value.version === "string"
    ) {
      return { id: value.id, version: value.version };
    }
  } catch {
    // Older drag payloads contained only the type id.
  }
  return encoded;
}
