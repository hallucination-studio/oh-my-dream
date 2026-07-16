use assistant::application::{AssistantToolCatalog, AssistantToolEffect};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct AssistantToolContractFixture {
    operations: Vec<AssistantToolContractDto>,
}

#[derive(Serialize)]
struct AssistantToolContractDto {
    id: String,
    description: String,
    effect: &'static str,
    needs_approval: bool,
    input_schema: Value,
    output_schema: Value,
}

pub fn fixture() -> AssistantToolContractFixture {
    let catalog = AssistantToolCatalog::try_new().expect("frozen Assistant tool catalog");
    AssistantToolContractFixture {
        operations: catalog
            .contracts()
            .iter()
            .map(|contract| AssistantToolContractDto {
                id: contract.id().as_str().to_owned(),
                description: contract.description().to_owned(),
                effect: match contract.effect() {
                    AssistantToolEffect::AuthoritativeRead => "authoritative_read",
                    AssistantToolEffect::AssistantStateMutation => "assistant_state_mutation",
                    AssistantToolEffect::HumanApprovalRequest => "human_approval_request",
                },
                needs_approval: contract.requires_human_approval(),
                input_schema: contract.input_schema().clone(),
                output_schema: contract.output_schema().clone(),
            })
            .collect(),
    }
}

pub fn assert_fixture(fixture: &AssistantToolContractFixture) {
    assert_eq!(fixture.operations.len(), 11);
    assert!(
        fixture
            .operations
            .iter()
            .all(|operation| operation.id.starts_with("assistant.") && operation.id.ends_with("@1"))
    );
}
