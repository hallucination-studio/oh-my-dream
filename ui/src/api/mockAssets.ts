import type {
  AssetDto,
  AssetKind,
  AssetListPageDto,
  AssetPreviewDto,
} from "./types.ts";

export async function mockAssetImport(
  _projectId: string,
  _expectedMediaKind: AssetKind,
): Promise<AssetDto | null> {
  return null;
}

export async function mockAssetGet(_projectId: string, assetId: string): Promise<AssetDto> {
  throw new Error(`Mock backend has no asset store; cannot fetch asset ${assetId}`);
}

export async function mockAssetList(): Promise<AssetListPageDto> {
  return { assets: [], next_cursor: null };
}

export async function mockAssetIssuePreview(
  _projectId: string,
  assetId: string,
): Promise<AssetPreviewDto> {
  throw new Error(`Mock backend has no preview for asset ${assetId}`);
}
