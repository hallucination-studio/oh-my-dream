import { describe, expect, it, vi } from "vitest";
import type {
  CapabilityBundle,
  CapabilitySearchPage,
  CapabilitySearchRequest,
} from "../api/types.ts";
import {
  CapabilityContractCache,
  capabilityRefKey,
  type CapabilityCacheApi,
} from "./contractCache.ts";

const prompt = { id: "TextPrompt", version: "1.0" };
const missing = { id: "TextPrompt", version: "9.9" };

describe("CapabilityContractCache", () => {
  it("uses an exact ref key and keeps immutable contract separate from status", async () => {
    const api = cacheApi([bundle(prompt), bundle(missing, true)]);
    const cache = new CapabilityContractCache(api);

    await cache.load([prompt]);
    const before = cache.get(prompt);
    cache.updateStatus(prompt, {
      availability: "unavailable",
      reason: "provider paused",
      provider_health: "down",
      status_revision: 4,
    });

    expect(capabilityRefKey(prompt)).not.toBe(capabilityRefKey({ id: "TextPrompt", version: "2.0" }));
    expect(cache.get(prompt)?.contract).toEqual(before?.contract);
    expect(cache.get(prompt)?.status.availability).toBe("unavailable");
    expect(cache.get(prompt)?.status.status_revision).toBe(4);
  });

  it("loads each exact bundle once and preserves a degraded unknown bundle", async () => {
    const api = cacheApi([bundle(prompt), bundle(missing, true)]);
    const cache = new CapabilityContractCache(api);

    await cache.load([prompt, missing]);
    await cache.load([prompt, missing]);

    expect(api.getCapabilityBundles).toHaveBeenCalledTimes(1);
    expect(cache.get(prompt)?.contract).not.toBeNull();
    expect(cache.get(missing)).toMatchObject({
      reference: missing,
      contract: null,
      presentation: null,
      status: { availability: "degraded" },
    });
  });

  it("deduplicates project refs and batches exact loads", async () => {
    const refs = Array.from({ length: 33 }, (_, index) => ({ id: `Capability${index}`, version: "1.0" }));
    const api = cacheApi(refs.map((ref) => bundle(ref)));
    const cache = new CapabilityContractCache(api);

    await cache.loadProject([...refs, refs[0]!]);

    expect(api.getCapabilityBundles).toHaveBeenCalledTimes(2);
    expect(api.getCapabilityBundles.mock.calls[0]?.[0]).toHaveLength(32);
    expect(api.getCapabilityBundles.mock.calls[1]?.[0]).toHaveLength(1);
  });

  it("records paged summaries without loading contracts", async () => {
    const request: CapabilitySearchRequest = { query: "video", cursor: null, limit: 1 };
    const page: CapabilitySearchPage = {
      capabilities: [{
        reference: prompt,
        presentation: presentation("Text Prompt"),
        status: status(),
      }],
      next_cursor: "1",
    };
    const api = cacheApi([], page);
    const cache = new CapabilityContractCache(api);

    await expect(cache.search(request)).resolves.toEqual(page);

    expect(api.searchCapabilities).toHaveBeenCalledWith(request);
    expect(cache.get(prompt)).toBeUndefined();
    expect(cache.snapshot().summaries).toEqual(page.capabilities);
  });

  it("replaces summaries when the palette query changes", async () => {
    const first: CapabilitySearchPage = {
      capabilities: [{ reference: prompt, presentation: presentation("Text Prompt"), status: status() }],
      next_cursor: null,
    };
    const secondRef = { id: "ImageToVideo", version: "1.0" };
    const second: CapabilitySearchPage = {
      capabilities: [{ reference: secondRef, presentation: presentation("Image to Video"), status: status() }],
      next_cursor: null,
    };
    const api = cacheApi([], first, second);
    const cache = new CapabilityContractCache(api);

    await cache.search({ query: "text", cursor: null });
    await cache.search({ query: "video", cursor: null });

    expect(cache.snapshot().summaries.map((summary) => summary.reference)).toEqual([secondRef]);
  });
});

type TestCacheApi = CapabilityCacheApi & {
  getCapabilityBundles: ReturnType<typeof vi.fn>;
  searchCapabilities: ReturnType<typeof vi.fn>;
};

function cacheApi(bundles: CapabilityBundle[], ...pages: CapabilitySearchPage[]): TestCacheApi {
  const getCapabilityBundles = vi.fn(async (refs: { id: string; version: string }[]) => ({
    capabilities: refs.map((ref) => bundles.find((candidate) => sameRef(candidate.reference, ref)) ?? bundle(ref, true)),
  }));
  const searchCapabilities = vi.fn(async () => pages.shift() ?? { capabilities: [], next_cursor: null });
  return {
    getCapabilityBundles,
    searchCapabilities,
  } as TestCacheApi;
}

function bundle(reference: { id: string; version: string }, degraded = false): CapabilityBundle {
  return {
    reference,
    contract: degraded ? null : {
      reference,
      inputs: [],
      outputs: [],
      params_schema: { type: "object", properties: {} },
      default_params: {},
      effects: ["pure"],
    },
    presentation: degraded ? null : presentation(reference.id),
    status: degraded ? {
      availability: "degraded",
      reason: "missing exact version",
      provider_health: null,
      status_revision: 0,
    } : status(),
  };
}

function presentation(label: string) {
  return { label, description: label, category: "input", search_terms: [label.toLowerCase()] };
}

function status() {
  return { availability: "available" as const, reason: null, provider_health: null, status_revision: 0 };
}

function sameRef(left: { id: string; version: string }, right: { id: string; version: string }) {
  return left.id === right.id && left.version === right.version;
}
