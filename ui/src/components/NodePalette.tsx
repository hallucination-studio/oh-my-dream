// Left node palette. Each entry shows the node's output type color so the
// library reads in the same visual language as the canvas wiring.

import { NODE_TYPES } from "../nodes/catalog.ts";
import { typeColor } from "../nodes/typeColor.ts";
import "./palette.css";

export function NodePalette({ onAdd }: { onAdd: (type: string) => void }) {
  return (
    <aside className="palette">
      <div className="palette__head">Nodes</div>
      <div className="palette__list">
        {NODE_TYPES.map((spec) => {
          const channel = spec.outputs[0]?.type ?? spec.inputs[0]?.type;
          return (
            <button key={spec.type} className="palette__item" onClick={() => onAdd(spec.type)}>
              <span className="palette__dot" style={{ background: typeColor(channel) }} />
              <span className="palette__label">{spec.label}</span>
              <span className="palette__plus" aria-hidden="true">
                +
              </span>
            </button>
          );
        })}
      </div>
      <p className="palette__foot">Patch nodes left to right: prompt → image → video → save.</p>
    </aside>
  );
}
