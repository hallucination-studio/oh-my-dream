# Backend Project Architecture

> Status: frozen MVP design
> Owner: `crates/projects`
> Scope: Project identity, metadata, selection, and current-Workflow discovery

A Project is the user's durable creative workspace. It is the source of `ProjectId` and the scope
shared by Workflow, Run, Asset, and Assistant state. It is not a folder path, UI tab, provider
account, or second Workflow authority.

## MVP Goal

```text
create or list Projects
  -> open one Project
  -> discover its current Workflow or create it once
  -> edit, run, and preview through the owning modules
  -> reopen the same Project after restart
```

The MVP supports multiple Projects but exactly one editable Workflow per Project. Project owns the
workspace identity and name. Workflow owns the one-current-Workflow invariant and all graph/Run
semantics. Asset and Assistant own their Project-scoped records.

## DDD Structure

```text
crates/projects/src/project/
  domain/       aggregate, identity, name, revision, errors
  application/  create, rename, get, list, open
  interfaces/   repository, Workflow summary reader, clock, IDs
```

`crates/projects` is pure business/application logic. It imports no Tauri, SQL, filesystem,
Workflow aggregate, Asset, Assistant, or provider type. Other business crates may import only the
authoritative `ProjectId`; Project aggregate/application types do not become a shared-kernel API.

## Project Aggregate

```rust
pub struct ProjectAggregate {
    pub id: ProjectId,
    pub name: ProjectName,
    pub revision: ProjectRevision,
    pub created_at: ProjectCreatedAt,
    pub updated_at: ProjectUpdatedAt,
}
```

`ProjectId` is opaque and immutable. `ProjectName` is trimmed, non-empty, length-bounded user text.
Names are not identity and need not be unique. Rename is the only MVP state transition; it advances
`ProjectRevision` and `updated_at`. Create time never changes.

Project does not store a Workflow graph, Asset IDs, Assistant plan, provider configuration, local
path, thumbnail, last-opened UI state, or denormalized counts. Archive, delete, duplicate, template,
and collaboration state are post-MVP.

## Frozen MVP Use Cases

| Use case and method | Responsibility |
| --- | --- |
| `ProjectCreateUseCase::create_project` | create one durable Project with an idempotent request ID |
| `ProjectRenameUseCase::rename_project` | rename one Project with revision compare-and-swap |
| `ProjectGetUseCase::get_project` | load one exact Project |
| `ProjectListUseCase::list_projects` | return one stable bounded Project page |
| `ProjectOpenUseCase::open_project` | return the Project plus its optional current Workflow summary |

Create and rename commands carry `ProjectMutationRequestId`. The repository atomically commits the
aggregate change and `ProjectMutationReceipt`. Reusing the ID with the same canonical command hash
returns the prior result; different content returns `ProjectMutationIdempotencyConflict`.

There is no `ProjectService`, `ProjectManager`, generic workspace repository, or hidden active-
Project singleton.

## Open And Current Workflow

Opening is a read, not a lifecycle transition:

```text
ProjectOpenUseCase::open_project
  -> ProjectRepositoryInterface::load_project
  -> ProjectWorkflowSummaryReaderInterface::read_current_project_workflow_summary
  -> ProjectWorkspaceView
```

`ProjectWorkspaceView` contains the authoritative Project and an optional Project-owned boundary
projection with Workflow ID, revision, and readiness summary. It contains no
Workflow aggregate, nodes, parameters, outputs, Asset rows, or preview URLs.

`DesktopProjectWorkflowBridgeAdapterImpl` implements `ProjectWorkflowSummaryReaderInterface` by calling
`WorkflowGetCurrentUseCase`. It translates Workflow values to `ProjectWorkflowSummary`; Project
never imports Workflow types or reconstructs readiness.

If no Workflow exists, the UI calls `workflow_create` with the opened `ProjectId`. Workflow
atomically enforces at most one current Workflow for that Project. Concurrent creation returns the
already-created Workflow for the same request ID or a structured
`WorkflowAlreadyExistsForProject` outcome; it never creates two editable graphs.

Opening does not persist a global selection. React owns the selected Project tab/window state and
may remember a last-opened ID as presentation preference. Every Project-scoped command still carries
`ProjectId`; the Desktop boundary resolves it through `ProjectGetUseCase` before attaching trusted
scope to the target use case.

## List Contract

`ProjectListQuery` accepts an opaque cursor and limit `1..=100`. Ordering is always
`(updated_at DESC, project_id DESC)`, and the cursor contains both values. Rename may move a Project
to the first page; the MVP promises deterministic keyset pages, not a cross-request database
snapshot. Callers deduplicate by `ProjectId` and refresh after rename.

The MVP has no search, tags, owner filter, recent-project table, or independently persisted UI
ordering.

## Consumer-Owned Interfaces

| Interface | Explicit behavior |
| --- | --- |
| `ProjectRepositoryInterface` | load Projects, list a stable page, and atomically commit create/rename plus receipt |
| `ProjectWorkflowSummaryReaderInterface` | read the optional current Workflow summary without importing Workflow types |
| `ProjectClockInterface` | provide deterministic Project timestamps |
| `ProjectIdentityGeneratorInterface` | create authoritative Project identities |

Production implementations are `SqliteProjectRepositoryAdapterImpl`,
`DesktopProjectWorkflowBridgeAdapterImpl`, `SystemProjectClockAdapterImpl`, and
`UuidProjectIdentityGeneratorAdapterImpl`. Interfaces end in `Interface`; their infrastructure
implementations end in `AdapterImpl`.

## Desktop Boundary

[`BACKEND_APPLICATION.md`](BACKEND_APPLICATION.md#frozen-tauri-surface) owns the command and DTO
surface for Project create, rename, get, list, and open.

Project IDs in DTOs are untrusted references. The Project use case validates existence and returns
the authoritative scope. Workflow, Asset, and Assistant commands receive only the resolved
`ProjectId`; they never infer scope from a Workflow/Asset ID supplied by React.

## Persistence

`SqliteProjectRow` stores Project ID, normalized name, revision, and timestamps.
`SqliteProjectMutationReceiptRow` stores request ID, canonical command hash, Project ID, committed
revision, and result fingerprint. Project rows contain no Workflow or Asset payload.

Project creation or rename is one short SQLite transaction. Opening and listing are bounded reads.
No filesystem, provider, sidecar, credential, or Tauri work occurs inside a Project transaction.

Workflow, Asset, and Assistant records reference `ProjectId` and enforce Project isolation in their
own repositories. Persistence may add foreign keys for corruption defense, but foreign keys do not
replace Project admission or context-owned business checks.

## Errors And Verification

`ProjectDomainError` covers invalid name and revision transition. `ProjectApplicationError` adds
not found, revision conflict, idempotency conflict, bounded-query violation, persistence failure,
and current-Workflow-summary failure. Errors contain typed IDs and safe details, never paths or
unbounded user content.

Verification covers:

- aggregate creation/rename invariants and revision compare-and-swap;
- create/rename receipt replay and mismatched-ID conflict;
- stable list ordering, cursor bounds, and duplicate names;
- open with no Workflow and with one translated current Workflow summary;
- concurrent Workflow creation preserving one current Workflow per Project;
- Project isolation across Workflow, Asset, Assistant, and Desktop command tests;
- architecture tests rejecting Project-owned copies of Workflow/Asset rules and concrete adapters
  outside composition.

## Post-MVP

Archive, restore, delete, duplicate, templates, search, tags, thumbnails, recent ordering, import and
export, multi-window coordination, collaboration, cloud sync, and Project-level retention require
separate product, migration, and recovery decisions.
