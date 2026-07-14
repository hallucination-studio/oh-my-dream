// Custom workflow node: a solid card with a type-colored accent, typed port
// gems, an inline parameter grid, and — once run — a status pill, progress bar,
// result preview, and a cost/time footer. Mirrors the states in docs/ui-pro.

import { Handle, Position, type NodeProps } from "@xyflow/react";
import { recoveryNodeSpec, type NodeTypeSpec } from "./catalog.ts";
import { ParameterInput } from "./ParameterInput.tsx";
import { typeColor, nodeAccent } from "./typeColor.ts";
import type { NodeExecutionState } from "../workflow/types.ts";
import "./nodeStyles.css";

export interface NodeRuntime {
  state: NodeExecutionState;
  progress?: number;
  cost?: number;
  preview?: { kind: "image" | "video" | "audio"; url: string | null };
  durationMs?: number;
}

export interface FlowNodeData {
  type: string;
  contractVersion?: string;
  capability?: NodeTypeSpec;
  params: Record<string, unknown>;
  runtime?: NodeRuntime;
  onParamChange: (name: string, value: unknown) => void;
  [key: string]: unknown;
}

const PORT_TOP = 64;

export function WorkflowFlowNode({ data, selected }: NodeProps) {
  const nodeData = data as FlowNodeData;
  const spec =
    nodeData.capability ??
    recoveryNodeSpec(
      { id: nodeData.type, version: nodeData.contractVersion ?? "1.0" },
      "exact capability bundle is not loaded",
    );

  const accent = nodeAccent(spec.outputs, spec.inputs);
  const rt = nodeData.runtime;
  const state = rt?.state ?? "idle";

  return (
    <div
      className={`wf-node is-${state}${selected ? " is-selected" : ""}${spec.status.availability !== "available" ? " is-degraded" : ""}`}
      style={{ ["--accent" as string]: accent }}
    >
      <div className="wf-node__bar" />
      <div className="wf-node__title">
        <span>
          {spec.selector?.type_id ?? spec.label}
          {spec.selector && <small> · {spec.selector.mode}</small>}
        </span>
        <StatePill state={state} />
      </div>

      {spec.status.availability !== "available" && (
        <div className="wf-node__recovery" role="status">
          {spec.status.reason ?? "Capability needs repair"}
        </div>
      )}

      {state === "running" && (
        <div className="wf-node__prog">
          <i style={{ width: `${Math.round((rt?.progress ?? 0) * 100)}%` }} />
        </div>
      )}

      {rt?.preview && <Preview preview={rt.preview} />}

      {spec.params.length > 0 && (
        <div className="wf-node__body">
          {spec.params.map((param) => (
            <Fragment key={param.name} label={param.label}>
              <ParameterInput
                spec={param}
                className={`wf-param__input${param.kind === "int" || param.kind === "float" ? " is-mono" : ""}`}
                value={nodeData.params[param.name] ?? param.default}
                onChange={(value) => nodeData.onParamChange(param.name, value)}
              />
            </Fragment>
          ))}
        </div>
      )}

      {state !== "idle" && <Footer rt={rt} />}

      {spec.inputs.map((port, i) => (
        <Handle
          key={`in-${port.name}`}
          type="target"
          position={Position.Left}
          id={port.name}
          className="wf-port"
          style={{ top: PORT_TOP + i * 24, background: typeColor(port.type) }}
        />
      ))}
      {spec.outputs.map((port, i) => (
        <Handle
          key={`out-${port.name}`}
          type="source"
          position={Position.Right}
          id={port.name}
          className="wf-port"
          style={{ top: PORT_TOP + i * 24, background: typeColor(port.type) }}
        />
      ))}
    </div>
  );
}

function Fragment({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="wf-param">
      <span className="wf-param__label">{label}</span>
      {children}
    </label>
  );
}

function StatePill({ state }: { state: NodeExecutionState }) {
  const text: Record<NodeExecutionState, string> = {
    idle: "Idle",
    running: "Running",
    done: "Done",
    cached: "Cached",
    error: "Error",
  };
  return (
    <span className={`wf-pill wf-pill--${state}`}>
      <span className="wf-pill__dot" />
      {text[state]}
    </span>
  );
}

function Preview({ preview }: { preview: NonNullable<NodeRuntime["preview"]> }) {
  if (preview.kind === "audio") {
    return <div className="wf-preview wf-preview--audio">♪ audio</div>;
  }
  return (
    <div className={`wf-preview wf-preview--${preview.kind}`}>
      {preview.url ? (
        <img className="wf-preview__img" src={preview.url} alt={preview.kind} />
      ) : (
        <span className="wf-preview__tag">{preview.kind}</span>
      )}
      {preview.kind === "video" && (
        <span className="wf-preview__play" aria-hidden="true">
          <span className="wf-preview__tri" />
        </span>
      )}
    </div>
  );
}

function Footer({ rt }: { rt?: NodeRuntime }) {
  const cost = rt?.cost;
  return (
    <div className="wf-node__foot">
      {typeof cost === "number" && (
        <span className="wf-credit">
          <span className="wf-credit__coin" />
          {formatCost(cost, rt?.state === "cached")}
        </span>
      )}
      {typeof rt?.durationMs === "number" && (
        <span className="wf-node__time">{(rt.durationMs / 1000).toFixed(1)}s</span>
      )}
    </div>
  );
}

function formatCost(microUsd: number, cached: boolean): string {
  if (cached) {
    return "0 · reused";
  }
  return `$${(microUsd / 1_000_000).toFixed(4)}`;
}
