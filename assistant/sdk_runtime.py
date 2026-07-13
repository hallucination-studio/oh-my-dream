"""Characterized OpenAI Agents SDK runtime boundary."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from agents import (
    Agent,
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
