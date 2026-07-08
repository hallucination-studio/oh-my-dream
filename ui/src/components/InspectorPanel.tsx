// Right inspector: parameters of the selected node in a calm glass panel, plus
// a note that generated media auto-saves to the library.

import { findNodeType } from "../nodes/catalog.ts";
import { nodeAccent } from "../nodes/typeColor.ts";
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
      <aside className="insp glass">
        <div className="insp__empty">
          <span className="insp__empty-mark" aria-hidden="true" />
          Select a node to edit its parameters.
        </div>
      </aside>
    );
  }

  const spec = findNodeType(node.type);
  const accent = spec ? nodeAccent(spec.outputs, spec.inputs) : "var(--ink-3)";
  const produces = spec && spec.outputs.some((o) => ["image", "video", "audio"].includes(o.type));

  return (
    <aside className="insp glass">
      <div className="insp__head">
        <span className="insp__badge" style={{ background: accent }} />
        <b>{spec?.label ?? node.type}</b>
      </div>

      {spec && spec.params.length > 0 ? (
        <div className="insp__fields">
          {spec.params.map((param) => (
            <label key={param.name} className="insp__field">
              <span className="insp__label">{param.label}</span>
              <input
                className={`insp__input${param.kind === "int" || param.kind === "float" ? " is-mono" : ""}`}
                value={String(node.params[param.name] ?? param.default)}
                onChange={(e) => onParamChange(node.id, param.name, e.target.value)}
              />
            </label>
          ))}
        </div>
      ) : (
        <p className="insp__note-plain">This node has no parameters.</p>
      )}

      {produces && (
        <div className="insp__note">
          Generated media saves to the Library automatically, tagged with this project and prompt.
        </div>
      )}
    </aside>
  );
}
