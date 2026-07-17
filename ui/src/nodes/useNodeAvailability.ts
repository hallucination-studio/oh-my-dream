import { useEffect, useMemo, useState } from "react";
import { api, type NodeCapabilityContractDto } from "../api/index.ts";
import {
  capabilityKey,
  isProfileBackedCapability,
  nodeSpecFromExactContract,
  profileProjectionForCapability,
  type ProfileQueryResult,
} from "./exactCapability.ts";

export function useNodeAvailability(contracts: readonly NodeCapabilityContractDto[]) {
  const [profileResults, setProfileResults] = useState<Record<string, ProfileQueryResult>>({});

  useEffect(() => {
    let active = true;
    const profileContracts = contracts.filter(isProfileBackedCapability);
    setProfileResults(Object.fromEntries(
      profileContracts.map((contract) => [capabilityKey(contract.capability_ref), { state: "loading" }]),
    ));
    for (const contract of profileContracts) {
      const key = capabilityKey(contract.capability_ref);
      void api.generationProfileListForCapability(contract.capability_ref).then(
        (profiles) => {
          if (active) {
            setProfileResults((current) => ({ ...current, [key]: { state: "ready", profiles } }));
          }
        },
        () => {
          if (active) {
            setProfileResults((current) => ({ ...current, [key]: { state: "error" } }));
          }
        },
      );
    }
    return () => {
      active = false;
    };
  }, [contracts]);

  return useMemo(() => {
    const projections = contracts.map((contract) => {
      const result = profileResults[capabilityKey(contract.capability_ref)] ?? { state: "loading" as const };
      return {
        contract,
        ...profileProjectionForCapability(contract, result),
      };
    });
    return {
      specs: projections.map(({ contract, status, defaultProfileRef }) =>
        nodeSpecFromExactContract(contract, status, defaultProfileRef)),
      hiddenCapabilityKeys: new Set(
        projections
          .filter((projection) => !projection.hasCompatibleProfile)
          .map((projection) => capabilityKey(projection.contract.capability_ref)),
      ),
    };
  }, [contracts, profileResults]);
}
