let sequence = 0;

export function createRunId(): string {
  sequence += 1;
  return `run-${Date.now().toString(36)}-${sequence.toString(36)}`;
}
