use assistant::domain::{
    AssistantPlanItemEntity, AssistantPlanItemGoal, AssistantPlanItemId, AssistantPlanItemState,
    AssistantPlanTitle, AssistantProductionPlanAggregate, AssistantProductionPlanError,
    AssistantProductionPlanId, AssistantSessionId,
};
use projects::project::domain::ProjectId;
use uuid::Uuid;

#[test]
fn production_plan_enforces_legal_item_transitions_and_revision_cas() {
    let mut plan = plan();
    let item_id = AssistantPlanItemId::new("draft_story").unwrap();

    plan.start_item(1, &item_id).unwrap();
    plan.block_item(2, &item_id, "Need a reference image").unwrap();
    plan.start_item(3, &item_id).unwrap();
    plan.complete_item(4, &item_id, "Story draft accepted").unwrap();

    assert_eq!(plan.revision().get(), 5);
    assert!(matches!(plan.items()[0].state(), AssistantPlanItemState::Completed { .. }));
}

#[test]
fn production_plan_rejects_stale_revision_and_illegal_completion() {
    let mut plan = plan();
    let item_id = AssistantPlanItemId::new("draft_story").unwrap();

    assert_eq!(
        plan.start_item(2, &item_id).unwrap_err(),
        AssistantProductionPlanError::RevisionConflict { expected: 2, actual: 1 }
    );
    assert_eq!(
        plan.complete_item(1, &item_id, "Not started").unwrap_err(),
        AssistantProductionPlanError::InvalidItemTransition
    );
}

#[test]
fn production_plan_rejects_duplicate_ids_and_text_outside_frozen_bounds() {
    let duplicate = AssistantPlanItemEntity::new("same_item", "First").unwrap();
    let error = AssistantProductionPlanAggregate::new(
        production_plan_id(),
        project_id(),
        session_id(),
        "Plan",
        vec![duplicate.clone(), duplicate],
    )
    .unwrap_err();

    assert_eq!(error, AssistantProductionPlanError::DuplicateItemId);
    assert!(AssistantPlanTitle::new(" ").is_err());
    assert!(AssistantPlanItemGoal::new("x".repeat(2_001)).is_err());
}

#[test]
fn production_plan_restore_revalidates_items_and_nonzero_revision() {
    let item = AssistantPlanItemEntity::try_restore(
        "draft_story",
        "Draft the story",
        AssistantPlanItemState::InProgress,
    )
    .unwrap();
    let restored = AssistantProductionPlanAggregate::try_restore(
        production_plan_id(),
        project_id(),
        session_id(),
        "Create a story",
        vec![item.clone()],
        7,
    )
    .unwrap();
    assert_eq!(restored.revision().get(), 7);
    assert_eq!(restored.items(), &[item]);
    assert_eq!(
        AssistantProductionPlanAggregate::try_restore(
            production_plan_id(),
            project_id(),
            session_id(),
            "Create a story",
            vec![],
            0,
        ),
        Err(AssistantProductionPlanError::InvalidRevision)
    );
}

#[test]
fn production_plan_replaces_title_and_items_under_revision_cas() {
    let mut plan = AssistantProductionPlanAggregate::new(
        production_plan_id(),
        project_id(),
        session_id(),
        "Initial",
        vec![AssistantPlanItemEntity::new("first", "First goal").unwrap()],
    )
    .unwrap();

    plan.replace(
        1,
        "Replacement",
        vec![AssistantPlanItemEntity::new("second", "Second goal").unwrap()],
    )
    .unwrap();

    assert_eq!(plan.revision().get(), 2);
    assert_eq!(plan.title().as_str(), "Replacement");
    assert_eq!(plan.items()[0].id().as_str(), "second");
    assert!(
        plan.replace(
            1,
            "Stale",
            vec![AssistantPlanItemEntity::new("third", "Third goal").unwrap()],
        )
        .is_err()
    );
}

fn plan() -> AssistantProductionPlanAggregate {
    AssistantProductionPlanAggregate::new(
        production_plan_id(),
        project_id(),
        session_id(),
        "Create a story",
        vec![AssistantPlanItemEntity::new("draft_story", "Draft the story").unwrap()],
    )
    .unwrap()
}

fn production_plan_id() -> AssistantProductionPlanId {
    AssistantProductionPlanId::from_uuid(uuid(1)).unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::from_uuid(uuid(2)).unwrap()
}

fn session_id() -> AssistantSessionId {
    AssistantSessionId::from_uuid(uuid(3)).unwrap()
}

fn uuid(seed: u8) -> Uuid {
    Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
}
