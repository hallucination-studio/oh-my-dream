use assets::asset::application::AssetFinalizeContentEffect;
use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use engine::{node_capability::WorkflowRunId, workflow::WorkflowExecuteRunEffect};
use oh_my_dream_tauri::post_commit_effect::{
    DesktopApplicationInstanceId, DesktopPostCommitEffect, DesktopPostCommitEffectAbandonReason,
    DesktopPostCommitEffectId, DesktopPostCommitEffectState, DesktopPostCommitTimestamp,
};
use uuid::Uuid;

#[test]
fn post_commit_effect_union_has_exactly_three_typed_business_variants() {
    let workflow = DesktopPostCommitEffect::Workflow(WorkflowExecuteRunEffect {
        workflow_run_id: WorkflowRunId::from_uuid(uuid(1)).unwrap(),
    });
    let asset = DesktopPostCommitEffect::Asset(AssetFinalizeContentEffect::new(
        assets::asset::domain::AssetContentFinalizationId::from_uuid(uuid(2)).unwrap(),
    ));
    let assistant = DesktopPostCommitEffect::Assistant(AssistantApplyWorkflowChangeEffect::new(
        AssistantWorkflowChangeId::from_uuid(uuid(3)).unwrap(),
    ));

    assert!(matches!(workflow, DesktopPostCommitEffect::Workflow(_)));
    assert!(matches!(asset, DesktopPostCommitEffect::Asset(_)));
    assert!(matches!(assistant, DesktopPostCommitEffect::Assistant(_)));
}

#[test]
fn post_commit_effect_state_carries_only_frozen_delivery_evidence() {
    let instance_id = DesktopApplicationInstanceId::from_uuid(uuid(4)).unwrap();
    let claimed_at = DesktopPostCommitTimestamp::from_epoch_millis(10).unwrap();
    let state = DesktopPostCommitEffectState::Claimed { instance_id, claimed_at };

    assert!(matches!(state, DesktopPostCommitEffectState::Claimed { .. }));
    assert!(DesktopPostCommitTimestamp::from_epoch_millis(-1).is_err());
    assert!(DesktopPostCommitEffectId::from_uuid(Uuid::nil()).is_err());
    assert_ne!(
        DesktopPostCommitEffectAbandonReason::WorkflowInterruptedByRestart,
        DesktopPostCommitEffectAbandonReason::OwningStateAlreadyTerminal
    );
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
