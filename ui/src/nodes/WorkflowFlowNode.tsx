// Custom workflow node: a titled card with type-colored port "gems" on each
// side and an inline parameter list. The left accent bar and the port colors
// come from the data types, making a patched graph legible at a glance.

import { Handle, Position, type NodeProps } from "@xyflow/react";
import { findNodeType } from "./catalog.ts";
import { typeColor } from "./typeColor.ts";
import "./nodeStyles.css";

export interface FlowNodeData {
  type: string;
  params: Record<string, unknown>;
  running?: boolean;
  done?: boolean;
  onParamChange: (name: string, value: unknown) => void;
  [key: string]: unknown;
}

const PORT_TOP = 46;
const PORT_GAP = 26;

export function WorkflowFlowNode({ data, selected }: NodeProps) {
  const nodeData = data as FlowNodeData;
  const spec = findNodeType(nodeData.type);
  if (!spec) {
    return <div className="wf-node wf-node--error">Unknown node: {nodeData.type}</div>;
  }

  const accent = typeColor(spec.outputs[0]?.type ?? spec.inputs[0]?.type);
  const stateClass = nodeData.running ? " is-running" : nodeData.done ? " is-done" : "";

  return (
    <div
      className={`wf-node${selected ? " is-selected" : ""}${stateClass}`}
      style={{ ["--node-accent" as string]: accent }}
    >
      <div className="wf-node__bar" />
      <div className="wf-node__title">{spec.label}</div>

      {spec.inputs.map((port, i) => (
        <PortRow key={`in-${port.name}`} side="target" name={port.name} type={port.type} y={PORT_TOP + i * PORT_GAP} />
      ))}
      {spec.outputs.map((port, i) => (
        <PortRow key={`out-${port.name}`} side="source" name={port.name} type={port.type} y={PORT_TOP + i * PORT_GAP} />
      ))}

      <div className="wf-node__body">
        {spec.params.map((param) => (
          <label key={param.name} className="wf-param">
            <span className="wf-param__label">{param.label}</span>
            <input
              className={`wf-param__input${param.kind === "int" || param.kind === "float" ? " is-mono" : ""}`}
              value={String(nodeData.params[param.name] ?? param.default)}
              onChange={(e) => nodeData.onParamChange(param.name, e.target.value)}
            />
          </label>
        ))}
        {spec.params.length === 0 && <p className="wf-node__hint">Terminal — saves to library</p>}
      </div>
    </div>
  );
}

function PortRow({
  side,
  name,
  type,
  y,
}: {
  side: "source" | "target";
  name: string;
  type: import("../workflow/types.ts").PortType;
  y: number;
}) {
  const isSource = side === "source";
  const color = typeColor(type);
  return (
    <>
      <Handle
        type={side}
        position={isSource ? Position.Right : Position.Left}
        id={name}
        className="wf-port"
        style={{ top: y, background: color, boxShadow: `0 0 0 3px color-mix(in srgb, ${color} 25%, transparent)` }}
      />
      <span className={`wf-port__label wf-port__label--${side}`} style={{ top: y - 9 }}>
        {name}
      </span>
    </>
  );
}
