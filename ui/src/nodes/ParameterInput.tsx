import { useEffect, useState } from "react";
import type { JsonValue } from "../api/types.ts";
import type { ParamSpec } from "./catalog.ts";
import { optionLabel } from "./parameterLabels.ts";

export type ParameterValue = JsonValue;
export type ParseResult = { ok: true; value: ParameterValue } | { ok: false; reason?: string };

/** Parses one visible control value according to the generated parameter schema. */
export function parseParameterInput(spec: ParamSpec, input: string): ParseResult;
export function parseParameterInput(spec: ParamSpec, input: string): ParseResult {
  if (input.trim() === "" && spec.nullable) {
    return { ok: true, value: null };
  }
  if (spec.kind === "text") {
    return { ok: true, value: input };
  }
  if (spec.kind === "enum") {
    return parseEnumInput(spec, input);
  }
  if (spec.kind === "boolean") {
    if (input === "true") return { ok: true, value: true };
    if (input === "false") return { ok: true, value: false };
    return { ok: false, reason: "expected a boolean" };
  }
  if (input.trim() === "") {
    return { ok: false, reason: "value is required" };
  }
  const value = Number(input);
  if (!Number.isFinite(value)) {
    return { ok: false, reason: "value must be finite" };
  }
  if (spec.kind === "int" && !Number.isSafeInteger(value)) {
    return { ok: false, reason: "value must be a safe integer" };
  }
  const constraints = spec.constraints ?? {};
  if (constraints.minimum !== undefined && value < constraints.minimum) {
    return { ok: false, reason: `value must be at least ${constraints.minimum}` };
  }
  if (constraints.exclusiveMinimum !== undefined && value <= constraints.exclusiveMinimum) {
    return { ok: false, reason: `value must be greater than ${constraints.exclusiveMinimum}` };
  }
  if (constraints.maximum !== undefined && value > constraints.maximum) {
    return { ok: false, reason: `value must be at most ${constraints.maximum}` };
  }
  if (constraints.exclusiveMaximum !== undefined && value >= constraints.exclusiveMaximum) {
    return { ok: false, reason: `value must be less than ${constraints.exclusiveMaximum}` };
  }
  return { ok: true, value };
}

function parseEnumInput(spec: ParamSpec, input: string): ParseResult {
  const options = spec.options ?? [];
  const option = options.find((candidate) => enumOptionValue(candidate) === input);
  if (option === undefined && !options.some((candidate) => candidate === null && input === "")) {
    return { ok: false, reason: "value is not in the enum" };
  }
  return { ok: true, value: option ?? null };
}

function enumOptionValue(value: JsonValue): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

function displayValue(value: unknown, spec: ParamSpec): string {
  if (value === null || value === undefined) return "";
  if (spec.kind === "enum") return enumOptionValue(value as JsonValue);
  return String(value);
}

/** Text parameters with a large (or unbounded) contract maximum deserve a multiline editor. */
function isLongText(spec: ParamSpec): boolean {
  const maximum = spec.constraints?.maximum;
  return maximum === undefined || maximum > 120;
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
  onChange: (value: ParameterValue) => void;
}) {
  const committedValue = displayValue(value !== undefined ? value : spec.default, spec);
  const [draft, setDraft] = useState(committedValue);

  useEffect(() => setDraft(committedValue), [committedValue]);

  if (spec.kind === "boolean") {
    if (spec.nullable) {
      return (
        <select
          aria-label={spec.label}
          className={className}
          name={spec.name}
          value={draft}
          onChange={(event) => {
            const parsed = parseParameterInput(spec, event.target.value);
            if (parsed.ok) onChange(parsed.value);
            setDraft(event.target.value);
          }}
        >
          <option value="">None</option>
          <option value="true">True</option>
          <option value="false">False</option>
        </select>
      );
    }
    const checked = value === true || (value === undefined && spec.default === true);
    return (
      <input
        aria-label={spec.label}
        className={className}
        name={spec.name}
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    );
  }

  if (spec.kind === "enum") {
    return (
      <select
        aria-label={spec.label}
        className={className}
        name={spec.name}
        value={draft}
        onChange={(event) => {
          const parsed = parseParameterInput(spec, event.target.value);
          if (parsed.ok) onChange(parsed.value);
          setDraft(event.target.value);
        }}
      >
        {spec.nullable && <option value="">None</option>}
        {(spec.options ?? []).filter((option) => option !== null).map((option) => (
          <option key={enumOptionValue(option)} value={enumOptionValue(option)}>
            {optionLabel(enumOptionValue(option))}
          </option>
        ))}
      </select>
    );
  }

  // Long-form text (prompts) edits multiline; short values stay single-line.
  if (spec.kind === "text" && isLongText(spec)) {
    return (
      <textarea
        aria-label={spec.label}
        className={className}
        name={spec.name}
        rows={4}
        value={draft}
        onChange={(event) => {
          setDraft(event.target.value);
          const parsed = parseParameterInput(spec, event.target.value);
          if (parsed.ok) onChange(parsed.value);
        }}
        onBlur={() => {
          if (!parseParameterInput(spec, draft).ok) setDraft(committedValue);
        }}
      />
    );
  }

  return (
    <input
      aria-label={spec.label}
      className={className}
      inputMode={spec.kind === "int" ? "numeric" : spec.kind === "float" ? "decimal" : undefined}
      name={spec.name}
      type="text"
      value={draft}
      onChange={(event) => {
        const input = event.target.value;
        setDraft(input);
        const parsed = parseParameterInput(spec, input);
        if (parsed.ok) onChange(parsed.value);
      }}
      onBlur={() => {
        if (!parseParameterInput(spec, draft).ok) setDraft(committedValue);
      }}
    />
  );
}
