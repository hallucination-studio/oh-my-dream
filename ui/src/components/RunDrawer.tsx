import { useEffect, useRef, useState } from "react";
import {
  api,
  type GenerationTaskDto,
  type GenerationTaskSummaryDto,
  type WorkflowApi,
  type WorkflowRunDto,
} from "../api/index.ts";
import type { RunOutputs } from "../workflow/types.ts";
import {
  elapsedMs,
  formatElapsed,
  projectRunTimeline,
  runHeadline,
  stepStateLabel,
} from "../workflow/runTimeline.ts";
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
  nodeLabel = () => "Step",
  canCancel = false,
  onCancel = () => undefined,
  autoFocus = true,
}: {
  open: boolean;
  onClose: () => void;
  projectId: string | null;
  run: WorkflowRunDto | null;
  activeNodeId?: string | null;
  outputPreview?: RunOutputs | null;
  taskApi?: TaskApi;
  nodeLabel?: (nodeId: string) => string;
  canCancel?: boolean;
  onCancel?: () => void;
  /** Explicit opens focus the close control; run-admission opens must not steal focus. */
  autoFocus?: boolean;
}) {
  const [tasks, setTasks] = useState<GenerationTaskSummaryDto[]>([]);
  const [state, setState] = useState<"idle" | "loading" | "error">("idle");
  const [now, setNow] = useState(() => Date.now());
  const closeButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open || !autoFocus) return;
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

  useEffect(() => {
    if (!open || run === null || (run.state !== "queued" && run.state !== "running")) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [open, run]);

  if (!open) return null;

  const task = selectTask(tasks, run, activeNodeId);
  return (
    <div className="rundrawer__host">
      <aside className="rundrawer" aria-labelledby="run-drawer-title">
        <header className="rundrawer__head">
          <div>
            <span className="rundrawer__eyebrow">Run information</span>
            <h2 id="run-drawer-title">Run details</h2>
          </div>
          <button ref={closeButton} className="rundrawer__close" onClick={onClose} aria-label="Close run details">
            ×
          </button>
        </header>
        <div className="rundrawer__body" aria-busy={state === "loading"}>
          {run !== null && (
            <section className="rundrawer__timeline" aria-label="Run steps">
              <div className="rundrawer__runhead">
                <strong>{runHeadline(run)}</strong>
                <span className="rundrawer__elapsed">{formatElapsed(elapsedMs(run, now))}</span>
                {canCancel && (
                  <button className="rundrawer__cancel" onClick={onCancel}>
                    Cancel run
                  </button>
                )}
              </div>
              <ol className="rundrawer__steps">
                {projectRunTimeline(run).map((step, index) => (
                  <li key={step.executionId} data-state={step.state}>
                    <span className="rundrawer__stepstate">{stepStateLabel(step.state)}</span>
                    <span className="rundrawer__stepname">{nodeLabel(step.nodeId)}</span>
                    <span className="rundrawer__stepmeta">
                      {step.progressBasisPoints !== null
                        ? `${Math.round(step.progressBasisPoints / 100)}%`
                        : `Step ${index + 1} of ${run.node_executions.length}`}
                    </span>
                  </li>
                ))}
              </ol>
            </section>
          )}
          {state === "loading" && <p className="rundrawer__status" role="status">Loading Task information…</p>}
          {state === "error" && (
            <p className="rundrawer__status rundrawer__status--error" role="status">
              Task information is unavailable.
            </p>
          )}
          {state === "idle" && run === null && (
            <p className="rundrawer__status" role="status">No Run is selected.</p>
          )}
          {state === "idle" && run !== null && task !== null && <TaskDetails task={task} nodeLabel={nodeLabel} />}
          {outputPreview && <OutputPreview outputs={outputPreview} />}
        </div>
      </aside>
    </div>
  );
}

function TaskDetails({
  task,
  nodeLabel,
}: {
  task: GenerationTaskSummaryDto | GenerationTaskDto;
  nodeLabel: (nodeId: string) => string;
}) {
  return (
    <section className="rundrawer__task" aria-label="Step details">
      <div className="rundrawer__taskhead">
        <div>
          <span className="rundrawer__label">Step details</span>
          <strong>{nodeLabel(task.workflow_node_id)}</strong>
        </div>
        <span className={`rundrawer__badge rundrawer__badge--${task.status}`}>
          {statusLabel(task.status)}
        </span>
      </div>
      <dl className="rundrawer__facts">
        <div><dt>Generation model</dt><dd><ModelName task={task} /></dd></div>
        <div><dt>Provider</dt><dd>{task.provider_display_name ?? "Configured provider"}</dd></div>
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
          <strong>{task.failure.message ?? "This step failed."}</strong>
        </div>
      )}
      {"result" in task && task.result && <ResultSummary result={task.result} />}
      {!("result" in task) && task.has_result && (
        <p className="rundrawer__result">Output is available in the step result.</p>
      )}
      <details className="rundrawer__diagnostics">
        <summary>Diagnostics</summary>
        <dl className="rundrawer__facts">
          <div><dt>Model reference</dt><dd className="is-mono">{task.generation_profile_ref}</dd></div>
          <div><dt>Provider id</dt><dd className="is-mono">{task.provider_id}</dd></div>
          <div><dt>Task id</dt><dd className="is-mono">{task.id}</dd></div>
          {task.failure && <div><dt>Failure code</dt><dd className="is-mono">{task.failure.code}</dd></div>}
        </dl>
      </details>
    </section>
  );
}

/** Resolves the model display name by joining the profile list; never leaks the raw ref. */
function ModelName({ task }: { task: GenerationTaskSummaryDto | GenerationTaskDto }) {
  const [name, setName] = useState<string | null>(null);
  useEffect(() => {
    const capability = capabilityForRequestKind(task.request_kind);
    if (!capability) return;
    let active = true;
    void api
      .generationProfileListForCapability(capability)
      .then((profiles) => {
        if (!active) return;
        setName(
          profiles.find((profile) => profile.profile_ref === task.generation_profile_ref)
            ?.display_name ?? null,
        );
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [task.request_kind, task.generation_profile_ref]);
  return <>{name ?? "Configured generation model"}</>;
}

function capabilityForRequestKind(
  kind: GenerationTaskSummaryDto["request_kind"],
): { id: string; version: string } | null {
  switch (kind) {
    case "image":
      return { id: "image.generate_from_text", version: "1.0" };
    case "video":
      return { id: "video.generate_from_image", version: "1.0" };
    case "voice":
      return { id: "audio.synthesize_speech_from_text", version: "1.0" };
    default:
      return null;
  }
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
