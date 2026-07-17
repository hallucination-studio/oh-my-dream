use std::io::Cursor;
use std::time::{Duration, Instant};

use assets::asset::application::{
    AssetFinalizeContentCommand, AssetFinalizeContentEffect, AssetImportCommand,
    AssetImportSourceLease, AssetPageLimit, AssetReconcileContentCommand,
    AssetReconcileContentResult,
};
use assets::asset::domain::{
    AssetContentFinalizationId, AssetDisplayName, AssetMediaKind, AssetOriginalFileName,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn import_command_retains_only_validated_scope_names_kind_and_source() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let command = AssetImportCommand::new(
        project_id(),
        AssetMediaKind::Image,
        AssetDisplayName::try_new("cover").unwrap(),
        AssetOriginalFileName::try_new("cover.png").unwrap(),
        AssetImportSourceLease::new(deadline, Box::pin(Cursor::new(vec![1]))),
    );

    assert_eq!(command.project_id(), project_id());
    assert_eq!(command.expected_media_kind(), AssetMediaKind::Image);
    assert_eq!(command.display_name().as_str(), "cover");
    assert_eq!(command.original_file_name().as_str(), "cover.png");
    assert_eq!(command.deadline(), deadline);
    assert_eq!(command.into_source_lease().deadline(), deadline);
}

#[test]
fn finalization_command_retains_exact_effect_and_deadline() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let effect = AssetFinalizeContentEffect::new(finalization_id());
    let command = AssetFinalizeContentCommand::new(effect, deadline);

    assert_eq!(command.effect(), effect);
    assert_eq!(command.deadline(), deadline);
}

#[test]
fn reconciliation_command_defaults_each_class_to_fifty() {
    let command = AssetReconcileContentCommand::new(Instant::now(), None, None, None, None);
    assert_eq!(command.limit().get(), 50);

    let explicit = AssetPageLimit::from_u16(7).unwrap();
    let command =
        AssetReconcileContentCommand::new(Instant::now(), None, None, None, Some(explicit));
    assert_eq!(command.limit(), explicit);
}

#[test]
fn reconciliation_result_keeps_three_cursor_kinds_independent() {
    let result = AssetReconcileContentResult::new(None, None, None);
    assert!(result.finalization_cursor().is_none());
    assert!(result.available_content_cursor().is_none());
    assert!(result.staged_content_cursor().is_none());
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(1)).unwrap()
}

fn finalization_id() -> AssetContentFinalizationId {
    AssetContentFinalizationId::from_uuid(uuid(2)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
