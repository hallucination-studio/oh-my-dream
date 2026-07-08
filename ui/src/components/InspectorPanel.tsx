// Right inspector: parameters of the currently selected node, edited in a
// calmer surface than the compact on-canvas fields. Empty state guides the
// user to pick a node.

import { findNodeType } from "../nodes/catalog.ts";
import { typeColor } from "../nodes/typeColor.ts";
import "./inspector.css";

export interface SelectedNode {
  id: string;
  type: string;
  params: Record<string, unknown>;
}

export function InspectorPanel({
  node,
  onParamChange,
}: {
  node: SelectedNode | null;
  onParamChange: (nodeId: string, name: string, value: unknown) => void;
}) {
  if (!node) {
    return (
      <aside className="inspector">
        <div className="inspector__empty">
          <span className="inspector__empty-mark" aria-hidden="true" />
          Select a node to edit its parameters.
        </div>
      </aside>
    );
  }

  const spec = findNodeType(node.type);
  const accent = typeColor(spec?.outputs[0]?.type ?? spec?.inputs[0]?.type);

  return (
    <aside className="inspector">
      <div className="inspector__head" style={{ ["--accent" as string]: accent }}>
        <span className="inspector__badge" />
        <span className="inspector__title">{spec?.label ?? node.type}</span>
      </div>

      {spec && spec.params.length > 0 ? (
        <div className="inspector__fields">
          {spec.params.map((param) => (
            <label key={param.name} className="inspector__field">
              <span className="inspector__label">{param.label}</span>
              <input
                className={`inspector__input${param.kind === "int" || param.kind === "float" ? " is-mono" : ""}`}
                value={String(node.params[param.name] ?? param.default)}
                onChange={(e) => onParamChange(node.id, param.name, e.target.value)}
              />
            </label>
          ))}
        </div>
      ) : (
        <p className="inspector__note">This node has no parameters.</p>
      )}
    </aside>
  );
}
