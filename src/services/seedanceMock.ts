import { sampleAudio, sampleVideo } from "../fixtures";
import type { AppConfig, AssetKind, GenerationParams } from "../types";
import { numberParam, stringParam } from "./generation";

export type SeedanceMockKind = "video" | "audio" | "compose";

export function createSeedanceMockJob({
  seedance,
  nodeParams,
  kind,
  prompt
}: {
  seedance: AppConfig["seedance"];
  nodeParams: GenerationParams;
  kind: SeedanceMockKind;
  prompt: string;
}) {
  const mediaKind: AssetKind = kind === "audio" ? "audio" : "video";
  const model = stringParam(
    nodeParams,
    "model",
    mediaKind === "audio" ? seedance.audioModel : seedance.videoModel
  );
  const generationParams: GenerationParams = {
    model,
    duration: numberParam(nodeParams, "duration", seedance.duration),
    ...(mediaKind === "video"
      ? {
          modeType: stringParam(nodeParams, "modeType", "text2video"),
          ratio: stringParam(nodeParams, "ratio", "16:9"),
          resolution: stringParam(nodeParams, "resolution", seedance.resolution),
          ...(kind === "compose" ? { transition: stringParam(nodeParams, "transition", "crossfade") } : {})
        }
      : {})
  };
  const steps = Math.max(4, Math.round(seedance.mockLatencyMs / 320));

  return {
    mediaKind,
    model,
    prompt,
    resultUrl: mediaKind === "audio" ? sampleAudio : sampleVideo,
    generationParams,
    steps,
    intervalMs: Math.max(220, seedance.mockLatencyMs / steps)
  };
}
