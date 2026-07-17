pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS generation_tasks (
    id BLOB PRIMARY KEY NOT NULL CHECK(length(id) = 16),
    project_id BLOB NOT NULL CHECK(length(project_id) = 16),
    workflow_id BLOB NOT NULL CHECK(length(workflow_id) = 16),
    workflow_run_id BLOB NOT NULL CHECK(length(workflow_run_id) = 16),
    workflow_node_id BLOB NOT NULL CHECK(length(workflow_node_id) = 16),
    workflow_node_execution_id BLOB NOT NULL CHECK(length(workflow_node_execution_id) = 16),
    idempotency_key BLOB NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 256),
    request_hash BLOB NOT NULL CHECK(length(request_hash) = 32),
    request_schema_version INTEGER NOT NULL CHECK(request_schema_version = 1),
    request_kind TEXT NOT NULL CHECK(request_kind IN ('Text', 'Image', 'Video', 'Voice')),
    request_json TEXT NOT NULL CHECK(length(request_json) BETWEEN 2 AND 262144),
    generation_profile_ref TEXT NOT NULL CHECK(length(generation_profile_ref) BETWEEN 3 AND 160),
    provider_id TEXT NOT NULL CHECK(length(provider_id) BETWEEN 1 AND 128),
    route_id TEXT NOT NULL CHECK(length(route_id) BETWEEN 1 AND 128),
    status TEXT NOT NULL CHECK(status IN (
        'Queued', 'Submitting', 'Running', 'CancelRequested',
        'Succeeded', 'Failed', 'Cancelled'
    )),
    progress_percent INTEGER NULL CHECK(progress_percent BETWEEN 0 AND 100),
    remote_task_id TEXT NULL CHECK(length(remote_task_id) BETWEEN 1 AND 512),
    result_kind TEXT NULL CHECK(result_kind IN ('Text', 'Asset')),
    result_text TEXT NULL CHECK(length(result_text) BETWEEN 1 AND 65536),
    result_asset_id BLOB NULL CHECK(length(result_asset_id) = 16),
    result_media_kind TEXT NULL CHECK(result_media_kind IN ('Image', 'Video', 'Audio')),
    failure_kind TEXT NULL,
    failure_code TEXT NULL CHECK(length(failure_code) BETWEEN 1 AND 64),
    failure_message TEXT NULL CHECK(length(failure_message) BETWEEN 1 AND 512),
    provider_deadline_at INTEGER NOT NULL CHECK(provider_deadline_at >= 0),
    completed_at INTEGER NULL CHECK(completed_at >= 0),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at),
    revision INTEGER NOT NULL CHECK(revision > 0),
    UNIQUE(project_id, idempotency_key),
    UNIQUE(project_id, workflow_node_execution_id),
    CHECK(progress_percent IS NULL OR status = 'Running'),
    CHECK(
        (status = 'Running' AND remote_task_id IS NOT NULL)
        OR (status = 'CancelRequested')
        OR (status NOT IN ('Running', 'CancelRequested') AND remote_task_id IS NULL)
    ),
    CHECK((status = 'Succeeded') = (result_kind IS NOT NULL)),
    CHECK(
        (result_kind IS NULL AND result_text IS NULL AND result_asset_id IS NULL
            AND result_media_kind IS NULL)
        OR (result_kind = 'Text' AND result_text IS NOT NULL AND result_asset_id IS NULL
            AND result_media_kind IS NULL)
        OR (result_kind = 'Asset' AND result_text IS NULL AND result_asset_id IS NOT NULL
            AND result_media_kind IS NOT NULL)
    ),
    CHECK((status = 'Failed') = (failure_kind IS NOT NULL)),
    CHECK(
        (failure_kind IS NULL AND failure_code IS NULL AND failure_message IS NULL)
        OR (failure_kind IS NOT NULL AND failure_code IS NOT NULL AND failure_message IS NOT NULL)
    ),
    CHECK((status IN ('Succeeded', 'Failed', 'Cancelled')) = (completed_at IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS generation_tasks_project_page
    ON generation_tasks(project_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS generation_tasks_workflow_origin
    ON generation_tasks(workflow_run_id, workflow_node_execution_id);

CREATE TABLE IF NOT EXISTS generation_task_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id BLOB NOT NULL REFERENCES generation_tasks(id),
    kind TEXT NOT NULL CHECK(kind IN (
        'SubmitTask', 'PollTask', 'CancelRemoteTask', 'NotifyWorkflow'
    )),
    payload_json TEXT NOT NULL CHECK(payload_json = '{}'),
    deduplication_key TEXT NOT NULL UNIQUE CHECK(length(deduplication_key) BETWEEN 1 AND 512),
    available_at INTEGER NOT NULL CHECK(available_at >= 0),
    state TEXT NOT NULL CHECK(state IN ('Ready', 'Claimed', 'Completed')),
    delivery_attempts INTEGER NOT NULL CHECK(delivery_attempts BETWEEN 0 AND 4294967295),
    processed_at INTEGER NULL CHECK(processed_at >= 0),
    last_error TEXT NULL CHECK(length(last_error) BETWEEN 1 AND 512),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    CHECK((state = 'Completed') = (processed_at IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS generation_task_outbox_due
    ON generation_task_outbox(processed_at, available_at, id);
";
