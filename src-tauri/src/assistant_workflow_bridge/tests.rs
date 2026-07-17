use assistant::{domain::AssistantWorkflowChangeId, interfaces::AssistantWorkflowMutationProposal};
use serde_json::json;
use uuid::Uuid;

use super::translate_proposals;

#[test]
fn proposal_translation_resolves_only_prior_aliases_and_emits_canonical_actions() {
    let proposals = vec![
        proposal(json!({
            "type": "add_node",
            "alias": "hero",
            "capability": {"id": "image.generate", "major": 1, "minor": 0},
            "parameters": {},
            "position": {"x": 10.0, "y": 20.0}
        })),
        proposal(json!({
            "type": "move_node",
            "node": {"kind": "alias", "alias": "hero"},
            "position": {"x": 30.0, "y": 40.0}
        })),
    ];

    let (actions, aliases) = translate_proposals(change_id(), &proposals).unwrap();

    assert_eq!(actions.len(), 2);
    assert_eq!(aliases.len(), 1);
    for action in actions {
        let bytes = action.canonical_bytes();
        assert_eq!(
            engine::workflow_graph::WorkflowMutationAction::try_from_canonical_bytes(&bytes)
                .unwrap(),
            action
        );
    }
}

#[test]
fn proposal_translation_rejects_forward_aliases_and_noncanonical_json() {
    let forward = proposal(json!({
        "type": "move_node",
        "node": {"kind": "alias", "alias": "later"},
        "position": {"x": 0.0, "y": 0.0}
    }));
    assert!(translate_proposals(change_id(), &[forward]).is_err());

    let noncanonical = AssistantWorkflowMutationProposal::new(
        br#"{ "type": "remove_node", "node": {"kind": "id", "id": "00000000-0000-4000-8000-000000000001"} }"#
            .to_vec(),
    )
    .unwrap();
    assert!(translate_proposals(change_id(), &[noncanonical]).is_err());
}

fn proposal(value: serde_json::Value) -> AssistantWorkflowMutationProposal {
    let value =
        serde_json::from_value::<assistant::interfaces::AssistantWorkflowMutationProposalDto>(
            value,
        )
        .unwrap();
    AssistantWorkflowMutationProposal::new(serde_json::to_vec(&value).unwrap()).unwrap()
}

fn change_id() -> AssistantWorkflowChangeId {
    AssistantWorkflowChangeId::from_uuid(Uuid::from_bytes([
        1, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
    ]))
    .unwrap()
}
