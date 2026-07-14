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
    user_intent: "Build a film",
    candidate_digest: "sha256:candidate",
    reviewer_version: "reviewer-v1",
    evidence_hash: "sha256:evidence",
    workflow: { version: "1.0", project_id: "project-1", nodes: [] },
    readiness_blockers: [],
  };
  invokeMock.mockResolvedValueOnce(approval);

  await expect(tauriApi.getPendingAssistantApproval("project-1")).resolves.toEqual(approval);
  expect(invokeMock).toHaveBeenCalledWith("assistant_get_pending_approval", {
    project_id: "project-1",
  });
});
