"""Characterized OpenAI Agents SDK runtime boundary."""

from __future__ import annotations

from pathlib import Path
from collections.abc import Mapping, Sequence
from typing import Any, cast

from agents import (
    Agent,
    ModelSettings,
    RunConfig,
    RunResultStreaming,
    RunState,
    SQLiteSession,
    StreamEvent,
    set_tracing_disabled,
)
from agents.memory import SessionInputCallback
from agents.run_config import CallModelInputFilter


set_tracing_disabled(True)

AGENT_NAME = "workflow_assistant"
SDK_VERSION = "0.18.1"
STATE_ENVELOPE_VERSION = 1
SDK_MAX_TURNS = 64


class StateEnvelopeError(ValueError):
    """State envelope metadata or shape is incompatible with this runtime."""

    def __init__(self, code: str, message: str) -> None:
        self.code = code
        self.message = message
        super().__init__(message)


def build_run_config(
    *,
    session_input_callback: SessionInputCallback | None = None,
    call_model_input_filter: CallModelInputFilter | None = None,
) -> RunConfig:
    """Create the shared SDK run configuration with tracing disabled."""
    return RunConfig(
        tracing_disabled=True,
        session_input_callback=session_input_callback,
        call_model_input_filter=call_model_input_filter,
    )


def build_model_settings() -> ModelSettings:
    """Keep the MVP on one ordered tool/approval path per model turn."""
    return ModelSettings(parallel_tool_calls=False)


def build_file_session(session_id: str, database: str | Path) -> SQLiteSession:
    """Open a durable SDK session backed by the given SQLite file."""
    return SQLiteSession(session_id=session_id, db_path=database)


async def drain_stream(result: RunResultStreaming) -> list[StreamEvent]:
    """Drain a streamed run to its settled result and return observed events."""
    return [event async for event in result.stream_events()]


async def restore_run_state(
    agent: Agent[Any],
    state_json: dict[str, Any],
    *,
    context_override: Any = None,
) -> RunState[Any, Agent[Any]]:
    """Restore SDK-owned run state with strict context handling enabled."""
    return await RunState.from_json(
        initial_agent=agent,
        state_json=state_json,
        context_override=context_override,
        strict_context=True,
    )


def build_state_envelope(
    operations: Sequence[Mapping[str, Any]], state_json: dict[str, Any]
) -> dict[str, Any]:
    """Wrap untouched strict SDK state with exact compatibility metadata."""
    return {
        "envelope_version": STATE_ENVELOPE_VERSION,
        "sdk_version": SDK_VERSION,
        "agent_name": AGENT_NAME,
        "operation_versions": _operation_versions(operations),
        "state_json": state_json,
    }


def validate_state_envelope(
    envelope: Mapping[str, Any], operations: Sequence[Mapping[str, Any]]
) -> dict[str, Any]:
    """Validate exact metadata and return the original opaque state object."""
    expected_fields = {
        "envelope_version",
        "sdk_version",
        "agent_name",
        "operation_versions",
        "state_json",
    }
    if set(envelope) != expected_fields:
        raise StateEnvelopeError(
            "invalid_state", "state envelope fields do not match the contract"
        )
    operation_versions = envelope["operation_versions"]
    metadata_matches = (
        type(envelope["envelope_version"]) is int
        and envelope["envelope_version"] == STATE_ENVELOPE_VERSION
        and isinstance(envelope["sdk_version"], str)
        and envelope["sdk_version"] == SDK_VERSION
        and isinstance(envelope["agent_name"], str)
        and envelope["agent_name"] == AGENT_NAME
        and _valid_operation_versions(operation_versions)
        and operation_versions == _operation_versions(operations)
    )
    if not metadata_matches:
        raise StateEnvelopeError(
            "state_metadata_mismatch", "state metadata does not match this runtime"
        )
    state_json = envelope["state_json"]
    if not isinstance(state_json, dict):
        raise StateEnvelopeError("invalid_state", "state_json must be an object")
    return cast(dict[str, Any], state_json)


def _valid_operation_versions(value: object) -> bool:
    if not isinstance(value, list):
        return False
    return all(
        isinstance(item, dict)
        and set(item) == {"id", "version"}
        and isinstance(item["id"], str)
        and type(item["version"]) is int
        for item in value
    )


def _operation_versions(
    operations: Sequence[Mapping[str, Any]],
) -> list[dict[str, str | int]]:
    versions: list[dict[str, str | int]] = []
    for operation in operations:
        operation_id = operation.get("id")
        version = operation.get("version")
        if not isinstance(operation_id, str):
            raise StateEnvelopeError(
                "invalid_operations", "operation id must be a string"
            )
        if isinstance(version, bool) or not isinstance(version, int):
            raise StateEnvelopeError(
                "invalid_operations", "operation version must be an integer"
            )
        versions.append({"id": operation_id, "version": version})
    return sorted(
        versions,
        key=lambda item: (cast(str, item["id"]), cast(int, item["version"])),
    )
