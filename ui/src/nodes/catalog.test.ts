import { describe, expect, it } from "vitest";
import catalogFixture from "../__fixtures__/capability_catalog.json";
import type { CapabilityCatalog } from "../api/types.ts";
import { capabilityRefKey } from "../workflow/contractCache.ts";
import {
  arePortTypesCompatible,
  findNodeType,
  nodeSpecFromBundle,
  nodeSpecsFromSnapshot,
  paletteCreation,
  paramsForMode,
  recoveryNodeSpec,
} from "./catalog.ts";

const catalog = catalogFixture as unknown as CapabilityCatalog;

describe("node catalog contract", () => {
  it("projects ports and params from exact Rust capability contracts", () => {
    const specs = nodeSpecsFromSnapshot(snapshot());
    const image = specs.find((spec) => spec.type === "TextToImage");

    expect(image?.inputs).toEqual([
      expect.objectContaining({ name: "prompt", type: "string", required: true, cardinality: "one" }),
    ]);
    expect(image?.outputs).toEqual([
      expect.objectContaining({ name: "image", type: "image", required: false, cardinality: "one" }),
    ]);
    expect(image?.params.map((param) => param.name)).toEqual([
      "model",
      "negative_prompt",
      "seed",
      "steps",
    ]);
  });

  it("uses the exact reference version when finding a node", () => {
    const current = snapshot();

    expect(findNodeType("Text", "1.0", { mode: "literal" }, current)?.ref).toEqual({ id: "TextPrompt", version: "1.0" });
    expect(findNodeType("Text", "9.9", { mode: "literal" }, current)).toBeUndefined();
  });

  it("rebuilds params from the selected mode contract", () => {
    const current = { mode: "image", duration: 4, model: "video-model", unknown: true };
    const concat = nodeSpecsFromSnapshot(snapshot()).find((spec) => spec.ref.id === "VideoConcat");
    if (!concat) throw new Error("missing concat spec");

    expect(paramsForMode(concat, current)).toEqual({ mode: "concat" });
  });

  it("keeps missing refs as recovery specs instead of dropping nodes", () => {
    const spec = recoveryNodeSpec({ id: "TextPrompt", version: "9.9" }, "repair required");

    expect(spec.status.availability).toBe("degraded");
    expect(spec.ref).toEqual({ id: "TextPrompt", version: "9.9" });
    expect(spec.inputs).toEqual([]);
    expect(spec.outputs).toEqual([]);
  });

  it("uses the engine's exact-match compatibility rule", () => {
    expect(arePortTypesCompatible("image", "image")).toBe(true);
    expect(arePortTypesCompatible("image", "string")).toBe(false);
  });

  it("ignores non-contract annotations and unsupported composed parameter schemas", () => {
    const spec = nodeSpecFromBundle({
      selector: { type_id: "Fixture", mode: "test" },
      reference: { id: "Fixture", version: "1.0" },
      contract: {
        reference: { id: "Fixture", version: "1.0" },
        inputs: [],
        outputs: [],
        params_schema: {
          type: "object",
          properties: {
            caption: {
              type: "string",
              format: "asset",
              "x-asset": true,
              "x-kind": "asset",
            },
            ambiguous: { anyOf: [{ type: "string" }, { type: "number" }] },
            steps: { type: "integer", minimum: 1 },
          },
        },
        default_params: {},
        contextual_creation: null,
        effects: [],
      },
      presentation: {
        label: "Fixture",
        description: "Fixture capability",
        category: "test",
        search_terms: [],
      },
      status: {
        availability: "available",
        reason: null,
        provider_health: null,
        status_revision: 0,
      },
    });

    expect(spec.params).toEqual([
      expect.objectContaining({ name: "caption", kind: "text" }),
      expect.objectContaining({ name: "steps", kind: "int" }),
    ]);
  });

  it("projects contextual creation without inventing default params", () => {
    const spec = nodeSpecFromBundle({
      selector: { type_id: "Image", mode: "asset" },
      reference: { id: "ImageAssetSource", version: "1.0" },
      contract: {
        reference: { id: "ImageAssetSource", version: "1.0" },
        inputs: [],
        outputs: [],
        params_schema: { type: "object" },
        default_params: null,
        contextual_creation: { route: "asset_library" },
        effects: ["local_read"],
      },
      presentation: {
        label: "Image Asset",
        description: "Managed image",
        category: "Assets",
        search_terms: [],
      },
      status: {
        availability: "available",
        reason: null,
        provider_health: null,
        status_revision: 0,
      },
    });

    expect(spec.contextualCreationRoute).toBe("asset_library");
    expect(spec.params).toEqual([]);
    expect(paletteCreation({
      selector: { type_id: "Image", mode: "asset" },
      reference: { id: "ImageAssetSource", version: "1.0" },
      presentation: {
        label: "Image Asset",
        description: "Managed image",
        category: "Assets",
        search_terms: [],
      },
      contextual_creation: { route: "asset_library" },
      status: {
        availability: "available",
        reason: null,
        provider_health: null,
        status_revision: 0,
      },
    })).toEqual({ canAdd: false, route: "asset_library" });
  });
});

function snapshot() {
  const bundles = new Map(
    catalog.capabilities.map((entry) => [
      capabilityRefKey(entry.contract.reference),
      {
        selector: entry.selector,
        reference: entry.contract.reference,
        contract: entry.contract,
        presentation: entry.presentation,
        status: entry.status,
      },
    ]),
  );
  return {
    bundles,
    summaries: catalog.capabilities.map(({ selector, contract, presentation, status }) => ({
      selector,
      reference: contract.reference,
      presentation,
      contextual_creation: contract.contextual_creation,
      status,
    })),
  };
}
