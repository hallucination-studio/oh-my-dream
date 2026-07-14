import { beforeEach, expect, it, vi } from "vitest";
import {
  decideAssistantApproval,
  getPendingAssistantApproval,
  primeMockAssistantApproval,
} from "./mockAssistant.ts";
import type { AssistantPendingApproval } from "./types.ts";

const APPROVAL: AssistantPendingApproval = {
  project_id: "project-1",
  approval_scope_id: "scope-1",
  user_intent: "Build a film",
  candidate_digest: "sha256:candidate",
  reviewer_version: "reviewer-v1",
  evidence_hash: "sha256:evidence",
  review_summary: "Ready",
  review_findings: [],
  effect: "apply_reviewed_workflow_candidate",
  workflow: { version: "1.0", project_id: "project-1", nodes: [] },
  readiness_blockers: [],
};

beforeEach(() => primeMockAssistantApproval(null));

it.each([true, false])("clears a pending approval after decision %s", async (approved) => {
  primeMockAssistantApproval(APPROVAL);
  const onEvent = vi.fn();
  await decideAssistantApproval(
    {
      project_id: "project-1",
      approval_scope_id: "scope-1",
      candidate_digest: "sha256:candidate",
      approved,
    },
    onEvent,
  );
  await expect(getPendingAssistantApproval()).resolves.toBeNull();
  expect(onEvent).toHaveBeenCalledWith({ type: "response.completed" });
});

it("rejects missing and cross-project pending approvals", async () => {
  await expect(
    decideAssistantApproval(
      {
        project_id: "project-1",
        approval_scope_id: "scope-1",
        candidate_digest: "sha256:candidate",
        approved: true,
      },
      vi.fn(),
    ),
  ).rejects.toThrow("ASSISTANT_APPROVAL_NOT_FOUND");
  primeMockAssistantApproval(APPROVAL);
  await expect(
    decideAssistantApproval(
      {
        project_id: "project-2",
        approval_scope_id: "scope-1",
        candidate_digest: "sha256:candidate",
        approved: true,
      },
      vi.fn(),
    ),
  ).rejects.toThrow("ASSISTANT_APPROVAL_SCOPE_MISMATCH");
});

it("rejects a decision for a replaced candidate", async () => {
  primeMockAssistantApproval(APPROVAL);
  await expect(
    decideAssistantApproval(
      {
        project_id: "project-1",
        approval_scope_id: "stale-scope",
        candidate_digest: "sha256:candidate",
        approved: true,
      },
      vi.fn(),
    ),
  ).rejects.toThrow("ASSISTANT_APPROVAL_STALE");
});
