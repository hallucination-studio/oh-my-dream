import type { JsonValue } from "./types.ts";

export type AssetKind = "image" | "video" | "audio";
export type AssetContentState = "pending" | "available" | "missing";

export interface AssetDto {
  asset_id: string;
  project_id: string;
  media_kind: AssetKind;
  content_state: AssetContentState;
  display_name: string;
  created_at_epoch_ms: string;
  content: {
    content_fingerprint_hex: string;
    byte_length: string;
    mime_type: string;
  };
  media_facts: JsonValue;
  origin: JsonValue;
}

export interface AssetListPageDto {
  assets: AssetDto[];
  next_cursor: string | null;
}

export interface AssetPreviewDto {
  asset_id: string;
  preview_uri: string;
  expires_at_epoch_ms: string;
}
