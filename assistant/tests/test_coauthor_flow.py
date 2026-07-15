from __future__ import annotations

import io
import json
from pathlib import Path
import tempfile
import unittest

from assistant.stdio_protocol import Frame, FrameKind, FrameReader, FrameWriter
from assistant.tests.agent_transport_fixture import decode_frames, encode_frames
from assistant.tests.coauthor_fixture import COAUTHOR_CALLS, CoauthorModel, SHOT_PROMPTS
from assistant.tests.stdio_protocol_fakes import RecordingWriter


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
OPERATIONS_FIXTURE = REPOSITORY_ROOT / "ui/src/__fixtures__/assistant_operations.json"
COAUTHOR_OPERATION_IDS = {
    "workspace_get_snapshot",
    "capability_search",
    "capability_describe",
    "workflow_apply_patch",
}


def load_operations() -> list[dict[str, object]]:
    fixture = json.loads(OPERATIONS_FIXTURE.read_text(encoding="utf-8"))
    return [operation for operation in fixture["operations"] if operation["id"] in COAUTHOR_OPERATION_IDS]


async def run_agent(
    frames: list[tuple[FrameKind, dict[str, object]]], model: CoauthorModel
) -> list[Frame]:
    from assistant.stdio_app import AgentStdioApp

    output = RecordingWriter()
    await AgentStdioApp(
        FrameReader(io.BytesIO(encode_frames(frames))),
        FrameWriter(output),
        model=model,
    ).run_once()
    return decode_frames(output.bytes)


class CoauthorFlowTests(unittest.IsolatedAsyncioTestCase):
    async def test_fake_agent_discovers_then_applies_one_atomic_three_shot_patch(self) -> None:
        operations = load_operations()
        model = CoauthorModel()
        responses = {
            call_id: '{"ok":true}' for _operation_id, _arguments, call_id in COAUTHOR_CALLS
        }

        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "coauthor.sqlite3")
            frames = await run_agent(
                [
                    (
                        FrameKind.INVOKE,
                        {
                            "invocation_id": "invoke-coauthor",
                            "session_id": "project:project-1",
                            "session_path": session_path,
                            "input": "Create a 12-second, three-shot video.",
                            "operations": operations,
                            "state": None,
                        },
                    ),
                    *[
                        (
                            FrameKind.TOOL_RESPONSE,
                            {
                                "invocation_id": "invoke-coauthor",
                                "call_id": call_id,
                                "output_json": responses[call_id],
                            },
                        )
                        for _operation_id, _arguments, call_id in COAUTHOR_CALLS
                    ],
                ],
                model,
            )

        tool_requests = [frame for frame in frames if frame.kind is FrameKind.TOOL_REQUEST]
        self.assertEqual(
            [frame.payload["operation_id"] for frame in tool_requests],
            [operation_id for operation_id, _arguments, _call_id in COAUTHOR_CALLS],
        )
        self.assertEqual(
            [frame.payload["arguments_json"] for frame in tool_requests],
            [arguments for _operation_id, arguments, _call_id in COAUTHOR_CALLS],
        )
        self.assertEqual(set(model.tool_names), COAUTHOR_OPERATION_IDS)
        self.assertEqual(frames[-1].kind, FrameKind.COMPLETED)
        self.assertEqual(frames[-1].payload["final_output"], "Workflow created.")

        prompt = model.system_instructions[0]
        self.assertIsNotNone(prompt)
        assert prompt is not None
        self.assertIn("workspace_get_snapshot", prompt)
        self.assertIn("capability_search", prompt)
        self.assertIn("ordered_many", prompt)
        self.assertIn("string, image, video, audio, model, int, float", prompt)
        self.assertNotIn("TextPrompt", prompt)
        self.assertNotIn("params_schema", prompt)
        self.assertNotIn("ReAct", prompt)

        patch = json.loads(tool_requests[-1].payload["arguments_json"])
        self.assertIsNone(patch["expected_revision"])
        self.assertEqual(len(patch["operations"]), 17)
        self.assertEqual(
            sum(
                operation["params"].get("duration", 0)
                for operation in patch["operations"]
                if operation["op"] == "add_node"
                and operation["capability"]["id"] == "ImageToVideo"
            ),
            12,
        )
        concat_input = patch["operations"][-1]
        self.assertEqual(
            [source["node"]["alias"] for source in concat_input["binding"]["sources"]],
            ["video-1", "video-2", "video-3"],
        )
        self.assertEqual(
            [operation["params"]["text"] for operation in patch["operations"] if operation["op"] == "add_node" and operation["capability"]["id"] == "TextPrompt"],
            list(SHOT_PROMPTS),
        )


if __name__ == "__main__":
    unittest.main()
