"""Frozen-only deterministic smoke entrypoint; never used by production."""

from __future__ import annotations

import asyncio
import sys

from agents.tracing import get_trace_provider

from assistant.stdio_app import AgentStdioApp
from assistant.stdio_protocol import FrameReader, FrameWriter
from assistant.tests.agent_transport_fixture import ToolThenMessageModel


def main() -> None:
    provider = get_trace_provider()
    if not provider._disabled:  # type: ignore[attr-defined]
        raise RuntimeError("assistant tracing must be disabled in the frozen sidecar")
    print("ASSISTANT_SMOKE_TRACING_DISABLED=1", file=sys.stderr)
    asyncio.run(
        AgentStdioApp(
            FrameReader(sys.stdin.buffer),
            FrameWriter(sys.stdout.buffer),
            model=ToolThenMessageModel(
                "workspace_get_snapshot", '{  "query" : "current" }'
            ),
        ).run_once()
    )


if __name__ == "__main__":
    main()
