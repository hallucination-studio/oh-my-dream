import { useEffect, useRef, useState } from "react";
import type { AssistantProviderSettingsDto, WorkflowApi } from "../api/types.ts";
import "./assistantProviderSettings.css";

export type AssistantProviderSettingsApi = Pick<
  WorkflowApi,
  | "assistantProviderSettingsGet"
  | "assistantProviderModelsList"
  | "assistantProviderSettingsTestAndApply"
  | "assistantProviderSettingsDisable"
>;

type BusyState = "idle" | "loading" | "discovering" | "testing" | "disabling";

export function AssistantProviderSettingsPanel({
  settingsApi,
  onSettingsChanged,
}: {
  settingsApi: AssistantProviderSettingsApi;
  onSettingsChanged?: () => void;
}) {
  const [settings, setSettings] = useState<AssistantProviderSettingsDto | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [modelId, setModelId] = useState("");
  const [busy, setBusy] = useState<BusyState>("loading");
  const [message, setMessage] = useState<string | null>(null);
  const baseUrlRef = useRef<HTMLInputElement>(null);
  const apiKeyRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let active = true;
    void settingsApi.assistantProviderSettingsGet().then(
      (value) => {
        if (!active) return;
        setSettings(value);
        setBaseUrl(value.base_url);
        setModelId(value.model_id ?? "");
        setBusy("idle");
      },
      () => {
        if (!active) return;
        setMessage("Assistant settings could not be loaded.");
        setBusy("idle");
      },
    );
    return () => {
      active = false;
    };
  }, [settingsApi]);

  const invalidateDiscovery = () => {
    setModels([]);
    setModelId("");
    setMessage(null);
  };

  const loadModels = async () => {
    if (!validBaseUrl(baseUrl)) {
      setMessage("Enter an HTTP or HTTPS Base URL.");
      baseUrlRef.current?.focus();
      return;
    }
    if (!apiKey && !settings?.has_api_key) {
      setMessage("Enter an API key.");
      apiKeyRef.current?.focus();
      return;
    }
    setBusy("discovering");
    setMessage(null);
    try {
      const result = await settingsApi.assistantProviderModelsList(baseUrl, apiKey || null);
      setModels(result.models);
      setModelId((current) => result.models.includes(current) ? current : result.models[0] ?? "");
      setMessage(result.models.length > 0 ? "Models loaded." : "No models were returned.");
    } catch (error) {
      setMessage(providerErrorMessage(error, "Models could not be loaded from this provider."));
    } finally {
      setBusy("idle");
    }
  };

  const testAndSave = async () => {
    if (settings === null || !models.includes(modelId)) return;
    setBusy("testing");
    setMessage(null);
    try {
      const value = await settingsApi.assistantProviderSettingsTestAndApply(
        settings.settings_revision,
        baseUrl,
        apiKey || null,
        modelId,
      );
      setSettings(value);
      setBaseUrl(value.base_url);
      setModelId(value.model_id ?? "");
      setApiKey("");
      setMessage("Assistant connected.");
      onSettingsChanged?.();
    } catch (error) {
      if (String(error).includes("revision_conflict")) {
        await reloadAfterConflict(settingsApi, setSettings, setBaseUrl, setModelId, setMessage);
        setModels([]);
      } else {
        setMessage(providerErrorMessage(error, "Assistant settings were not saved."));
      }
    } finally {
      setBusy("idle");
    }
  };

  const toggleAssistant = async () => {
    if (settings === null || busy !== "idle") return;
    if (!settings.enabled) {
      setMessage("Load models, choose one, then test and save.");
      baseUrlRef.current?.focus();
      return;
    }
    setBusy("disabling");
    setMessage(null);
    try {
      setSettings(await settingsApi.assistantProviderSettingsDisable(settings.settings_revision));
      setMessage("Assistant disabled.");
      onSettingsChanged?.();
    } catch (error) {
      setMessage(providerErrorMessage(error, "Assistant could not be disabled."));
    } finally {
      setBusy("idle");
    }
  };

  const isBusy = busy !== "idle";
  return (
    <section className="aset" aria-busy={isBusy}>
      <div className="mset__head">
        <div>
          <h2>Assistant</h2>
          <p>Connect one OpenAI Responses-compatible text model.</p>
        </div>
      </div>
      <div className="aset__enable">
        <div className="aset__enable-text">
          <b>Enable Assistant</b>
          <span>{settings?.enabled ? "Connected" : "Not connected"}</span>
        </div>
        <button
          type="button"
          role="switch"
          aria-label="Enable Assistant"
          aria-checked={settings?.enabled ?? false}
          className={`aset__tog${settings?.enabled ? " is-on" : ""}`}
          disabled={isBusy || settings === null}
          onClick={() => void toggleAssistant()}
        />
      </div>
      <div className="aset__field">
        <label className="aset__k" htmlFor="assistant-base-url">Base URL</label>
        <input
          ref={baseUrlRef}
          id="assistant-base-url"
          className="aset__input is-mono"
          value={baseUrl}
          disabled={isBusy}
          onChange={(event) => {
            setBaseUrl(event.target.value);
            invalidateDiscovery();
          }}
        />
      </div>
      <div className="aset__field">
        <label className="aset__k" htmlFor="assistant-api-key">
          API key
          {settings?.has_api_key && <small>Configured</small>}
        </label>
        <input
          ref={apiKeyRef}
          id="assistant-api-key"
          type="password"
          autoComplete="off"
          className="aset__input is-mono"
          value={apiKey}
          disabled={isBusy}
          onChange={(event) => {
            setApiKey(event.target.value);
            invalidateDiscovery();
          }}
        />
      </div>
      {models.length > 0 && (
        <div className="aset__field">
          <label className="aset__k" htmlFor="assistant-model">Model</label>
          <input
            id="assistant-model"
            role="combobox"
            aria-label="Model"
            list="assistant-models"
            className="aset__input is-mono"
            value={modelId}
            disabled={isBusy}
            onChange={(event) => {
              setModelId(event.target.value);
              setMessage(null);
            }}
          />
          <datalist id="assistant-models">
            {models.map((model) => <option key={model} value={model} />)}
          </datalist>
        </div>
      )}
      <div className="aset__actions">
        <button type="button" disabled={isBusy} onClick={() => void loadModels()}>
          {busy === "discovering" ? "Loading models..." : "Load models"}
        </button>
        <button
          type="button"
          className="aset__primary"
          disabled={isBusy || !models.includes(modelId)}
          onClick={() => void testAndSave()}
        >
          {busy === "testing" ? "Testing..." : "Test and save"}
        </button>
      </div>
      <p className="aset__status" role="status" aria-live="polite">{message ?? " "}</p>
    </section>
  );
}

function validBaseUrl(value: string): boolean {
  try {
    return ["http:", "https:"].includes(new URL(value).protocol);
  } catch {
    return false;
  }
}

function providerErrorMessage(error: unknown, fallback: string): string {
  const value = String(error);
  if (value.includes("authentication_rejected")) return "The API key was rejected.";
  if (value.includes("provider_unavailable") || value.includes("provider_timed_out")) {
    return "The Assistant provider could not be reached.";
  }
  if (value.includes("incompatible")) {
    return "This model does not support the Assistant response tools.";
  }
  return fallback;
}

async function reloadAfterConflict(
  settingsApi: AssistantProviderSettingsApi,
  setSettings: (value: AssistantProviderSettingsDto) => void,
  setBaseUrl: (value: string) => void,
  setModelId: (value: string) => void,
  setMessage: (value: string) => void,
) {
  try {
    const latest = await settingsApi.assistantProviderSettingsGet();
    setSettings(latest);
    setBaseUrl(latest.base_url);
    setModelId(latest.model_id ?? "");
    setMessage("Settings changed. Load models and test again.");
  } catch {
    setMessage("Assistant settings could not be reloaded.");
  }
}
