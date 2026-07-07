// Custom React Flow node rendering a workflow node: title, typed handles, and
// an editable parameter panel.

import { Handle, Position, type NodeProps } from "@xyflow/react";
import { findNodeType } from "./catalog.ts";

export interface FlowNodeData {
  type: string;
  params: Record<string, unknown>;
  running?: boolean;
  onParamChange: (name: string, value: unknown) => void;
  [key: string]: unknown;
}

export function WorkflowFlowNode({ data }: NodeProps) {
  const nodeData = data as FlowNodeData;
  const spec = findNodeType(nodeData.type);
  if (!spec) {
    return <div className="node node--error">Unknown node: {nodeData.type}</div>;
  }

  return (
    <div className={`node${nodeData.running ? " node--running" : ""}`}>
      <div className="node__title">{spec.label}</div>

      {spec.inputs.map((port, i) => (
        <Handle
          key={`in-${port.name}`}
          type="target"
          position={Position.Left}
          id={port.name}
          style={{ top: 40 + i * 22 }}
          title={`${port.name}: ${port.type}`}
        />
      ))}

      <div className="node__params">
        {spec.params.map((param) => (
          <label key={param.name} className="node__param">
            <span>{param.label}</span>
            <input
              value={String(nodeData.params[param.name] ?? param.default)}
              onChange={(e) => nodeData.onParamChange(param.name, e.target.value)}
            />
          </label>
        ))}
      </div>

      {spec.outputs.map((port, i) => (
        <Handle
          key={`out-${port.name}`}
          type="source"
          position={Position.Right}
          id={port.name}
          style={{ top: 40 + i * 22 }}
          title={`${port.name}: ${port.type}`}
        />
      ))}
    </div>
  );
}
