# Backend Node Capability Architecture

> Status: frozen MVP interface and named roadmap
> Owner: interface in `crates/engine`; exact implementations in `crates/nodes`
> Scope: semantic operations executable by Workflow nodes

A Node Capability is one versioned business operation. It is not a UI shell, provider feature,
native model, or generic task.

## Decision

1. Workflow owns one consumer-facing `WorkflowNodeCapabilityInterface`.
2. Every exact operation has one behavior-revealing implementation such as
   `ImageToVideoCapabilityImpl`.
3. One immutable `WorkflowNodeCapabilityRegistry` holds active implementations by exact
   `NodeCapabilityContractRef`.
4. Each implementation owns its contract, parameter normalization, external readiness, typed
   request mapping, result validation, and execution behavior.
5. Each implementation receives only the focused external interfaces it consumes.
6. Model-powered implementations require a provider-independent `GenerationProfileRef`.

There is no separate catalog/executor pair, generic options map, optional capability method, or
`Unsupported` implementation.

## Consumer-Owned Interface

```rust
#[async_trait]
pub trait WorkflowNodeCapabilityInterface: Send + Sync {
    fn node_capability_contract(&self) -> &NodeCapabilityContract;

    fn normalize_node_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NodeCapabilityNormalizedParameters, NodeCapabilityParameterError>;

    async fn check_node_external_readiness(
        &self,
        request: NodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue>;

    async fn execute_node_capability(
        &self,
        request: NodeCapabilityExecutionRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError>;
}
```

The names make all four behaviors explicit. Workflow uses the contract for structural graph checks,
normalization for an immutable execution plan, external readiness for Assets and Generation
Profiles, and execution only after durable Run admission.

`NodeCapabilityNormalizedParameters` contains normalized parameters plus mechanically extracted
stable input-item references. Exact typed parameters and provider requests remain private to the
implementation.

`WorkflowNodeCapabilityRegistry` is a concrete immutable collection, not another trait. It rejects
duplicate refs, lists the same contracts used at execution, resolves one exact implementation, and
never reimplements capability rules.

## Versioned Contract

```rust
pub struct NodeCapabilityContract {
    pub contract_ref: NodeCapabilityContractRef,
    pub parameters: Vec<NodeCapabilityParameterContract>,
    pub inputs: Vec<NodeCapabilityInputContract>,
    pub outputs: Vec<NodeCapabilityOutputContract>,
    pub execution_kind: NodeCapabilityExecutionKind,
}
```

Changing parameter meaning, input cardinality, role meaning, output type, or result guarantee
requires a new contract version. The exact implementation is the semantic owner; Workflow, DTOs,
forms, and provider routes only consume or translate the contract.

`NodeCapabilityExecutionKind` is a closed business classification:
`PureValue`, `ManagedAssetRead`, `ContentGeneration`, `MediaTransformation`, or `ContentAnalysis`.
It never means local versus remote.

## Frozen MVP Implementations

[`BACKEND.md`](BACKEND.md#active-node-capabilities) is the single authority for the seven active
contract refs, implementation names, and external dependencies. `WorkflowNodeCapabilityRegistry`
contains exactly that set; this document does not maintain a second copy.

The active runtime data types are `Text`, `Image`, `Video`, and `Audio`. The UI derives those four
shells from each contract's primary output; shell kind is never persisted as domain identity.

## Implementation Shape

```rust
pub struct ImageToVideoCapabilityImpl<R, P, W> {
    managed_media_reader: R,
    image_to_video_provider: P,
    produced_media_writer: W,
    contract: NodeCapabilityContract,
}
```

`ImageToVideoCapabilityImpl`:

1. validates and normalizes its stable parameters;
2. checks the selected `GenerationProfileRef` and input Asset readiness;
3. converts Workflow values into `ImageToVideoProviderRequest`;
4. calls `ImageToVideoProviderInterface::generate_video_from_image`;
5. validates the `GeneratedVideoPayload`;
6. stores it through `NodeCapabilityProducedMediaWriterInterface::write_node_output_media`;
7. returns one managed Video in a complete `WorkflowNodeOutputSet`.

It never reads a provider name, route ID, path, URL, Asset repository, or concrete adapter.

## MVP External Interfaces

| Interface | Behavior method | Request | Result |
| --- | --- | --- | --- |
| `NodeCapabilityManagedMediaReaderInterface` | `read_managed_media` | `NodeCapabilityManagedMediaReadRequest` | `NodeCapabilityReadableMediaInput` |
| `NodeCapabilityProducedMediaWriterInterface` | `write_node_output_media` | `NodeCapabilityProducedMediaWriteRequest` | typed managed Workflow media ref |
| `TextToImageProviderInterface` | `generate_image_from_text` | `TextToImageProviderRequest` | `GeneratedImagePayload` |
| `ImageToVideoProviderInterface` | `generate_video_from_image` | `ImageToVideoProviderRequest` | `GeneratedVideoPayload` |
| `TextToSpeechProviderInterface` | `synthesize_speech_from_text` | `TextToSpeechProviderRequest` | `SynthesizedSpeechPayload` |

Every exact provider request contains semantic inputs, typed parameters, `GenerationProfileRef`,
and `WorkflowNodeExecutionId`. It contains no provider name, native model ID, credential, endpoint,
URL, path, provider task, wire DTO, or generic options map.

The media-write request includes `NodeCapabilityProducedMediaOutputKey`, derived from Workflow Run,
node execution, output key, and ordinal. The Desktop bridge translates it to the Asset-owned
`AssetNodeOutputKey`. Repeating the same key and digest returns the same Asset; a different digest
is a conflict.

## Named Roadmap Contracts

These operations have approved semantic names but are not part of the MVP registry. Their traits
and implementations are introduced only when scheduled; this table prevents future provider-shaped
or ambiguous names.

| Planned contract ref | Reserved implementation name | Reserved external interface and method |
| --- | --- | --- |
| `image.generate_from_image@1.0` | `ImageToImageCapabilityImpl` | `ImageToImageProviderInterface::generate_image_from_image` |
| `image.generate_from_reference_images@1.0` | `ReferenceImagesToImageCapabilityImpl` | `ReferenceImagesToImageProviderInterface::generate_image_from_reference_images` |
| `image.crop@1.0` | `ImageCropCapabilityImpl` | `ImageCropInterface::crop_image` |
| `video.generate_from_text@1.0` | `TextToVideoCapabilityImpl` | `TextToVideoProviderInterface::generate_video_from_text` |
| `video.generate_from_reference_images@1.0` | `ReferenceImagesToVideoCapabilityImpl` | `ReferenceImagesToVideoProviderInterface::generate_video_from_reference_images` |
| `video.generate_from_first_frame@1.0` | `FirstFrameToVideoCapabilityImpl` | `FirstFrameToVideoProviderInterface::generate_video_from_first_frame` |
| `video.generate_from_first_and_last_frames@1.0` | `FirstAndLastFramesToVideoCapabilityImpl` | `FirstAndLastFramesToVideoProviderInterface::generate_video_from_first_and_last_frames` |
| `video.generate_from_mixed_media@1.0` | `MixedMediaToVideoCapabilityImpl` | `MixedMediaToVideoProviderInterface::generate_video_from_mixed_media` |
| `video.upscale@1.0` | `VideoUpscaleCapabilityImpl` | `VideoUpscaleProviderInterface::upscale_video` |
| `video.extract_frames@1.0` | `VideoFrameExtractionCapabilityImpl` | `VideoFrameExtractionInterface::extract_video_frames` |
| `video.concatenate@1.0` | `VideoConcatenationCapabilityImpl` | `VideoConcatenationInterface::concatenate_videos` |
| `video.analyze_storyboard@1.0` | `VideoStoryboardAnalysisCapabilityImpl` | `VideoStoryboardAnalysisProviderInterface::analyze_storyboard_from_video` |
| `text.generate_from_text@1.0` | `TextToTextGenerationCapabilityImpl` | `TextToTextGenerationProviderInterface::generate_text_from_text` |
| `text.generate_from_mixed_media@1.0` | `MixedMediaToTextGenerationCapabilityImpl` | `MixedMediaToTextGenerationProviderInterface::generate_text_from_mixed_media` |
| `audio.generate_music_from_text@1.0` | `TextToMusicCapabilityImpl` | `TextToMusicProviderInterface::generate_music_from_text` |

The distinctions are contractual:

- `ImageToVideo` uses an image as conditioning input but does not promise exact first-frame pixels;
- `FirstFrameToVideo` guarantees the supplied image is the first frame;
- `FirstAndLastFramesToVideo` guarantees both endpoint frames;
- reference-image operations use ordered role-bearing subject/style/material references;
- mixed-media operations accept typed Image and Video items with explicit roles;
- speech and music remain separate operations, profiles, results, and failure semantics.

Activating a roadmap operation requires its complete parameter, input-role, result, error, mock,
provider/media interface, Asset provenance, UI contract, and E2E tests in the same change.

## Results And Errors

Provider media payloads contain one fixed media kind, declared facts, and a bounded asynchronous
byte stream. Text and Storyboard results are validated semantic values, never provider JSON hidden
inside a string.

A capability returns all declared outputs or one structured error. Required media becomes a
Workflow output only after every corresponding Asset is Available; partial Asset creation never
becomes a partial `WorkflowNodeOutputSet`.

`NodeCapabilityParameterError` and `NodeCapabilityReadinessIssue` occur before dispatch.
`NodeCapabilityProviderFailure` and `NodeCapabilityMediaFailure` use closed categories,
retryability, and optional safe retry time. `NodeCapabilityExecutionError` adds capability, stage,
and safe target. Raw provider text, native IDs, URLs, paths, credentials, and response bodies never
cross these errors.

## Contract Tests

- registry tests reject duplicate refs and assert the exact seven-contract MVP set;
- every active implementation passes the shared `WorkflowNodeCapabilityInterface` behavior suite;
- exact tests cover normalization, stable input order, request mapping, result validation,
  provenance, idempotent media write, and error translation;
- deterministic and production routers pass the same exact provider-interface suites;
- fault-injection tests cover cancellation, timeout, malformed payload, Asset write failure, and
  duplicate output-key conflict;
- architecture tests reject a second catalog/executor, roadmap registration, broad provider
  interface, optional unsupported method, generic options map, and concrete adapter selection
  outside composition.
