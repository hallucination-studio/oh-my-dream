import { useMemo, useState } from "react";
import type { CapabilityRef, CapabilitySummary, NodeCapabilityContractDto } from "../api/types.ts";
import { capabilityKey } from "../nodes/exactCapability.ts";
import type { NodeTypeSpec } from "../nodes/catalog.ts";
import { paletteCreation } from "../nodes/catalog.ts";
import { presentationFor } from "../nodes/exactCapability.ts";
import { nodeAccent } from "../nodes/typeColor.ts";
import "./nodeLibrary.css";

interface NodeLibraryProps {
  contracts: readonly NodeCapabilityContractDto[];
  loadedSpecs: readonly NodeTypeSpec[];
  hiddenCapabilityKeys?: ReadonlySet<string>;
  savedCapabilityKeys?: ReadonlySet<string>;
  onAdd: (reference: CapabilityRef) => void;
  onOpenAssets: () => void;
}

/** Creator-language group order: Inputs first, then Generate, then Assets. */
const CATEGORY_ORDER = ["Inputs", "Generate", "Assets"];

/** Presents the authoritative capability registry with current profile status. */
export function NodeLibrary({
  contracts,
  loadedSpecs,
  hiddenCapabilityKeys = new Set(),
  savedCapabilityKeys = new Set(),
  loading = false,
  onAdd,
  onOpenAssets,
}: NodeLibraryProps & { loading?: boolean }) {
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const offersAssetRoute = query.toLowerCase().includes("asset");

  const groups = useMemo(() => {
    const grouped: { category: string; nodes: CapabilitySummary[] }[] = [];
    const normalizedQuery = query.toLowerCase();
    const visibleSummaries = contracts
      .map((contract) => contractSummary(contract, loadedSpecs))
      .filter((summary) =>
        // Asset nodes are created by dragging from the Asset Library, never
        // from the palette (docs/DESKTOP_UI.md, Graph Editing).
        summary.contextual_creation === null &&
        (!hiddenCapabilityKeys.has(capabilityKey(summary.reference)) ||
          savedCapabilityKeys.has(capabilityKey(summary.reference))) &&
        (summary.presentation.label.toLowerCase().includes(normalizedQuery) ||
          summary.reference.id.includes(normalizedQuery) ||
          summary.presentation.search_terms.some((term) =>
            term.toLowerCase().includes(normalizedQuery),
          )),
      );
    for (const summary of visibleSummaries) {
      let group = grouped.find((candidate) => candidate.category === summary.presentation.category);
      if (!group) {
        group = { category: summary.presentation.category, nodes: [] };
        grouped.push(group);
      }
      group.nodes.push(summary);
    }
    const order = (category: string) => {
      const index = CATEGORY_ORDER.indexOf(category);
      return index === -1 ? CATEGORY_ORDER.length : index;
    };
    return grouped.sort(
      (left, right) =>
        order(left.category) - order(right.category) ||
        left.category.localeCompare(right.category),
    );
  }, [contracts, hiddenCapabilityKeys, loadedSpecs, query, savedCapabilityKeys]);

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

      <div className="nlib__tree" aria-busy={loading}>
        {loading ? (
          <div className="nlib__skels" role="status" aria-label="Loading node types">
            {Array.from({ length: 6 }, (_, index) => (
              <div key={index} className="nlib__skel" aria-hidden="true" />
            ))}
          </div>
        ) : (
          <>
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
                <span className="nlib__cdot" style={{ background: categoryColor(group.nodes[0], loadedSpecs) }} />
                <span className="nlib__cn">{group.category}</span>
                <span className="nlib__cc">{group.nodes.length}</span>
              </button>
              {!isCollapsed && (
                <div className="nlib__leaves">
                  {group.nodes.map((summary) => {
                    const creation = paletteCreation(summary);
                    const reason = summary.status.reason;
                    return (
                      <button
                        key={`${summary.reference.id}@${summary.reference.version}`}
                        className="nlib__leaf"
                        draggable={creation.canAdd}
                        disabled={!creation.canAdd}
                        aria-label={reason ? `${summary.presentation.label} — ${reason}` : summary.presentation.label}
                        title={reason ?? summary.presentation.description}
                        onDragStart={(event) =>
                          event.dataTransfer.setData(
                            "application/oh-node",
                            JSON.stringify(summary.reference),
                          )
                        }
                        onClick={() => onAdd(summary.reference)}
                      >
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
          </>
        )}
      </div>

    </aside>
  );
}

function contractSummary(
  contract: NodeCapabilityContractDto,
  specs: readonly NodeTypeSpec[],
): CapabilitySummary {
  const presentation = presentationFor(contract.capability_ref.id);
  const contextual_creation = contract.capability_ref.id.endsWith(".read_asset")
    ? { route: "asset_library" }
    : null;
  const spec = specs.find((candidate) => sameRef(candidate.ref, contract.capability_ref));
  return {
    selector: { type_id: contract.capability_ref.id, mode: "" },
    reference: contract.capability_ref,
    presentation,
    contextual_creation,
    status: spec?.status ?? { availability: "available", reason: null, provider_health: null, status_revision: 0 },
  };
}


function categoryColor(summary: CapabilitySummary | undefined, specs: readonly NodeTypeSpec[]): string {
  if (!summary) return "var(--ink-3)";
  const spec = specs.find((candidate) => sameRef(candidate.ref, summary.reference));
  return spec ? nodeAccent(spec.outputs, spec.inputs) : "var(--ink-3)";
}

function sameRef(left: CapabilityRef, right: CapabilityRef): boolean {
  return left.id === right.id && left.version === right.version;
}
