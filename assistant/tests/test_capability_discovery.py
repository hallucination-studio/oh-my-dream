from __future__ import annotations

import io
import json
from pathlib import Path
import tempfile
import unittest

from assistant.stdio_protocol import Frame, FrameKind, FrameReader, FrameWriter
from assistant.tests.agent_transport_fixture import (
    SequencedDiscoveryModel,
    decode_frames,
    encode_frames,
)
from assistant.tests.stdio_protocol_fakes import RecordingWriter


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_PATH = REPOSITORY_ROOT / "ui/src/__fixtures__/assistant_operations.json"


def load_operations() -> list[dict[str, object]]:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    operations = fixture["operations"]
    if not isinstance(operations, list):
        raise TypeError("assistant operation fixture must contain an operations list")
    return operations


async def run_agent(
    frames: list[tuple[FrameKind, dict[str, object]]], model: SequencedDiscoveryModel
) -> list[Frame]:
    from assistant.stdio_app import AgentStdioApp

    output = RecordingWriter()
    await AgentStdioApp(
        FrameReader(io.BytesIO(encode_frames(frames))),
        FrameWriter(output),
        model=model,
    ).run_once()
    return decode_frames(output.bytes)


class CapabilityDiscoveryAgentTests(unittest.IsolatedAsyncioTestCase):
    async def test_fake_agent_searches_then_describes_exact_capability(self) -> None:
        operations = load_operations()
        operation_ids = {operation["id"] for operation in operations}
        self.assertIn("capability_search", operation_ids)
        self.assertIn("capability_describe", operation_ids)
        model = SequencedDiscoveryModel()

        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "discovery.sqlite3")
            frames = await run_agent(
                [
                    (
                        FrameKind.INVOKE,
                        {
                            "invocation_id": "invoke-discovery",
                            "session_id": "discovery-session",
                            "session_path": session_path,
                            "input": "Find a video capability and inspect it.",
                            "operations": operations,
                            "state": None,
                        },
                    ),
                    (
                        FrameKind.TOOL_RESPONSE,
                        {
                            "invocation_id": "invoke-discovery",
                            "call_id": "search-call",
                            "output_json": '{"capabilities":[{"reference":{"id":"ImageToVideo","version":"1.0"}}]}',
                        },
                    ),
                    (
                        FrameKind.TOOL_RESPONSE,
                        {
                            "invocation_id": "invoke-discovery",
                            "call_id": "describe-call",
                            "output_json": '{"capabilities":[]}',
                        },
                    ),
                ],
                model,
            )

        self.assertEqual(
            [frame.payload["operation_id"] for frame in frames if frame.kind is FrameKind.TOOL_REQUEST],
            ["capability_search", "capability_describe"],
        )
        self.assertEqual(
            [frame.payload["arguments_json"] for frame in frames if frame.kind is FrameKind.TOOL_REQUEST],
            [
                '{"query":"video","kinds":null}',
                '{"refs":[{"id":"ImageToVideo","version":"1.0"}]}',
            ],
        )
        self.assertIn("capability_search", model.tool_names)
        self.assertIn("capability_describe", model.tool_names)
        self.assertEqual(frames[-1].kind, FrameKind.COMPLETED)
        self.assertEqual(frames[-1].payload["final_output"], "discovery complete")


if __name__ == "__main__":
    unittest.main()
