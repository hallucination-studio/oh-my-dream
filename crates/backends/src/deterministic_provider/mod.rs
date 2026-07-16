//! Deterministic exact provider routes used by tests and local composition.

use std::io::Cursor;
use std::time::Instant;

use async_trait::async_trait;
use engine::node_capability::{
    NodeCapabilityProviderFailure, NodeCapabilityProviderFailureCategory,
    NodeCapabilityProviderFailureConstructionError,
};
use nodes::{
    GeneratedImagePayload, GeneratedVideoPayload, NodeCapabilityDeclaredMediaFacts,
    NodeCapabilityMediaContentDigest, NodeCapabilityMediaSourceLease, SynthesizedSpeechPayload,
};
use sha2::{Digest, Sha256};

use crate::provider_routing::{
    GenerationProviderRouteAvailability, GenerationProviderRouteId,
    ImageToVideoProviderRouteInterface, ImageToVideoProviderRouteRequest,
    TextToImageProviderRouteInterface, TextToImageProviderRouteRequest,
    TextToSpeechProviderRouteInterface, TextToSpeechProviderRouteRequest,
};

macro_rules! deterministic_route {
    (
        $name:ident,
        $interface:ident,
        $request:ident,
        $payload:ident,
        $method:ident,
        $route_id:literal,
        $facts:expr,
        $bytes:expr
    ) => {
        #[doc = concat!("Deterministic implementation of `", stringify!($interface), "`.")]
        pub struct $name {
            deadline_failure: NodeCapabilityProviderFailure,
            invalid_response_failure: NodeCapabilityProviderFailure,
        }

        impl $name {
            /// Creates the route with its fixed safe failure values.
            pub fn try_new() -> Result<Self, NodeCapabilityProviderFailureConstructionError> {
                Ok(Self {
                    deadline_failure: failure(
                        NodeCapabilityProviderFailureCategory::DeadlineExceeded,
                    )?,
                    invalid_response_failure: failure(
                        NodeCapabilityProviderFailureCategory::InvalidResponse,
                    )?,
                })
            }
        }

        #[async_trait]
        impl $interface for $name {
            fn generation_provider_route_id(&self) -> GenerationProviderRouteId {
                GenerationProviderRouteId::new($route_id)
            }

            async fn observe_provider_route_availability(
                &self,
            ) -> GenerationProviderRouteAvailability {
                GenerationProviderRouteAvailability::Available
            }

            async fn $method(
                &self,
                request: $request,
            ) -> Result<$payload, NodeCapabilityProviderFailure> {
                if request.context().deadline.is_reached_at(Instant::now()) {
                    return Err(self.deadline_failure.clone());
                }
                let bytes: Vec<u8> = $bytes;
                let source = source(bytes, request.context().deadline.monotonic_instant())
                    .map_err(|_| self.invalid_response_failure.clone())?;
                let facts = $facts.map_err(|_| self.invalid_response_failure.clone())?;
                $payload::try_new(facts, source).map_err(|_| self.invalid_response_failure.clone())
            }
        }
    };
}

deterministic_route!(
    DeterministicTextToImageProviderRouteImpl,
    TextToImageProviderRouteInterface,
    TextToImageProviderRouteRequest,
    GeneratedImagePayload,
    generate_image_from_text,
    "deterministic.text_to_image",
    NodeCapabilityDeclaredMediaFacts::try_image(32, 32),
    vec![1; 16]
);
deterministic_route!(
    DeterministicImageToVideoProviderRouteImpl,
    ImageToVideoProviderRouteInterface,
    ImageToVideoProviderRouteRequest,
    GeneratedVideoPayload,
    generate_video_from_image,
    "deterministic.image_to_video",
    NodeCapabilityDeclaredMediaFacts::try_video(32, 32, 5_000, false),
    vec![2; 24]
);
deterministic_route!(
    DeterministicTextToSpeechProviderRouteImpl,
    TextToSpeechProviderRouteInterface,
    TextToSpeechProviderRouteRequest,
    SynthesizedSpeechPayload,
    synthesize_speech_from_text,
    "deterministic.text_to_speech",
    NodeCapabilityDeclaredMediaFacts::try_audio(1_000, 44_100, 1),
    vec![3; 12]
);

fn source(
    bytes: Vec<u8>,
    deadline: Instant,
) -> Result<NodeCapabilityMediaSourceLease, nodes::NodeCapabilityMediaValueError> {
    NodeCapabilityMediaSourceLease::try_new(
        bytes.len() as u64,
        NodeCapabilityMediaContentDigest::from_bytes(Sha256::digest(&bytes).into()),
        deadline,
        Box::pin(Cursor::new(bytes)),
    )
}

fn failure(
    category: NodeCapabilityProviderFailureCategory,
) -> Result<NodeCapabilityProviderFailure, NodeCapabilityProviderFailureConstructionError> {
    NodeCapabilityProviderFailure::try_new(category, false, Instant::now(), None)
}
