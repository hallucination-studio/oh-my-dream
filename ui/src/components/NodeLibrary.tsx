// Node library as a searchable, collapsible category tree (Input / Image /
// Video / Audio). Leaves add a node to the canvas on click and are draggable.

import { useMemo, useState } from "react";
import { nodesByCategory, type NodeTypeSpec } from "../nodes/catalog.ts";
import { nodeAccent, typeColor } from "../nodes/typeColor.ts";
import "./nodeLibrary.css";

export function NodeLibrary({ onAdd }: { onAdd: (type: string) => void }) {
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    return nodesByCategory()
      .map((g) => ({
        category: g.category,
        nodes: q ? g.nodes.filter((n) => n.label.toLowerCase().includes(q)) : g.nodes,
      }))
      .filter((g) => g.nodes.length > 0);
  }, [query]);

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
            value={query}
            placeholder="Search nodes…"
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      <div className="nlib__tree">
        {groups.map((g) => {
          const isCollapsed = collapsed[g.category] && !query;
          return (
            <div className="nlib__cat" key={g.category}>
              <button
                className={`nlib__cathead${isCollapsed ? "" : " is-open"}`}
                onClick={() => setCollapsed((c) => ({ ...c, [g.category]: !c[g.category] }))}
              >
                <span className="nlib__tw" aria-hidden="true" />
                <span className="nlib__cdot" style={{ background: categoryColor(g.nodes[0]) }} />
                <span className="nlib__cn">{g.category}</span>
                <span className="nlib__cc">{g.nodes.length}</span>
              </button>
              {!isCollapsed && (
                <div className="nlib__leaves">
                  {g.nodes.map((n) => (
                    <button
                      key={n.type}
                      className="nlib__leaf"
                      draggable
                      onDragStart={(e) => e.dataTransfer.setData("application/oh-node", n.type)}
                      onClick={() => onAdd(n.type)}
                    >
                      <span
                        className="nlib__ld"
                        style={{ background: typeColor(n.outputs[0]?.type ?? n.inputs[0]?.type) }}
                      />
                      {n.label}
                      <span className="nlib__lg" aria-hidden="true">⠿</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          );
        })}
        {groups.length === 0 && <p className="nlib__empty">No nodes match “{query}”.</p>}
      </div>

      <p className="nlib__foot">Drag onto the canvas, or double-click the canvas to search.</p>
    </aside>
  );
}

function categoryColor(sample: NodeTypeSpec): string {
  return nodeAccent(sample.outputs, sample.inputs);
}
