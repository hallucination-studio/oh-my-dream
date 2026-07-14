import { useEffect, useRef, useState } from "react";
import {
  api,
  type AssistantContext,
  type AssistantPendingApproval,
  type ResponsesStreamEvent,
  type WorkflowApi,
  type WorkflowHead,
} from "../api/index.ts";
import "./assistantDock.css";
import { AssistantApprovalCard } from "./AssistantApprovalCard.tsx";

// Keep the product projection small while preserving the order of native stream events.
type StreamItem =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
  | {
      kind: "step";
      callId: string;
      capability: string;
      state: "running" | "done" | "error";
      error?: string;
    }

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
  onWorkflowHead,
}: {
  onClose: () => void;
  apiClient?: WorkflowApi;
  getContext?: () => AssistantContext;
  beforeSend?: (restoreFocus: () => void) => Promise<void>;
  onWorkflowHead?: (head: WorkflowHead) => void | Promise<void>;
}) {
  const [draft, setDraft] = useState("");
  const [items, setItems] = useState<StreamItem[]>([]);
  const [status, setStatus] = useState<{ text: string; connected: boolean }>({
    text: "Ready",
    connected: true,
  });
  const [pendingApproval, setPendingApproval] = useState<AssistantPendingApproval | null>(null);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const streamRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const sendingRef = useRef(false);

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
      void apiClient.getPendingAssistantApproval(projectId).then(setPendingApproval).catch(() => {});
    }
  }, [apiClient, getContext]);

  const handleEvent = (event: ResponsesStreamEvent) => {
    switch (event.type) {
      case "response.output_text.delta":
        if (typeof event.delta === "string") {
          const delta = event.delta;
          setItems((current) => appendAssistantToken(current, delta));
        }
        break;
      case "response.output_item.added": {
        const call = functionCallFromEvent(event);
        if (call) {
          setItems((current) => [
            ...current,
            { kind: "step", callId: call.callId, capability: call.name, state: "running" },
          ]);
        }
        break;
      }
      case "response.output_item.done": {
        const call = functionCallFromEvent(event);
        if (call) {
          setItems((current) => setStepState(current, call.callId, "done"));
        }
        break;
      }
      case "response.completed":
        setStatus((current) => ({ ...current, text: "Ready" }));
        break;
      case "response.failed":
      case "response.incomplete":
      case "error":
        setStatus((current) => ({ ...current, text: responseError(event) }));
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
      const workflowHead = await apiClient.sendAssistant(
        {
          project_id,
          ...workspace,
          text,
        },
        handleEvent,
      );
      if (workflowHead !== null) {
        await onWorkflowHead?.(workflowHead);
      }
      setStatus((current) => ({ ...current, text: "Ready" }));
      setDraft("");
    } catch (error: unknown) {
      const message = String(error);
      if (message.includes("ASSISTANT_APPROVAL_DEFERRED")) {
        const projectId = getContext().project_id;
        setPendingApproval(
          projectId ? await apiClient.getPendingAssistantApproval(projectId) : null,
        );
        setStatus((current) => ({ ...current, text: "Waiting for approval" }));
        setDraft("");
      } else {
        setStatus((current) => ({ ...current, text: message }));
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
      const workflowHead = await apiClient.decideAssistantApproval(
        {
          project_id: pendingApproval.project_id,
          candidate_digest: pendingApproval.candidate_digest,
          approved,
        },
        handleEvent,
      );
      if (workflowHead !== null) await onWorkflowHead?.(workflowHead);
      setPendingApproval(null);
      setStatus((current) => ({ ...current, text: approved ? "Applied" : "Rejected" }));
    } catch (error: unknown) {
      setStatus((current) => ({ ...current, text: String(error) }));
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
          />
        ) : null}
      </div>

      <div className="adock__composer">
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

function appendAssistantToken(items: StreamItem[], delta: string): StreamItem[] {
  const last = items.at(-1);
  if (last?.kind === "assistant") {
    return [...items.slice(0, -1), { kind: "assistant", text: last.text + delta }];
  }
  return [...items, { kind: "assistant", text: delta }];
}

function setStepState(
  items: StreamItem[],
  callId: string,
  state: "done" | "error",
): StreamItem[] {
  return items.map((item) =>
    item.kind === "step" && item.callId === callId ? { ...item, state } : item,
  );
}

function functionCallFromEvent(
  event: ResponsesStreamEvent,
): { callId: string; name: string } | null {
  if (!isJsonObject(event.item) || event.item.type !== "function_call") {
    return null;
  }
  if (typeof event.item.call_id !== "string" || typeof event.item.name !== "string") {
    return null;
  }
  return { callId: event.item.call_id, name: event.item.name };
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function responseError(event: ResponsesStreamEvent): string {
  if (typeof event.error === "string") {
    return event.error;
  }
  if (isJsonObject(event.error) && typeof event.error.message === "string") {
    return event.error.message;
  }
  return "Assistant response failed";
}

function BrandMark() {
  return (
    <svg className="adock__mk" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="1.2" y="1.2" width="21.6" height="21.6" rx="6.4" fill="#14161d" />
      <path d="M9.2 7.6 L16.6 12 L9.2 16.4 Z" fill="#fff" />
    </svg>
  );
}

function BrandGlyph({ large }: { large?: boolean } = {}) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" width={large ? 24 : 12} height={large ? 24 : 12}>
      <path d="M9.2 7.6 L16.6 12 L9.2 16.4 Z" fill="#fff" />
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
