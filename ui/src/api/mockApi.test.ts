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
