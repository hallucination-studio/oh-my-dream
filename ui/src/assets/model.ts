import type { AssetDto, AssetKind, AssetPreviewDto } from "../api/index.ts";

export interface AssetViewModel {
  id: string;
  kind: AssetKind;
  contentState: AssetDto["content_state"];
  previewUrl: string | null;
  displayName: string;
  projectId: string;
  sourceNodeId: string | null;
  sourceNodeType: string | null;
  mimeType: string;
  byteLength: string;
  facts: string | null;
  createdAtEpochMs: string;
}

export function assetFromDto(
  dto: AssetDto,
  preview: AssetPreviewDto | null,
): AssetViewModel {
  const origin = object(dto.origin);
  return {
    id: dto.asset_id,
    kind: dto.media_kind,
    contentState: dto.content_state,
    previewUrl: preview?.preview_uri ?? null,
    displayName: dto.display_name,
    projectId: dto.project_id,
    sourceNodeId:
      origin?.kind === "workflow_node_output" && typeof origin.workflow_node_id === "string"
        ? origin.workflow_node_id
        : null,
    sourceNodeType: origin?.kind === "workflow_node_output" ? "Workflow node" : null,
    mimeType: dto.content.mime_type,
    byteLength: dto.content.byte_length,
    facts: mediaFactsLabel(dto),
    createdAtEpochMs: dto.created_at_epoch_ms,
  };
}

/** One-line media facts (dimensions or duration) when the DTO supplies them. */
export function mediaFactsLabel(dto: AssetDto): string | null {
  const facts = object(dto.media_facts);
  if (!facts) return null;
  const width = number(facts.width);
  const height = number(facts.height);
  if (width !== null && height !== null) return `${width}×${height}`;
  const duration = number(facts.duration_seconds);
  const fps = number(facts.fps);
  if (duration !== null && fps !== null) return `${duration.toFixed(1)}s · ${fps}fps`;
  if (duration !== null) return `${duration.toFixed(1)}s`;
  return null;
}

function number(value: unknown): number | null {
  const parsed = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  return Number.isFinite(parsed) ? parsed : null;
}

function object(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}
