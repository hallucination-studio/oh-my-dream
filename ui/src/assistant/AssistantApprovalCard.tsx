import type { AssistantPendingWorkflowChange } from "../api/index.ts";

export function AssistantApprovalCard({
  approval,
  busy,
  onDecision,
}: {
  approval: AssistantPendingWorkflowChange;
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
        <div>
          <dt>Requested production</dt>
          <dd>
            {approval.lineage.kind === "user_message"
              ? approval.lineage.intent
              : `Repair Run ${approval.lineage.failed_workflow_run_id}`}
          </dd>
        </div>
        <div><dt>Base revision</dt><dd>{approval.base_workflow_revision}</dd></div>
        <div><dt>Mutations</dt><dd>{approval.mutations.length}</dd></div>
        <div><dt>Readiness issues</dt><dd>{approval.readiness_issues.length}</dd></div>
        <div><dt>Candidate</dt><dd>{approval.mutation_digest_hex}</dd></div>
      </dl>
      <div className="adock__preview" aria-label="Candidate Workflow preview">
        <strong>Editable Workflow preview</strong>
        {approval.mutations.length === 0 ? (
          <span>No mutations</span>
        ) : (
          <ul>
            {approval.mutations.map((mutation, index) => (
              <li key={index}><code>{JSON.stringify(mutation)}</code></li>
            ))}
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
