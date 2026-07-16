import { useEffect, useMemo, useState } from "react";
import type {
  CapabilityRef,
  CapabilitySearchRequest,
  CapabilitySearchPage,
  CapabilitySummary,
} from "../api/types.ts";
import { api, type NodeCapabilityContractDto } from "../api/index.ts";
import type { NodeTypeSpec } from "../nodes/catalog.ts";
import { isPaletteVisible, paletteCreation } from "../nodes/catalog.ts";
import { nodeAccent } from "../nodes/typeColor.ts";
import "./nodeLibrary.css";

interface NodeLibraryProps {
  summaries: readonly CapabilitySummary[];
  loadedSpecs: readonly NodeTypeSpec[];
  onSearch: (request: CapabilitySearchRequest) => Promise<CapabilitySearchPage>;
  onAdd: (reference: CapabilityRef) => void;
  onOpenAssets: () => void;
}

/** Paged presentation/status palette; exact contracts load only on addition. */
export function NodeLibrary({ summaries, loadedSpecs, onSearch, onAdd, onOpenAssets }: NodeLibraryProps) {
  const [query, setQuery] = useState("");
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [contracts, setContracts] = useState<NodeCapabilityContractDto[] | null>();
  const offersAssetRoute = query.toLowerCase().includes("asset");

  useEffect(() => {
    if (contracts !== null) return;
    let active = true;
    void api
      .nodeCapabilityList()
      .then((items) => {
        if (active) setContracts(items.length > 0 ? items : null);
      })
      .catch(() => {
        if (active) setContracts(null);
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    setLoading(true);
    void onSearch({ query, cursor: null, limit: 24 })
      .then((page) => {
        if (active) setNextCursor(page.next_cursor);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [contracts, onSearch, query]);

  const groups = useMemo(() => {
    const grouped: { category: string; nodes: CapabilitySummary[] }[] = [];
    const visibleSummaries = Array.isArray(contracts)
      ? contracts.map(contractSummary).filter((summary) =>
          summary.presentation.label.toLowerCase().includes(query.toLowerCase())
          || summary.reference.id.includes(query.toLowerCase()))
      : summaries.filter(isPaletteVisible);
    for (const summary of visibleSummaries) {
      let group = grouped.find((candidate) => candidate.category === summary.presentation.category);
      if (!group) {
        group = { category: summary.presentation.category, nodes: [] };
        grouped.push(group);
      }
      group.nodes.push(summary);
    }
    return grouped;
  }, [contracts, query, summaries]);

  const loadMore = () => {
    if (!nextCursor || loading) return;
    setLoading(true);
    void onSearch({ query, cursor: nextCursor, limit: 24 })
      .then((page) => setNextCursor(page.next_cursor))
      .finally(() => setLoading(false));
  };

  return (
    <aside className="nlib">
      <div className="nlib__head">
        <div className="nlib__title">Nodes</div>
        <div className="nlib__search">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <circle cx="11" cy="11" r="7" />
            <path d="M21 21l-4-4" />
          </svg>
          <input
            aria-label="Search nodes"
            name="node-search"
            value={query}
            placeholder="Search nodes..."
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
      </div>

      <div className="nlib__tree">
        {offersAssetRoute && (
          <button
            className="nlib__route"
            aria-label="Use an existing asset"
            onClick={onOpenAssets}
          >
            <span>Use an existing asset</span>
            <small>Open Assets →</small>
          </button>
        )}
        {groups.map((group) => {
          const isCollapsed = collapsed[group.category] && !query;
          return (
            <div className="nlib__cat" key={group.category}>
              <button
                className={`nlib__cathead${isCollapsed ? "" : " is-open"}`}
                onClick={() => setCollapsed((current) => ({ ...current, [group.category]: !current[group.category] }))}
              >
                <span className="nlib__tw" aria-hidden="true" />
                <span className="nlib__cdot" style={{ background: categoryColor(group.nodes[0], loadedSpecs) }} />
                <span className="nlib__cn">{group.category}</span>
                <span className="nlib__cc">{group.nodes.length}</span>
              </button>
              {!isCollapsed && (
                <div className="nlib__leaves">
                  {group.nodes.map((summary) => {
                    const spec = loadedSpecs.find((candidate) => sameRef(candidate.ref, summary.reference));
                    const creation = paletteCreation(summary);
                    return (
                      <button
                        key={`${summary.reference.id}@${summary.reference.version}`}
                        className="nlib__leaf"
                        draggable={creation.canAdd}
                        disabled={!creation.canAdd}
                        title={creation.route
                          ? `Create from ${creation.route}`
                          : summary.status.reason ?? summary.presentation.description}
                        onDragStart={(event) => event.dataTransfer.setData("application/oh-node", JSON.stringify(summary.reference))}
                        onClick={() => onAdd(summary.reference)}
                      >
                        <span
                          className="nlib__ld"
                          style={{ background: spec ? nodeAccent(spec.outputs, spec.inputs) : "var(--ink-3)" }}
                        />
                        {summary.presentation.label}
                        <span className="nlib__lg" aria-hidden="true">{summary.reference.version}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
        {groups.length === 0 && !offersAssetRoute && (
          <p className="nlib__empty">No nodes match "{query}".</p>
        )}
        {nextCursor && (
          <button className="nlib__load-more" disabled={loading} onClick={loadMore}>
            {loading ? "Loading..." : "Load more"}
          </button>
        )}
      </div>

      <p className="nlib__foot">Drag onto the canvas, or select a node to load its exact contract.</p>
    </aside>
  );
}

function contractSummary(contract: NodeCapabilityContractDto): CapabilitySummary {
  const presentation = presentationFor(contract.capability_ref.id);
  return {
    selector: { type_id: contract.capability_ref.id, mode: "" },
    reference: contract.capability_ref,
    presentation,
    contextual_creation: null,
    status: { availability: "available", reason: null, provider_health: null, status_revision: 0 },
  };
}

function presentationFor(id: string) {
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

function categoryColor(summary: CapabilitySummary | undefined, specs: readonly NodeTypeSpec[]): string {
  if (!summary) return "var(--ink-3)";
  const spec = specs.find((candidate) => sameRef(candidate.ref, summary.reference));
  return spec ? nodeAccent(spec.outputs, spec.inputs) : "var(--ink-3)";
}

function sameRef(left: CapabilityRef, right: CapabilityRef): boolean {
  return left.id === right.id && left.version === right.version;
}
