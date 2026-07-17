import { describe, expect, it, vi } from "vitest";
import { editorMutationActions } from "./canonical.ts";
import { CanonicalWorkspaceController } from "./canonicalWorkspaceController.ts";
import type { NodeCapabilityContractDto, WorkflowDto } from "../api/types.ts";

const PROJECT_ID = "10000000-0000-4000-8000-000000000001";
const WORKFLOW_ID = "20000000-0000-4000-8000-000000000001";
const NODE_ID = "30000000-0000-4000-8000-000000000001";

describe("canonical Workflow editor boundary", () => {
  it("derives tagged D0.7 parameter values from the authoritative constraint", () => {
    expect(editorMutationActions(workflow(), {
      version: "1.0",
      project_id: PROJECT_ID,
      nodes: [{
        id: NODE_ID,
        type: "text.provide_literal",
        contract_version: "1.0",
        params: { text: "edited" },
        inputs: {},
        position: [1, 2],
      }],
    }, contracts())).toEqual([{
      kind: "replace_node_parameters",
      node_id: NODE_ID,
      parameters: [{ key: "text", value: { kind: "text", value: "edited" } }],
    }]);
  });

  it("derives a queued draft against the latest accepted revision", async () => {
    const applyMutation = vi.fn()
      .mockImplementation(async (
        _projectId: string,
        _workflowId: string,
        revision: string,
        actions: unknown[],
      ) => ({
        workflow: {
          ...workflow(),
          revision: String(Number(revision) + 1),
          nodes: [{
            ...workflow().nodes[0]!,
            parameters: (actions[0] as { parameters: WorkflowDto["nodes"][number]["parameters"] })
              .parameters,
          }],
        },
        readiness: { state: "ready" as const },
      }));
    const controller = new CanonicalWorkspaceController({
      applyMutation,
      workflowChanged: vi.fn(),
      contracts,
    });
    controller.activate(PROJECT_ID, workflow());

    await Promise.all([
      controller.enqueueDraft(editor("first")),
      controller.enqueueDraft(editor("second")),
    ]);

    expect(applyMutation.mock.calls.map((call) => call[2])).toEqual(["1", "2"]);
  });
});

function editor(text: string) {
  return {
    version: "1.0",
    project_id: PROJECT_ID,
    nodes: [{
      id: NODE_ID,
      type: "text.provide_literal",
      contract_version: "1.0",
      params: { text },
      inputs: {},
      position: [1, 2] as [number, number],
    }],
  };
}

function workflow(): WorkflowDto {
  return {
    schema_version: 1,
    workflow_id: WORKFLOW_ID,
    project_id: PROJECT_ID,
    revision: "1",
    created_at_epoch_ms: "1",
    updated_at_epoch_ms: "1",
    nodes: [{
      node_id: NODE_ID,
      capability_id: "text.provide_literal",
      capability_version: "1.0",
      parameters: [{ key: "text", value: { kind: "text", value: "base" } }],
      canvas_position: { x: 1, y: 2 },
    }],
    input_bindings: [],
  };
}

function contracts(): NodeCapabilityContractDto[] {
  return [{
    capability_ref: { id: "text.provide_literal", version: "1.0" },
    parameters: [{
      key: "text",
      constraint: { kind: "text_utf8_bytes", minimum: 1, maximum: 65536 },
      presence: { kind: "required" },
    }],
    inputs: [],
    outputs: [{ key: "text", data_type: "text", is_primary: true }],
    execution_kind: "pure_value",
  }];
}
