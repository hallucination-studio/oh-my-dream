import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AssistantDock } from "./AssistantDock.tsx";
import type { WorkflowApi } from "../api/index.ts";

const CONTEXT = {
  project_id: "project-1",
  workflow_present: false,
  workflow_revision: null,
  selected_node_ids: [],
  selected_asset_ids: [],
};

describe("AssistantDock", () => {
  it("sends user text through the Project-scoped assistant command", async () => {
    const sendAssistant = vi.fn(async (_input, onEvent) => {
      onEvent({ type: "future.responses.event" });
      onEvent({ type: "response.output_text.delta", delta: "Assistant response" });
      return null;
    });
    const apiClient = workflowApi({ sendAssistant });

    render(
      <AssistantDock
        onClose={() => {}}
        apiClient={apiClient}
        getContext={() => CONTEXT}
      />,
    );

    const composer = screen.getByPlaceholderText("Message the assistant");
    fireEvent.change(composer, { target: { value: "Add a prompt node" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(sendAssistant).toHaveBeenCalledTimes(1));
    expect(sendAssistant.mock.calls[0]?.[0]).toEqual({
      project_id: "project-1",
      workflow_present: false,
      workflow_revision: null,
      selected_node_ids: [],
      selected_asset_ids: [],
      text: "Add a prompt node",
    });
    expect(screen.getByText("Assistant response")).toBeTruthy();
  });

  it("keeps the draft focused and sends nothing when the write barrier fails", async () => {
    const sendAssistant = vi.fn();
    const composerBarrier = vi.fn().mockRejectedValue(new Error("patch conflict"));
    render(
      <AssistantDock
        onClose={() => {}}
        apiClient={workflowApi({ sendAssistant })}
        beforeSend={composerBarrier}
        getContext={() => CONTEXT}
      />,
    );

    const composer = screen.getByPlaceholderText("Message the assistant");
    fireEvent.change(composer, { target: { value: "Keep this draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(document.activeElement).toBe(composer));
    expect((composer as HTMLTextAreaElement).value).toBe("Keep this draft");
    expect(sendAssistant).not.toHaveBeenCalled();
  });

  it("forwards the canonical Workflow head returned by the assistant command", async () => {
    const workflowHead = {
      project_id: "project-1",
      revision: 1,
      workflow: {
        version: "1.0",
        project_id: "project-1",
        nodes: [],
      },
    };
    const sendAssistant = vi.fn(async (_input, onEvent) => {
      onEvent({ type: "response.completed" });
      return workflowHead;
    });
    const onWorkflowHead = vi.fn();

    render(
      <AssistantDock
        onClose={() => {}}
        apiClient={workflowApi({ sendAssistant })}
        getContext={() => CONTEXT}
        onWorkflowHead={onWorkflowHead}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText("Message the assistant"), {
      target: { value: "Build the workflow" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(onWorkflowHead).toHaveBeenCalledWith(workflowHead));
  });
});

function workflowApi(overrides: Partial<WorkflowApi> = {}): WorkflowApi {
  return {
    runWorkflow: vi.fn(),
    assetsRoot: vi.fn(),
    listAssets: vi.fn(),
    getAsset: vi.fn(),
    listProjects: vi.fn(),
    createProject: vi.fn(),
    openProject: vi.fn(),
    searchCapabilities: vi.fn(),
    getCapabilityBundles: vi.fn(),
    applyWorkflowPatch: vi.fn(),
    getProviders: vi.fn(),
    setActiveProvider: vi.fn(),
    setProviderKey: vi.fn(),
    getAssistantConfig: vi.fn(),
    setAssistantConfig: vi.fn(),
    sendAssistant: vi.fn(),
    ...overrides,
  };
}
