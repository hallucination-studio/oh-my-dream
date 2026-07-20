import { expect, it, vi } from "vitest";
import { mockApi } from "./mockApi.ts";

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function buildAcceptanceWorkflow(projectId: string) {
  const workflow = await mockApi.workflowCreate(projectId);
  const textId = crypto.randomUUID();
  const imageId = crypto.randomUUID();
  const videoId = crypto.randomUUID();
  const withNodes = await mockApi.workflowApplyMutation(
    projectId,
    workflow.workflow_id,
    workflow.revision,
    [
      {
        kind: "add_node",
        node_id: textId,
        capability: { id: "text.provide_literal", version: "1.0" },
        parameters: [{ key: "text", value: { kind: "text", value: "a sunset city" } }],
        canvas_position: { x: 0, y: 0 },
      },
      {
        kind: "add_node",
        node_id: imageId,
        capability: { id: "image.generate_from_text", version: "1.0" },
        parameters: [{
          key: "generation_profile_ref",
          value: { kind: "generation_profile", profile_id: "image.high_quality_general", version: "1" },
        }],
        canvas_position: { x: 1, y: 0 },
      },
      {
        kind: "add_node",
        node_id: videoId,
        capability: { id: "video.generate_from_image", version: "1.0" },
        parameters: [{
          key: "generation_profile_ref",
          value: { kind: "generation_profile", profile_id: "video.cinematic_image_animation", version: "1" },
        }],
        canvas_position: { x: 2, y: 0 },
      },
    ],
  );
  const bound = await mockApi.workflowApplyMutation(
    projectId,
    workflow.workflow_id,
    withNodes.workflow.revision,
    [
      {
        kind: "bind_single_input",
        target: { node_id: imageId, input_key: "prompt" },
        item: {
          input_item_id: crypto.randomUUID(),
          source_node_id: textId,
          source_output_key: "text",
          input_role_key: null,
        },
      },
      {
        kind: "bind_single_input",
        target: { node_id: videoId, input_key: "image" },
        item: {
          input_item_id: crypto.randomUUID(),
          source_node_id: imageId,
          source_output_key: "image",
          input_role_key: null,
        },
      },
    ],
  );
  return { workflow, revision: bound.workflow.revision, textId, imageId, videoId };
}

it("produces one Image and one Video Asset with previews on the acceptance path", async () => {
  const project = await mockApi.createProject("Acceptance");
  const { workflow, revision, imageId } = await buildAcceptanceWorkflow(project.id);

  const run = await mockApi.workflowStartRun(project.id, workflow.workflow_id, revision, {
    kind: "whole_workflow",
  });
  let terminal = run;
  for (let i = 0; i < 60 && terminal.state !== "succeeded"; i += 1) {
    await delay(100);
    terminal = await mockApi.workflowGetRun(project.id, run.workflow_run_id);
  }
  expect(terminal.state).toBe("succeeded");
  expect(terminal.node_executions.map((execution) => execution.state)).toEqual([
    "succeeded",
    "succeeded",
    "succeeded",
  ]);

  const listed = await mockApi.assetList(project.id, null, null, 100);
  expect(listed.assets.map((asset) => asset.media_kind)).toEqual(["image", "video"]);
  expect(listed.assets[0]?.origin).toMatchObject({
    kind: "workflow_node_output",
    workflow_node_id: imageId,
  });
  expect(listed.assets[0]?.display_name).toBe("a sunset city");

  const preview = await mockApi.assetIssuePreview(project.id, listed.assets[1]!.asset_id);
  expect(decodeURIComponent(preview.preview_uri)).toContain("DETERMINISTIC SAMPLE");

  const presentation = await mockApi.workflowGetNodePresentation(
    project.id,
    workflow.workflow_id,
    imageId,
  );
  expect(presentation.presentation.kind).toBe("image");
  expect(
    presentation.presentation.kind === "image" && presentation.presentation.preview_uri,
  ).toBeTruthy();
  expect(presentation.latest_execution?.state).toBe("succeeded");

  const events = await mockApi.workflowListRunEvents(project.id, run.workflow_run_id);
  const succeeded = events.events.filter((event) => event.payload.type === "node_succeeded");
  expect(succeeded).toHaveLength(3);
  const videoOutputs = succeeded.at(-1)?.payload.outputs;
  expect(Array.isArray(videoOutputs) && videoOutputs).toHaveLength(1);
});

it("honors cancellation between timed steps", async () => {
  const project = await mockApi.createProject("Cancel");
  const { workflow, revision } = await buildAcceptanceWorkflow(project.id);

  const run = await mockApi.workflowStartRun(project.id, workflow.workflow_id, revision, {
    kind: "whole_workflow",
  });
  await mockApi.workflowCancelRun(project.id, run.workflow_run_id);
  await delay(700);

  const terminal = await mockApi.workflowGetRun(project.id, run.workflow_run_id);
  expect(terminal.state).toBe("cancelled");
  const listed = await mockApi.assetList(project.id, null, null, 100);
  expect(listed.assets).toHaveLength(0);
  const events = await mockApi.workflowListRunEvents(project.id, run.workflow_run_id);
  expect(events.events.some((event) => event.payload.type === "run_succeeded")).toBe(false);
});

it("transitions the run through running and rejects cancelling a terminal run", async () => {
  const project = await mockApi.createProject("States");
  const { workflow, revision } = await buildAcceptanceWorkflow(project.id);

  const run = await mockApi.workflowStartRun(project.id, workflow.workflow_id, revision, {
    kind: "whole_workflow",
  });
  let running = run;
  for (let i = 0; i < 30 && running.state === "queued"; i += 1) {
    await delay(50);
    running = await mockApi.workflowGetRun(project.id, run.workflow_run_id);
  }
  expect(running.state).toBe("running");
  const events = await mockApi.workflowListRunEvents(project.id, run.workflow_run_id);
  expect(events.events.some((event) => event.payload.type === "run_started")).toBe(true);

  let terminal = running;
  for (let i = 0; i < 60 && terminal.state !== "succeeded"; i += 1) {
    await delay(100);
    terminal = await mockApi.workflowGetRun(project.id, run.workflow_run_id);
  }
  expect(terminal.state).toBe("succeeded");
  await expect(mockApi.workflowCancelRun(project.id, run.workflow_run_id)).rejects.toThrow(
    "run_already_terminal",
  );
  const after = await mockApi.workflowGetRun(project.id, run.workflow_run_id);
  expect(after.state).toBe("succeeded");
});

it("keeps a generation step running for a complete edge-flow cycle", async () => {
  vi.useFakeTimers();
  try {
    const project = await mockApi.createProject("Visible edge flow");
    const { workflow, revision, imageId } = await buildAcceptanceWorkflow(project.id);
    const run = await mockApi.workflowStartRun(project.id, workflow.workflow_id, revision, {
      kind: "whole_workflow",
    });

    await vi.advanceTimersByTimeAsync(120);
    await vi.advanceTimersByTimeAsync(899);

    const active = await mockApi.workflowGetRun(project.id, run.workflow_run_id);
    expect(active.node_executions.find((execution) => execution.node_id === imageId)?.state)
      .toBe("running");
  } finally {
    vi.clearAllTimers();
    vi.useRealTimers();
  }
});

it("persists a canonical Workflow mutation in the browser mock", async () => {
  const project = await mockApi.createProject("Canonical");
  const workflow = await mockApi.workflowCreate(project.id);
  const result = await mockApi.workflowApplyMutation(
    project.id,
    workflow.workflow_id,
    workflow.revision,
    [{
      kind: "add_node",
      node_id: crypto.randomUUID(),
      capability: { id: "text.provide_literal", version: "1.0" },
      parameters: [{ key: "text", value: { kind: "text", value: "hello" } }],
      canvas_position: { x: 1, y: 2 },
    }],
  );

  expect(result.workflow.revision).toBe("2");
  expect(result.workflow.nodes[0]?.capability_id).toBe("text.provide_literal");
  await expect(mockApi.workflowGetCurrent(project.id)).resolves.toEqual(result);
});

it("applies Mock Settings with no-op and revision conflict semantics", async () => {
  const initial = await mockApi.generationProviderSettingsGet();
  const image = initial.profiles.find((profile) => profile.generation_kind === "image")!;
  const action = {
    kind: "remove_binding" as const,
    profile_ref: image.profile_ref,
    generation_kind: image.generation_kind,
  };

  const removed = await mockApi.generationProviderSettingsApply(initial.settings_revision, action);
  expect(Number(removed.settings_revision)).toBe(Number(initial.settings_revision) + 1);
  expect(removed.profiles.find((profile) => profile.generation_kind === "image")?.selected_binding)
    .toBeNull();
  const unchanged = await mockApi.generationProviderSettingsApply(removed.settings_revision, action);
  expect(unchanged.settings_revision).toBe(removed.settings_revision);
  await expect(mockApi.generationProviderSettingsApply(initial.settings_revision, action))
    .rejects.toThrow("generation_provider_settings.revision_conflict");
  const route = image.provider_choices[0]!.routes[0]!;
  const restored = await mockApi.generationProviderSettingsApply(removed.settings_revision, {
    kind: "set_binding",
    profile_ref: image.profile_ref,
    generation_kind: image.generation_kind,
    provider_id: image.provider_choices[0]!.provider_id,
    route_id: route.route_id,
  });
  expect(restored.profiles.find((profile) => profile.generation_kind === "image")?.selected_binding)
    .toEqual({ provider_id: "mock", route_id: route.route_id });
});

it("lists and gets Project-scoped mock Generation Tasks with bounded filters", async () => {
  const projectId = "123e4567-e89b-42d3-a456-426600000001";
  const page = await mockApi.generationTaskList(projectId, "failed", "image", null, 1);
  expect(page.tasks).toHaveLength(1);
  expect(page.tasks[0]?.status).toBe("failed");
  expect(page.tasks[0]).not.toHaveProperty("route_id");
  await expect(mockApi.generationTaskGet(projectId, page.tasks[0]!.id)).resolves.toMatchObject({
    id: page.tasks[0]!.id,
    result: null,
  });
  await expect(mockApi.generationTaskGet("other", page.tasks[0]!.id))
    .rejects.toThrow("generation_task.not_found");
  await expect(mockApi.generationTaskList(projectId, null, null, null, 0))
    .rejects.toThrow("generation_task.invalid_request");
});

it("returns the frozen Mock profile for each model capability", async () => {
  await expect(mockApi.generationProfileListForCapability({
    id: "image.generate_from_text",
    version: "1.0",
  })).resolves.toMatchObject([{
    profile_ref: "image.high_quality_general@1",
    availability: { state: "available" },
  }]);
  await expect(mockApi.generationProfileListForCapability({
    id: "video.generate_from_image",
    version: "1.0",
  })).resolves.toMatchObject([{
    profile_ref: "video.cinematic_image_animation@1",
  }]);
  await expect(mockApi.generationProfileListForCapability({
    id: "text.provide_literal",
    version: "1.0",
  })).resolves.toEqual([]);
});

it("reports canonical readiness and rejects a blocked workflow", async () => {
  const project = await mockApi.createProject("Readiness");
  const workflow = await mockApi.workflowCreate(project.id);
  const nodeId = crypto.randomUUID();
  const blocked = await mockApi.workflowApplyMutation(
    project.id,
    workflow.workflow_id,
    workflow.revision,
    [{
      kind: "add_node",
      node_id: nodeId,
      capability: { id: "image.generate_from_text", version: "1.0" },
      parameters: [],
      canvas_position: { x: 0, y: 0 },
    }],
  );

  expect(blocked.readiness.state).toBe("blocked");
  if (blocked.readiness.state !== "blocked") throw new Error("expected blocked readiness");
  const kinds = blocked.readiness.issues.map((issue) =>
    typeof issue === "object" && issue !== null && !Array.isArray(issue)
      ? (issue as { kind?: string }).kind
      : undefined,
  );
  expect(kinds).toContain("required_input_missing");
  expect(kinds).toContain("required_parameter_missing");

  await expect(
    mockApi.workflowStartRun(project.id, workflow.workflow_id, blocked.workflow.revision, {
      kind: "whole_workflow",
    }),
  ).rejects.toThrow("workflow.not_ready");
});

it("admits a fully bound and configured workflow", async () => {
  const project = await mockApi.createProject("Ready");
  const { workflow, revision } = await buildAcceptanceWorkflow(project.id);
  const current = await mockApi.workflowGetCurrent(project.id);
  expect(current.readiness.state).toBe("ready");
  const run = await mockApi.workflowStartRun(project.id, workflow.workflow_id, revision, {
    kind: "whole_workflow",
  });
  expect(run.state).toBe("queued");
});
