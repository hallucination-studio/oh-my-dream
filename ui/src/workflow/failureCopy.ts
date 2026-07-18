/** Verb-first failure copy: names the failed action and the reason, never a bare `String(error)`. */
export function failureCopy(action: string, error: unknown): string {
  const reason = errorMessage(error);
  return reason ? `${action} failed · ${reason}` : `${action} failed`;
}

/** Extracts a human-safe reason from Error instances and structured error DTOs. */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message.replace(/^Error:\s*/, "").trim();
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return String(error).replace(/^Error:\s*/, "").trim();
}

/** Extracts the structured code from an error DTO, when present. */
export function errorCode(error: unknown): string | null {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof (error as { code: unknown }).code === "string"
  ) {
    return (error as { code: string }).code;
  }
  return null;
}
