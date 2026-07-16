"""Deterministic public-SDK fixtures for the stdio agent transport."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
import io
import sys
from typing import Any, cast

from agents import (
    AgentOutputSchemaBase,
    Handoff,
    Model,
    ModelResponse,
    ModelSettings,
    ModelTracing,
    Tool,
    TResponseInputItem,
)
from openai.types.responses import (
    Response,
    ResponseCompletedEvent,
    ResponseFunctionToolCall,
    ResponseOutputMessage,
    ResponseOutputText,
    ResponseStreamEvent,
)
from openai.types.responses.response_prompt_param import ResponsePromptParam

from assistant.stdio_protocol import Frame, FrameKind, FrameReader, FrameWriter, JsonValue
from assistant.tests.stdio_protocol_fakes import RecordingWriter


APPROVAL_ARGUMENTS = '{\n  "value": "approved"\n}'


def operation(operation_id: str, *, needs_approval: bool = False) -> dict[str, object]:
    return {
        "id": operation_id,
        "version": 3,
        "description": "Deterministic operation.",
        "input_schema": {
            "type": "object",
            "properties": {"value": {"type": "string"}},
            "required": ["value"],
            "additionalProperties": False,
        },
        "strict_json_schema": True,
        "needs_approval": needs_approval,
    }


def encode_frames(frames: list[tuple[FrameKind, dict[str, object]]]) -> bytes:
    output = RecordingWriter()
    writer = FrameWriter(output)
    for sequence, (kind, payload) in enumerate(frames):
        writer.write_frame(Frame(1, sequence, kind, cast(dict[str, JsonValue], payload)))
    return output.bytes


def decode_frames(data: bytes) -> list[Frame]:
    stream = io.BytesIO(data)
    reader = FrameReader(stream)
    frames: list[Frame] = []
    while stream.tell() < len(data):
        frames.append(reader.read_frame())
    return frames


async def pause_approval(session_path: str) -> dict[str, Any]:
    """Create one pending approval state through the real stdio app."""
    from assistant.tests.legacy_stdio_app import AgentStdioApp

    input_bytes = encode_frames(
        [
            (
                FrameKind.INVOKE,
                {
                    "invocation_id": "invoke-pause",
                    "session_id": "approval-session",
                    "session_path": session_path,
                    "input": "Execute the proposal.",
                    "operations": [operation("proposal_execute", needs_approval=True)],
                    "state": None,
                },
            )
        ]
    )
    output = RecordingWriter()
    await AgentStdioApp(
        FrameReader(io.BytesIO(input_bytes)),
        FrameWriter(output),
        model=ToolThenMessageModel("proposal_execute", APPROVAL_ARGUMENTS),
    ).run_once()
    approval = next(
        frame for frame in decode_frames(output.bytes) if frame.kind is FrameKind.APPROVAL_REQUEST
    )
    state = approval.payload["state"]
    if not isinstance(state, dict):
        raise TypeError("approval fixture state must be an object")
    return state


def _response(output: list[Any], response_id: str) -> Response:
    return Response(
        id=response_id,
        created_at=0.0,
        model="agent-transport-fixture",
        object="response",
        output=output,
        parallel_tool_calls=False,
        tool_choice="auto",
        tools=[],
    )


class ToolThenMessageModel(Model):
    """Call one configured tool, then emit a deterministic final message."""

    def __init__(self, operation_id: str, arguments_json: str) -> None:
        self.operation_id = operation_id
        self.arguments_json = arguments_json
        self.inputs: list[str | list[TResponseInputItem]] = []
        self.events: list[ResponseStreamEvent] = []

    async def get_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> ModelResponse:
        raise AssertionError("streamed runs must use stream_response")

    async def stream_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> AsyncIterator[ResponseStreamEvent]:
        self.inputs.append(input)
        has_tool_output = isinstance(input, list) and any(
            (
                item.get("type") if isinstance(item, dict) else getattr(item, "type", None)
            )
            == "function_call_output"
            for item in input
        )
        if has_tool_output:
            output: list[Any] = [
                ResponseOutputMessage(
                    id="message-final",
                    content=[
                        ResponseOutputText(
                            annotations=[], text="tool completed", type="output_text"
                        )
                    ],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
            response_id = "response-final"
        else:
            output = [
                ResponseFunctionToolCall(
                    arguments=self.arguments_json,
                    call_id="call-1",
                    name=self.operation_id,
                    type="function_call",
                    status="completed",
                )
            ]
            response_id = "response-tool"
        event = ResponseCompletedEvent(
            response=_response(output, response_id),
            sequence_number=0,
            type="response.completed",
        )
        self.events.append(event)
        yield event


class SequencedDiscoveryModel(Model):
    """Call capability search, describe its result, then finish."""

    def __init__(self) -> None:
        self.inputs: list[str | list[TResponseInputItem]] = []
        self.tool_names: list[str] = []
        self.calls = [
            ("capability_search", '{"query":"video","kinds":null}', "search-call"),
            (
                "capability_describe",
                '{"refs":[{"id":"ImageToVideo","version":"1.0"}]}',
                "describe-call",
            ),
        ]

    async def get_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> ModelResponse:
        raise AssertionError("streamed runs must use stream_response")

    async def stream_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> AsyncIterator[ResponseStreamEvent]:
        self.inputs.append(input)
        self.tool_names = [tool.name for tool in tools]
        completed_calls = 0
        if isinstance(input, list):
            completed_calls = sum(
                (
                    item.get("type") if isinstance(item, dict) else getattr(item, "type", None)
                )
                == "function_call_output"
                for item in input
            )
        if completed_calls < len(self.calls):
            operation_id, arguments_json, call_id = self.calls[completed_calls]
            output: list[Any] = [
                ResponseFunctionToolCall(
                    arguments=arguments_json,
                    call_id=call_id,
                    name=operation_id,
                    type="function_call",
                    status="completed",
                )
            ]
            response_id = f"response-{completed_calls + 1}"
        else:
            output = [
                ResponseOutputMessage(
                    id="message-discovery-final",
                    content=[
                        ResponseOutputText(
                            annotations=[], text="discovery complete", type="output_text"
                        )
                    ],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
            response_id = "response-discovery-final"
        yield ResponseCompletedEvent(
            response=_response(output, response_id),
            sequence_number=0,
            type="response.completed",
        )


class FinalMessageModel(Model):
    """Record SDK input and complete without calling a tool."""

    def __init__(self, text: str = "recorded") -> None:
        self.text = text
        self.inputs: list[str | list[TResponseInputItem]] = []
        self.events: list[ResponseStreamEvent] = []

    async def get_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> ModelResponse:
        raise AssertionError("streamed runs must use stream_response")

    async def stream_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> AsyncIterator[ResponseStreamEvent]:
        self.inputs.append(input)
        message = ResponseOutputMessage(
            id="message-recorded",
            content=[ResponseOutputText(annotations=[], text=self.text, type="output_text")],
            role="assistant",
            status="completed",
            type="message",
        )
        event = ResponseCompletedEvent(
            response=_response([message], "response-recorded"),
            sequence_number=0,
            type="response.completed",
        )
        self.events.append(event)
        yield event


def main() -> None:
    """Run a deterministic transport fixture for Rust integration tests."""
    if len(sys.argv) != 2 or sys.argv[1] not in {"tool", "approval", "coauthor"}:
        raise SystemExit(2)
    if sys.argv[1] == "coauthor":
        from assistant.tests.coauthor_fixture import CoauthorModel

        model = CoauthorModel()
    elif sys.argv[1] == "tool":
        operation_id = "workspace_get_snapshot"
        arguments_json = '{  "query" : "current" }'
        model = ToolThenMessageModel(operation_id, arguments_json)
    else:
        operation_id = "proposal_execute"
        arguments_json = '{\n  "proposal_id": "proposal-42"\n}'
        model = ToolThenMessageModel(operation_id, arguments_json)

    from assistant.tests.legacy_stdio_app import AgentStdioApp

    app = AgentStdioApp(
        FrameReader(sys.stdin.buffer),
        FrameWriter(sys.stdout.buffer),
        model=model,
    )
    asyncio.run(app.run_once())


if __name__ == "__main__":
    main()
