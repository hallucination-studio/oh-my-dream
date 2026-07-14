import { useSyncExternalStore } from "react";
import type {
  CapabilityBundle,
  CapabilityBundles,
  CapabilityContract,
  CapabilityPresentation,
  CapabilityRef,
  CapabilitySearchPage,
  CapabilitySearchRequest,
  CapabilitySummary,
  CapabilityStatus,
} from "../api/types.ts";

const MAX_BUNDLE_BATCH = 32;

export interface CapabilityCacheApi {
  getCapabilityBundles: (refs: CapabilityRef[]) => Promise<CapabilityBundles>;
  searchCapabilities: (request: CapabilitySearchRequest) => Promise<CapabilitySearchPage>;
}

export interface CapabilityCacheSnapshot {
  bundles: ReadonlyMap<string, CapabilityBundle>;
  summaries: readonly CapabilitySummary[];
}

/**
 * Ref-keyed cache for the three capability projections consumed by React.
 * Contracts are immutable for an exact ref; presentation and status may be
 * refreshed independently without changing graph identity.
 */
export class CapabilityContractCache {
  private readonly contracts = new Map<string, CapabilityContract | null>();
  private readonly presentations = new Map<string, CapabilityPresentation | null>();
  private readonly statuses = new Map<string, CapabilityStatus>();
  private readonly references = new Map<string, CapabilityRef>();
  private readonly summaries = new Map<string, CapabilitySummary>();
  private readonly listeners = new Set<() => void>();
  private searchGeneration = 0;
  private activeSearchKey = "";
  private currentSnapshot: CapabilityCacheSnapshot = {
    bundles: new Map(),
    summaries: [],
  };

  constructor(private readonly api: CapabilityCacheApi) {}

  /** Returns a collision-safe key for one exact `{id, version}` reference. */
  getKey(reference: CapabilityRef): string {
    return capabilityRefKey(reference);
  }

  /** Returns the loaded bundle, or `undefined` when it has not been requested. */
  get(reference: CapabilityRef): CapabilityBundle | undefined {
    const key = capabilityRefKey(reference);
    const status = this.statuses.get(key);
    if (!status) return undefined;
    return {
      reference: this.references.get(key) ?? reference,
      contract: this.contracts.get(key) ?? null,
      presentation: this.presentations.get(key) ?? null,
      status,
    };
  }

  /** Loads exact refs, retaining degraded placeholders returned by the boundary. */
  async load(references: CapabilityRef[]): Promise<void> {
    const missing = uniqueReferences(references).filter((reference) => !this.get(reference));
    if (missing.length === 0) return;
    const result = await this.api.getCapabilityBundles(missing);
    for (const reference of missing) {
      const bundle = result.capabilities.find((candidate) => sameRef(candidate.reference, reference));
      this.store(reference, bundle ?? degradedBundle(reference, "capability bundle was omitted"));
    }
    this.publish();
  }

  /** Loads a bounded project-open batch in sequential request-sized chunks. */
  async loadProject(references: CapabilityRef[]): Promise<void> {
    const unique = uniqueReferences(references);
    for (let start = 0; start < unique.length; start += MAX_BUNDLE_BATCH) {
      await this.load(unique.slice(start, start + MAX_BUNDLE_BATCH));
    }
  }

  /** Searches the server-side paged palette without loading any contract body. */
  async search(request: CapabilitySearchRequest): Promise<CapabilitySearchPage> {
    const generation = ++this.searchGeneration;
    const searchKey = `${request.query.trim().toLowerCase()}|${request.category?.trim().toLowerCase() ?? ""}`;
    const append = request.cursor !== null && request.cursor !== undefined && searchKey === this.activeSearchKey;
    const page = await this.api.searchCapabilities(request);
    if (generation !== this.searchGeneration) return page;
    this.activeSearchKey = searchKey;
    if (!append) this.summaries.clear();
    for (const summary of page.capabilities) {
      const key = capabilityRefKey(summary.reference);
      this.references.set(key, summary.reference);
      this.summaries.set(key, summary);
    }
    this.publish();
    return page;
  }

  /** Refreshes mutable status while preserving the exact contract projection. */
  updateStatus(reference: CapabilityRef, status: CapabilityStatus): void {
    const key = capabilityRefKey(reference);
    this.references.set(key, reference);
    this.statuses.set(key, status);
    const summary = this.summaries.get(key);
    if (summary) this.summaries.set(key, { ...summary, status });
    this.publish();
  }

  /** Refreshes mutable presentation while preserving the exact contract projection. */
  updatePresentation(reference: CapabilityRef, presentation: CapabilityPresentation | null): void {
    const key = capabilityRefKey(reference);
    this.references.set(key, reference);
    this.presentations.set(key, presentation);
    const summary = this.summaries.get(key);
    if (summary) this.summaries.set(key, { ...summary, presentation: presentation ?? summary.presentation });
    this.publish();
  }

  /** Returns the stable external-store snapshot used by React components. */
  snapshot = (): CapabilityCacheSnapshot => this.currentSnapshot;

  /** Subscribes to cache changes for `useSyncExternalStore`. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private store(reference: CapabilityRef, bundle: CapabilityBundle): void {
    const key = capabilityRefKey(reference);
    const current = this.contracts.get(key);
    if (current && bundle.contract && JSON.stringify(current) !== JSON.stringify(bundle.contract)) {
      throw new Error(`CAPABILITY_CONTRACT_CHANGED:${key}`);
    }
    this.references.set(key, reference);
    this.contracts.set(key, bundle.contract);
    this.presentations.set(key, bundle.presentation);
    this.statuses.set(key, bundle.status);
    const summary = this.summaries.get(key);
    if (summary) {
      this.summaries.set(key, {
        ...summary,
        presentation: bundle.presentation ?? summary.presentation,
        status: bundle.status,
      });
    }
  }

  private publish(): void {
    const bundles = new Map<string, CapabilityBundle>();
    for (const key of this.statuses.keys()) {
      const reference = this.references.get(key);
      if (reference) {
        const bundle = this.get(reference);
        if (bundle) bundles.set(key, bundle);
      }
    }
    this.currentSnapshot = { bundles, summaries: [...this.summaries.values()] };
    for (const listener of this.listeners) listener();
  }
}

/** React subscription for one cache instance shared by the workspace. */
export function useCapabilityCache(cache: CapabilityContractCache): CapabilityCacheSnapshot {
  return useSyncExternalStore(cache.subscribe, cache.snapshot, cache.snapshot);
}

export function capabilityRefKey(reference: CapabilityRef): string {
  return `${encodeURIComponent(reference.id)}@${encodeURIComponent(reference.version)}`;
}

function uniqueReferences(references: CapabilityRef[]): CapabilityRef[] {
  const seen = new Set<string>();
  return references.filter((reference) => {
    const key = capabilityRefKey(reference);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function sameRef(left: CapabilityRef, right: CapabilityRef): boolean {
  return left.id === right.id && left.version === right.version;
}

function degradedBundle(reference: CapabilityRef, reason: string): CapabilityBundle {
  return {
    reference,
    contract: null,
    presentation: null,
    status: {
      availability: "degraded",
      reason,
      provider_health: null,
      status_revision: 0,
    },
  };
}
