import { useEffect, useState } from "react";
import { api, type WorkflowDto, type WorkflowReadinessDto } from "../api/index.ts";

/** Re-queries engine-owned readiness whenever the Workflow head revision changes. */
export function useWorkflowReadiness(workflow: WorkflowDto | null): WorkflowReadinessDto | null {
  const [readiness, setReadiness] = useState<WorkflowReadinessDto | null>(null);
  const projectId = workflow?.project_id ?? null;
  const workflowId = workflow?.workflow_id ?? null;
  const revision = workflow?.revision ?? null;

  useEffect(() => {
    if (!projectId || !workflowId) {
      setReadiness(null);
      return;
    }
    let active = true;
    void api
      .workflowCheckReadiness(projectId, workflowId)
      .then((result) => {
        if (active) setReadiness(result);
      })
      .catch(() => {
        if (active) setReadiness(null);
      });
    return () => {
      active = false;
    };
  }, [projectId, workflowId, revision]);

  return readiness;
}
