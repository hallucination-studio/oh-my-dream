import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { AssistantDock } from "./AssistantDock.tsx";
import type {
  AssistantPresentationEvent,
  WorkflowApi,
} from "../api/index.ts";

const CONTEXT = {
  project_id: "10000000-0000-4000-8000-000000000001",
  workflow_present: false,
  workflow_revision: null,
  selected_node_ids: [],
  selected_asset_ids: [],
};

it("sends through the canonical message command", async () => {
  const send = vi.fn().mockResolvedValue({ invocation_id: "invocation", final_text: "done" });
  render(
    <AssistantDock
      onClose={() => {}}
      apiClient={api({ assistantSendMessage: send })}
      getContext={() => CONTEXT}
    />,
  );
  fireEvent.change(screen.getByPlaceholderText("Message the assistant"), {
    target: { value: "Build a film" },
  });
  fireEvent.click(screen.getByLabelText("Send"));
  await waitFor(() => expect(send).toHaveBeenCalledWith(expect.objectContaining({
    project_id: CONTEXT.project_id,
    text: "Build a film",
  })));
});

it("renders typed deltas and repairs a sequence gap through pending authority", async () => {
  let observer: ((event: AssistantPresentationEvent) => void) | undefined;
  const pending = vi.fn().mockResolvedValue(null);
  render(
    <AssistantDock
      onClose={() => {}}
      apiClient={api({
        assistantGetPendingWorkflowChange: pending,
        observeAssistantPresentationEvents: async (value) => {
          observer = value;
          return () => {};
        },
      })}
      getContext={() => CONTEXT}
    />,
  );
  await waitFor(() => expect(observer).toBeDefined());
  observer?.({
    invocation_id: "40000000-0000-4000-8000-000000000001",
    sequence: "1",
    kind: "text_delta",
    text: "Hello",
  });
  expect(await screen.findByText("Hello")).toBeTruthy();
  observer?.({
    invocation_id: "40000000-0000-4000-8000-000000000001",
    sequence: "3",
    kind: "invocation_completed",
  });
  await waitFor(() => expect(pending).toHaveBeenCalledWith(CONTEXT.project_id));
});

it("shows the unavailable state from the authoritative error and recovers on success", async () => {
  const unavailable = Object.assign(new Error("The selected generation provider is unavailable."), {
    code: "provider.unavailable",
  });
  const send = vi.fn()
    .mockRejectedValueOnce(unavailable)
    .mockResolvedValueOnce({ invocation_id: "invocation", final_text: "done" });
  render(
    <AssistantDock
      onClose={() => {}}
      apiClient={api({ assistantSendMessage: send })}
      getContext={() => CONTEXT}
    />,
  );

  const composer = screen.getByPlaceholderText("Message the assistant");
  fireEvent.change(composer, { target: { value: "Build a film" } });
  fireEvent.click(screen.getByLabelText("Send"));

  expect(
    await screen.findByText(/The assistant is unavailable right now/),
  ).toBeTruthy();
  expect(screen.queryByText(/provider\.unavailable/)).toBeNull();

  fireEvent.change(composer, { target: { value: "Try again" } });
  fireEvent.click(screen.getByLabelText("Send"));
  await waitFor(() => expect(send).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.queryByText(/The assistant is unavailable right now/)).toBeNull(),
  );
});

function api(overrides: Partial<WorkflowApi>): WorkflowApi {
  return {
    assistantSendMessage: vi.fn(),
    assistantGetPendingWorkflowChange: vi.fn().mockResolvedValue(null),
    assistantDecideWorkflowChange: vi.fn(),
    observeAssistantPresentationEvents: vi.fn().mockResolvedValue(() => {}),
    ...overrides,
  } as WorkflowApi;
}
