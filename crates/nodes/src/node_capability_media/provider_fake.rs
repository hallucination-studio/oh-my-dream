use std::io::Cursor;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory,
    NodeCapabilityProviderFailureConstructionError,
};
use sha2::{Digest, Sha256};

use super::*;

macro_rules! provider_fake {
    ($name:ident, $interface:ident, $method:ident, $request:ty, $payload:ty, $facts:expr, $bytes:expr) => {
        #[doc = concat!("Deterministic ", stringify!($interface), " fake implementation.")]
        pub struct $name {
            deadline_failure: NodeCapabilityProviderFailure,
            invalid_response_failure: NodeCapabilityProviderFailure,
        }
        impl $name {
            /// Creates a fake with fixed valid provider failures.
            pub fn try_new() -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
                Ok(Self {
                    deadline_failure: provider_failure(
                        NodeCapabilityProviderFailureCategory::DeadlineExceeded,
                    )?,
                    invalid_response_failure: provider_failure(
                        NodeCapabilityProviderFailureCategory::InvalidResponse,
                    )?,
                })
            }
        }
        #[async_trait]
        impl $interface for $name {
            async fn $method(
                &self,
                request: $request,
            ) -> Result<$payload, NodeCapabilityProviderFailure> {
                if request.context().deadline.is_reached_at(Instant::now()) {
                    return Err(self.deadline_failure.clone());
                }
                let bytes: Vec<u8> = $bytes;
                let source = NodeCapabilityMediaSourceLease::try_new(
                    bytes.len() as u64,
                    provider_digest(&bytes),
                    request.context().deadline.monotonic_instant(),
                    Box::pin(Cursor::new(bytes)),
                )
                .map_err(|_| self.invalid_response_failure.clone())?;
                let facts = $facts.map_err(|_| self.invalid_response_failure.clone())?;
                <$payload>::try_new(facts, source)
                    .map_err(|_| self.invalid_response_failure.clone())
            }
        }
    };
}

provider_fake!(
    TextToImageProviderFakeImpl,
    TextToImageProviderInterface,
    generate_image_from_text,
    TextToImageProviderRequest,
    GeneratedImagePayload,
    NodeCapabilityDeclaredMediaFacts::try_image(32, 32),
    vec![1; 16]
);
provider_fake!(
    ImageToVideoProviderFakeImpl,
    ImageToVideoProviderInterface,
    generate_video_from_image,
    ImageToVideoProviderRequest,
    GeneratedVideoPayload,
    NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 5_000, false),
    vec![2; 24]
);
provider_fake!(
    TextToSpeechProviderFakeImpl,
    TextToSpeechProviderInterface,
    synthesize_speech_from_text,
    TextToSpeechProviderRequest,
    SynthesizedSpeechPayload,
    NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 1),
    vec![3; 12]
);

fn provider_digest(bytes: &[u8]) -> NodeCapabilityMediaContentDigest {
    NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(bytes).into())
}

fn provider_failure(
    category: NodeCapabilityProviderFailureCategory,
) -> Result<NodeCapabilityProviderFailure, NodeCapabilityProviderFailureConstructionError> {
    NodeCapabilityProviderFailure::try_new(category, false, Instant::now(), None)
}
