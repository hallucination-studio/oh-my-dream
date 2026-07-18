import { useEffect, useState } from "react";
import { api, type WorkflowDto, type WorkflowReadinessDto } from "../api/index.ts";

export type WorkflowReadiness = WorkflowReadinessDto | "error" | null;

/**
 * Re-queries engine-owned readiness whenever the Workflow head revision changes.
 * A failed query retries with backoff instead of wedging the Run button in a
 * silent "checking" state; an explicit "error" state survives only until the
 * next revision or a successful retry.
 */
export function useWorkflowReadiness(workflow: WorkflowDto | null): WorkflowReadiness {
  const [readiness, setReadiness] = useState<WorkflowReadiness>(null);
  const projectId = workflow?.project_id ?? null;
  const workflowId = workflow?.workflow_id ?? null;
  const revision = workflow?.revision ?? null;

  useEffect(() => {
    if (!projectId || !workflowId) {
      setReadiness(null);
      return;
    }
    let active = true;
    let attempt = 0;
    let timer: number | undefined;
    const query = () => {
      void api
        .workflowCheckReadiness(projectId, workflowId)
        .then((result) => {
          if (active) setReadiness(result);
        })
        .catch(() => {
          if (!active) return;
          attempt += 1;
          if (attempt >= 3) setReadiness("error");
          timer = window.setTimeout(query, Math.min(1500 * attempt, 6000));
        });
    };
    query();
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [projectId, workflowId, revision]);

  return readiness;
}
