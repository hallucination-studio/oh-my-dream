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

`ProjectId` is opaque and immutable. `ProjectName` is bounded normalized user text. Names are not
identity and need not be unique. Rename is the only MVP state transition; it advances
`ProjectRevision` and `updated_at`. Create time never changes.

Project does not store a Workflow graph, Asset IDs, Assistant plan, provider configuration, local
path, thumbnail, last-opened UI state, or denormalized counts. Archive, delete, duplicate, template,
and collaboration state are post-MVP.

## Frozen Project Value Contracts

Project value semantics are closed for the MVP:

| Value | Frozen contract |
| --- | --- |
| `ProjectId` | distinct newtype containing one valid RFC 9562 UUIDv4; generated only through `ProjectIdentityGeneratorInterface` |
| `ProjectMutationRequestId` | separate UUIDv4 newtype supplied for create or rename idempotency; never interchangeable with `ProjectId` |
| `ProjectName` | trim leading/trailing Unicode whitespace; retain case and interior text; reject empty, control characters, or more than 120 Unicode scalar values |
| `ProjectRevision` | non-zero `u64`; creation is revision `1`; each successful rename adds exactly one; overflow is rejected |
| `ProjectCreatedAt` | non-negative signed `i64` UTC milliseconds since the Unix epoch |
| `ProjectUpdatedAt` | same representation; creation equals `created_at`; rename uses `max(observed_now, previous_updated_at + 1)` |

Names receive no case folding, Unicode normalization, interior-whitespace collapsing, or uniqueness
check. Renaming to the current normalized name returns `ProjectNameUnchanged`; it writes no Project
row or receipt. The aggregate owns revision and timestamp monotonicity.

UUID text, timestamp text, cursor text, and JSON field encoding are Desktop representation choices
owned by `BACKEND_APPLICATION.md`; they are not Project domain semantics. Persistence must preserve
the exact UUID and timestamp values; `BACKEND_STORAGE.md` owns their row representation.

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

`ProjectMutationCommandHash` is the 32-byte SHA-256 digest of one length-delimited binary encoding.
It excludes the request ID and uses these exact inputs:

```text
create: "oh-my-dream/project-create/v1" + normalized ProjectName UTF-8 bytes
rename: "oh-my-dream/project-rename/v1" + ProjectId bytes
        + base ProjectRevision as big-endian u64 + normalized ProjectName UTF-8 bytes
```

Every variable-length field is prefixed by its big-endian `u32` byte length. The operation-specific
domain string is encoded by the same rule, so concatenated fields are never ambiguous.

`ProjectMutationReceipt` stores the request ID, command hash, operation, and exact committed
`ProjectMutationOutcome`: Project ID, normalized name, revision, creation time, and update time. Its
`ProjectMutationResultFingerprint` is SHA-256 over the length-delimited
`"oh-my-dream/project-result/v1"` domain plus those outcome fields in that order. The domain and name
use big-endian `u32` byte lengths followed by UTF-8 bytes; Project ID uses its 16 UUID bytes;
revision uses big-endian `u64`; and both timestamps use big-endian `i64`. A matching replay returns
this stored outcome even if the current Project has since been renamed. Restore rejects a receipt
whose fingerprint does not match its stored outcome.

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

`ProjectWorkspaceView` contains the authoritative Project and
`Option<ProjectWorkflowSummary>`. The Project-owned summary has exactly:

```rust
pub struct ProjectWorkflowSummary {
    pub workflow_id: ProjectWorkflowIdBoundaryValue,
    pub workflow_revision: ProjectWorkflowRevisionBoundaryValue,
    pub readiness: ProjectWorkflowReadinessSummary,
}

pub enum ProjectWorkflowReadinessSummary {
    Ready,
    Blocked,
}
```

The Desktop bridge constructs the opaque, non-empty, at-most-128-byte Workflow ID boundary value
and non-zero `u64` revision boundary value from authoritative Workflow values. `Ready` means the
Workflow readiness issue set is empty; every non-empty issue set maps to `Blocked`. Project never
copies issue categories, counts, severity, or readiness rules. The view contains no Workflow
aggregate, nodes, parameters, outputs, Asset rows, or preview URLs.

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

`ProjectListQuery` requires `ProjectListLimit` in `1..=100` and accepts an optional
`ProjectListCursor { updated_at, project_id }`. Ordering is always
`(updated_at DESC, project_id DESC)`. A cursor selects rows whose tuple is strictly less than the
cursor tuple. The repository reads at most `limit + 1`, returns at most `limit` Projects, and emits
`next_cursor` from the last returned Project only when another row exists.

The cursor is an application value; D0.6 owns its opaque Desktop encoding. Rename may move a Project
to the first page. The MVP promises deterministic keyset pages, not a cross-request database
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
`SqliteProjectMutationReceiptRow` stores request ID, command hash, operation, exact committed outcome,
and result fingerprint. Project rows contain no Workflow or Asset payload.

Project creation or rename is one short SQLite transaction. Opening and listing are bounded reads.
No filesystem, provider, sidecar, credential, or Tauri work occurs inside a Project transaction.

Workflow, Asset, and Assistant records reference `ProjectId` and enforce Project isolation in their
own repositories. Persistence may add foreign keys for corruption defense, but foreign keys do not
replace Project admission or context-owned business checks.

## Errors And Verification

`ProjectDomainError` is closed to `ProjectNameEmpty`, `ProjectNameTooLong`,
`ProjectNameContainsControl`, `ProjectNameUnchanged`, `ProjectRevisionOverflow`,
`ProjectTimestampOutOfRange`, and `ProjectTimestampOverflow`. `ProjectApplicationError` adds
`ProjectNotFound`, `ProjectRevisionConflict`, `ProjectMutationIdempotencyConflict`,
`ProjectListLimitOutOfBounds`, `ProjectPersistenceFailure`, and
`ProjectWorkflowSummaryReadFailure`. Errors contain typed IDs, revisions, and safe bounds, never
paths, full names, or other unbounded user text.

Verification covers:

- aggregate creation/rename invariants and revision compare-and-swap;
- create/rename receipt replay and mismatched-ID conflict;
- receipt replay after later rename and corrupt-fingerprint rejection;
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
