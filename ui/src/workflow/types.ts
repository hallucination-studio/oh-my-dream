// React editor view models. Canonical DTO translation lives only in canonical.ts.

export type PortType = "string" | "image" | "video" | "audio" | "model" | "int" | "float";

/** A reference to an upstream node's named output. */
export type OutputRef =
  | [nodeId: string, outputName: string]
  | { node_id: string; output: string };

export type WorkflowInputBinding =
  | OutputRef
  | { kind: "single"; source: OutputRef }
  | { kind: "ordered_many"; sources: OutputRef[] };

export interface WorkflowNode {
  id: string;
  type: string;
  /** Exact capability contract version; omitted only for legacy input. */
  contract_version?: string;
  params: Record<string, unknown>;
  inputs: Record<string, WorkflowInputBinding>;
  position?: [number, number];
}

export interface Workflow {
  version: string;
  project_id: string;
  nodes: WorkflowNode[];
}

export type NodeExecutionState = "idle" | "running" | "done" | "cached" | "error";

export interface NodeProgressEvent {
  node_id: string;
  state: NodeExecutionState;
  progress: number | null;
  cost: number | null;
}

export interface RunProgress {
  nodeId: string;
  progress: number;
  nodeState: NodeExecutionState;
  cost?: number;
}

export type RunTerminalStatus =
  | { state: "cancelled" }
  | { state: "succeeded"; outputs: RunOutputs; steps: number }
  | { state: "failed"; reason: string };

export type RunLifecycleStatus =
  | { state: "cancelling" }
  | { state: "cancel_failed"; reason: string }
  | RunTerminalStatus;

/** Status of a running workflow. */
export type RunStatus =
  | { state: "idle" }
  | {
      state: "running";
      nodeId: string;
      progress: number;
      nodeState?: NodeExecutionState;
      cost?: number;
    }
  | RunLifecycleStatus;

/** A produced artifact reference for a node (asset id / URL placeholder). */
export interface RunOutput {
  kind: "image" | "video" | "audio" | "string" | "model" | "int" | "float";
  value: string;
}

/**
 * Nested legacy Assistant outputs retained until the V5 presentation hard cut:
 * nodeId -> outputName -> RunOutput. The engine can expose several named
 * outputs per node, so we preserve output names rather than flattening.
 */
export type RunOutputs = Record<string, Record<string, RunOutput>>;
