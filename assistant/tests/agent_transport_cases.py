"""Subprocess cases imported by the focused agent transport suite."""

from __future__ import annotations

from pathlib import Path
import io
import subprocess
import sys
import tempfile
import unittest

from assistant.stdio_protocol import FrameKind, FrameReader, FrameWriter
from assistant.tests.agent_transport_fixture import (
    APPROVAL_ARGUMENTS,
    ToolThenMessageModel,
    decode_frames,
    encode_frames,
    operation,
    pause_approval,
)
from assistant.tests.stdio_protocol_fakes import RecordingWriter


class FailClosedTests(unittest.IsolatedAsyncioTestCase):
    async def test_state_metadata_mismatch_emits_one_error(self) -> None:
        await self.assert_resume_error(
            lambda state: state.__setitem__("sdk_version", "0.18.0"),
            expected_code="state_metadata_mismatch",
        )

    async def test_state_metadata_types_are_exact(self) -> None:
        await self.assert_resume_error(
            lambda state: state.__setitem__("envelope_version", True),
            expected_code="state_metadata_mismatch",
        )

    async def test_invalid_state_json_emits_one_error(self) -> None:
        await self.assert_resume_error(
            lambda state: state.__setitem__("state_json", "not-an-object"),
            expected_code="invalid_state",
        )

    async def test_approval_call_mismatch_emits_one_error(self) -> None:
        await self.assert_resume_error(
            lambda _state: None,
            expected_code="correlation_mismatch",
            call_id="different-call",
        )

    async def assert_resume_error(
        self,
        mutate_state: object,
        *,
        expected_code: str,
        call_id: str = "call-1",
    ) -> None:
        from assistant.stdio_app import AgentStdioApp

        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "invalid.sqlite3")
            state = await pause_approval(session_path)
            if not callable(mutate_state):
                self.fail("state mutator must be callable")
            mutate_state(state)
            input_bytes = encode_frames(
                [
                    (
                        FrameKind.INVOKE,
                        {
                            "invocation_id": "invoke-invalid",
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
                            "invocation_id": "invoke-invalid",
                            "call_id": call_id,
                            "approved": True,
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
        self.assertEqual(len(frames), 1)
        self.assertEqual(frames[0].kind, FrameKind.ERROR)
        self.assertEqual(frames[0].payload["invocation_id"], "invoke-invalid")
        self.assertEqual(frames[0].payload["code"], expected_code)


class FixtureProcessTests(unittest.TestCase):
    def test_tool_mode_writes_protocol_frames_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "tool.sqlite3")
            input_bytes = encode_frames(
                [
                    (
                        FrameKind.INVOKE,
                        {
                            "invocation_id": "fixture-tool",
                            "session_id": "fixture-session",
                            "session_path": session_path,
                            "input": "Use the tool.",
                            "operations": [operation("workspace_get_snapshot")],
                            "state": None,
                        },
                    ),
                    (
                        FrameKind.TOOL_RESPONSE,
                        {
                            "invocation_id": "fixture-tool",
                            "call_id": "call-1",
                            "output_json": '{ "result" : "fixture" }',
                        },
                    ),
                ]
            )

            process = self.run_fixture("tool", input_bytes)

        self.assertEqual(process.returncode, 0)
        self.assertEqual(process.stderr, b"")
        frames = decode_frames(process.stdout)
        self.assertEqual(
            next(frame for frame in frames if frame.kind is FrameKind.TOOL_REQUEST).kind,
            FrameKind.TOOL_REQUEST,
        )
        self.assertEqual(frames[-1].kind, FrameKind.COMPLETED)

    def test_approval_mode_restores_state_in_a_fresh_process(self) -> None:
        approval_operation = operation("proposal_execute", needs_approval=True)
        with tempfile.TemporaryDirectory() as directory:
            session_path = str(Path(directory) / "approval.sqlite3")
            paused = self.run_fixture(
                "approval",
                encode_frames(
                    [
                        (
                            FrameKind.INVOKE,
                            {
                                "invocation_id": "fixture-pause",
                                "session_id": "fixture-session",
                                "session_path": session_path,
                                "input": "Execute the proposal.",
                                "operations": [approval_operation],
                                "state": None,
                            },
                        )
                    ]
                ),
            )
            paused_frames = decode_frames(paused.stdout)
            approval = next(
                frame
                for frame in paused_frames
                if frame.kind is FrameKind.APPROVAL_REQUEST
            )
            state = approval.payload["state"]
            resumed = self.run_fixture(
                "approval",
                encode_frames(
                    [
                        (
                            FrameKind.INVOKE,
                            {
                                "invocation_id": "fixture-resume",
                                "session_id": "fixture-session",
                                "session_path": session_path,
                                "input": None,
                                "operations": [approval_operation],
                                "state": state,
                            },
                        ),
                        (
                            FrameKind.APPROVAL_RESPONSE,
                            {
                                "invocation_id": "fixture-resume",
                                "call_id": "call-1",
                                "approved": True,
                            },
                        ),
                        (
                            FrameKind.TOOL_RESPONSE,
                            {
                                "invocation_id": "fixture-resume",
                                "call_id": "call-1",
                                "output_json": '{"result":"started"}',
                            },
                        ),
                    ]
                ),
            )

        self.assertEqual(paused.returncode, 0)
        self.assertEqual(paused.stderr, b"")
        self.assertEqual(
            [frame.kind for frame in paused_frames],
            [FrameKind.RESPONSES_EVENT, FrameKind.APPROVAL_REQUEST, FrameKind.SNAPSHOT],
        )
        self.assertEqual(resumed.returncode, 0)
        self.assertEqual(resumed.stderr, b"")
        resumed_frames = decode_frames(resumed.stdout)
        self.assertEqual(
            next(frame for frame in resumed_frames if frame.kind is FrameKind.TOOL_REQUEST).kind,
            FrameKind.TOOL_REQUEST,
        )
        self.assertEqual(resumed_frames[-1].kind, FrameKind.COMPLETED)

    def run_fixture(self, mode: str, input_bytes: bytes) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, "-m", "assistant.tests.agent_transport_fixture", mode],
            input=input_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            cwd=Path(__file__).resolve().parents[2],
        )
