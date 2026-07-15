# Backend Node Capability Architecture

> Status: proposed target architecture
> Owner: capability interface in `crates/engine`; exact implementations in `crates/nodes`
> Scope: built-in Text, Image, Video, Audio, media-processing, and analysis operations

A node capability is one versioned business operation. It is not a UI shell, provider feature,
native model, or generic job.

## Decision

1. Workflow owns one consumer-facing `WorkflowNodeCapabilityPort` interface.
2. Every exact operation has one named implementation such as `TextToVideoCapability`.
3. One concrete `WorkflowNodeCapabilityRegistry` stores the implementations by immutable
   `NodeCapabilityContractRef`. It replaces separate catalog and executor abstractions.
4. Each implementation owns its contract, parameter rules, readiness, request mapping, result
   validation, and execution behavior.
5. An implementation depends only on the focused external ports it actually consumes.
6. Model-powered implementations persist a provider-independent `GenerationProfileRef`.

All capability implementations satisfy the same complete interface. There are no optional methods,
`Unsupported` results, provider feature probes, or untyped parameter maps.

## Capability Interface

The interface is owned by its Workflow consumer in `crates/engine`:

```rust
#[async_trait]
pub trait WorkflowNodeCapabilityPort: Send + Sync {
    fn node_capability_contract(&self) -> &NodeCapabilityContract;

    fn normalize_parameters(
        &self,
        parameters: &NodeCapabilityParameterSet,
    ) -> Result<NormalizedNodeCapabilityParameters, NodeCapabilityParameterError>;

    async fn check_readiness(
        &self,
        request: CheckNodeCapabilityReadinessRequest,
    ) -> Vec<NodeCapabilityReadinessIssue>;

    async fn execute(
        &self,
        request: ExecuteNodeCapabilityRequest,
    ) -> Result<WorkflowNodeOutputSet, NodeCapabilityExecutionError>;
}
```

These four behaviors are cohesive: Workflow needs all of them for editing, validation, planning,
and execution. `NormalizedNodeCapabilityParameters` contains the normalized parameter set and its
mechanically projected input-item references. Exact typed parameters and provider requests remain
private to the implementation.

`WorkflowNodeCapabilityRegistry` is a stable internal collection, not another trait. It rejects
duplicate refs, exposes immutable contract iteration, resolves one exact implementation, and
delegates behavior without reimplementing it. The composition root registers implementations once;
forms, readiness, and execution therefore cannot drift between separate registries.

## Capability Implementations

```text
crates/engine/src/workflow/node_capability.rs
  WorkflowNodeCapabilityPort, WorkflowNodeCapabilityRegistry, shared contract values

crates/nodes/src/
  text_to_video/{capability,parameters,provider_port}.rs
  first_and_last_frames_to_video/{capability,parameters,provider_port}.rs
  video_concatenation/{capability,parameters,media_port}.rs
  ...one business-capability directory per exact implementation
```

```rust
pub struct TextToVideoCapability<P, W> {
    text_to_video_provider: P,
    produced_media_writer: W,
    contract: NodeCapabilityContract,
}
```

`TextToVideoCapability` implements `WorkflowNodeCapabilityPort`. It converts generic Workflow
values into a private `TextToVideoParameters` and `TextToVideoProviderRequest`, calls
`TextToVideoProviderPort`, validates `GeneratedVideoPayload`, stores the result through
`NodeCapabilityProducedMediaWriterPort`, and returns a managed Video output.

The same pattern applies to every exact operation. Implementations do not inspect provider names,
look up concrete adapters, access Asset repositories, or branch on unsupported operations.

## Versioned Contract

```rust
pub struct NodeCapabilityContract {
    pub node_capability_contract_ref: NodeCapabilityContractRef,
    pub parameters: Vec<NodeCapabilityParameterContract>,
    pub inputs: Vec<NodeCapabilityInputPortContract>,
    pub outputs: Vec<NodeCapabilityOutputPortContract>,
    pub execution_kind: NodeCapabilityExecutionKind,
}
```

An exact version is immutable. Changing parameter meaning, input cardinality, role semantics,
output type, or result guarantees requires a new version. `execution_kind` describes business
behavior: `PureValue`, `ManagedAssetRead`, `ContentGeneration`, `MediaTransformation`, or
`ContentAnalysis`; it does not describe local versus remote deployment.

The implementation is the single semantic owner. DTOs, forms, provider routes, and Workflow graph
code only consume its contract or translate boundary values.

## Built-In Implementations

| Contract ref | Implementation | External port |
| --- | --- | --- |
| `text.provide_literal@1.0` | `ProvideLiteralTextCapability` | none |
| `image.read_asset@1.0` | `ReadImageAssetCapability` | `NodeCapabilityManagedMediaReaderPort` |
| `video.read_asset@1.0` | `ReadVideoAssetCapability` | `NodeCapabilityManagedMediaReaderPort` |
| `audio.read_asset@1.0` | `ReadAudioAssetCapability` | `NodeCapabilityManagedMediaReaderPort` |
| `image.generate_from_text@1.0` | `TextToImageCapability` | `TextToImageProviderPort` |
| `image.generate_from_image@1.0` | `ImageToImageCapability` | `ImageToImageProviderPort` |
| `image.generate_from_reference_images@1.0` | `ReferenceImagesToImageCapability` | `ReferenceImagesToImageProviderPort` |
| `image.crop@1.0` | `CropImageCapability` | `ImageCropPort` |
| `video.generate_from_text@1.0` | `TextToVideoCapability` | `TextToVideoProviderPort` |
| `video.generate_from_image@1.0` | `ImageToVideoCapability` | `ImageToVideoProviderPort` |
| `video.generate_from_reference_images@1.0` | `ReferenceImagesToVideoCapability` | `ReferenceImagesToVideoProviderPort` |
| `video.generate_from_first_frame@1.0` | `FirstFrameToVideoCapability` | `FirstFrameToVideoProviderPort` |
| `video.generate_from_first_and_last_frames@1.0` | `FirstAndLastFramesToVideoCapability` | `FirstAndLastFramesToVideoProviderPort` |
| `video.generate_from_mixed_media@1.0` | `MixedMediaToVideoCapability` | `MixedMediaToVideoProviderPort` |
| `video.upscale@1.0` | `UpscaleVideoCapability` | `VideoUpscaleProviderPort` |
| `video.extract_frames@1.0` | `ExtractVideoFramesCapability` | `VideoFrameExtractionPort` |
| `video.concatenate@1.0` | `ConcatenateVideosCapability` | `VideoConcatenationPort` |
| `video.analyze_storyboard@1.0` | `AnalyzeVideoStoryboardCapability` | `VideoStoryboardProviderPort` |
| `text.generate_from_text@1.0` | `TextToTextCapability` | `TextToTextProviderPort` |
| `text.generate_from_mixed_media@1.0` | `MultimodalToTextCapability` | `MultimodalToTextProviderPort` |
| `audio.synthesize_speech_from_text@1.0` | `TextToSpeechCapability` | `TextToSpeechProviderPort` |
| `audio.generate_music_from_text@1.0` | `TextToMusicCapability` | `TextToMusicProviderPort` |

The contract IDs remain explicit and stable. Rust names use familiar input-to-output terminology;
verbs are reserved for methods. `TextToAudioCapability` is prohibited because speech and music
have different parameters, results, profiles, and failure semantics.

## Exact External Interfaces

Each model-powered port has one behavior-revealing method and exact request/result types:

| Port | Method | Request | Result |
| --- | --- | --- | --- |
| `TextToImageProviderPort` | `generate_image_from_text` | `TextToImageProviderRequest` | `GeneratedImagePayload` |
| `ImageToImageProviderPort` | `generate_image_from_image` | `ImageToImageProviderRequest` | `GeneratedImagePayload` |
| `ReferenceImagesToImageProviderPort` | `generate_image_from_references` | `ReferenceImagesToImageProviderRequest` | `GeneratedImagePayload` |
| `TextToVideoProviderPort` | `generate_video_from_text` | `TextToVideoProviderRequest` | `GeneratedVideoPayload` |
| `ImageToVideoProviderPort` | `generate_video_from_image` | `ImageToVideoProviderRequest` | `GeneratedVideoPayload` |
| `ReferenceImagesToVideoProviderPort` | `generate_video_from_references` | `ReferenceImagesToVideoProviderRequest` | `GeneratedVideoPayload` |
| `FirstFrameToVideoProviderPort` | `generate_video_from_first_frame` | `FirstFrameToVideoProviderRequest` | `GeneratedVideoPayload` |
| `FirstAndLastFramesToVideoProviderPort` | `generate_video_from_frames` | `FirstAndLastFramesToVideoProviderRequest` | `GeneratedVideoPayload` |
| `MixedMediaToVideoProviderPort` | `generate_video_from_media` | `MixedMediaToVideoProviderRequest` | `GeneratedVideoPayload` |
| `VideoUpscaleProviderPort` | `upscale_video` | `VideoUpscaleProviderRequest` | `UpscaledVideoPayload` |
| `TextToTextProviderPort` | `generate_text` | `TextToTextProviderRequest` | `GeneratedTextValue` |
| `MultimodalToTextProviderPort` | `generate_text_from_media` | `MultimodalToTextProviderRequest` | `GeneratedTextValue` |
| `TextToSpeechProviderPort` | `synthesize_speech` | `TextToSpeechProviderRequest` | `SynthesizedSpeechPayload` |
| `TextToMusicProviderPort` | `generate_music` | `TextToMusicProviderRequest` | `GeneratedMusicPayload` |
| `VideoStoryboardProviderPort` | `analyze_storyboard` | `VideoStoryboardProviderRequest` | `VideoStoryboardResult` |

Media operations are separate interfaces because they do not select a generation profile:

| Port | Method | Request | Result |
| --- | --- | --- | --- |
| `ImageCropPort` | `crop_image` | `ImageCropRequest` | `CroppedImagePayload` |
| `VideoFrameExtractionPort` | `extract_frames` | `VideoFrameExtractionRequest` | `ExtractedVideoFramesPayload` |
| `VideoConcatenationPort` | `concatenate_videos` | `VideoConcatenationRequest` | `ConcatenatedVideoPayload` |

Requests preserve semantic input order, stable input-item IDs, explicit roles, typed parameters,
`GenerationProfileRef` when applicable, and `WorkflowNodeDispatchId`. They contain no provider name,
native model ID, credential, URL, path, provider task, wire DTO, or generic options map.

## Structured Results

Media result payloads contain one fixed media kind, declared facts, and a bounded asynchronous byte
stream. Text and Storyboard results are validated semantic values. Frame extraction produces an
ordered timestamped `WorkflowImageSequenceValue`; storyboard analysis produces a structured
`WorkflowVideoStoryboardValue`, never provider JSON disguised as Text.

`NodeCapabilityProducedMediaWriterPort` stores one payload or an ordered frame set. A capability
publishes outputs only after every required Asset is available. Partial storage never becomes a
partial Workflow result.

## Errors

Exact parameter and readiness errors occur before external calls. Provider ports return the shared
`NodeCapabilityProviderFailure`; media ports return `NodeCapabilityMediaFailure`. Both carry a
stable category, retryability, and optional safe retry time. Provider strings, native IDs, URLs,
paths, credentials, and response bodies never cross the interface.

Every implementation translates these failures once into `NodeCapabilityExecutionError`, which
identifies the capability, execution stage, safe target, and structured cause. A provider route
rejecting a semantically valid request is `ProviderRouteContractViolation`, not invalid user input.

## Test Implementations

Capability tests register real capability implementations with deterministic external routes or
fault-injecting media adapters:

```text
TextToVideoCapability<DeterministicTextToVideoProviderRoute, TestProducedMediaWriter>
TextToSpeechCapability<DeterministicTextToSpeechProviderRoute, TestProducedMediaWriter>
AnalyzeVideoStoryboardCapability<DeterministicVideoStoryboardProviderRoute>
ConcatenateVideosCapability<FaultInjectingVideoConcatenationAdapter>
```

There is no configurable mega-mock. The same parameterized `WorkflowNodeCapabilityPort` contract
suite runs against every registered implementation. Provider port suites separately run against
deterministic and configured routers.

## Verification

- registry tests reject duplicate refs and prove one implementation serves contract discovery,
  normalization, readiness, and execution;
- every capability interface implementation passes the shared lifecycle contract suite;
- exact tests cover typed parameter preparation, input order and roles, request mapping, result
  validation, provenance, and error translation;
- provider and media port suites prove behavioral equivalence across all implementations;
- architecture tests reject a second capability catalog, generic provider interface, optional
  unsupported methods, generic options maps, and concrete adapter selection outside composition.
