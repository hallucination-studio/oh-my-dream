use super::*;

const INVOCATION_ID: &str = "03000000-0000-4000-8000-000000000003";

#[test]
fn shared_python_fixture_decodes_with_exact_frame_fields_and_one_based_sequence() {
    let fixture = include_bytes!("../../tests/fixtures/protocol_v1_valid.ndjson");
    let mut decoder = AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust);
    let frames = fixture
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| decoder.decode(line))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].direction_sequence, 1);
    assert!(matches!(frames[0].payload, AssistantProtocolPayload::InvocationAccepted(_)));
    assert!(matches!(frames[2].payload, AssistantProtocolPayload::InvocationCompleted(_)));
}

#[test]
fn decoder_rejects_duplicate_unknown_partial_oversized_and_deep_frames() {
    let duplicate = format!(
        "{{\"protocol_version\":1,\"invocation_id\":\"{INVOCATION_ID}\",\"direction_sequence\":1,\"direction_sequence\":1,\"kind\":\"InvocationAccepted\",\"payload\":{{\"agent_id\":\"workflow_coauthor@1\"}}}}\n"
    );
    assert_eq!(decode_one(duplicate.as_bytes()), Err(AssistantProtocolError::InvalidJson));

    let unknown = format!(
        "{{\"protocol_version\":1,\"invocation_id\":\"{INVOCATION_ID}\",\"direction_sequence\":1,\"kind\":\"InvocationAccepted\",\"payload\":{{\"agent_id\":\"workflow_coauthor@1\"}},\"extra\":true}}\n"
    );
    assert_eq!(decode_one(unknown.as_bytes()), Err(AssistantProtocolError::InvalidFrame));
    assert_eq!(decode_one(b"{}"), Err(AssistantProtocolError::PartialFrame));

    let oversized = vec![b'x'; MAX_ASSISTANT_PROTOCOL_FRAME_BYTES + 1];
    assert_eq!(decode_one(&oversized), Err(AssistantProtocolError::FrameTooLarge));

    let deep = format!(
        "{{\"protocol_version\":1,\"invocation_id\":\"{INVOCATION_ID}\",\"direction_sequence\":1,\"kind\":\"InvocationAccepted\",\"payload\":{{\"agent_id\":{}}}}}\n",
        nested_json_string(MAX_ASSISTANT_PROTOCOL_JSON_DEPTH + 1)
    );
    assert_eq!(decode_one(deep.as_bytes()), Err(AssistantProtocolError::JsonTooDeep));
}

#[test]
fn decoder_rejects_wrong_direction_gaps_duplicate_calls_and_frames_after_terminal() {
    let mut decoder = AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust);
    assert_eq!(
        decoder.decode(
            frame(
                1,
                "ToolResult",
                r#"{"call_id":"call-1","tool_id":"assistant.workspace.get_snapshot@1","result":{}}"#
            )
            .as_bytes()
        ),
        Err(AssistantProtocolError::WrongDirection)
    );
    assert_eq!(
        decoder.decode(
            frame(2, "InvocationAccepted", r#"{"agent_id":"workflow_coauthor@1"}"#).as_bytes()
        ),
        Err(AssistantProtocolError::InvalidSequence)
    );

    decoder
        .decode(frame(1, "InvocationAccepted", r#"{"agent_id":"workflow_coauthor@1"}"#).as_bytes())
        .unwrap();
    decoder
        .decode(
            frame(
                2,
                "ToolCall",
                r#"{"call_id":"call-1","tool_id":"assistant.workspace.get_snapshot@1","arguments":{}}"#,
            )
            .as_bytes(),
        )
        .unwrap();
    assert_eq!(
        decoder.decode(
            frame(
                3,
                "ToolCall",
                r#"{"call_id":"call-1","tool_id":"assistant.workspace.get_snapshot@1","arguments":{}}"#
            )
            .as_bytes()
        ),
        Err(AssistantProtocolError::DuplicateCallId)
    );

    let mut terminal = AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust);
    terminal
        .decode(frame(1, "InvocationCompleted", r#"{"final_text":"done"}"#).as_bytes())
        .unwrap();
    assert_eq!(
        terminal.decode(frame(2, "ModelOutputDelta", r#"{"text":"late"}"#).as_bytes()),
        Err(AssistantProtocolError::FrameAfterTerminal)
    );
}

#[test]
fn conversation_requires_start_acceptance_serial_tool_pairs_and_failure_after_cancel() {
    let mut rust = AssistantProtocolDecoder::new(AssistantProtocolDirection::RustToPython);
    let mut python = AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust);
    let mut conversation = AssistantProtocolConversationValidator::default();

    let start = rust.decode(frame(1, "InvocationStart", &start_payload()).as_bytes()).unwrap();
    conversation.admit(AssistantProtocolDirection::RustToPython, 1, &start).unwrap();
    let accepted = python
        .decode(frame(1, "InvocationAccepted", r#"{"agent_id":"workflow_coauthor@1"}"#).as_bytes())
        .unwrap();
    conversation.admit(AssistantProtocolDirection::PythonToRust, 1, &accepted).unwrap();
    let call = python
        .decode(
            frame(
                2,
                "ToolCall",
                r#"{"call_id":"call-1","tool_id":"assistant.workspace.get_snapshot@1","arguments":{}}"#,
            )
            .as_bytes(),
        )
        .unwrap();
    conversation.admit(AssistantProtocolDirection::PythonToRust, 1, &call).unwrap();
    let result = rust
        .decode(
            frame(
                2,
                "ToolResult",
                r#"{"call_id":"call-1","tool_id":"assistant.workspace.get_snapshot@1","result":{}}"#,
            )
            .as_bytes(),
        )
        .unwrap();
    conversation.admit(AssistantProtocolDirection::RustToPython, 1, &result).unwrap();
    let cancel =
        rust.decode(frame(3, "InvocationCancel", r#"{"reason":"Deadline"}"#).as_bytes()).unwrap();
    conversation.admit(AssistantProtocolDirection::RustToPython, 1, &cancel).unwrap();
    let completed = python
        .decode(frame(3, "InvocationCompleted", r#"{"final_text":"forged success"}"#).as_bytes())
        .unwrap();
    assert_eq!(
        conversation.admit(AssistantProtocolDirection::PythonToRust, 1, &completed),
        Err(AssistantProtocolError::InvalidFrame)
    );
}

fn decode_one(line: &[u8]) -> Result<AssistantProtocolFrame, AssistantProtocolError> {
    AssistantProtocolDecoder::new(AssistantProtocolDirection::PythonToRust).decode(line)
}

fn frame(sequence: u64, kind: &str, payload: &str) -> String {
    format!(
        "{{\"protocol_version\":1,\"invocation_id\":\"{INVOCATION_ID}\",\"direction_sequence\":{sequence},\"kind\":\"{kind}\",\"payload\":{payload}}}\n"
    )
}

fn nested_json_string(depth: usize) -> String {
    let mut value = "\"workflow_coauthor@1\"".to_owned();
    for _ in 0..depth {
        value = format!("[{value}]");
    }
    value
}

fn start_payload() -> String {
    let tool_ids = [
        "assistant.workspace.get_snapshot@1",
        "assistant.node_capability.list@1",
        "assistant.node_capability.describe@1",
        "assistant.production_plan.get@1",
        "assistant.production_plan.create@1",
        "assistant.production_plan.replace@1",
        "assistant.production_plan.update_item@1",
        "assistant.workflow.evaluate_mutation@1",
        "assistant.workflow.propose_change@1",
        "assistant.workflow.get_change@1",
        "assistant.workflow.request_apply@1",
    ];
    let contracts = tool_ids
        .iter()
        .map(|tool_id| {
            format!(
                r#"{{"tool_id":"{tool_id}","description":"tool","input_schema":{{}},"output_schema":{{}},"effect":"read","requires_human_approval":false}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"start":{{"kind":"UserMessage","message":"hello"}},"trusted_context":{{"project_id":"01000000-0000-4000-8000-000000000001","session_id":"02000000-0000-4000-8000-000000000002","workspace_snapshot":{{}}}},"tool_contracts":[{contracts}],"budgets":{{"maximum_frame_bytes":8388608,"maximum_events":512,"maximum_tool_calls":64,"maximum_model_turns":16,"maximum_direction_bytes":16777216,"deadline_ms":600000}}}}"#
    )
}
