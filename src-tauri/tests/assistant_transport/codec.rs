use std::sync::atomic::{AtomicUsize, Ordering};

use oh_my_dream_tauri::assistant_transport::{
    AssistantFrame, AssistantFrameKind, AssistantFrameReader, AssistantFrameWriter,
    AssistantTransportError, MAX_FRAME_SIZE, MAX_JSON_DEPTH, MAX_SAFE_JSON_NUMBER,
    PROTOCOL_VERSION,
};
use serde_json::{Value, json};
use tokio::io::BufReader;

use super::common::{TestWriter, encoded_wire_frame, json_line, read_one, wire_value};

const ALL_KINDS: [AssistantFrameKind; 10] = [
    AssistantFrameKind::Invoke,
    AssistantFrameKind::ResponsesEvent,
    AssistantFrameKind::ToolRequest,
    AssistantFrameKind::ToolResponse,
    AssistantFrameKind::ApprovalRequest,
    AssistantFrameKind::ApprovalResponse,
    AssistantFrameKind::Cancel,
    AssistantFrameKind::Snapshot,
    AssistantFrameKind::Completed,
    AssistantFrameKind::Error,
];

#[tokio::test]
async fn assistant_transport_round_trips_all_supported_kinds_and_payloads() {
    let mut writer = AssistantFrameWriter::new(TestWriter::default());
    for (sequence, kind) in ALL_KINDS.into_iter().enumerate() {
        let frame = AssistantFrame::new(
            sequence as u64,
            kind,
            json!({"kind_index": sequence, "nested": {"preserved": true}}),
        )
        .expect("test frame should be valid");
        writer.write_frame(&frame).await.expect("frame should encode");
    }

    let encoded = writer.into_inner().bytes;
    let mut reader = AssistantFrameReader::new(BufReader::new(std::io::Cursor::new(encoded)));
    for (sequence, kind) in ALL_KINDS.into_iter().enumerate() {
        let frame = reader.read_frame().await.expect("frame should decode");
        assert_eq!(frame.protocol_version(), PROTOCOL_VERSION);
        assert_eq!(frame.sequence(), sequence as u64);
        assert_eq!(frame.kind(), kind);
        assert_eq!(
            frame.payload(),
            &json!({"kind_index": sequence, "nested": {"preserved": true}})
        );
    }
}

#[tokio::test]
async fn assistant_transport_encodes_exact_top_level_keys_and_flushes() {
    let mut writer = AssistantFrameWriter::new(TestWriter::default());
    let frame = AssistantFrame::new(0, AssistantFrameKind::Invoke, json!({"prompt": "draw"}))
        .expect("test frame should be valid");
    writer.write_frame(&frame).await.expect("frame should encode");

    let sink = writer.into_inner();
    assert_eq!(sink.flushes, 1);
    assert_eq!(sink.bytes.last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(&sink.bytes[..sink.bytes.len() - 1])
        .expect("encoded frame should be JSON");
    let mut keys = value
        .as_object()
        .expect("encoded frame should be an object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, ["kind", "payload", "protocol_version", "sequence"]);
}

#[test]
fn assistant_transport_rejects_invalid_payload_before_encoding() {
    let error = AssistantFrame::new(0, AssistantFrameKind::Invoke, json!("not an object"))
        .expect_err("payload must be an object");
    assert!(matches!(error, AssistantTransportError::PayloadMustBeObject));
}

#[tokio::test]
async fn assistant_transport_rejects_invalid_frames_before_writing() {
    let mut sequence_writer = AssistantFrameWriter::new(TestWriter::default());
    let wrong_sequence = AssistantFrame::new(1, AssistantFrameKind::Invoke, json!({}))
        .expect("test frame should be valid");
    let sequence_error = sequence_writer
        .write_frame(&wrong_sequence)
        .await
        .expect_err("first outbound sequence must be zero");
    assert!(matches!(
        sequence_error,
        AssistantTransportError::UnexpectedSequence { expected: 0, actual: 1 }
    ));
    assert!(sequence_writer.into_inner().bytes.is_empty());

    let mut number_writer = AssistantFrameWriter::new(TestWriter::default());
    let unsafe_number = AssistantFrame::new(
        0,
        AssistantFrameKind::Invoke,
        json!({"value": MAX_SAFE_JSON_NUMBER + 1}),
    )
    .expect("payload remains a JSON object");
    let number_error = number_writer
        .write_frame(&unsafe_number)
        .await
        .expect_err("unsafe JSON number must be rejected");
    assert!(matches!(number_error, AssistantTransportError::JsonNumberOutOfRange { .. }));
    assert!(number_writer.into_inner().bytes.is_empty());
}

#[tokio::test]
async fn assistant_transport_accepts_maximum_encoded_frame_and_rejects_one_byte_more() {
    let overhead = encoded_wire_frame(0, "invoke", json!({"data": ""})).len();
    let maximum = AssistantFrame::new(
        0,
        AssistantFrameKind::Invoke,
        json!({"data": "x".repeat(MAX_FRAME_SIZE - overhead)}),
    )
    .expect("maximum frame payload should be valid");
    let mut maximum_writer = AssistantFrameWriter::new(TestWriter::default());
    maximum_writer.write_frame(&maximum).await.expect("frame at the limit should encode");
    assert_eq!(maximum_writer.into_inner().bytes.len(), MAX_FRAME_SIZE);

    let oversized = AssistantFrame::new(
        0,
        AssistantFrameKind::Invoke,
        json!({"data": "x".repeat(MAX_FRAME_SIZE - overhead + 1)}),
    )
    .expect("oversized payload remains structurally valid");
    let mut oversized_writer = AssistantFrameWriter::new(TestWriter::default());
    let error = oversized_writer
        .write_frame(&oversized)
        .await
        .expect_err("oversized frame must be rejected");
    assert!(matches!(
        error,
        AssistantTransportError::FrameTooLarge { max, .. } if max == MAX_FRAME_SIZE
    ));
    assert!(oversized_writer.into_inner().bytes.is_empty());
}

#[tokio::test]
async fn assistant_transport_accepts_safe_number_boundaries_and_fractional_values() {
    let payload = json!({
        "positive": MAX_SAFE_JSON_NUMBER,
        "negative": -(MAX_SAFE_JSON_NUMBER as i64),
        "fractional": 1234.5
    });
    let frame = read_one(encoded_wire_frame(0, "invoke", payload.clone()))
        .await
        .expect("safe JSON numbers should decode");
    assert_eq!(frame.payload(), &payload);
}

#[tokio::test]
async fn assistant_transport_accepts_depth_64_and_rejects_depth_65_before_dispatch() {
    let accepted =
        read_one(encoded_wire_frame(0, "invoke", nested_payload_at_depth(MAX_JSON_DEPTH)))
            .await
            .expect("frame at the depth limit should decode");
    assert!(accepted.payload().is_object());

    let executions = AtomicUsize::new(0);
    let input = encoded_wire_frame(0, "invoke", nested_payload_at_depth(MAX_JSON_DEPTH + 1));
    let mut reader = AssistantFrameReader::new(BufReader::new(std::io::Cursor::new(input)));
    let error = reader
        .read_and_dispatch(|_frame| executions.fetch_add(1, Ordering::SeqCst))
        .await
        .expect_err("frame beyond the depth limit must fail");
    assert!(matches!(
        error,
        AssistantTransportError::JsonDepthExceeded { maximum } if maximum == MAX_JSON_DEPTH
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_transport_depth_counts_containers_not_scalar_leaves() {
    let mut scalar_leaf = json!({"leaf": true});
    for _ in 1..MAX_JSON_DEPTH {
        scalar_leaf = json!({"child": scalar_leaf});
    }
    read_one(encoded_wire_frame(0, "invoke", scalar_leaf))
        .await
        .expect("scalar beneath a depth-64 container should decode");

    let error =
        read_one(encoded_wire_frame(0, "invoke", nested_payload_at_depth(MAX_JSON_DEPTH + 1)))
            .await
            .expect_err("container at depth 65 must fail");
    assert!(matches!(
        error,
        AssistantTransportError::JsonDepthExceeded { maximum } if maximum == MAX_JSON_DEPTH
    ));
}

#[tokio::test]
async fn assistant_transport_writer_accepts_depth_64_and_writes_nothing_for_depth_65() {
    let boundary =
        AssistantFrame::new(0, AssistantFrameKind::Invoke, nested_payload_at_depth(MAX_JSON_DEPTH))
            .expect("boundary payload should be an object");
    let mut boundary_writer = AssistantFrameWriter::new(TestWriter::default());
    boundary_writer.write_frame(&boundary).await.expect("frame at the depth limit should encode");
    assert!(!boundary_writer.into_inner().bytes.is_empty());

    let exceeded = AssistantFrame::new(
        0,
        AssistantFrameKind::Invoke,
        nested_payload_at_depth(MAX_JSON_DEPTH + 1),
    )
    .expect("deep payload remains an object");
    let mut exceeded_writer = AssistantFrameWriter::new(TestWriter::default());
    let error = exceeded_writer
        .write_frame(&exceeded)
        .await
        .expect_err("frame beyond the depth limit must fail");
    assert!(matches!(
        error,
        AssistantTransportError::JsonDepthExceeded { maximum } if maximum == MAX_JSON_DEPTH
    ));
    assert!(exceeded_writer.into_inner().bytes.is_empty());
}

#[tokio::test]
async fn assistant_transport_rejects_out_of_range_json_numbers() {
    let positive = MAX_SAFE_JSON_NUMBER + 1;
    let negative = -(positive as i64);
    let tokens = [
        positive.to_string(),
        negative.to_string(),
        "184467440737095516160".to_owned(),
        "9.007199254740992e15".to_owned(),
    ];

    for token in tokens {
        let input = format!(
            "{{\"protocol_version\":1,\"sequence\":0,\"kind\":\"invoke\",\"payload\":{{\"nested\":[{{\"value\":{token}}}]}}}}\n"
        );
        let error =
            read_one(input.into_bytes()).await.expect_err("out-of-range JSON number must fail");
        assert!(matches!(error, AssistantTransportError::JsonNumberOutOfRange { .. }));
    }
}

#[tokio::test]
async fn assistant_transport_rejects_non_finite_json_numbers() {
    for token in ["NaN", "Infinity", "-Infinity", "1e400"] {
        let input = format!(
            "{{\"protocol_version\":1,\"sequence\":0,\"kind\":\"invoke\",\"payload\":{{\"value\":{token}}}}}\n"
        );
        let error =
            read_one(input.into_bytes()).await.expect_err("non-finite JSON number must fail");
        assert!(matches!(error, AssistantTransportError::MalformedJson { .. }));
    }
}

#[tokio::test]
async fn assistant_transport_rejects_malformed_json_utf8_and_duplicate_keys() {
    let cases = [
        b"{not json}\n".to_vec(),
        vec![b'{', b'}', 0xff, b'\n'],
        br#"{"protocol_version":1,"sequence":0,"sequence":0,"kind":"invoke","payload":{}}
"#
        .to_vec(),
        br#"{"protocol_version":1,"sequence":0,"kind":"invoke","payload":{"value":1,"value":2}}
"#
        .to_vec(),
    ];

    for input in cases {
        assert!(read_one(input).await.is_err());
    }
}

#[tokio::test]
async fn assistant_transport_rejects_unknown_and_invalid_envelopes() {
    let cases = [
        json!([]),
        json!({
            "protocol_version": 1,
            "sequence": 0,
            "kind": "invoke",
            "payload": {},
            "extra": true
        }),
        json!({"protocol_version": 1, "sequence": 0, "kind": "invoke"}),
        wire_value(2, 0, "invoke", json!({})),
        wire_value(1, 0, "future_kind", json!({})),
        wire_value(1, 0, "invoke", json!(["not", "object"])),
        json!({"protocol_version": 1, "sequence": -1, "kind": "invoke", "payload": {}}),
    ];

    for value in cases {
        assert!(read_one(json_line(value)).await.is_err());
    }
}

#[tokio::test]
async fn assistant_transport_rejects_oversized_partial_and_clean_eof_frames() {
    let mut oversized = vec![b'x'; MAX_FRAME_SIZE];
    oversized.push(b'\n');
    assert!(matches!(
        read_one(oversized).await,
        Err(AssistantTransportError::FrameTooLarge { max, .. }) if max == MAX_FRAME_SIZE
    ));
    assert!(matches!(
        read_one(br#"{"protocol_version":1"#.to_vec()).await,
        Err(AssistantTransportError::PartialFrameAtEof { .. })
    ));
    assert!(matches!(
        read_one(Vec::new()).await,
        Err(AssistantTransportError::UnexpectedEof { expected_sequence: 0 })
    ));
}

#[tokio::test]
async fn assistant_transport_rejects_duplicate_out_of_order_and_gapped_sequences() {
    for sequences in [vec![0, 0], vec![0, 1, 0], vec![0, 2]] {
        let input = sequences
            .iter()
            .flat_map(|sequence| encoded_wire_frame(*sequence, "invoke", json!({})))
            .collect::<Vec<_>>();
        let mut reader = AssistantFrameReader::new(BufReader::new(std::io::Cursor::new(input)));
        let mut error = None;
        for _ in sequences {
            if let Err(source) = reader.read_frame().await {
                error = Some(source);
                break;
            }
        }
        assert!(matches!(error, Some(AssistantTransportError::UnexpectedSequence { .. })));
    }
}

#[tokio::test]
async fn assistant_transport_invalid_input_and_lone_surrogate_never_dispatch() {
    let executions = AtomicUsize::new(0);
    let inputs = [
        b"not-json\n".to_vec(),
        br#"{"protocol_version":1,"sequence":0,"kind":"invoke","payload":{"text":"\ud800"}}
"#
        .to_vec(),
    ];

    for input in inputs {
        let mut reader = AssistantFrameReader::new(BufReader::new(std::io::Cursor::new(input)));
        let result =
            reader.read_and_dispatch(|_frame| executions.fetch_add(1, Ordering::SeqCst)).await;
        assert!(result.is_err());
    }
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn assistant_transport_writer_closes_after_partial_write_failure() {
    let mut writer = AssistantFrameWriter::new(TestWriter::partial_then_fail(12));
    let frame = AssistantFrame::new(0, AssistantFrameKind::Invoke, json!({"prompt": "draw"}))
        .expect("test frame should be valid");
    assert!(matches!(writer.write_frame(&frame).await, Err(AssistantTransportError::Write { .. })));
    assert!(matches!(writer.write_frame(&frame).await, Err(AssistantTransportError::Closed)));
    assert_eq!(writer.into_inner().bytes.len(), 12);
}

#[tokio::test]
async fn assistant_transport_writer_closes_after_flush_failure() {
    let mut writer = AssistantFrameWriter::new(TestWriter::fail_flush());
    let frame = AssistantFrame::new(0, AssistantFrameKind::Invoke, json!({"prompt": "draw"}))
        .expect("test frame should be valid");
    assert!(matches!(writer.write_frame(&frame).await, Err(AssistantTransportError::Flush { .. })));
    assert!(matches!(writer.write_frame(&frame).await, Err(AssistantTransportError::Closed)));
    assert_eq!(writer.into_inner().flushes, 1);
}

fn nested_payload_at_depth(deepest_depth: usize) -> Value {
    let mut value = json!({});
    for _ in 1..deepest_depth {
        value = json!({"child": value});
    }
    value
}
