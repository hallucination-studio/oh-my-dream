// Assistant settings section: a master enable toggle, model configuration
// (OpenAI-protocol base URL / model / API key / tuning), a compact searchable
// skills list, and the developer-mode gate. Kept separate from the provider
// section so the two never mix, and to keep SettingsDialog within the file-size
// budget. Mirrors docs/ui-settings-assistant.html.

import { useEffect, useMemo, useState } from "react";
import { api, type AssistantConfig, type Skill } from "../api/index.ts";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";

interface Draft {
  enabled: boolean;
  base_url: string;
  model: string;
  temperature: string;
  max_tool_iters: string;
  developer_mode: boolean;
  system_prompt_extra: string | null;
}

function toDraft(config: AssistantConfig): Draft {
  return {
    enabled: config.enabled,
    base_url: config.base_url,
    model: config.model,
    temperature: String(config.temperature),
    max_tool_iters: String(config.max_tool_iters),
    developer_mode: config.developer_mode,
    system_prompt_extra: config.system_prompt_extra,
  };
}

export function AssistantSettings() {
  const [config, setConfig] = useState<AssistantConfig | null>(null);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [query, setQuery] = useState("");

  const reload = () => {
    void api.getAssistantConfig().then((next) => {
      setConfig(next);
      setDraft(toDraft(next));
    });
    void api.listSkills().then(setSkills);
  };

  useEffect(reload, []);

  // Persist the current draft plus any per-skill enable state.
  const persist = (overrides: Partial<Draft>, keyOverride?: string) => {
    if (!draft) {
      return;
    }
    const merged = { ...draft, ...overrides };
    setDraft(merged);
    const key = keyOverride ?? apiKey;
    void api
      .setAssistantConfig({
        enabled: merged.enabled,
        base_url: merged.base_url.trim() || DEFAULT_BASE_URL,
        model: merged.model.trim(),
        api_key: key ? key : null,
        clear_api_key: false,
        temperature: Number(merged.temperature) || 0,
        max_tool_iters: Math.max(1, Math.trunc(Number(merged.max_tool_iters) || 1)),
        system_prompt_extra: merged.system_prompt_extra,
        developer_mode: merged.developer_mode,
        enabled_skills: skills.filter((s) => s.enabled).map((s) => s.name),
      })
      .then(reload);
  };

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? skills.filter((s) => s.name.toLowerCase().includes(q)) : skills;
  }, [skills, query]);

  const toggleSkill = (skill: Skill) => {
    void api.setSkillEnabled(skill.name, !skill.enabled).then(reload);
  };
  const removeSkill = (skill: Skill) => {
    void api.uninstallSkill(skill.name).then(reload);
  };
  const installSkill = () => {
    const path = window.prompt("Skill folder path");
    if (path) {
      void api.installSkill(path).then(reload);
    }
  };

  if (!config || !draft) {
    return <p className="dialog__grp">Loading assistant…</p>;
  }

  const enabledCount = skills.filter((s) => s.enabled).length;

  return (
    <>
      <div className="aset__enable">
        <div className="aset__enable-text">
          <b>AI assistant</b>
          <span>Let an agent drive the canvas, configuration, and workflows.</span>
        </div>
        <Toggle on={draft.enabled} onClick={() => persist({ enabled: !draft.enabled })} label="Enable assistant" />
      </div>

      <div className={draft.enabled ? "" : "aset__gated"}>
        <p className="dialog__grp">Model (OpenAI protocol)</p>
        <Field label="Base URL" hint="OpenAI-compatible">
          <input
            className="aset__input is-mono"
            value={draft.base_url}
            onChange={(e) => setDraft({ ...draft, base_url: e.target.value })}
            onBlur={() => persist({})}
          />
        </Field>
        <Field label="Model">
          <input
            className="aset__input is-mono"
            value={draft.model}
            onChange={(e) => setDraft({ ...draft, model: e.target.value })}
            onBlur={() => persist({})}
          />
        </Field>
        <Field label="API key">
          <input
            className="aset__input"
            type="password"
            placeholder={config.has_key ? "•••••••••••• (set)" : "Paste key…"}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            onBlur={() => apiKey && persist({}, apiKey)}
          />
        </Field>
        <Field label="Tuning">
          <div className="aset__row2">
            <input
              className="aset__input is-mono"
              value={draft.temperature}
              onChange={(e) => setDraft({ ...draft, temperature: e.target.value })}
              onBlur={() => persist({})}
              aria-label="Temperature"
            />
            <input
              className="aset__input is-mono"
              value={draft.max_tool_iters}
              onChange={(e) => setDraft({ ...draft, max_tool_iters: e.target.value })}
              onBlur={() => persist({})}
              aria-label="Max tool iterations"
            />
          </div>
        </Field>

        <div className="aset__skhead">
          <p className="dialog__grp aset__skhead-title">Skills</p>
          <span className="aset__count">
            {enabledCount} / {skills.length}
          </span>
          <button className="aset__install" onClick={installSkill}>
            <PlusIcon />
            Install
          </button>
        </div>
        <div className="aset__sksearch">
          <SearchIcon />
          <input placeholder="Search skills…" value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        {skills.length === 0 ? (
          <p className="aset__skempty">No skills installed yet.</p>
        ) : (
          <div className="aset__sklist">
            {filtered.map((skill) => {
              const lockedByDev = skill.developer_mode_required && !draft.developer_mode;
              return (
                <div key={skill.name} className={`aset__skrow${skill.enabled ? "" : " is-off"}`}>
                  <Toggle
                    on={skill.enabled}
                    disabled={lockedByDev}
                    onClick={() => !lockedByDev && toggleSkill(skill)}
                    label={`Enable ${skill.name}`}
                  />
                  <div className="aset__sm">
                    <span className="aset__sn">{skill.name}</span>
                    <span className="aset__sv">{skill.version}</span>
                    {skill.developer_mode_required && <span className="aset__devtag">code</span>}
                    <span className="aset__sd">{skill.description}</span>
                  </div>
                  <button className="aset__rm" onClick={() => removeSkill(skill)} aria-label={`Remove ${skill.name}`}>
                    ×
                  </button>
                </div>
              );
            })}
          </div>
        )}

        <p className="dialog__grp aset__advanced">Advanced</p>
        <div className="aset__dev">
          <div className="aset__dev-text">
            Developer mode
            <small>Allow skills that run custom code in the sidecar. Only enable for skills you trust.</small>
          </div>
          <Toggle
            on={draft.developer_mode}
            onClick={() => persist({ developer_mode: !draft.developer_mode })}
            label="Enable developer mode"
          />
        </div>
      </div>
    </>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="aset__field">
      <span className="aset__k">
        {label}
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
  disabled,
}: {
  on: boolean;
  onClick: () => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      className={`aset__tog${on ? " is-on" : ""}${disabled ? " is-disabled" : ""}`}
      onClick={onClick}
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
    />
  );
}

function PlusIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" aria-hidden="true">
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

function SearchIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <circle cx="11" cy="11" r="7" />
      <path d="M21 21l-4-4" />
    </svg>
  );
}
