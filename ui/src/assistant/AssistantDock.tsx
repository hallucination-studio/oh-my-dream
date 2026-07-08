import { useEffect, useRef, useState } from "react";
import { api, type CapabilityManifest, type WorkflowApi } from "../api/index.ts";
import { AssistantClient, type AssistantEvent, type AssistantSocket } from "./assistantClient.ts";
import "./assistantDock.css";

// The stream is a list of typed entries rather than plain messages, so the dock
// can render user bubbles, full-width assistant text, tool-call step rows, and
// human-in-the-loop confirm cards inline, in the order they arrive.
type StreamItem =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
  | {
      kind: "step";
      callId: string;
      capability: string;
      args: Record<string, unknown>;
      state: "running" | "done" | "error";
      error?: string;
    }
  | { kind: "confirm"; callId: string; capability: string; summary: string; resolved: "approved" | "denied" | null };

const SUGGESTIONS = [
  "Build a text-to-image → image-to-video pipeline",
  "Add a Text Prompt node and describe a sunset city",
  "Explain what each node in my canvas does",
];

export function AssistantDock({
  onClose,
  apiClient = api,
  executeCapability = async () => null,
  getContext = () => ({}),
  socketFactory,
}: {
  onClose: () => void;
  apiClient?: WorkflowApi;
  executeCapability?: (capability: string, args: Record<string, unknown>) => Promise<unknown>;
  getContext?: () => unknown;
  socketFactory?: (url: string) => AssistantSocket;
}) {
  const [draft, setDraft] = useState("");
  const [items, setItems] = useState<StreamItem[]>([]);
  const [status, setStatus] = useState<{ text: string; connected: boolean }>({
    text: "Connecting",
    connected: false,
  });
  const client = useRef<AssistantClient | null>(null);
  const streamRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([apiClient.getAssistantSession(), apiClient.getCapabilityManifest()])
      .then(([session, manifest]) => {
        if (cancelled) {
          return;
        }
        if (session.port <= 0 || !session.token) {
          setStatus({ text: "Unavailable", connected: false });
          return;
        }
        const next = new AssistantClient({
          url: `ws://127.0.0.1:${session.port}`,
          token: session.token,
          manifest: compactManifest(manifest),
          socketFactory,
          execute: executeCapability,
          onEvent: handleEvent,
        });
        client.current = next;
        next.connect();
        setStatus({ text: "Connected", connected: true });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setStatus({ text: error instanceof Error ? error.message : String(error), connected: false });
        }
      });
    return () => {
      cancelled = true;
      client.current?.close();
      client.current = null;
    };
  }, [apiClient, executeCapability, socketFactory]);

  // Keep the newest content in view as it streams in.
  useEffect(() => {
    const el = streamRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [items]);

  const handleEvent = (event: AssistantEvent) => {
    switch (event.type) {
      case "token":
        setItems((current) => appendAssistantToken(current, event.delta));
        break;
      case "status":
        setStatus((s) => ({ ...s, text: event.text }));
        break;
      case "error":
        setStatus((s) => ({ ...s, text: event.message }));
        break;
      case "closed":
        setStatus({ text: "Disconnected", connected: false });
        break;
      case "tool_started":
        setItems((current) => [
          ...current,
          { kind: "step", callId: event.callId, capability: event.capability, args: event.args, state: "running" },
        ]);
        break;
      case "tool_succeeded":
        setItems((current) => setStepState(current, event.callId, "done"));
        break;
      case "tool_failed":
        setItems((current) => setStepState(current, event.callId, "error", event.error));
        break;
      case "confirm_request":
        setItems((current) => [
          ...current,
          { kind: "confirm", callId: event.callId, capability: event.capability, summary: event.summary, resolved: null },
        ]);
        break;
    }
  };

  const send = () => {
    const text = draft.trim();
    if (!text) {
      return;
    }
    setItems((current) => [...current, { kind: "user", text }]);
    if (client.current) {
      client.current.sendUserMessage(text, getContext());
    } else {
      setItems((current) => [...current, { kind: "assistant", text: "Assistant sidecar is not connected." }]);
    }
    setDraft("");
  };

  const resolveConfirm = (callId: string, approved: boolean) => {
    client.current?.resolveConfirm(callId, approved);
    setItems((current) =>
      current.map((item) =>
        item.kind === "confirm" && item.callId === callId
          ? { ...item, resolved: approved ? "approved" : "denied" }
          : item,
      ),
    );
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
          items.map((item, index) => <StreamRow key={index} item={item} onConfirm={resolveConfirm} />)
        ) : (
          <EmptyState onPick={(text) => setDraft(text)} />
        )}
      </div>

      <div className="adock__composer">
        <div className="adock__cbox">
          <textarea
            placeholder="Message the assistant"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                send();
              }
            }}
          />
          <button className="adock__send" onClick={send} aria-label="Send">
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
  onConfirm,
}: {
  item: StreamItem;
  onConfirm: (callId: string, approved: boolean) => void;
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
  if (item.kind === "step") {
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
        <span className="adock__sarg">{formatArgs(item.args)}</span>
      </div>
    );
  }
  // confirm
  return (
    <div className="adock__confirm">
      <div className="adock__ch">
        <WarnIcon />
        Confirm action
      </div>
      <div className="adock__cb">
        <code>{item.capability}</code> — {item.summary}
      </div>
      {item.resolved === null ? (
        <div className="adock__ca">
          <button className="adock__deny" onClick={() => onConfirm(item.callId, false)}>
            Cancel
          </button>
          <button className="adock__approve" onClick={() => onConfirm(item.callId, true)}>
            Run &amp; confirm
          </button>
        </div>
      ) : (
        <div className="adock__cresolved">{item.resolved === "approved" ? "Approved" : "Cancelled"}</div>
      )}
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

function compactManifest(manifest: CapabilityManifest): CapabilityManifest {
  return { capabilities: manifest.capabilities.map((capability) => ({ ...capability })) };
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
  error?: string,
): StreamItem[] {
  return items.map((item) =>
    item.kind === "step" && item.callId === callId ? { ...item, state, error } : item,
  );
}

function formatArgs(args: Record<string, unknown>): string {
  const parts = Object.entries(args).map(([key, value]) => `${key}: ${formatValue(value)}`);
  const joined = parts.join(", ");
  return joined.length > 48 ? `${joined.slice(0, 47)}…` : joined;
}

function formatValue(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (value === null || typeof value !== "object") {
    return String(value);
  }
  return Array.isArray(value) ? `[${value.length}]` : "{…}";
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

function WarnIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
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
