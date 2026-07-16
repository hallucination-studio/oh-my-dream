import { useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { Node } from "@xyflow/react";
import { api, type WorkflowDto } from "../api/index.ts";
import type { FlowNodeData } from "../nodes/WorkflowFlowNode.tsx";

export function useNodePresentation(
  workflow: WorkflowDto | null,
  selectedNodeId: string | null,
  setNodes: Dispatch<SetStateAction<Node[]>>,
): void {
  useEffect(() => {
    if (!workflow || !selectedNodeId) return;
    let active = true;
    void api
      .workflowGetNodePresentation(
        workflow.project_id,
        workflow.workflow_id,
        selectedNodeId,
      )
      .then((view) => {
        if (!active || view.current_revision !== workflow.revision) return;
        setNodes((nodes) =>
          nodes.map((node) => {
            if (node.id !== selectedNodeId) return node;
            const data = node.data as FlowNodeData;
            const state = executionState(view.latest_execution?.state);
            if (view.presentation.kind === "text") {
              return {
                ...node,
                data: {
                  ...data,
                  runtime: { ...data.runtime, state },
                  textPresentation: textValue(view.presentation.value),
                },
              };
            }
            return {
              ...node,
              data: {
                ...data,
                runtime: {
                  ...data.runtime,
                  state,
                  preview: {
                    kind: view.presentation.kind,
                    url: view.presentation.preview_uri,
                  },
                },
              },
            };
          }),
        );
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [selectedNodeId, setNodes, workflow]);
}

function textValue(
  parts:
    | Array<
        | { kind: "literal"; value: string }
        | { kind: "input_item_reference"; input_item_id: string }
      >
    | null,
): string | null {
  if (!parts) return null;
  return parts
    .map((part) => (part.kind === "literal" ? part.value : `{{${part.input_item_id}}}`))
    .join("");
}

function executionState(
  state:
    | "pending"
    | "running"
    | "succeeded"
    | "failed"
    | "cancelled"
    | "blocked"
    | undefined,
) {
  if (state === "running") return "running" as const;
  if (state === "succeeded") return "done" as const;
  if (state === "failed" || state === "blocked") return "error" as const;
  return "idle" as const;
}
