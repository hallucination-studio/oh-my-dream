import { expect, it } from "vitest";
import { mockApi } from "./mockApi.ts";

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
