import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api/index.ts";
import type { ProjectWorkspace, WorkflowHead } from "./api/types.ts";
import { App } from "./App.tsx";
import { selectProject } from "./test/appFixtures.ts";
import type { Workflow, WorkflowInputBinding, WorkflowNode } from "./workflow/types.ts";

const COAUTHOR_TEXT =
  "Create a 12-second, three-shot video: sunrise over a city, a cyclist crossing a bridge, and coffee steaming by a window.";

afterEach(() => vi.restoreAllMocks());

describe("M3 co-author flow", () => {
  it("projects the assistant head, persists a manual edit, and reopens it", async () => {
    const alpha = projectWorkspace("alpha", "Alpha", null);
    const beta = projectWorkspace("beta", "Beta", null);
    let persisted = projectWorkspace("alpha", "Alpha", null).workflow_head;
    const openProject = vi.spyOn(api, "openProject").mockImplementation(async (id) => {
      if (id === "alpha") return { ...alpha, workflow_head: persisted };
      return beta;
    });
    vi.spyOn(api, "listProjects").mockResolvedValue([alpha.project, beta.project]);
    vi.spyOn(api, "getAssistantConfig").mockResolvedValue({
      enabled: true,
      base_url: "https://api.openai.com/v1",
      model: "gpt-5.4",
      has_key: false,
    });
    const assistantHead = coauthorHead("alpha");
    const sendAssistant = vi.spyOn(api, "sendAssistant").mockImplementation(async (_input, onEvent) => {
      onEvent({ type: "response.output_text.delta", delta: "Workflow created." });
      onEvent({ type: "response.completed" });
      return assistantHead;
    });
    const applyWorkflowPatch = vi.spyOn(api, "applyWorkflowPatch").mockImplementation(
      async (_projectId, _requestId, input) => {
        const replace = input.operations.find((operation) => operation.op === "replace_params");
        const text = replace?.op === "replace_params" ? String(replace.params.text ?? "") : "";
        const next = structuredClone(assistantHead);
        const node = next.workflow.nodes.find((candidate) => candidate.id === "n4");
        if (node) node.params = { ...node.params, text };
        next.revision = 2;
        persisted = next;
        return {
          workflow_head: next,
          aliases: [],
          readiness_blockers: [],
          assets: [],
          changed: true,
          deduplicated: false,
          undo_id: "workflow:alpha:2",
        };
      },
    );

    render(<App />);
    await selectProject("No project", "Alpha");
    await waitFor(() => expect(screen.getByRole("button", { name: "Assistant" })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Assistant" }));
    expect(document.querySelectorAll(".react-flow__node")).toHaveLength(0);
    fireEvent.change(screen.getByPlaceholderText("Message the assistant"), {
      target: { value: COAUTHOR_TEXT },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(sendAssistant).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(document.querySelectorAll(".react-flow__node")).toHaveLength(10));
    expect(screen.getByText("Workflow created.")).toBeTruthy();

    const prompts = screen.getAllByLabelText("Text");
    fireEvent.change(prompts[1]!, { target: { value: "a cyclist crossing a bridge at dusk" } });
    await waitFor(() => expect(applyWorkflowPatch).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByDisplayValue("a cyclist crossing a bridge at dusk")).toBeTruthy());

    await selectProject("Alpha", "Beta");
    await waitFor(() => expect(screen.queryAllByDisplayValue("a cyclist crossing a bridge at dusk")).toHaveLength(0));
    await selectProject("Beta", "Alpha");
    await waitFor(() => expect(screen.getByDisplayValue("a cyclist crossing a bridge at dusk")).toBeTruthy());
    expect(openProject).toHaveBeenCalledWith("alpha");
  });
});

function projectWorkspace(id: string, name: string, head: WorkflowHead | null): ProjectWorkspace {
  return {
    project: project(id, name),
    current_workflow_summary: null,
    workflow_head: head,
  };
}

function project(id: string, name: string) {
  return {
    id,
    name,
    revision: "1",
    created_at_epoch_ms: "0",
    updated_at_epoch_ms: "0",
  };
}

function coauthorHead(projectId: string): WorkflowHead {
  const prompts = [
    "sunrise over a city",
    "a cyclist crossing a bridge",
    "coffee steaming by a window",
  ];
  const nodes: Workflow["nodes"] = [];
  for (let index = 0; index < prompts.length; index += 1) {
    const number = index + 1;
    nodes.push(
      node(`n${(number - 1) * 3 + 1}`, "TextPrompt", { text: prompts[index] }),
      node(`n${(number - 1) * 3 + 2}`, "TextToImage", { model: "mock-image" }, {
        prompt: { kind: "single", source: { node_id: `n${(number - 1) * 3 + 1}`, output: "text" } },
      }),
      node(`n${(number - 1) * 3 + 3}`, "ImageToVideo", { model: "mock-video", duration: 4 }, {
        image: { kind: "single", source: { node_id: `n${(number - 1) * 3 + 2}`, output: "image" } },
      }),
    );
  }
  nodes.push(
    node("n10", "VideoConcat", {}, {
      clips: {
        kind: "ordered_many",
        sources: [
          { node_id: "n3", output: "video" },
          { node_id: "n6", output: "video" },
          { node_id: "n9", output: "video" },
        ],
      },
    }),
  );
  return {
    project_id: projectId,
    revision: 1,
    workflow: { version: "1.0", project_id: projectId, nodes },
  };
}

function node(
  id: string,
  type: string,
  params: Record<string, unknown>,
  inputs: Record<string, WorkflowInputBinding> = {},
): WorkflowNode {
  return { id, type, contract_version: "1.0", params, inputs, position: [100, 100] as [number, number] };
}
