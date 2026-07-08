import { describe, expect, it, vi } from "vitest";
import { AssistantClient, type AssistantSocket } from "./assistantClient.ts";

describe("AssistantClient", () => {
  it("authenticates, sends the manifest, streams tokens, and returns tool results", async () => {
    const socket = new FakeSocket();
    const execute = vi.fn().mockResolvedValue({ id: "n1" });
    const events: unknown[] = [];
    const client = new AssistantClient({
      url: "ws://127.0.0.1:55123",
      token: "secret-token",
      manifest: { capabilities: [{ name: "workflow.add_node" }] },
      socketFactory: () => socket,
      execute,
      onEvent: (event) => events.push(event),
    });

    client.connect();
    socket.open();
    socket.receive({ type: "auth_ok" });
    client.sendUserMessage("add a prompt node", { project_id: "default" });
    socket.receive({ type: "token", delta: "Done" });
    socket.receive({
      type: "tool_call",
      call_id: "call-1",
      capability: "workflow.add_node",
      args: { node_type: "TextPrompt" },
    });
    await flushPromises();

    expect(socket.sent).toEqual([
      { type: "auth", token: "secret-token" },
      { type: "client_ready", manifest: { capabilities: [{ name: "workflow.add_node" }] } },
      {
        type: "user_message",
        text: "add a prompt node",
        context: { project_id: "default" },
      },
      { type: "tool_result", call_id: "call-1", ok: true, result: { id: "n1" } },
    ]);
    expect(execute).toHaveBeenCalledWith("workflow.add_node", { node_type: "TextPrompt" });
    expect(events).toContainEqual({ type: "token", delta: "Done" });
  });

  it("returns tool errors without throwing out of the socket handler", async () => {
    const socket = new FakeSocket();
    const client = new AssistantClient({
      url: "ws://127.0.0.1:55123",
      token: "secret-token",
      manifest: { capabilities: [] },
      socketFactory: () => socket,
      execute: vi.fn().mockRejectedValue(new Error("bad args")),
      onEvent: vi.fn(),
    });

    client.connect();
    socket.open();
    socket.receive({ type: "auth_ok" });
    socket.receive({ type: "tool_call", call_id: "call-2", capability: "missing", args: {} });
    await flushPromises();

    expect(socket.sent.at(-1)).toEqual({
      type: "tool_result",
      call_id: "call-2",
      ok: false,
      error: "bad args",
    });
  });

  it("surfaces tool-call lifecycle and confirm requests, and resolves confirms", async () => {
    const socket = new FakeSocket();
    const events: unknown[] = [];
    const client = new AssistantClient({
      url: "ws://127.0.0.1:55123",
      token: "secret-token",
      manifest: { capabilities: [] },
      socketFactory: () => socket,
      execute: vi.fn().mockResolvedValue({ id: "n9" }),
      onEvent: (event) => events.push(event),
    });

    client.connect();
    socket.open();
    socket.receive({ type: "auth_ok" });
    socket.receive({
      type: "tool_call",
      call_id: "call-3",
      capability: "workflow.add_node",
      args: { node_type: "TextPrompt" },
    });
    await flushPromises();

    expect(events).toContainEqual({
      type: "tool_started",
      callId: "call-3",
      capability: "workflow.add_node",
      args: { node_type: "TextPrompt" },
    });
    expect(events).toContainEqual({ type: "tool_succeeded", callId: "call-3" });

    socket.receive({
      type: "confirm_request",
      call_id: "call-4",
      capability: "run_workflow",
      args: { project_id: "default" },
      summary: "Run the workflow (incurs cost)",
    });
    expect(events).toContainEqual({
      type: "confirm_request",
      callId: "call-4",
      capability: "run_workflow",
      args: { project_id: "default" },
      summary: "Run the workflow (incurs cost)",
    });

    client.resolveConfirm("call-4", true);
    expect(socket.sent.at(-1)).toEqual({ type: "confirm_result", call_id: "call-4", approved: true });
  });
});

class FakeSocket implements AssistantSocket {
  sent: unknown[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;

  send(data: string): void {
    this.sent.push(JSON.parse(data) as unknown);
  }

  close(): void {
    this.onclose?.();
  }

  open(): void {
    this.onopen?.();
  }

  receive(frame: unknown): void {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }
}

async function flushPromises(): Promise<void> {
  for (let index = 0; index < 5; index += 1) {
    await Promise.resolve();
  }
}
