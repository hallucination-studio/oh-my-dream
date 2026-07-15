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
        <div><dt>Summary</dt><dd>{approval.review_summary}</dd></div>
        {approval.review_findings.map((finding) => (
          <div key={finding}><dt>Finding</dt><dd>{finding}</dd></div>
        ))}
        <div><dt>Candidate</dt><dd>{approval.candidate_digest}</dd></div>
        {approval.assets.map((asset) => (
          <div key={`${asset.kind}:${asset.asset_id}`}><dt>Reused {asset.kind} Asset</dt><dd>{asset.asset_id}</dd></div>
        ))}
      </dl>
      <div className="adock__preview" aria-label="Candidate Workflow preview">
        <strong>Editable Workflow preview</strong>
        {approval.workflow.nodes.length === 0 ? (
          <span>No nodes</span>
        ) : (
          <ul>
            {approval.workflow.nodes.map((node) => <li key={node.id}>{node.type}</li>)}
          </ul>
        )}
      </div>
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
