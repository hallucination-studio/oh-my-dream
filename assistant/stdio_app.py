"""Async OpenAI Agents SDK application over inherited framed stdio."""

from __future__ import annotations

import asyncio
from collections.abc import Mapping
from dataclasses import dataclass
import sys
from typing import Any, cast

from agents import Agent, ItemHelpers, MessageOutputItem, Model, Runner, RunState, Tool
from agents.stream_events import RunItemStreamEvent

from .sdk_runtime import (
    AGENT_NAME,
    StateEnvelopeError,
    build_file_session,
    build_run_config,
    build_state_envelope,
    restore_run_state,
    validate_state_envelope,
)
from .stdio_protocol import (
    PROTOCOL_VERSION,
    Frame,
    FrameKind,
    FrameReader,
    FrameWriter,
    JsonValue,
    ProtocolError,
)
from .tool_contract import ToolRequest, ToolResponse, build_function_tools


class AgentTransportError(Exception):
    """A fail-closed application protocol violation."""

    def __init__(self, code: str, message: str) -> None:
        self.code = code
        self.message = message
        super().__init__(message)


@dataclass(frozen=True, slots=True)
class Invocation:
    invocation_id: str
    session_id: str
    session_path: str
    input: str | None
    operations: list[Mapping[str, Any]]
    state: Mapping[str, Any] | None


class AgentStdioApp:
    """Run one SDK Agent invocation over synchronous framed streams."""

    def __init__(
        self,
        reader: FrameReader,
        writer: FrameWriter,
        *,
        model: Model | str | None = None,
    ) -> None:
        self._reader = reader
        self._writer = writer
        self._model = model

    async def run_once(self) -> None:
        """Consume one invocation and emit its terminal protocol frames."""
        invocation_id = ""
        try:
            frame = await self._read_frame()
            candidate_id = frame.payload.get("invocation_id")
            if isinstance(candidate_id, str):
                invocation_id = candidate_id
            invocation = _parse_invocation(frame)
            await self._run_invocation(invocation)
        except AgentTransportError as error:
            self._emit_error(invocation_id, error.code, error.message)
        except ProtocolError:
            self._emit_error(invocation_id, "protocol_error", "assistant protocol failure")
        except Exception:
            self._emit_error(invocation_id, "sdk_error", "assistant SDK failure")

    async def _run_invocation(self, invocation: Invocation) -> None:
        session = build_file_session(invocation.session_id, invocation.session_path)
        try:
            tools = build_function_tools(
                invocation.operations,
                lambda request: self._invoke_tool(invocation.invocation_id, request),
            )
            agent: Agent[Any] = Agent(
                name=AGENT_NAME,
                model=self._model,
                tools=cast(list[Tool], tools),
            )
            run_input: str | RunState[Any, Agent[Any]]
            if invocation.input is None:
                run_input = await self._restore_and_decide(invocation, agent)
            elif invocation.state is None:
                run_input = invocation.input
            else:
                raise AgentTransportError(
                    "invalid_invoke", "new invocation state must be null"
                )
            result = Runner.run_streamed(
                agent,
                run_input,
                session=session,
                run_config=build_run_config(),
            )
            async for event in result.stream_events():
                self._emit_message(invocation.invocation_id, event)
            if result.interruptions:
                self._emit_interruption(invocation, result)
                return
            self._write(
                FrameKind.SNAPSHOT,
                {
                    "invocation_id": invocation.invocation_id,
                    "session_id": invocation.session_id,
                    "status": "completed",
                    "state": None,
                },
            )
            self._write(
                FrameKind.COMPLETED,
                {
                    "invocation_id": invocation.invocation_id,
                    "final_output": cast(JsonValue, result.final_output),
                },
            )
        finally:
            session.close()

    async def _restore_and_decide(
        self, invocation: Invocation, agent: Agent[Any]
    ) -> RunState[Any, Agent[Any]]:
        if invocation.state is None:
            raise AgentTransportError(
                "invalid_invoke", "resume invocation requires state"
            )
        try:
            state_json = validate_state_envelope(
                invocation.state, invocation.operations
            )
        except StateEnvelopeError as error:
            raise AgentTransportError(error.code, error.message) from error
        restored = await restore_run_state(agent, state_json, context_override=None)
        interruptions = restored.get_interruptions()
        interruption = _single_interruption(interruptions)
        call_id = _interruption_call_id(interruption)
        frame = await self._read_frame()
        if frame.kind is not FrameKind.APPROVAL_RESPONSE:
            raise AgentTransportError(
                "unexpected_frame", "expected approval_response after resume invoke"
            )
        _require_exact_fields(
            frame.payload,
            {"invocation_id", "call_id", "approved"},
            "approval_response",
        )
        response_invocation = _require_string(frame.payload, "invocation_id")
        response_call_id = _require_string(frame.payload, "call_id")
        approved = frame.payload["approved"]
        if not isinstance(approved, bool):
            raise AgentTransportError(
                "invalid_payload", "approved must be a boolean"
            )
        if (
            response_invocation != invocation.invocation_id
            or response_call_id != call_id
        ):
            raise AgentTransportError(
                "correlation_mismatch",
                "approval_response does not match the pending interruption",
            )
        if approved:
            restored.approve(interruption)
        else:
            restored.reject(interruption)
        return restored

    def _emit_interruption(self, invocation: Invocation, result: Any) -> None:
        interruption = _single_interruption(result.interruptions)
        operation_id = _interruption_operation_id(interruption)
        call_id = _interruption_call_id(interruption)
        arguments_json = _interruption_arguments(interruption)
        operation_ids = {
            _operation_id(operation) for operation in invocation.operations
        }
        if operation_id not in operation_ids:
            raise AgentTransportError(
                "invalid_interruption", "interruption references an unknown operation"
            )
        state_json = result.to_state().to_json(strict_context=True)
        if not isinstance(state_json, dict):
            raise AgentTransportError(
                "invalid_state", "SDK state must serialize to an object"
            )
        try:
            envelope = cast(
                dict[str, JsonValue],
                build_state_envelope(invocation.operations, state_json),
            )
        except StateEnvelopeError as error:
            raise AgentTransportError(error.code, error.message) from error
        self._write(
            FrameKind.APPROVAL_REQUEST,
            {
                "invocation_id": invocation.invocation_id,
                "operation_id": operation_id,
                "call_id": call_id,
                "arguments_json": arguments_json,
                "state": envelope,
            },
        )
        self._write(
            FrameKind.SNAPSHOT,
            {
                "invocation_id": invocation.invocation_id,
                "session_id": invocation.session_id,
                "status": "waiting_approval",
                "state": envelope,
            },
        )

    async def _invoke_tool(
        self, invocation_id: str, request: ToolRequest
    ) -> ToolResponse:
        self._write(
            FrameKind.TOOL_REQUEST,
            {
                "invocation_id": invocation_id,
                "operation_id": request.operation_id,
                "call_id": request.call_id,
                "arguments_json": request.arguments_json,
            },
        )
        frame = await self._read_frame()
        if frame.kind is not FrameKind.TOOL_RESPONSE:
            raise AgentTransportError(
                "unexpected_frame", "expected tool_response after tool_request"
            )
        _require_exact_fields(
            frame.payload,
            {"invocation_id", "call_id", "output_json"},
            "tool_response",
        )
        response_invocation = _require_string(frame.payload, "invocation_id")
        call_id = _require_string(frame.payload, "call_id")
        output_json = _require_string(frame.payload, "output_json")
        if response_invocation != invocation_id or call_id != request.call_id:
            raise AgentTransportError(
                "correlation_mismatch", "tool_response does not match the pending call"
            )
        return ToolResponse(call_id=call_id, output_json=output_json)

    async def _read_frame(self) -> Frame:
        return await asyncio.to_thread(self._reader.read_frame)

    def _emit_message(self, invocation_id: str, event: object) -> None:
        if not isinstance(event, RunItemStreamEvent):
            return
        if event.name != "message_output_created" or not isinstance(
            event.item, MessageOutputItem
        ):
            return
        text = ItemHelpers.text_message_output(event.item)
        if text:
            self._write(
                FrameKind.ASSISTANT_MESSAGE,
                {"invocation_id": invocation_id, "text": text},
            )

    def _write(self, kind: FrameKind, payload: dict[str, JsonValue]) -> None:
        self._writer.write_frame(
            Frame(PROTOCOL_VERSION, self._writer.next_sequence, kind, payload)
        )

    def _emit_error(self, invocation_id: str, code: str, message: str) -> None:
        try:
            self._write(
                FrameKind.ERROR,
                {"invocation_id": invocation_id, "code": code, "message": message},
            )
        except ProtocolError:
            return


def run() -> None:
    """Run one production SDK invocation over inherited binary stdio."""
    app = AgentStdioApp(
        FrameReader(sys.stdin.buffer),
        FrameWriter(sys.stdout.buffer),
    )
    asyncio.run(app.run_once())


def _parse_invocation(frame: Frame) -> Invocation:
    if frame.kind is not FrameKind.INVOKE:
        raise AgentTransportError("unexpected_frame", "first frame must be invoke")
    _require_exact_fields(
        frame.payload,
        {"invocation_id", "session_id", "session_path", "input", "operations", "state"},
        "invoke",
    )
    input_value = frame.payload["input"]
    if input_value is not None and not isinstance(input_value, str):
        raise AgentTransportError("invalid_invoke", "input must be a string or null")
    operations_value = frame.payload["operations"]
    if not isinstance(operations_value, list) or not all(
        isinstance(operation, dict) for operation in operations_value
    ):
        raise AgentTransportError("invalid_invoke", "operations must be an array of objects")
    state_value = frame.payload["state"]
    if state_value is not None and not isinstance(state_value, dict):
        raise AgentTransportError("invalid_invoke", "state must be an object or null")
    return Invocation(
        invocation_id=_require_string(frame.payload, "invocation_id"),
        session_id=_require_string(frame.payload, "session_id"),
        session_path=_require_string(frame.payload, "session_path"),
        input=input_value,
        operations=cast(list[Mapping[str, Any]], operations_value),
        state=cast(Mapping[str, Any] | None, state_value),
    )


def _require_exact_fields(
    payload: Mapping[str, object], expected: set[str], label: str
) -> None:
    if set(payload) != expected:
        raise AgentTransportError(
            "invalid_payload", f"{label} payload fields do not match the contract"
        )


def _require_string(payload: Mapping[str, object], key: str) -> str:
    value = payload[key]
    if not isinstance(value, str):
        raise AgentTransportError("invalid_payload", f"{key} must be a string")
    return value


def _operation_id(operation: Mapping[str, Any]) -> str:
    operation_id = operation.get("id")
    if not isinstance(operation_id, str):
        raise AgentTransportError("invalid_operations", "operation id must be a string")
    return operation_id


def _single_interruption(interruptions: list[Any]) -> Any:
    if len(interruptions) != 1:
        raise AgentTransportError(
            "invalid_interruption", "exactly one pending interruption is required"
        )
    return interruptions[0]


def _interruption_operation_id(interruption: Any) -> str:
    operation_id = interruption.tool_name
    if not isinstance(operation_id, str):
        raise AgentTransportError(
            "invalid_interruption", "interruption tool_name must be a string"
        )
    return operation_id


def _interruption_call_id(interruption: Any) -> str:
    call_id = interruption.call_id
    if not isinstance(call_id, str):
        raise AgentTransportError(
            "invalid_interruption", "interruption call_id must be a string"
        )
    return call_id


def _interruption_arguments(interruption: Any) -> str:
    arguments = interruption.arguments
    if not isinstance(arguments, str):
        raise AgentTransportError(
            "invalid_interruption", "interruption arguments must be a string"
        )
    return arguments


if __name__ == "__main__":
    run()
