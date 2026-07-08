export interface AssistantSocket {
  onopen: (() => void) | null;
  onmessage: ((event: { data: string }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  send: (data: string) => void;
  close: () => void;
}

/**
 * Events surfaced to the dock. Beyond raw token/status streaming, the client
 * exposes the lifecycle of a tool call (running → succeeded/failed) and a
 * confirm request for cost-incurring or destructive capabilities, so the UI can
 * render step rows and a human-in-the-loop confirmation card.
 */
export type AssistantEvent =
  | { type: "token"; delta: string }
  | { type: "status"; text: string }
  | { type: "message_done" }
  | { type: "error"; message: string }
  | { type: "closed" }
  | { type: "tool_started"; callId: string; capability: string; args: Record<string, unknown> }
  | { type: "tool_succeeded"; callId: string }
  | { type: "tool_failed"; callId: string; error: string }
  | { type: "confirm_request"; callId: string; capability: string; args: Record<string, unknown>; summary: string };

interface AssistantClientOptions {
  url: string;
  token: string;
  manifest: unknown;
  socketFactory?: (url: string) => AssistantSocket;
  execute: (capability: string, args: Record<string, unknown>) => Promise<unknown>;
  onEvent: (event: AssistantEvent) => void;
}

export class AssistantClient {
  private socket: AssistantSocket | null = null;

  constructor(private readonly options: AssistantClientOptions) {}

  connect(): void {
    const socket = this.options.socketFactory
      ? this.options.socketFactory(this.options.url)
      : browserSocket(this.options.url);
    this.socket = socket;
    socket.onopen = () => this.send({ type: "auth", token: this.options.token });
    socket.onmessage = (event) => this.handleFrame(event.data);
    socket.onclose = () => this.options.onEvent({ type: "closed" });
    socket.onerror = () => this.options.onEvent({ type: "error", message: "Assistant socket error" });
  }

  sendUserMessage(text: string, context: unknown): void {
    this.send({ type: "user_message", text, context });
  }

  cancel(): void {
    this.send({ type: "cancel" });
  }

  /** Resolve a pending confirm_request with the user's decision. */
  resolveConfirm(callId: string, approved: boolean): void {
    this.send({ type: "confirm_result", call_id: callId, approved });
  }

  close(): void {
    this.socket?.close();
  }

  private handleFrame(data: string): void {
    const frame = JSON.parse(data) as Record<string, unknown>;
    switch (frame.type) {
      case "auth_ok":
        this.send({ type: "client_ready", manifest: this.options.manifest });
        break;
      case "token":
        this.options.onEvent({ type: "token", delta: stringField(frame, "delta") });
        break;
      case "status":
        this.options.onEvent({ type: "status", text: stringField(frame, "text") });
        break;
      case "message_done":
        this.options.onEvent({ type: "message_done" });
        break;
      case "error":
        this.options.onEvent({ type: "error", message: stringField(frame, "message") });
        break;
      case "confirm_request":
        this.options.onEvent({
          type: "confirm_request",
          callId: stringField(frame, "call_id"),
          capability: stringField(frame, "capability"),
          args: recordField(frame, "args"),
          summary: stringField(frame, "summary"),
        });
        break;
      case "tool_call":
        void this.handleToolCall(frame);
        break;
    }
  }

  private async handleToolCall(frame: Record<string, unknown>): Promise<void> {
    const callId = stringField(frame, "call_id");
    const capability = stringField(frame, "capability");
    const args = recordField(frame, "args");
    this.options.onEvent({ type: "tool_started", callId, capability, args });
    try {
      const result = await this.options.execute(capability, args);
      this.send({ type: "tool_result", call_id: callId, ok: true, result });
      this.options.onEvent({ type: "tool_succeeded", callId });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.send({ type: "tool_result", call_id: callId, ok: false, error: message });
      this.options.onEvent({ type: "tool_failed", callId, error: message });
    }
  }

  private send(frame: unknown): void {
    this.socket?.send(JSON.stringify(frame));
  }
}

function browserSocket(url: string): AssistantSocket {
  const socket = new WebSocket(url);
  const wrapped: AssistantSocket = {
    onopen: null,
    onmessage: null,
    onclose: null,
    onerror: null,
    send: (data) => socket.send(data),
    close: () => socket.close(),
  };
  socket.onopen = () => wrapped.onopen?.();
  socket.onmessage = (event) => wrapped.onmessage?.({ data: String(event.data) });
  socket.onclose = () => wrapped.onclose?.();
  socket.onerror = () => wrapped.onerror?.();
  return wrapped;
}

function stringField(frame: Record<string, unknown>, name: string): string {
  const value = frame[name];
  if (typeof value !== "string") {
    throw new Error(`Assistant frame field \`${name}\` must be a string`);
  }
  return value;
}

function recordField(frame: Record<string, unknown>, name: string): Record<string, unknown> {
  const value = frame[name];
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Assistant frame field \`${name}\` must be an object`);
  }
  return value as Record<string, unknown>;
}
