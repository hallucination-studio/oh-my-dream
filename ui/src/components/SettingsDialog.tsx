import { useEffect, useRef, useState } from "react";
import { api, type WorkflowApi } from "../api/index.ts";
import { APP_NAME, APP_VERSION } from "../appInfo.ts";
import { useFocusTrap } from "./useFocusTrap.ts";
import type {
  GenerationProviderSettingsActionDto,
  GenerationProviderSettingsDto,
  GenerationProviderSettingsProfileDto,
} from "../api/types.ts";
import "./settings.css";
import "./modelSettings.css";

const SECTIONS = ["models", "canvas", "about"] as const;
type SettingsSection = (typeof SECTIONS)[number];
type SettingsApi = Pick<
  WorkflowApi,
  "generationProviderSettingsGet" | "generationProviderSettingsApply"
>;

export function SettingsDialog({
  open,
  onClose,
  settingsApi = api,
}: {
  open: boolean;
  onClose: () => void;
  settingsApi?: SettingsApi;
}) {
  const [section, setSection] = useState<SettingsSection>("models");
  const [settings, setSettings] = useState<GenerationProviderSettingsDto | null>(null);
  const [state, setState] = useState<"idle" | "loading" | "saving">("idle");
  const [message, setMessage] = useState<string | null>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const dialogRef = useFocusTrap<HTMLDivElement>(open);

  useEffect(() => {
    if (!open) return;
    closeButton.current?.focus();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, open]);

  useEffect(() => {
    if (!open) return;
    let active = true;
    setState("loading");
    setMessage(null);
    void settingsApi.generationProviderSettingsGet().then(
      (value) => {
        if (active) {
          setSettings(value);
          setState("idle");
        }
      },
      () => {
        if (active) {
          setMessage("Generation models could not be loaded.");
          setState("idle");
        }
      },
    );
    return () => {
      active = false;
    };
  }, [open, settingsApi]);

  if (!open) return null;

  const apply = async (action: GenerationProviderSettingsActionDto) => {
    if (settings === null || state === "saving") return;
    setState("saving");
    setMessage(null);
    try {
      const value = await settingsApi.generationProviderSettingsApply(
        settings.settings_revision,
        action,
      );
      setSettings(value);
    } catch (error) {
      if (String(error).includes("generation_provider_settings.revision_conflict")) {
        try {
          setSettings(await settingsApi.generationProviderSettingsGet());
          setMessage("Settings changed. Latest values loaded.");
        } catch {
          setMessage("Generation models could not be reloaded.");
        }
      } else {
        setMessage("The model route change was not saved.");
      }
    } finally {
      setState("idle");
    }
  };

  return (
    <div className="scrim" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title" ref={dialogRef}>
        <div className="dialog__head">
          <b id="settings-title">Settings</b>
          <button ref={closeButton} className="dialog__close" onClick={onClose} aria-label="Close">
            ×
          </button>
        </div>
        <div className="dialog__body">
          <nav className="dialog__nav" aria-label="Settings sections">
            {SECTIONS.map((value) => (
              <button
                key={value}
                className={`dialog__navitem${section === value ? " is-on" : ""}`}
                onClick={() => setSection(value)}
                aria-current={section === value ? "page" : undefined}
              >
                {value[0].toUpperCase() + value.slice(1)}
              </button>
            ))}
          </nav>
          <div className="dialog__panel">
            {section === "models" ? (
              <ModelRoutesPanel
                settings={settings}
                busy={state !== "idle"}
                message={message}
                onApply={(action) => void apply(action)}
              />
            ) : section === "canvas" ? (
              <p className="dialog__grp">No canvas preferences yet.</p>
            ) : (
              <div className="dialog__about">
                <b>{APP_NAME}</b>
                <p>Version {APP_VERSION}</p>
                <p>A local desktop AI creation client.</p>
              </div>
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

function ModelRoutesPanel({
  settings,
  busy,
  message,
  onApply,
}: {
  settings: GenerationProviderSettingsDto | null;
  busy: boolean;
  message: string | null;
  onApply: (action: GenerationProviderSettingsActionDto) => void;
}) {
  return (
    <section className="mset" aria-busy={busy}>
      <div className="mset__head">
        <div>
          <h2>Generation models</h2>
          <p>Choose which provider route serves each generation model.</p>
        </div>
      </div>
      {message && <p className="mset__message" role="status" aria-live="polite">{message}</p>}
      {settings === null ? (
        <p className="mset__loading" role="status">Loading model routes…</p>
      ) : (
        <div className="mset__list">
          {settings.profiles.map((profile) => (
            <ModelRouteRow
              key={`${profile.profile_ref}:${profile.generation_kind}`}
              profile={profile}
              disabled={busy}
              onApply={onApply}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function ModelRouteRow({
  profile,
  disabled,
  onApply,
}: {
  profile: GenerationProviderSettingsProfileDto;
  disabled: boolean;
  onApply: (action: GenerationProviderSettingsActionDto) => void;
}) {
  const enabled = profile.selected_binding !== null;
  const choices = profile.provider_choices.flatMap((provider) =>
    provider.routes.map((route) => ({
      value: `${provider.provider_id}\u0000${route.route_id}`,
      providerId: provider.provider_id,
      routeId: route.route_id,
      label: `${provider.display_name} · ${route.display_name}`,
    }))
  );
  const selected = profile.selected_binding
    ? `${profile.selected_binding.provider_id}\u0000${profile.selected_binding.route_id}`
    : choices[0]?.value ?? "";
  const name = choices[0]?.label.split(" · ")[1] ?? profile.profile_ref;
  const inputId = `route-${profile.generation_kind}`;
  return (
    <div className={`mset__row mset__row--${profile.generation_kind}`}>
      <span className="mset__kind" aria-hidden="true">{kindLabel(profile.generation_kind)}</span>
      <div className="mset__route">
        <label htmlFor={inputId}>{name}</label>
      </div>
      <select
        id={inputId}
        value={selected}
        disabled={disabled || !enabled || choices.length === 0}
        onChange={(event) => {
          const choice = choices.find((candidate) => candidate.value === event.target.value);
          if (choice) onApply({
            kind: "set_binding",
            profile_ref: profile.profile_ref,
            generation_kind: profile.generation_kind,
            provider_id: choice.providerId,
            route_id: choice.routeId,
          });
        }}
      >
        {choices.map((choice) => <option key={choice.value} value={choice.value}>{choice.label}</option>)}
      </select>
      <input
        type="checkbox"
        checked={enabled}
        disabled={disabled || choices.length === 0}
        aria-label={`Enable ${name}`}
        onChange={() => {
          if (enabled) {
            onApply({
              kind: "remove_binding",
              profile_ref: profile.profile_ref,
              generation_kind: profile.generation_kind,
            });
          } else if (choices[0]) {
            onApply({
              kind: "set_binding",
              profile_ref: profile.profile_ref,
              generation_kind: profile.generation_kind,
              provider_id: choices[0].providerId,
              route_id: choices[0].routeId,
            });
          }
        }}
      />
    </div>
  );
}

function kindLabel(kind: GenerationProviderSettingsProfileDto["generation_kind"]): string {
  switch (kind) {
    case "image": return "IMG";
    case "video": return "VID";
    case "voice": return "VOX";
    case "text": return "TXT";
  }
}
