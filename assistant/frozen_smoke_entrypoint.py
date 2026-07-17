"""Frozen-only deterministic smoke entrypoint; never used by production."""

from __future__ import annotations

import asyncio
import sys

from agents.tracing import get_trace_provider

from assistant.protocol_v1_app import ProtocolV1App
from assistant.tests.agent_transport_fixture import ToolThenMessageModel


def main() -> None:
    provider = get_trace_provider()
    if not provider._disabled:  # type: ignore[attr-defined]
        raise RuntimeError("assistant tracing must be disabled in the frozen sidecar")
    print("ASSISTANT_SMOKE_TRACING_DISABLED=1", file=sys.stderr)
    asyncio.run(
        ProtocolV1App(
            sys.stdin.buffer,
            sys.stdout.buffer,
            model=ToolThenMessageModel(
                "assistant.workspace.get_snapshot@1",
                "{}",
            ),
        ).run_once()
    )


if __name__ == "__main__":
    main()
