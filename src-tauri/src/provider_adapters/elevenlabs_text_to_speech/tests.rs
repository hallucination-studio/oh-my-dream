use engine::node_capability::{WorkflowTextPart, WorkflowTextValue};
use serde_json::json;

use super::*;

#[test]
fn freezes_voice_path_and_request_body() {
    assert!(ElevenLabsVoiceId::try_new("JBFqnCBsd6RMkjVDRZzb").is_ok());
    assert!(ElevenLabsVoiceId::try_new("../voice").is_err());
    assert!(ElevenLabsVoiceId::try_new("").is_err());

    let text =
        WorkflowTextValue::try_new([WorkflowTextPart::Literal("Hello world".into())]).unwrap();
    let body =
        serde_json::to_value(RequestDto { text: literal_text(&text).unwrap(), model_id: MODEL_ID })
            .unwrap();

    assert_eq!(
        body,
        json!({
            "text": "Hello world",
            "model_id": "eleven_multilingual_v2"
        })
    );
    assert_eq!(
        endpoint("JBFqnCBsd6RMkjVDRZzb"),
        "https://api.elevenlabs.io/v1/text-to-speech/JBFqnCBsd6RMkjVDRZzb?output_format=mp3_44100_128"
    );
}

#[test]
fn public_configuration_errors_never_include_candidate_secrets() {
    let candidate = "secret/voice";
    let Err(error) = ElevenLabsVoiceId::try_new(candidate) else {
        panic!("invalid voice ID was accepted");
    };

    assert_eq!(error.to_string(), "invalid ElevenLabs voice ID");
    assert!(!format!("{error:?}").contains(candidate));
}
