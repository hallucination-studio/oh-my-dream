// Workflow types mirroring the engine's serialized format (docs/DESIGN.md §5).
// The UI edits this shape; on submit it is what will be sent to the backend.

export type PortType = "string" | "image" | "video" | "model" | "int" | "float";

/** A reference to an upstream node's named output: [nodeId, outputName]. */
export type OutputRef = [string, string];

export interface WorkflowNode {
  id: string;
  type: string;
  params: Record<string, unknown>;
  inputs: Record<string, OutputRef>;
  position?: [number, number];
}

export interface Workflow {
  version: string;
  nodes: WorkflowNode[];
}

/** Status of a running workflow, mirroring backends::TaskStatus in spirit. */
export type RunStatus =
  | { state: "idle" }
  | { state: "running"; nodeId: string; progress: number }
  | { state: "succeeded"; outputs: RunOutputs }
  | { state: "failed"; reason: string };

/** A produced artifact reference for a node (asset id / URL placeholder). */
export interface RunOutput {
  kind: "image" | "video" | "string" | "model" | "int" | "float";
  value: string;
}

/**
 * Nested run outputs, matching the backend `run_workflow` DTO exactly:
 * nodeId -> outputName -> RunOutput. The engine can expose several named
 * outputs per node, so we preserve output names rather than flattening.
 */
export type RunOutputs = Record<string, Record<string, RunOutput>>;
