import { beforeEach, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  convertFileSrc: vi.fn(),
  Channel: class<T> {
    onmessage = (_event: T) => {};
  },
}));

beforeEach(() => invokeMock.mockReset());

it("loads the exact pending reviewed Assistant candidate", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const approval = {
    project_id: "project-1",
    approval_scope_id: "scope-1",
    user_intent: "Build a film",
    candidate_digest: "sha256:candidate",
    reviewer_version: "reviewer-v1",
    evidence_hash: "sha256:evidence",
    review_summary: "Ready to apply",
    review_findings: [],
    effect: "apply_reviewed_workflow_candidate",
    workflow: { version: "1.0", project_id: "project-1", nodes: [] },
    readiness_blockers: [],
    assets: [],
  };
  invokeMock.mockResolvedValueOnce(approval);

  await expect(tauriApi.getPendingAssistantApproval("project-1")).resolves.toEqual(approval);
  expect(invokeMock).toHaveBeenCalledWith("assistant_get_pending_approval", {
    project_id: "project-1",
  });
});

it("propagates pending approval command errors", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  invokeMock.mockRejectedValueOnce(new Error("ASSISTANT_APPROVAL_NOT_FOUND"));
  await expect(tauriApi.getPendingAssistantApproval("project-1")).rejects.toThrow(
    "ASSISTANT_APPROVAL_NOT_FOUND",
  );
});

it("resumes the exact pending approval scope", async () => {
  const { tauriApi } = await import("./tauriApi.ts");
  const input = {
    project_id: "project-1",
    approval_scope_id: "scope-1",
    candidate_digest: "sha256:candidate",
    approved: true,
  };
  invokeMock.mockResolvedValueOnce(null);

  await expect(tauriApi.decideAssistantApproval(input, vi.fn())).resolves.toBeNull();
  expect(invokeMock).toHaveBeenCalledWith("assistant_decide_approval", {
    input,
    on_event: expect.anything(),
  });
});
