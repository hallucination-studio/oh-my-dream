use oh_my_dream_tauri::assistant_operations::RequestContext;
use oh_my_dream_tauri::production_plan::operations::ProductionPlanOperations;
use oh_my_dream_tauri::production_plan::{
    NewPlanItem, PlanItemStatus, ProductionPlan, ProductionPlanError, ProductionPlanService,
    ProductionPlanSqliteRepository,
};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

fn items() -> Vec<NewPlanItem> {
    vec![
        NewPlanItem { id: "shot-a".to_owned(), summary: "Opening shot".to_owned() },
        NewPlanItem { id: "shot-b".to_owned(), summary: "Closing shot".to_owned() },
    ]
}

#[test]
fn agent_can_advance_multiple_items_without_a_next_item_operation() {
    let mut plan = ProductionPlan::create("project", "Launch film", items()).expect("plan");

    plan.start_item(1, "shot-a").expect("start first");
    plan.complete_item(2, "shot-a", "Opening accepted").expect("complete first");
    plan.start_item(3, "shot-b").expect("start second");
    plan.block_item(4, "shot-b", "Missing voice reference").expect("block second");

    assert_eq!(plan.revision(), 5);
    assert_eq!(plan.items()[0].status(), PlanItemStatus::Completed);
    assert_eq!(plan.items()[0].note(), Some("Opening accepted"));
    assert_eq!(plan.items()[1].status(), PlanItemStatus::Blocked);
    assert_eq!(plan.items()[1].note(), Some("Missing voice reference"));
}

#[test]
fn stale_revision_does_not_change_the_plan() {
    let mut plan = ProductionPlan::create("project", "Launch film", items()).expect("plan");
    plan.start_item(1, "shot-a").expect("start first");

    let error =
        plan.complete_item(1, "shot-a", "stale completion").expect_err("stale revision must fail");

    assert_eq!(error, ProductionPlanError::RevisionConflict { expected: 1, actual: 2 });
    assert_eq!(plan.items()[0].status(), PlanItemStatus::InProgress);
}

#[test]
fn replace_rejects_duplicate_item_ids() {
    let mut plan = ProductionPlan::create("project", "Launch film", items()).expect("plan");
    let duplicate = vec![
        NewPlanItem { id: "shot-a".to_owned(), summary: "One".to_owned() },
        NewPlanItem { id: "shot-a".to_owned(), summary: "Two".to_owned() },
    ];

    let error = plan.replace(1, "Replacement", duplicate).expect_err("duplicate ids must fail");

    assert_eq!(error, ProductionPlanError::DuplicateItemId { id: "shot-a".to_owned() });
    assert_eq!(plan.revision(), 1);
}

#[test]
fn sqlite_plan_survives_a_fresh_service_and_preserves_cas() {
    let root = tempdir().expect("config root");
    let path = ProductionPlanSqliteRepository::path(root.path());
    let first = ProductionPlanService::new(Arc::new(
        ProductionPlanSqliteRepository::open(&path).expect("open plan repository"),
    ));
    first.create("project", "Launch film".to_owned(), items()).expect("create durable plan");

    let reopened = ProductionPlanService::new(Arc::new(
        ProductionPlanSqliteRepository::open(&path).expect("reopen plan repository"),
    ));
    let started = reopened.start_item("project", 1, "shot-a").expect("start persisted item");
    assert_eq!(started.revision(), 2);

    let error = first.start_item("project", 1, "shot-b").expect_err("stale service must fail CAS");
    assert_eq!(error, ProductionPlanError::RevisionConflict { expected: 1, actual: 2 });
}

#[tokio::test]
async fn operation_surface_is_memory_tools_without_a_next_item_scheduler() {
    let root = tempdir().expect("config root");
    let service = Arc::new(ProductionPlanService::new(Arc::new(
        ProductionPlanSqliteRepository::open(ProductionPlanSqliteRepository::path(root.path()))
            .expect("open plan repository"),
    )));
    let registrations =
        ProductionPlanOperations::new(service).registrations().expect("plan registrations");
    let ids = registrations.iter().map(|registration| registration.id()).collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "production_plan_get",
            "production_plan_create",
            "production_plan_replace",
            "production_plan_update_item",
        ]
    );
    assert!(ids.iter().all(|id| !id.contains("next") && !id.contains("claim")));

    let create = &registrations[1];
    let context = RequestContext::new("project", "session", "request", create.version(), None);
    let output = create
        .dispatch(
            &context,
            json!({
                "title": "Launch film",
                "items": [
                    {"id": "shot-a", "summary": "Opening shot"},
                    {"id": "shot-b", "summary": "Closing shot"}
                ]
            }),
        )
        .await
        .expect("create through operation");

    assert_eq!(output["plan"]["revision"], 1);
    assert_eq!(output["plan"]["items"][1]["id"], "shot-b");
}
