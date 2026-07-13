import type { AssetDto, AssetKind } from "../api/index.ts";

export interface AssetViewModel {
  id: string;
  kind: AssetKind;
  fileUrl: string;
  thumbnailUrl: string | null;
  prompt: string | null;
  projectName: string | null;
  sourceNodeType: string | null;
  model: string | null;
  seed: number | null;
  cost: number | null;
  createdAt: number;
}

export function assetFromDto(dto: AssetDto): AssetViewModel {
  return {
    id: dto.id,
    kind: dto.kind,
    fileUrl: dto.file_path,
    thumbnailUrl: dto.thumbnail_path,
    prompt: dto.prompt,
    projectName: dto.project_name,
    sourceNodeType: dto.source_node_type,
    model: dto.model,
    seed: dto.seed,
    cost: dto.cost,
    createdAt: dto.created_at,
  };
}
