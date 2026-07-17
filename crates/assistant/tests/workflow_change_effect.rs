use assistant::{
    application::AssistantApplyWorkflowChangeEffect, domain::AssistantWorkflowChangeId,
};
use uuid::Uuid;

#[test]
fn apply_effect_carries_only_the_exact_workflow_change_identity() {
    let change_id = AssistantWorkflowChangeId::from_uuid(Uuid::from_bytes([
        7, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 7,
    ]))
    .unwrap();

    let effect = AssistantApplyWorkflowChangeEffect::new(change_id);

    assert_eq!(effect.workflow_change_id(), change_id);
}
