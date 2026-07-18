/** Verb-first failure copy: names the failed action and the reason, never a bare `String(error)`. */
export function failureCopy(action: string, error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const reason = raw.replace(/^Error:\s*/, "").trim();
  return reason ? `${action} failed · ${reason}` : `${action} failed`;
}
