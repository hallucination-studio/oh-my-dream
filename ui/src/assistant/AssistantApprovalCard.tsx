import type { AssistantPendingApproval } from "../api/index.ts";

export function AssistantApprovalCard({
  approval,
  busy,
  onDecision,
}: {
  approval: AssistantPendingApproval;
  busy: boolean;
  onDecision: (approved: boolean) => void;
}) {
  return (
    <section className="adock__approval" aria-labelledby="assistant-approval-title">
      <div className="adock__approval-kicker">Workflow change ready</div>
      <h3 id="assistant-approval-title">Review before applying</h3>
      <p>
        The Assistant prepared and reviewed an immutable candidate. Approving applies that exact
        candidate as one editable Workflow revision.
      </p>
      <dl className="adock__approval-details">
        <div><dt>Requested production</dt><dd>{approval.user_intent}</dd></div>
        <div><dt>Workflow</dt><dd>{approval.workflow.nodes.length} nodes</dd></div>
        <div><dt>Review</dt><dd>{approval.reviewer_version}</dd></div>
        <div><dt>Candidate</dt><dd>{approval.candidate_digest}</dd></div>
      </dl>
      <div className="adock__approval-actions">
        <button type="button" disabled={busy} onClick={() => onDecision(false)}>
          Reject
        </button>
        <button
          type="button"
          className="is-primary"
          disabled={busy}
          onClick={() => onDecision(true)}
        >
          {busy ? "Applying…" : "Approve and apply"}
        </button>
      </div>
    </section>
  );
}
