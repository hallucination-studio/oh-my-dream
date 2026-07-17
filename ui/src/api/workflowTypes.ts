import type { CapabilityRef, JsonObject, JsonValue } from "./types.ts";

export type WorkflowParameterValueDto =
  | { kind: "unsigned_integer"; value: string }
  | { kind: "text"; value: string }
  | { kind: "choice"; value: string }
  | { kind: "generation_profile"; profile_id: string; version: string }
  | { kind: "managed_asset"; asset_id: string };

export interface WorkflowDto {
  schema_version: number;
  workflow_id: string;
  project_id: string;
  revision: string;
  created_at_epoch_ms: string;
  updated_at_epoch_ms: string;
  nodes: WorkflowNodeDto[];
  input_bindings: WorkflowInputBindingDto[];
}

export interface WorkflowNodeDto {
  node_id: string;
  capability_id: string;
  capability_version: string;
  parameters: Array<{ key: string; value: WorkflowParameterValueDto }>;
  canvas_position: { x: number; y: number };
}

export interface WorkflowInputBindingDto {
  target_node_id: string;
  input_key: string;
  kind: "single" | "ordered_references";
  items: WorkflowInputItemDto[];
}

export interface WorkflowInputItemDto {
  input_item_id: string;
  source_node_id: string;
  source_output_key: string;
  input_role_key: string | null;
}

export type WorkflowReadinessDto =
  | { state: "ready" }
  | { state: "blocked"; issues: JsonValue[] };

export interface WorkflowWithReadinessDto {
  workflow: WorkflowDto;
  readiness: WorkflowReadinessDto;
}

export type WorkflowMutationActionDto =
  | {
      kind: "add_node";
      node_id: string;
      capability: CapabilityRef;
      parameters: Array<{ key: string; value: WorkflowParameterValueDto }>;
      canvas_position: { x: number; y: number };
    }
  | { kind: "remove_node"; node_id: string }
  | {
      kind: "replace_node_parameters";
      node_id: string;
      parameters: Array<{ key: string; value: WorkflowParameterValueDto }>;
    }
  | {
      kind: "select_node_capability";
      node_id: string;
      capability: CapabilityRef;
      parameters: Array<{ key: string; value: WorkflowParameterValueDto }>;
    }
  | { kind: "move_node"; node_id: string; canvas_position: { x: number; y: number } }
  | { kind: "bind_single_input"; target: WorkflowInputTargetDto; item: WorkflowInputItemMutationDto }
  | {
      kind: "insert_reference_item";
      target: WorkflowInputTargetDto;
      item: WorkflowInputItemMutationDto;
      insertion_index: number;
    }
  | {
      kind: "move_reference_item";
      target: WorkflowInputTargetDto;
      input_item_id: string;
      insertion_index_after_removal: number;
    }
  | { kind: "remove_input_item"; target: WorkflowInputTargetDto; input_item_id: string }
  | {
      kind: "set_input_item_role";
      target: WorkflowInputTargetDto;
      input_item_id: string;
      input_role_key: string;
    };

export interface WorkflowInputTargetDto {
  node_id: string;
  input_key: string;
}

export interface WorkflowInputItemMutationDto {
  input_item_id: string;
  source_node_id: string;
  source_output_key: string;
  input_role_key: string | null;
}

export type WorkflowRunScopeDto =
  | { kind: "whole_workflow" }
  | { kind: "through_node"; node_id: string };

export interface WorkflowRunDto {
  workflow_run_id: string;
  project_id: string;
  workflow_id: string;
  workflow_revision: string;
  scope: WorkflowRunScopeDto;
  state: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  created_at_epoch_ms: string;
  updated_at_epoch_ms: string;
  node_executions: Array<{
    node_id: string;
    node_execution_id: string;
    state: "pending" | "running" | "succeeded" | "failed" | "cancelled" | "blocked";
    progress_basis_points: number | null;
  }>;
}

export interface DurableWorkflowRunEventDto {
  workflow_run_id: string;
  sequence: string;
  occurred_at_epoch_ms: string;
  payload: JsonObject & { type: string };
}

export interface WorkflowRunEventPageDto {
  events: DurableWorkflowRunEventDto[];
  next_sequence: string | null;
}

export interface WorkflowNodePresentationDto {
  node_id: string;
  current_revision: string;
  capability_id: string;
  capability_version: string;
  readiness: WorkflowReadinessDto;
  latest_execution: {
    workflow_run_id: string;
    node_execution_id: string;
    state: "pending" | "running" | "succeeded" | "failed" | "cancelled" | "blocked";
    progress_basis_points: number | null;
    producing_revision: string;
    is_stale: boolean;
    failure: JsonValue | null;
    block_reason: JsonValue | null;
  } | null;
  presentation:
    | {
        kind: "text";
        value: Array<
          | { kind: "literal"; value: string }
          | { kind: "input_item_reference"; input_item_id: string }
        > | null;
      }
    | {
        kind: "image" | "video" | "audio";
        value: {
          asset_id: string;
          content_fingerprint_hex: string;
        } | null;
        preview_uri: string | null;
      };
}
