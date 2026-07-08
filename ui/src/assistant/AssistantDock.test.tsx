import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AssistantDock } from "./AssistantDock.tsx";
import type { AssistantSocket } from "./assistantClient.ts";
import type { WorkflowApi } from "../api/index.ts";

describe("AssistantDock", () => {
  it("connects to the assistant session and sends user messages", async () => {
    const socket = new FakeSocket();
    render(
      <AssistantDock
        onClose={() => {}}
        apiClient={workflowApi()}
        executeCapability={vi.fn()}
        getContext={() => ({ project_id: "default" })}
        socketFactory={() => socket}
      />,
    );

    await waitFor(() => expect(socket.onopen).not.toBeNull());
    socket.open();
    socket.receive({ type: "auth_ok" });
    fireEvent.change(screen.getByPlaceholderText("Message the assistant"), {
      target: { value: "Add a prompt node" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    expect(socket.sent).toContainEqual({ type: "auth", token: "secret-token" });
    expect(socket.sent).toContainEqual({
      type: "client_ready",
      manifest: { capabilities: [{ name: "workflow.add_node" }] },
    });
    expect(socket.sent).toContainEqual({
      type: "user_message",
      text: "Add a prompt node",
      context: { project_id: "default" },
    });
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

function workflowApi(): WorkflowApi {
  return {
    runWorkflow: vi.fn(),
    assetsRoot: vi.fn(),
    listAssets: vi.fn(),
    getAsset: vi.fn(),
    listProjects: vi.fn(),
    createProject: vi.fn(),
    openProject: vi.fn(),
    saveWorkflow: vi.fn(),
    loadWorkflow: vi.fn(),
    getProviders: vi.fn(),
    setActiveProvider: vi.fn(),
    setProviderKey: vi.fn(),
    getAssistantConfig: vi.fn(),
    setAssistantConfig: vi.fn(),
    getAssistantSession: vi.fn().mockResolvedValue({ port: 55123, token: "secret-token" }),
    getCapabilityManifest: vi.fn().mockResolvedValue({
      capabilities: [{ name: "workflow.add_node" }],
    }),
    listSkills: vi.fn(),
    installSkill: vi.fn(),
    setSkillEnabled: vi.fn(),
    uninstallSkill: vi.fn(),
  };
}
