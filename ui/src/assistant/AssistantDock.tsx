import { useEffect, useRef, useState } from "react";
import {
  api,
  type AssistantContext,
  type AssistantPendingWorkflowChange,
  type AssistantPresentationEvent,
  type WorkflowApi,
  type WorkflowHead,
} from "../api/index.ts";
import "./assistantDock.css";
import { AssistantApprovalCard } from "./AssistantApprovalCard.tsx";
import {
  StrongAssistantTask,
} from "./StrongAssistantTask.tsx";
import { errorCode, failureCopy } from "../workflow/failureCopy.ts";
import {
  appendAssistantToken,
  setStepState,
  type AssistantStreamItem,
} from "./assistantStream.ts";

// Keep the product projection small while preserving the order of native stream events.
type StreamItem = AssistantStreamItem;

const SUGGESTIONS = [
  "Build a text-to-image → image-to-video pipeline",
  "Add a Text Prompt node and describe a sunset city",
  "Explain what each node in my canvas does",
];
const EMPTY_CONTEXT: AssistantContext = {
  project_id: null,
  workflow_present: false,
  workflow_revision: null,
  selected_node_ids: [],
  selected_asset_ids: [],
};
const PASS_BARRIER = async () => undefined;
const GET_EMPTY_CONTEXT = () => EMPTY_CONTEXT;

export function AssistantDock({
  onClose,
  apiClient = api,
  getContext = GET_EMPTY_CONTEXT,
  beforeSend = PASS_BARRIER,
  nodeLabel,
}: {
  onClose: () => void;
  apiClient?: WorkflowApi;
  getContext?: () => AssistantContext;
  beforeSend?: (restoreFocus: () => void) => Promise<void>;
  onWorkflowHead?: (head: WorkflowHead) => void | Promise<void>;
  nodeLabel?: (nodeId: string) => string;
}) {
  const [draft, setDraft] = useState("");
  const [items, setItems] = useState<StreamItem[]>([]);
  const [unavailable, setUnavailable] = useState<string | null>(null);
  const [status, setStatus] = useState<{ text: string; connected: boolean }>({
    text: "Ready",
    connected: true,
  });
  const [pendingApproval, setPendingApproval] = useState<AssistantPendingWorkflowChange | null>(null);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const streamRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const sendingRef = useRef(false);
  const presentationSequenceRef = useRef(new Map<string, number>());

  // Keep the newest content in view as it streams in.
  useEffect(() => {
    const el = streamRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [items]);

  useEffect(() => {
    const projectId = getContext().project_id;
    if (projectId) {
      void apiClient.assistantGetPendingWorkflowChange(projectId).then(setPendingApproval).catch(() => {});
    }
  }, [apiClient, getContext]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void apiClient.observeAssistantPresentationEvents((event) => {
      if (!disposed) handleEvent(event);
    }).then((value) => {
      if (disposed) value();
      else unlisten = value;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [apiClient, getContext]);

  const handleEvent = (event: AssistantPresentationEvent) => {
    const sequence = Number(event.sequence);
    const previous = presentationSequenceRef.current.get(event.invocation_id) ?? 0;
    if (!Number.isSafeInteger(sequence) || sequence !== previous + 1) {
      const projectId = getContext().project_id;
      if (projectId) {
        void apiClient.assistantGetPendingWorkflowChange(projectId).then(setPendingApproval);
      }
      return;
    }
    presentationSequenceRef.current.set(event.invocation_id, sequence);
    switch (event.kind) {
      case "text_delta":
        setItems((current) => appendAssistantToken(current, event.text));
        break;
      case "tool_activity":
        if (event.state === "started") {
          setItems((current) => [
            ...current,
            { kind: "step", callId: event.tool_id, capability: event.tool_id, state: "running" },
          ]);
        } else {
          setItems((current) =>
            setStepState(current, event.tool_id, event.state === "completed" ? "done" : "error")
          );
        }
        break;
      case "workflow_change_ready": {
        const projectId = getContext().project_id;
        if (projectId) {
          void apiClient.assistantGetPendingWorkflowChange(projectId).then((change) => {
            if (change?.workflow_change_id === event.workflow_change_id) {
              setPendingApproval(change);
              setStatus((current) => ({ ...current, text: "Waiting for approval" }));
            }
          });
        }
        break;
      }
      case "invocation_completed":
        presentationSequenceRef.current.delete(event.invocation_id);
        setStatus((current) => ({ ...current, text: "Ready" }));
        break;
      case "invocation_failed":
        presentationSequenceRef.current.delete(event.invocation_id);
        setStatus((current) => ({ ...current, text: event.error.message }));
        break;
    }
  };

  const send = async () => {
    const text = draft.trim();
    if (!text || sendingRef.current) {
      return;
    }
    sendingRef.current = true;
    try {
      await beforeSend(() => composerRef.current?.focus());
      setItems((current) => [...current, { kind: "user", text }]);
      const context = getContext();
      const { project_id, ...workspace } = context;
      if (!project_id) throw new Error("Open a project before using the assistant.");
      setStatus({ text: "Thinking", connected: true });
      await apiClient.assistantSendMessage({
        project_id,
        ...workspace,
        workflow_revision: workspace.workflow_revision?.toString() ?? null,
        text,
      });
      setUnavailable(null);
      setStatus({ text: "Ready", connected: true });
      setDraft("");
    } catch (error: unknown) {
      if (errorCode(error) === "provider.unavailable") {
        setUnavailable(
          "The assistant is unavailable right now — the local model provider did not respond. Check your model configuration, then try again.",
        );
        setStatus({ text: "Unavailable", connected: false });
      } else {
        setStatus((current) => ({ ...current, text: failureCopy("Send message", error) }));
      }
      composerRef.current?.focus();
    } finally {
      sendingRef.current = false;
    }
  };

  const decideApproval = async (approved: boolean) => {
    if (!pendingApproval || approvalBusy) return;
    setApprovalBusy(true);
    setStatus((current) => ({ ...current, text: approved ? "Applying" : "Rejecting" }));
    try {
      await apiClient.assistantDecideWorkflowChange({
        project_id: pendingApproval.project_id,
        workflow_change_id: pendingApproval.workflow_change_id,
        approval_scope_id: pendingApproval.approval_scope_id,
        mutation_digest_hex: pendingApproval.mutation_digest_hex,
        decision: approved ? "approve" : "reject",
      });
      setPendingApproval(null);
      setStatus((current) => ({ ...current, text: approved ? "Applied" : "Rejected" }));
    } catch (error: unknown) {
      const replacement =
        await apiClient.assistantGetPendingWorkflowChange(pendingApproval.project_id);
      setPendingApproval(replacement);
      setStatus((current) => ({ ...current, text: failureCopy("Apply change", error) }));
    } finally {
      setApprovalBusy(false);
    }
  };

  const hasContent = items.length > 0;

  return (
    <aside className="adock" aria-label="Assistant">
      <div className="adock__head">
        <BrandMark />
        <b>Assistant</b>
        <span
          className={`adock__dot${status.connected ? "" : " is-off"}`}
          title={status.text}
          aria-label={status.text}
        />
        <button className="adock__ico" onClick={onClose} aria-label="Close assistant">
          <CloseIcon />
        </button>
      </div>

      <div className="adock__stream" ref={streamRef}>
        {hasContent ? (
          items.map((item, index) => <StreamRow key={index} item={item} />)
        ) : (
          <EmptyState onPick={(text) => setDraft(text)} />
        )}
        {pendingApproval ? (
          <AssistantApprovalCard
            approval={pendingApproval}
            busy={approvalBusy}
            onDecision={(value) => void decideApproval(value)}
            nodeLabel={nodeLabel}
          />
        ) : null}
      </div>

      <div className="adock__composer">
        {unavailable && (
          <p className="adock__unavailable" role="status">
            {unavailable}
          </p>
        )}
        <div className="adock__cbox">
          <textarea
            ref={composerRef}
            name="assistant-message"
            placeholder="Message the assistant"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                void send();
              }
            }}
          />
          <button className="adock__send" onClick={() => void send()} aria-label="Send">
            <SendIcon />
          </button>
        </div>
        <div className="adock__hint">The assistant can edit the canvas and configuration.</div>
      </div>
    </aside>
  );
}

function StreamRow({
  item,
}: {
  item: StreamItem;
}) {
  if (item.kind === "user") {
    return <div className="adock__u">{item.text}</div>;
  }
  if (item.kind === "assistant") {
    return (
      <div className="adock__a">
        <span className="adock__amk">
          <BrandGlyph />
        </span>
        <div className="adock__abody">{item.text}</div>
      </div>
    );
  }
  if (item.kind === "plan" || item.kind === "run") {
    return <StrongAssistantTask item={item} />;
  }
  return (
    <div className={`adock__step is-${item.state}`}>
      <span className="adock__si">
        {item.state === "running" ? (
          <span className="adock__spin" />
        ) : item.state === "done" ? (
          <CheckIcon />
        ) : (
          <ErrorIcon />
        )}
      </span>
      <span className="adock__scap">{item.capability}</span>
    </div>
  );
}

function EmptyState({ onPick }: { onPick: (text: string) => void }) {
  return (
    <div className="adock__empty">
      <span className="adock__glow">
        <BrandGlyph large />
      </span>
      <h4>How can I help?</h4>
      <p>Ask me to build workflows, add nodes, or change settings.</p>
      <div className="adock__sugg">
        {SUGGESTIONS.map((text) => (
          <button key={text} onClick={() => onPick(text)}>
            {text}
          </button>
        ))}
      </div>
    </div>
  );
}

function BrandMark() {
  return (
    <svg className="adock__mk" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="1.2" y="1.2" width="21.6" height="21.6" rx="6.4" fill="var(--accent)" />
      <path d="M9.2 7.6 L16.6 12 L9.2 16.4 Z" fill="var(--on-accent)" />
    </svg>
  );
}

function BrandGlyph({ large }: { large?: boolean } = {}) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" width={large ? 24 : 12} height={large ? 24 : 12}>
      <path d="M9.2 7.6 L16.6 12 L9.2 16.4 Z" fill="var(--on-accent)" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
      <path d="M5 12l5 5L20 6" />
    </svg>
  );
}

function ErrorIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  );
}

function SendIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M4 12h15M13 6l6 6-6 6" />
    </svg>
  );
}
