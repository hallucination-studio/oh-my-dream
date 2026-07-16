use oh_my_dream_tauri::asset_command_dto::{AssetContentDto, AssetDto};
use serde_json::json;

pub(super) fn fixture() -> AssetDto {
    AssetDto {
        asset_id: "123e4567-e89b-42d3-a456-426600000020".to_owned(),
        project_id: "123e4567-e89b-42d3-a456-426600000001".to_owned(),
        media_kind: "video".to_owned(),
        content_state: "available".to_owned(),
        display_name: "A red fox".to_owned(),
        created_at_epoch_ms: "0".to_owned(),
        content: AssetContentDto {
            content_fingerprint_hex: "00".repeat(32),
            byte_length: "1024".to_owned(),
            mime_type: "video/mp4".to_owned(),
        },
        media_facts: json!({
            "kind":"video","width":1920,"height":1080,"duration_ms":"1000","has_audio":true
        }),
        origin: json!({"kind":"imported","original_file_name":"fox.mp4"}),
    }
}
