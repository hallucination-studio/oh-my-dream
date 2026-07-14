from __future__ import annotations

import io
import inspect
from pathlib import Path
import tempfile
from typing import Any, cast
import unittest

from assistant.stdio_protocol import Frame, FrameKind, FrameReader, FrameWriter, JsonValue
from assistant.tests.agent_transport_cases import FailClosedTests, FixtureProcessTests
from assistant.tests.agent_transport_fixture import (
    APPROVAL_ARGUMENTS,
    FinalMessageModel,
    ToolThenMessageModel,
    decode_frames,
    encode_frames,
    operation,
    pause_approval,
)
from assistant.tests.stdio_protocol_fakes import RecordingWriter


def invoke_payload(
    invocation_id: str,
    session_id: str,
    session_path: str,
    input_value: str | None,
    operations: list[dict[str, object]],
    state: object = None,
) -> dict[str, object]:
    return {
        "invocation_id": invocation_id,
        "session_id": session_id,
        "session_path": session_path,
        "input": input_value,
        "operations": operations,
        "state": state,
    }


async def run_agent(
    frames: list[tuple[FrameKind, dict[str, object]]], model: Any
) -> list[Frame]:
    from assistant.stdio_app import AgentStdioApp

    output = RecordingWriter()
    await AgentStdioApp(
        FrameReader(io.BytesIO(encode_frames(frames))),
        FrameWriter(output),
        model=model,
    ).run_once()
    return decode_frames(output.bytes)


def assert_paused_approval(
    test: unittest.TestCase, frames: list[Frame], arguments_json: str
) -> dict[str, JsonValue]:
    test.assertEqual(
        [frame.kind for frame in frames],
        [FrameKind.RESPONSES_EVENT, FrameKind.APPROVAL_REQUEST, FrameKind.SNAPSHOT],
    )
    approval_payload = frames[1].payload
    state = approval_payload["state"]
    test.assertIsInstance(state, dict)
    state_object = cast(dict[str, JsonValue], state)
    test.assertEqual(
        state_object,
        {
            "envelope_version": 1,
            "sdk_version": "0.18.1",
            "agent_name": "workflow_assistant",
            "operation_versions": [{"id": "proposal_execute", "version": 3}],
            "state_json": state_object["state_json"],
        },
    )
    test.assertIsInstance(state_object["state_json"], dict)
    test.assertEqual(
        approval_payload,
        {
            "invocation_id": "invoke-pause",
            "operation_id": "proposal_execute",
            "call_id": "call-1",
            "arguments_json": arguments_json,
            "state": state_object,
        },
    )
    test.assertEqual(
        frames[2].payload,
        {
            "invocation_id": "invoke-pause",
            "session_id": "approval-session",
            "status": "waiting_approval",
            "state": state_object,
        },
    )
    return state_object


async def pause_agent(session_path: str, operation_contract: dict[str, object]) -> list[Frame]:
    return await run_agent(
        [
            (
                FrameKind.INVOKE,
                invoke_payload(
                    "invoke-pause",
                    "approval-session",
                    session_path,
                    "Execute the proposal.",
                    [operation_contract],
                ),
            )
        ],
        ToolThenMessageModel("proposal_execute", APPROVAL_ARGUMENTS),
    )


async def resume_agent(
    session_path: str,
    operation_contract: dict[str, object],
    state: dict[str, JsonValue],
) -> list[Frame]:
    return await run_agent(
        [
            (
                FrameKind.INVOKE,
                invoke_payload(
                    "invoke-resume",
                    "approval-session",
                    session_path,
                    None,
                    [operation_contract],
                    state,
                ),
            ),
            (
                FrameKind.APPROVAL_RESPONSE,
                {
                    "invocation_id": "invoke-resume",
                    "call_id": "call-1",
                    "approved": True,
                },
            ),
            (
                FrameKind.TOOL_RESPONSE,
                {
                    "invocation_id": "invoke-resume",
                    "call_id": "call-1",
                    "output_json": '{"result":"started"}',
                },
            ),
        ],
        ToolThenMessageModel("proposal_execute", APPROVAL_ARGUMENTS),
    )


class AgentTransportTests(unittest.IsolatedAsyncioTestCase):
    async def test_production_stdio_entrypoint_has_no_fixture_mode(self) -> None:
        from assistant.stdio_app import run

        self.assertEqual(list(inspect.signature(run).parameters), [])

    async def test_streamed_agent_round_trips_opaque_tool_json_and_completes(self) -> None:
        arguments_json = '{  "value" : "canvas" }'
        output_json = '{ "result" : "unchanged" }'
        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "session.sqlite3")
            model = ToolThenMessageModel("workspace_get_snapshot", arguments_json)
            frames = await run_agent(
                [
                    (
                        FrameKind.INVOKE,
                        invoke_payload(
                            "invoke-1",
                            "session-1",
                            session_path,
                            "Use the operation.",
                            [operation("workspace_get_snapshot")],
                        ),
                    ),
                    (
                        FrameKind.TOOL_RESPONSE,
                        {
                            "invocation_id": "invoke-1",
                            "call_id": "call-1",
                            "output_json": output_json,
                        },
                    ),
                ],
                model,
            )
        self.assertEqual(
            [(frame.kind, frame.payload) for frame in frames],
            [
                (
                    FrameKind.RESPONSES_EVENT,
                    {
                        "invocation_id": "invoke-1",
                        "event": model.events[0].model_dump(mode="json"),
                    },
                ),
                (
                    FrameKind.TOOL_REQUEST,
                    {
                        "invocation_id": "invoke-1",
                        "operation_id": "workspace_get_snapshot",
                        "call_id": "call-1",
                        "arguments_json": arguments_json,
                    },
                ),
                (
                    FrameKind.RESPONSES_EVENT,
                    {
                        "invocation_id": "invoke-1",
                        "event": model.events[1].model_dump(mode="json"),
                    },
                ),
                (
                    FrameKind.SNAPSHOT,
                    {
                        "invocation_id": "invoke-1",
                        "session_id": "session-1",
                        "status": "completed",
                        "state": None,
                    },
                ),
                (
                    FrameKind.COMPLETED,
                    {"invocation_id": "invoke-1", "final_output": "tool completed"},
                ),
            ],
        )

    async def test_file_session_history_survives_a_new_app_instance(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "durable.sqlite3")
            await run_agent(
                [
                    (
                        FrameKind.INVOKE,
                        invoke_payload(
                            "invoke-first",
                            "durable-session",
                            session_path,
                            "first turn",
                            [],
                        ),
                    )
                ],
                FinalMessageModel("first answer"),
            )
            second_model = FinalMessageModel("second answer")
            await run_agent(
                [
                    (
                        FrameKind.INVOKE,
                        invoke_payload(
                            "invoke-second",
                            "durable-session",
                            session_path,
                            "second turn",
                            [],
                        ),
                    )
                ],
                second_model,
            )

        self.assertIsInstance(second_model.inputs[0], list)
        second_model_input = second_model.inputs[0]
        assert isinstance(second_model_input, list)
        contents = [
            item.get("content")
            for item in second_model_input
            if isinstance(item, dict) and item.get("role") in {"user", "assistant"}
        ]
        self.assertIn("first turn", contents)
        self.assertIn("second turn", contents)
        self.assertTrue(
            any(
                isinstance(content, list)
                and any(
                    isinstance(part, dict) and part.get("text") == "first answer"
                    for part in content
                )
                for content in contents
            )
        )

    async def test_pending_approval_restores_into_fresh_agent_and_executes_once(self) -> None:
        operation_contract = operation("proposal_execute", needs_approval=True)
        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "approval.sqlite3")
            paused_frames = await pause_agent(session_path, operation_contract)
            state = assert_paused_approval(self, paused_frames, APPROVAL_ARGUMENTS)
            resumed_frames = await resume_agent(session_path, operation_contract, state)

        self.assertEqual(
            [frame.kind for frame in resumed_frames],
            [
                FrameKind.TOOL_REQUEST,
                FrameKind.RESPONSES_EVENT,
                FrameKind.SNAPSHOT,
                FrameKind.COMPLETED,
            ],
        )
        self.assertEqual(
            sum(frame.kind is FrameKind.TOOL_REQUEST for frame in resumed_frames), 1
        )
    async def test_rejected_approval_completes_without_executing_tool(self) -> None:
        from assistant.stdio_app import AgentStdioApp

        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "rejected.sqlite3")
            state = await pause_approval(session_path)
            input_bytes = encode_frames(
                [
                    (
                        FrameKind.INVOKE,
                        {
                            "invocation_id": "invoke-reject",
                            "session_id": "approval-session",
                            "session_path": session_path,
                            "input": None,
                            "operations": [operation("proposal_execute", needs_approval=True)],
                            "state": state,
                        },
                    ),
                    (
                        FrameKind.APPROVAL_RESPONSE,
                        {
                            "invocation_id": "invoke-reject",
                            "call_id": "call-1",
                            "approved": False,
                        },
                    ),
                ]
            )
            output = RecordingWriter()
            await AgentStdioApp(
                FrameReader(io.BytesIO(input_bytes)),
                FrameWriter(output),
                model=ToolThenMessageModel("proposal_execute", APPROVAL_ARGUMENTS),
            ).run_once()

        frames = decode_frames(output.bytes)
        self.assertNotIn(FrameKind.TOOL_REQUEST, [frame.kind for frame in frames])
        self.assertEqual(frames[-1].kind, FrameKind.COMPLETED)

if __name__ == "__main__":
    unittest.main()
