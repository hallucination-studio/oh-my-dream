import { useEffect, useState } from "react";
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

  useEffect(() => {
    let active = true;
    void api
      .generationProfileListForCapability(capability)
      .then((items) => {
        if (active) setProfiles(items);
      })
      .catch(() => {
        if (active) setProfiles([]);
      });
    return () => {
      active = false;
    };
  }, [capability.id, capability.version]);

  return (
    <select
      className="insp__input"
      aria-label="Generation profile"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      <option value="">Select a profile</option>
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
