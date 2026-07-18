import type { AssistantPendingWorkflowChange } from "../api/index.ts";
import { summarizeMutations, type MutationLabeler } from "./mutationSummary.ts";

export function AssistantApprovalCard({
  approval,
  busy,
  onDecision,
  nodeLabel = () => "a node",
}: {
  approval: AssistantPendingWorkflowChange;
  busy: boolean;
  onDecision: (approved: boolean) => void;
  nodeLabel?: MutationLabeler;
}) {
  const summary = summarizeMutations(approval.mutations, nodeLabel);
  return (
    <section className="adock__approval" aria-labelledby="assistant-approval-title">
      <div className="adock__approval-kicker">Workflow change ready</div>
      <h3 id="assistant-approval-title">Review before applying</h3>
      <p>
        {approval.lineage.kind === "user_message"
          ? `The Assistant prepared this change for: ${approval.lineage.intent}`
          : "The Assistant prepared a repair for the failed run."}{" "}
        Approving applies exactly this change; rejecting discards it.
      </p>
      {summary.length > 0 ? (
        <ul className="adock__approval-summary">
          {summary.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      ) : (
        <p className="adock__approval-summary">No graph changes — settings only.</p>
      )}
      {approval.readiness_issues.length > 0 && (
        <p className="adock__approval-note">
          {approval.readiness_issues.length === 1
            ? "1 issue still needs attention after applying."
            : `${approval.readiness_issues.length} issues still need attention after applying.`}
        </p>
      )}
      <details className="adock__approval-diagnostics">
        <summary>Diagnostics</summary>
        <dl>
          <div><dt>Base revision</dt><dd>{approval.base_workflow_revision}</dd></div>
          <div><dt>Candidate</dt><dd>{approval.mutation_digest_hex}</dd></div>
          <div><dt>Change id</dt><dd>{approval.workflow_change_id}</dd></div>
          {approval.lineage.kind === "reviewed_repair" && (
            <div><dt>Failed run</dt><dd>{approval.lineage.failed_workflow_run_id}</dd></div>
          )}
        </dl>
      </details>
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
