import { useEffect, useRef, useState } from "react";
import { api, type CapabilityRef, type GenerationProfileForCapability } from "../api/index.ts";

export function GenerationProfileSelector({
  capability,
  value,
  onChange,
}: {
  capability: CapabilityRef;
  value: string;
  onChange: (profileRef: string) => void;
}) {
  const [profiles, setProfiles] = useState<GenerationProfileForCapability[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const autoSelected = useRef<string | null>(null);

  useEffect(() => {
    let active = true;
    setProfiles([]);
    setState("loading");
    void api
      .generationProfileListForCapability(capability)
      .then((items) => {
        if (active) {
          setProfiles(items);
          setState("ready");
        }
      }, () => {
        if (active) {
          setProfiles([]);
          setState("error");
        }
      });
    return () => {
      active = false;
    };
  }, [capability.id, capability.version]);

  useEffect(() => {
    if (profiles.length !== 1 || profiles[0]?.availability.state !== "available") return;
    const profile = profiles[0];
    const key = `${capability.id}@${capability.version}:${profile.profile_ref}`;
    if (value !== profile.profile_ref && autoSelected.current !== key) {
      autoSelected.current = key;
      onChange(profile.profile_ref);
    }
  }, [capability.id, capability.version, onChange, profiles, value]);

  if (state === "error") {
    return (
      <div className="insp__input insp__model-state" role="status">
        Generation model availability is indeterminate
      </div>
    );
  }
  if (state === "loading") {
    return <div className="insp__input insp__model-state" role="status">Checking generation model availability</div>;
  }
  if (state === "ready" && profiles.length === 0) {
    return (
      <div className="insp__input insp__model-state" role="status">
        No generation model supports this node type
      </div>
    );
  }
  if (state === "ready" && profiles.length === 1) {
    const profile = profiles[0]!;
    const reason = profile.availability.state === "available"
      ? "Ready"
      : profile.availability.reason ?? profile.availability.state;
    return (
      <div className="insp__input insp__model-state" role="status" aria-label="Generation model">
        <span>{profile.display_name}</span>
        <small>{reason}</small>
      </div>
    );
  }

  return (
    <select
      className="insp__input"
      aria-label="Generation model"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      <option value="">Select a model</option>
      {profiles.map((profile) => (
        <option
          key={profile.profile_ref}
          value={profile.profile_ref}
          disabled={profile.availability.state !== "available"}
        >
          {profile.display_name}
          {profile.availability.state === "available"
            ? ""
            : ` — ${profile.availability.reason ?? profile.availability.state}`}
        </option>
      ))}
    </select>
  );
}
