import { useEffect, useState } from "react";
import type { ParamSpec } from "./catalog.ts";

type ParseResult = { ok: true; value: string | number } | { ok: false };

export function parseParameterInput(kind: ParamSpec["kind"], input: string): ParseResult {
  if (kind === "text" || kind === "model") {
    return { ok: true, value: input };
  }
  if (input.trim() === "") {
    return { ok: false };
  }
  const value = Number(input);
  if (!Number.isFinite(value) || (kind === "int" && !Number.isSafeInteger(value))) {
    return { ok: false };
  }
  return { ok: true, value };
}

export function ParameterInput({
  spec,
  value,
  className,
  onChange,
}: {
  spec: ParamSpec;
  value: unknown;
  className?: string;
  onChange: (value: string | number) => void;
}) {
  const committedValue = String(value ?? spec.default);
  const [draft, setDraft] = useState(committedValue);

  useEffect(() => setDraft(committedValue), [committedValue]);

  return (
    <input
      aria-label={spec.label}
      className={className}
      inputMode={spec.kind === "int" ? "numeric" : spec.kind === "float" ? "decimal" : undefined}
      name={spec.name}
      value={draft}
      onChange={(event) => {
        const input = event.target.value;
        setDraft(input);
        const parsed = parseParameterInput(spec.kind, input);
        if (parsed.ok) {
          onChange(parsed.value);
        }
      }}
      onBlur={() => {
        if (!parseParameterInput(spec.kind, draft).ok) {
          setDraft(committedValue);
        }
      }}
    />
  );
}
