export type StrongAssistantTaskItem =
  | { kind: "plan"; itemId: string; status: string }
  | {
      kind: "run";
      runId: string;
      state: "running" | "succeeded" | "failed" | "cancelled";
      detail: string;
    };

export function StrongAssistantTask({ item }: { item: StrongAssistantTaskItem }) {
  if (item.kind === "plan") {
    return (
      <section className="adock__task" aria-label="Production plan progress">
        <span>Plan</span>
        <b>{item.itemId}</b>
        <em>{item.status}</em>
      </section>
    );
  }
  return (
    <section className={`adock__task is-${item.state}`} aria-label="Workflow run progress">
      <span>Run</span>
      <b>{item.detail}</b>
      <em>{item.state}</em>
    </section>
  );
}
