# Backend Workflow Graph Architecture

> Status: frozen MVP graph architecture
> Owner: Workflow domain in `crates/engine`
> Scope: Workflow aggregate, typed input bindings, ordered references, and graph invariants

Naming follows [`BACKEND_GLOSSARY.md`](BACKEND_GLOSSARY.md). This document owns editable graph
semantics. [`BACKEND_WORKFLOW.md`](BACKEND_WORKFLOW.md) owns readiness, execution planning, Run
lifecycle, and preview association.

## Workflow Aggregate

`ProjectId` is imported from `crates/projects`; Workflow never defines a second Project identity.

```rust
pub struct WorkflowAggregate {
    pub schema_version: WorkflowSchemaVersion,
    pub id: WorkflowId,
    pub project_id: ProjectId,
    pub revision: WorkflowRevision,
    pub nodes: BTreeMap<WorkflowNodeId, WorkflowNodeEntity>,
    pub input_bindings:
        BTreeMap<WorkflowInputTarget, WorkflowInputBinding>,
}

pub struct WorkflowNodeEntity {
    pub id: WorkflowNodeId,
    pub capability_contract: NodeCapabilityContractRef,
    pub parameter_set: NodeCapabilityParameterSet,
    pub canvas_position: WorkflowCanvasPosition,
}

pub struct WorkflowInputTarget {
    pub node_id: WorkflowNodeId,
    pub input_key: NodeCapabilityInputKey,
}

pub enum WorkflowInputBinding {
    Single { item: WorkflowInputItemEntity },
    OrderedReferences { items: NonEmptyVec<WorkflowInputItemEntity> },
}

pub struct WorkflowInputItemEntity {
    pub id: WorkflowInputItemId,
    pub source_node_id: WorkflowNodeId,
    pub source_output_key: NodeCapabilityOutputKey,
    pub input_role_key: Option<NodeCapabilityInputRoleKey>,
}
```

An input item is the first-class directed graph edge. Its target is the
`WorkflowInputTarget` that owns the binding, so the target is not duplicated on every item.
The non-empty sequence in `OrderedReferences.items` is the authoritative order; an absent binding
represents zero items. There is no separately maintained ordinal, rank, or ordering index.

`WorkflowInputItemId` is stable for the item's lifetime. Reordering changes only its vector
position, so a structured prompt continues to reference the same item.

The selected `WorkflowNodeCapabilityInterface` implementation normalizes `NodeCapabilityParameterSet`;
graph code treats its business fields as opaque structured data. The implementation mechanically exposes referenced
`WorkflowInputItemId` values so Workflow can enforce referential integrity without duplicating
prompt semantics. Canvas position is persisted for reopen but excluded from readiness and
execution.

Nodes never persist input/output definitions, resolved runtime values, outputs, progress, errors, provider
tasks, URLs, paths, previews, or playback state. UI interaction state remains in React.

## Persistence And Restore

Persistence Rows and API DTOs mechanically encode the `Single`/`OrderedReferences` union. Every
ordered item stores its stable ID, exact source output, and optional role key in array order. A
repository round-trips that array verbatim; it never reconstructs order from IDs, timestamps,
canvas position, roles, or database row order.

The role-bearing input shape belongs to an explicit `WorkflowSchemaVersion`. A reader never
interprets an anonymous source list as this model. Restore reconstructs the aggregate and applies
the same draft-validity rules before execution.

Starting a Run copies frozen binding sequences into the execution plan. A retry creates another Run
with another frozen plan. Later graph
edits, presentation sorting, provider translation, and asynchronous source resolution cannot
reorder them.

## Input Contract Model

```rust
pub enum WorkflowDataType {
    Text,
    Image,
    Video,
    Audio,
}

pub struct WorkflowAcceptedDataTypeSet(BTreeSet<WorkflowDataType>);

pub struct NodeCapabilityInputContract {
    pub key: NodeCapabilityInputKey,
    pub binding: NodeCapabilityInputBindingContract,
}

pub enum NodeCapabilityInputBindingContract {
    OptionalSingleValue { data_type: WorkflowDataType },
    RequiredSingleValue { data_type: WorkflowDataType },
    OrderedReferences {
        minimum_items: u32,
        maximum_items: Option<u32>,
        accepted_data_types_by_role:
            BTreeMap<NodeCapabilityInputRoleKey, WorkflowAcceptedDataTypeSet>,
    },
}
```

`OptionalSingleValue` and `RequiredSingleValue` accept one exact `WorkflowDataType`.
`OrderedReferences`
requires a declared role key on every item and a non-empty concrete accepted-type set for every
role. Mixed media is therefore a sequence of individually tagged Image, Video, or Audio values,
not a `Media` wildcard.

Cardinality belongs to the exact capability. A capability needing at least nine references declares
that bound; nine is not a Workflow-wide limit. Transport size limits are boundary protection, not
business cardinality.

`NodeCapabilityInputRoleKey` meaning is owned by the exact capability module. Examples include
`subject`, `style`, `composition`, `scene`, `motion`, and `audio_guidance`. Workflow stores and
validates declared keys mechanically but never interprets them. Semantic positions use separately
named single inputs such as `first_frame` and `last_frame`, not inferred list roles.

MVP outputs publish one exact Text, Image, Video, or Audio `WorkflowDataType`. Accepting multiple concrete input types never permits
an ambiguous output. The catalog rejects empty role maps, empty accepted-type sets, `Text` inside a
reference-media set, and inverted cardinality bounds as contract-definition errors.

`ImageSequence` and `VideoStoryboard` are reserved roadmap output concepts. They enter
`WorkflowDataType` only with the frame-extraction or storyboard capability change and a compatible
Workflow schema version; the frozen MVP does not serialize placeholder variants.

## Input Binding Invariants

- every binding names one existing target node and one declared input;
- `Single` is valid only for an optional or required single-value contract;
- `OrderedReferences` uses its declared contract and vector order is semantic;
- every input item ID is Workflow-unique and names one exact source output;
- each source output type satisfies the target value rule or selected role's accepted-type set;
- ordered-reference items require one declared role key and single-value items reject roles;
- an output may fan out to several target bindings;
- duplicate endpoints, self-edges, missing endpoints, and cycles are rejected;
- removing a node removes its outgoing items and target bindings atomically;
- incoming and outgoing graph indexes are derived, never persisted.

Binding an occupied single input never silently replaces it. One mutation explicitly removes the
old item and adds the new item. Ordered input changes address stable item IDs, never array positions
as identity.

## Verification

- aggregate tests cover single and ordered bindings, stable IDs, reorder, fan-out, removal, and
  cycles;
- contract tests cover the four MVP data types, capability-owned roles, cardinality, and type rejection;
- persistence tests round-trip at least nine mixed-media items without changing IDs or order;
- prompt-reference tests prove item identity survives reorder;
- architecture tests reject a second graph model in nodes, DTOs, persistence, or React.
