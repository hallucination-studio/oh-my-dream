import { useEffect, useState } from "react";
import { api, type AssistantConfig } from "../api/index.ts";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";

interface Draft {
  enabled: boolean;
  base_url: string;
  model: string;
}

function toDraft(config: AssistantConfig): Draft {
  return {
    enabled: config.enabled,
    base_url: config.base_url,
    model: config.model,
  };
}

export function AssistantSettings() {
  const [config, setConfig] = useState<AssistantConfig | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [apiKey, setApiKey] = useState("");

  const reload = () => {
    void api.getAssistantConfig().then((next) => {
      setConfig(next);
      setDraft(toDraft(next));
    });
  };

  useEffect(reload, []);

  const persist = (overrides: Partial<Draft>, keyOverride?: string) => {
    if (!draft) return;
    const merged = { ...draft, ...overrides };
    setDraft(merged);
    const key = keyOverride ?? apiKey;
    void api
      .setAssistantConfig({
        enabled: merged.enabled,
        base_url: merged.base_url.trim() || DEFAULT_BASE_URL,
        model: merged.model.trim(),
        api_key: key || null,
        clear_api_key: false,
      })
      .then(reload);
  };

  if (!config || !draft) {
    return <p className="dialog__grp">Loading assistant…</p>;
  }

  return (
    <form onSubmit={(event) => event.preventDefault()}>
      <div className="aset__enable">
        <div className="aset__enable-text">
          <b>AI assistant</b>
          <span>Use the Project-scoped assistant in the workflow workspace.</span>
        </div>
        <Toggle
          on={draft.enabled}
          onClick={() => persist({ enabled: !draft.enabled })}
          label="Enable assistant"
        />
      </div>

      <div className={draft.enabled ? "" : "aset__gated"}>
        <p className="dialog__grp">Model (OpenAI protocol)</p>
        <Field id="assistant-base-url" label="Base URL" hint="OpenAI-compatible">
          <input
            className="aset__input is-mono"
            id="assistant-base-url"
            name="base-url"
            value={draft.base_url}
            onChange={(event) => setDraft({ ...draft, base_url: event.target.value })}
            onBlur={() => persist({})}
          />
        </Field>
        <Field id="assistant-model" label="Model">
          <input
            className="aset__input is-mono"
            id="assistant-model"
            name="model"
            value={draft.model}
            onChange={(event) => setDraft({ ...draft, model: event.target.value })}
            onBlur={() => persist({})}
          />
        </Field>
        <Field id="assistant-api-key" label="API key">
          <input
            className="aset__input"
            id="assistant-api-key"
            name="api-key"
            type="password"
            autoComplete="current-password"
            placeholder={config.has_key ? "•••••••••••• (set)" : "Paste key…"}
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            onBlur={() => apiKey && persist({}, apiKey)}
          />
        </Field>
      </div>
    </form>
  );
}

function Field({
  id,
  label,
  hint,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="aset__field">
      <span className="aset__k">
        <label htmlFor={id}>{label}</label>
        {hint && <small>{hint}</small>}
      </span>
      {children}
    </div>
  );
}

function Toggle({
  on,
  onClick,
  label,
}: {
  on: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      className={`aset__tog${on ? " is-on" : ""}`}
      onClick={onClick}
      role="switch"
      aria-checked={on}
      aria-label={label}
    />
  );
}
