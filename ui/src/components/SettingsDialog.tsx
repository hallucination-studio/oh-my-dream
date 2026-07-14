// Settings dialog: provider configuration (active provider + API keys), plus
// placeholder sections. Keys are stored locally by the backend and never
// returned — the UI only knows whether a key is set.

import { useEffect, useState } from "react";
import { api, type Provider } from "../api/index.ts";
import { AssistantSettings } from "./AssistantSettings.tsx";
import "./settings.css";

export function SettingsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [section, setSection] = useState("providers");

  useEffect(() => {
    if (!open) {
      return;
    }
    api
      .getProviders()
      .then(setProviders)
      .catch(() => setProviders([]));
  }, [open]);

  if (!open) {
    return null;
  }

  const activate = (id: string) => {
    void api.setActiveProvider(id).then(() => api.getProviders().then(setProviders));
  };
  const saveKey = (id: string, key: string) => {
    void api.setProviderKey(id, key);
  };

  return (
    <div className="scrim" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog">
        <div className="dialog__head">
          <b>Settings</b>
          <button className="dialog__close" onClick={onClose} aria-label="Close">×</button>
        </div>
        <div className="dialog__body">
          <nav className="dialog__nav">
            {["providers", "assistant", "canvas", "storage", "about"].map((s) => (
              <button
                key={s}
                className={`dialog__navitem${section === s ? " is-on" : ""}`}
                onClick={() => setSection(s)}
              >
                {s[0].toUpperCase() + s.slice(1)}
              </button>
            ))}
          </nav>
          <div className="dialog__panel">
            {section === "providers" && (
              <>
                <p className="dialog__grp">Generation Providers</p>
                <div className="prov">
                  {providers.map((p) => (
                    <ProviderRow key={p.id} provider={p} onActivate={activate} onSaveKey={saveKey} />
                  ))}
                  {providers.length === 0 && <p className="dialog__grp">No providers available.</p>}
                </div>
              </>
            )}
            {section === "assistant" && <AssistantSettings />}
            {section !== "providers" && section !== "assistant" && (
              <p className="dialog__grp">Nothing here yet.</p>
            )}
          </div>
        </div>
        <div className="dialog__foot">
          <button className="dialog__done" onClick={onClose}>Done</button>
        </div>
      </div>
    </div>
  );
}

function ProviderRow({
  provider,
  onActivate,
  onSaveKey,
}: {
  provider: Provider;
  onActivate: (id: string) => void;
  onSaveKey: (id: string, key: string) => void;
}) {
  const [key, setKey] = useState("");
  const needsKey = provider.id !== "mock";
  return (
    <div className={`prow${provider.active ? " is-on" : ""}`}>
      <div className="prow__head">
        <button
          className="prow__radio"
          onClick={() => onActivate(provider.id)}
          aria-label={`Activate ${provider.name}`}
        />
        <span className="prow__name">{provider.name}</span>
        {provider.active && <span className="prow__tag">Active</span>}
      </div>
      {needsKey && (
        <div className="prow__key">
          <span className="prow__k">API key</span>
          <input
            className="prow__v"
            type="password"
            autoComplete="current-password"
            placeholder={provider.has_key ? "•••••••••••• (set)" : "Paste key…"}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onBlur={() => key && onSaveKey(provider.id, key)}
          />
        </div>
      )}
    </div>
  );
}
