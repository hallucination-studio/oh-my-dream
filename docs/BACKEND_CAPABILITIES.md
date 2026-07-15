# Backend MVP Node Capabilities

> Status: proposed MVP design
> Owner: `crates/nodes`
> Scope: seven exact operations behind Text, Image, Video, and Audio node shells

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). Node capabilities are an executable
sub-capability of the Workflow bounded context, not provider or UI models.

## Responsibility

Each module owns one exact operation: identity, versioned contract, typed parameters, readiness,
request mapping, result validation, and execution. The Workflow context owns graph and Run
semantics; Asset owns managed media; provider adapters implement capability-owned generation ports.

React displays four shells, but no broad node union mixes source selection, generation, provider,
Run, and preview state. A persisted `WorkflowNodeEntity` selects one exact
`NodeCapabilityContractRef`.

## DDD Layers

```text
crates/nodes/src/node_capability/
  domain/       seven contract instances, typed parameters, result rules, errors
  application/  built-in catalog and exact capability executors
  ports/        managed media and three generation provider traits
```

`crates/engine` defines the shared `NodeCapabilityContract` shape and the two ports it consumes.
`crates/nodes` depends on those Workflow contracts, owns the seven exact contract instances and their
parameter semantics, and implements `WorkflowNodeCapabilityCatalogPort` and
`NodeCapabilityExecutorPort`. This direction avoids a crate cycle and avoids a duplicate graph-side
capability model. Asset and provider dependencies enter exact executors through constructor-injected
ports.

## Versioned Contract

```rust
pub struct NodeCapabilityContractRef {
    pub id: NodeCapabilityId,
    pub version: NodeCapabilityVersion,
}

pub struct NodeCapabilityContract {
    pub reference: NodeCapabilityContractRef,
    pub parameters: Vec<NodeCapabilityParameterContract>,
    pub inputs: Vec<NodeCapabilityInputPortContract>,
    pub outputs: Vec<NodeCapabilityOutputPortContract>,
    pub effect: NodeCapabilityEffectKind,
}
```

An exact version is immutable. Parameter meaning, port keys, data types, required inputs, effect,
and output meaning require a new version when changed. The MVP has no automatic upgrade or plugin
loading.

The built-in catalog is the single source of the seven exact contract instances used for graph
validation, parameter forms, readiness, execution, connection hints, and catalog DTOs.
`WorkflowNodeShellKindDto` is mechanically derived from the primary output's `WorkflowDataType`; it
is not part of this domain contract.

## Parameter Contract

`NodeCapabilityParameterContract` is a closed declarative schema for one field. Unknown fields are rejected.
Draft normalization validates present values and inserts deterministic defaults while allowing a
required field to be absent. Execution preparation requires all necessary values and returns a
private typed parameter value for that exact capability.

The persisted `NodeCapabilityParameterSet` contains no credentials, paths, preview URLs, provider
names, provider task IDs, progress, errors, or outputs. Startup configuration selects one provider
and model for each generation capability.

React receives `NodeCapabilityParameterContractDto` values for form rendering. Business validation
remains in the capability module.

## Port And Runtime Types

Capabilities use the engine-owned `WorkflowDataType` and `WorkflowRuntimeValue` definitions in
[`BACKEND_WORKFLOW.md`](BACKEND_WORKFLOW.md#runtime-values). Each MVP input is optional-single or
required-single; every output publishes one value. Types match exactly. Managed references contain
stable Asset identity and a content fingerprint, never bytes, paths, provider URLs, or presentation
URLs.

## Exact MVP Catalog

### `text.literal@1.0`

```text
Parameters: text: bounded string
Inputs:     none
Outputs:    text: Text
Effect:     Pure
```

This is the editable prompt and speech-text source. Empty text is valid in a draft but not Run-ready.

### `image.asset@1.0`

```text
Parameters: asset_id: Image Asset ID
Inputs:     none
Outputs:    image: Image
Effect:     Managed read
```

Execution resolves an available Image Asset in the current Project. It does not copy bytes or put a
URL into the Workflow.

### `image.text_to_image@1.0`

```text
Parameters: aspect_ratio, optional seed
Inputs:     prompt: Text
Outputs:    image: Image
Effect:     External generation
```

The output becomes an available Image Asset before the executor publishes `image`.

### `video.asset@1.0`

```text
Parameters: asset_id: Video Asset ID
Inputs:     none
Outputs:    video: Video
Effect:     Managed read
```

### `video.image_to_video@1.0`

```text
Parameters: bounded duration, aspect_ratio
Inputs:     image: Image, prompt: Text optional
Outputs:    video: Video
Effect:     External generation
```

The result must be one complete playable video accepted by Asset validation. Embedded audio remains
part of that video; this capability does not publish a separate Audio output.

### `audio.asset@1.0`

```text
Parameters: asset_id: Audio Asset ID
Inputs:     none
Outputs:    audio: Audio
Effect:     Managed read
```

### `audio.text_to_audio@1.0`

```text
Parameters: voice_profile, speed
Inputs:     text: Text
Outputs:    audio: Audio
Effect:     External generation
```

The result is one standalone playable Audio Asset. Video/audio muxing is outside the MVP.

## Execution Contract

`NodeCapabilityExecutorPort` is owned by the Workflow application and implemented by the built-in
capability runtime:

```text
execute(
  capability_contract_ref,
  parameter_set,
  workflow_node_input_set,
  workflow_node_execution_context
) -> WorkflowNodeOutputSet | NodeCapabilityExecutionError
```

`WorkflowNodeExecutionContext` contains only call-scoped Project, Workflow revision, Run, node
execution, dispatch identity, deadline, cancellation, and progress sink. Long-lived media and
provider ports are constructor dependencies of the exact executor.

Every executor must:

- accept only declared named inputs with exact types;
- prepare a private typed parameter value before side effects;
- observe cancellation before dispatch and before publishing outputs;
- report monotonic progress in `[0.0, 1.0]`;
- use the dispatch identity for one-Run idempotency;
- validate the operation result;
- persist generated media before returning a managed reference;
- return every declared output or no outputs;
- return structured errors instead of provider message text.

`text.literal` is synchronous and pure. Managed reads and generation are asynchronous.

## Asset Ports

Asset source and media-consuming executors use `NodeCapabilityManagedMediaReaderPort`:

```text
resolve_asset_reference(project_id, NodeCapabilityAssetRefValue, WorkflowDataType)
  -> Workflow managed-media reference

open_managed_media(project_id, managed_media_reference, WorkflowDataType)
  -> NodeCapabilityReadableMediaInput
```

Generation executors consume `NodeCapabilityGeneratedMediaWriterPort`:

```text
store_generated_media(
  project_id,
  WorkflowGeneratedMediaOriginValue,
  NodeCapabilityGeneratedMediaPayload
)
  -> Workflow managed-media reference
```

`src-tauri` implements both ports over Asset use cases. Exact capability modules receive neither an
Asset repository nor a filesystem path. Asset source parameters expose
`NodeCapabilityAssetRefValue` for readiness validation; the Desktop bridge translates it to
`AssetId`. Validation never scans arbitrary JSON or URLs.

## Provider Ports

Only three consumer-owned external generation ports exist:

```text
TextToImageProviderPort
ImageToVideoProviderPort
TextToAudioProviderPort
```

Each uses a capability-owned semantic request and returns `NodeCapabilityGeneratedMediaPayload`. There is no
broad provider interface, optional unsupported operation, capability probe, or provider-specific
parameter map.

## Generated Media Payload

```rust
pub struct NodeCapabilityGeneratedMediaPayload {
    pub media_kind: NodeCapabilityGeneratedMediaKind,
    pub declared_mime_type: NodeCapabilityGeneratedMediaMimeTypeValue,
    pub declared_byte_length: Option<u64>,
    pub stream: NodeCapabilityGeneratedMediaStream,
}
```

`NodeCapabilityGeneratedMediaStream` is a bounded asynchronous stream, never a provider URL or local path.
`NodeCapabilityGeneratedMediaWriterPort` revalidates kind, MIME, size, and media facts before returning a
managed reference. These are node-owned pre-storage types; the Desktop bridge translates them to
Asset application input. Provider DTOs never leave their adapter.

## Constructor Injection

Exact executors declare only the ports they consume:

```rust
pub struct TextToImageNodeCapabilityExecutor<P, W> {
    text_to_image_provider: P,
    generated_media_writer: W,
}
```

`P: TextToImageProviderPort` and `W: NodeCapabilityGeneratedMediaWriterPort` are supplied by the
composition root. Asset source executors receive only `NodeCapabilityManagedMediaReaderPort`; the
literal executor has no external dependency. No executor looks up an adapter by name at runtime.

## Errors

`NodeCapabilityExecutionError` has stable categories for invalid parameters, missing input,
unavailable Asset, authentication, rate limit, provider unavailability, invalid provider output,
storage failure, timeout, cancellation, and operation failure. Retryability and an optional safe
retry time are structured fields. Secrets, raw provider bodies, paths, and signed URLs are excluded.

## Verification

- catalog tests reject duplicate refs and invalid fixed ports;
- each capability tests draft normalization and execution preparation;
- executor tests cover exact inputs, output completeness, progress, cancellation, and errors;
- Asset port tests prove Project scope, media kind, fingerprint, and provenance;
- provider port contract tests run against deterministic and configured adapters;
- boundary tests prove forms, shells, and ports are derived from the catalog.

## Post-MVP

Text generation, reference generation, multiview, text-to-video, concat, multiple inputs, batch
outputs, per-node provider selection, and dynamic capabilities are not part of the first version.
3D and scene capabilities are not product scope.
