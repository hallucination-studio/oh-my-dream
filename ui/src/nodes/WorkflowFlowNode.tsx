// Custom workflow node: a solid card with a type-colored accent, typed port
// gems, an inline parameter grid, and — once run — a status pill, progress bar,
// result preview, and a cost/time footer. Mirrors the states in docs/ui-pro.

import { Handle, Position, type NodeProps } from "@xyflow/react";
import { recoveryNodeSpec, type NodeTypeSpec } from "./catalog.ts";
import { ParameterInput } from "./ParameterInput.tsx";
import { typeColor, nodeAccent, portTypeLabel } from "./typeColor.ts";
import type { NodeExecutionState, PortType } from "../workflow/types.ts";
import "./nodeStyles.css";

/** State of an in-flight connection drag or keyboard connection intent. */
export interface ConnectHighlight {
  sourceId: string;
  sourceHandle: string;
  sourceType: PortType;
  keyboard: boolean;
}

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
  assetPresentation?: { title: string; available: boolean };
  textPresentation?: string | null;
  connect?: ConnectHighlight | null;
  onParamChange: (name: string, value: unknown) => void;
  onStartKeyboardConnect?: (nodeId: string, handle: string) => void;
  onCompleteKeyboardConnect?: (nodeId: string, handle: string) => void;
  [key: string]: unknown;
}


export function WorkflowFlowNode({ data, selected, id: nodeId }: NodeProps) {
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
  const isAsset = spec.contextualCreationRoute === "asset_library";
  const isGeneration = spec.presentation?.category === "Generate";

  return (
    <div
      className={`wf-node${isGeneration ? " wf-node--generation" : ""} is-${state}${selected ? " is-selected" : ""}${spec.status.availability !== "available" ? " is-degraded" : ""}`}
      style={{ ["--type" as string]: accent }}
    >
      <div className="wf-node__title">
        <span>
          {isAsset ? spec.label : (spec.selector?.type_id ?? spec.label)}
        </span>
        <StatePill state={state} />
      </div>

      {isAsset && (
        <div className="wf-node__asset" title={nodeData.assetPresentation?.title}>
          {nodeData.assetPresentation?.available === false
            ? "Asset unavailable"
            : nodeData.assetPresentation?.title ?? `Untitled ${spec.selector?.type_id.toLowerCase() ?? "asset"}`}
        </div>
      )}

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

      {rt?.preview && (isGeneration || rt.preview.url) && (
        <Preview
          preview={rt.preview}
          emptyHint={isGeneration && state === "idle"
            ? `Run this step to create ${previewNoun(rt.preview.kind)}.`
            : null}
        />
      )}
      {nodeData.textPresentation && (
        <div className="wf-node__asset">{nodeData.textPresentation}</div>
      )}

      {spec.params.length > 0 && !isAsset && (
        <div className="wf-node__body">
          {spec.params
            .filter((param) => param.name !== "generation_profile_ref")
            .map((param) => (
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

      {(spec.inputs.length > 0 || spec.outputs.length > 0) && (
        <div className="wf-node__ports">
          <div className="wf-node__ports-col">
            {spec.inputs.map((port) => (
              <TargetPortRow key={`in-${port.name}`} port={port} nodeId={nodeId} data={nodeData} />
            ))}
          </div>
          <div className="wf-node__ports-col wf-node__ports-col--out">
            {spec.outputs.map((port) => (
              <SourcePortRow key={`out-${port.name}`} port={port} nodeId={nodeId} data={nodeData} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function TargetPortRow({
  port,
  nodeId,
  data,
}: {
  port: { name: string; type: PortType };
  nodeId: string;
  data: FlowNodeData;
}) {
  const connect = data.connect ?? null;
  const active = connect !== null;
  const compatible =
    active && connect.sourceId !== nodeId && connect.sourceType === port.type;
  const keyboardTarget = connect?.keyboard === true && compatible;
  return (
    <div
      className={`wf-port-row${active ? (compatible ? " is-compatible" : " is-incompatible") : ""}`}
      role={keyboardTarget ? "button" : undefined}
      tabIndex={keyboardTarget ? 0 : undefined}
      data-connect-target={keyboardTarget ? "compatible" : undefined}
      aria-label={keyboardTarget ? `Connect to ${port.name}` : undefined}
      onClick={keyboardTarget ? () => data.onCompleteKeyboardConnect?.(nodeId, port.name) : undefined}
      onKeyDown={
        keyboardTarget
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                data.onCompleteKeyboardConnect?.(nodeId, port.name);
              }
            }
          : undefined
      }
    >
      <Handle
        type="target"
        position={Position.Left}
        id={port.name}
        className="wf-port"
        style={{ background: typeColor(port.type) }}
      />
      <span className="wf-port-row__name">{port.name}</span>
      {port.name.toLowerCase() !== portTypeLabel(port.type).toLowerCase() && (
        <span className="wf-port-row__type">{portTypeLabel(port.type)}</span>
      )}
    </div>
  );
}

function SourcePortRow({
  port,
  nodeId,
  data,
}: {
  port: { name: string; type: PortType };
  nodeId: string;
  data: FlowNodeData;
}) {
  const connect = data.connect ?? null;
  const isSource =
    connect !== null && connect.sourceId === nodeId && connect.sourceHandle === port.name;
  const canStart = connect === null;
  return (
    <div
      className={`wf-port-row wf-port-row--out${isSource ? " is-source" : ""}`}
      role={canStart ? "button" : undefined}
      tabIndex={canStart ? 0 : undefined}
      aria-label={canStart ? `Connect from ${port.name}` : undefined}
      onKeyDown={
        canStart
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                data.onStartKeyboardConnect?.(nodeId, port.name);
              }
            }
          : undefined
      }
    >
      {port.name.toLowerCase() !== portTypeLabel(port.type).toLowerCase() && (
        <span className="wf-port-row__type">{portTypeLabel(port.type)}</span>
      )}
      <span className="wf-port-row__name">{port.name}</span>
      <Handle
        type="source"
        position={Position.Right}
        id={port.name}
        className="wf-port"
        style={{ background: typeColor(port.type) }}
      />
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
    idle: "Not run",
    running: "Running",
    done: "Complete",
    cached: "Complete",
    error: "Needs attention",
  };
  return (
    <span className={`wf-pill wf-pill--${state}`}>
      <span className="wf-pill__dot" />
      {text[state]}
    </span>
  );
}

function Preview({
  preview,
  emptyHint,
}: {
  preview: NonNullable<NodeRuntime["preview"]>;
  emptyHint?: string | null;
}) {
  if (preview.kind === "audio") {
    return preview.url
      ? <audio className="wf-preview wf-preview--audio" controls src={preview.url} aria-label="Asset audio preview" />
      : <div className="wf-preview wf-preview--audio" role="status">{emptyHint ?? "Audio unavailable"}</div>;
  }
  return (
    <div className={`wf-preview wf-preview--${preview.kind}`}>
      {preview.url ? (
        <img className="wf-preview__img" src={preview.url} alt={preview.kind} />
      ) : (
        <span className="wf-preview__empty">{emptyHint ?? <span className="wf-preview__tag">{preview.kind}</span>}</span>
      )}
      {preview.kind === "video" && preview.url ? (
        <span className="wf-preview__play" aria-hidden="true">
          <span className="wf-preview__tri" />
        </span>
      ) : null}
    </div>
  );
}

function previewNoun(kind: "image" | "video" | "audio"): string {
  return kind === "image" ? "an image" : kind === "video" ? "a video" : "audio";
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
