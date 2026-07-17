import { useEffect, useRef, useState } from "react";
import {
  api,
  type GenerationTaskDto,
  type GenerationTaskSummaryDto,
  type WorkflowApi,
  type WorkflowRunDto,
} from "../api/index.ts";
import type { RunOutputs } from "../workflow/types.ts";
import "./runDrawer.css";

type TaskApi = Pick<WorkflowApi, "generationTaskList">;

export function RunDrawer({
  open,
  onClose,
  projectId,
  run,
  activeNodeId,
  outputPreview,
  taskApi = api,
}: {
  open: boolean;
  onClose: () => void;
  projectId: string | null;
  run: WorkflowRunDto | null;
  activeNodeId?: string | null;
  outputPreview?: RunOutputs | null;
  taskApi?: TaskApi;
}) {
  const [tasks, setTasks] = useState<GenerationTaskSummaryDto[]>([]);
  const [state, setState] = useState<"idle" | "loading" | "error">("idle");
  const closeButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    closeButton.current?.focus();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, open]);

  useEffect(() => {
    if (!open || projectId === null || run === null) {
      setTasks([]);
      setState("idle");
      return;
    }
    let active = true;
    setState("loading");
    void taskApi.generationTaskList(projectId, null, null, null, 100).then(
      (page) => {
        if (active) {
          setTasks(page.tasks.filter((task) => task.workflow_run_id === run.workflow_run_id));
          setState("idle");
        }
      },
      () => {
        if (active) {
          setTasks([]);
          setState("error");
        }
      },
    );
    return () => {
      active = false;
    };
  }, [open, projectId, run, taskApi]);

  if (!open) return null;

  const task = selectTask(tasks, run, activeNodeId);
  return (
    <div className="rundrawer__scrim" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="rundrawer" role="dialog" aria-modal="true" aria-labelledby="run-drawer-title">
        <header className="rundrawer__head">
          <div>
            <span className="rundrawer__eyebrow">Run information</span>
            <h2 id="run-drawer-title">Task details</h2>
          </div>
          <button ref={closeButton} className="rundrawer__close" onClick={onClose} aria-label="Close run details">
            ×
          </button>
        </header>
        <div className="rundrawer__body" aria-busy={state === "loading"}>
          {state === "loading" && <p className="rundrawer__status" role="status">Loading Task information…</p>}
          {state === "error" && (
            <p className="rundrawer__status rundrawer__status--error" role="status">
              Task information is unavailable.
            </p>
          )}
          {state === "idle" && run === null && (
            <p className="rundrawer__status" role="status">No Run is selected.</p>
          )}
          {state === "idle" && run !== null && task !== null && <TaskDetails task={task} />}
          {state === "idle" && run !== null && task === null && (
            <p className="rundrawer__status" role="status">
              This Run has no available Task for the selected Step.
            </p>
          )}
          {outputPreview && <OutputPreview outputs={outputPreview} />}
        </div>
      </div>
    </div>
  );
}

function TaskDetails({ task }: { task: GenerationTaskSummaryDto | GenerationTaskDto }) {
  return (
    <section className="rundrawer__task" aria-label="Generation Task">
      <div className="rundrawer__taskhead">
        <div>
          <span className="rundrawer__label">Generation Task</span>
          <strong>{task.request_kind}</strong>
        </div>
        <span className={`rundrawer__badge rundrawer__badge--${task.status}`}>
          {statusLabel(task.status)}
        </span>
      </div>
      <dl className="rundrawer__facts">
        <div><dt>Profile</dt><dd>{task.generation_profile_ref}</dd></div>
        <div><dt>Provider</dt><dd>{task.provider_display_name ?? task.provider_id}</dd></div>
        <div><dt>Created</dt><dd>{formatEpoch(task.created_at_epoch_ms)}</dd></div>
      </dl>
      {task.progress_percent !== null && (
        <div className="rundrawer__progress" aria-label={`Progress ${task.progress_percent}%`}>
          <div className="rundrawer__progressbar"><span style={{ width: `${task.progress_percent}%` }} /></div>
          <span>{task.progress_percent}%</span>
        </div>
      )}
      {task.prompt_preview && <p className="rundrawer__prompt">{task.prompt_preview}</p>}
      {task.failure && (
        <div className="rundrawer__failure" role="alert">
          <strong>{task.failure.code}</strong>
          <span>{task.failure.message}</span>
        </div>
      )}
      {"result" in task && task.result && <ResultSummary result={task.result} />}
      {!("result" in task) && task.has_result && (
        <p className="rundrawer__result">Output is available in the Task result.</p>
      )}
    </section>
  );
}

function ResultSummary({ result }: { result: GenerationTaskDto["result"] }) {
  if (!result) return null;
  return result.kind === "text"
    ? <p className="rundrawer__result">{result.content}</p>
    : <p className="rundrawer__result">{result.media_kind} output is available.</p>;
}

function OutputPreview({ outputs }: { outputs: RunOutputs }) {
  const items = Object.entries(outputs).flatMap(([nodeId, values]) =>
    Object.entries(values).map(([name, value]) => ({ nodeId, name, kind: value.kind })),
  );
  if (items.length === 0) return null;
  return (
    <section className="rundrawer__outputs" aria-label="Run outputs">
      <span className="rundrawer__label">Outputs</span>
      {items.map((item) => <div key={`${item.nodeId}:${item.name}`}><span>{item.kind}</span><strong>{item.name}</strong></div>)}
    </section>
  );
}

function selectTask(
  tasks: GenerationTaskSummaryDto[],
  run: WorkflowRunDto | null,
  activeNodeId: string | null | undefined,
): GenerationTaskSummaryDto | null {
  if (run === null) return null;
  const waiting = run.node_executions.find((execution) => execution.state === "waiting_for_external_completion");
  if (waiting) {
    return tasks.find((task) => task.workflow_node_execution_id === waiting.node_execution_id) ?? null;
  }
  if (activeNodeId) {
    return tasks.find((task) => task.workflow_node_id === activeNodeId) ?? null;
  }
  return tasks.length === 1 ? tasks[0]! : null;
}

function statusLabel(status: GenerationTaskSummaryDto["status"]): string {
  return status === "cancel_requested" ? "Cancel requested" : status[0]!.toUpperCase() + status.slice(1);
}

function formatEpoch(value: string): string {
  const date = new Date(Number(value));
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
