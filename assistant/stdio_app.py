"""Async OpenAI Agents SDK application over inherited framed stdio."""

from __future__ import annotations

import asyncio
import sys
from typing import Any, cast

from agents import Agent, Model, Runner, RunState, Tool
from agents.stream_events import RawResponsesStreamEvent

from .sdk_runtime import (
    AGENT_NAME,
    SDK_MAX_TURNS,
    StateEnvelopeError,
    build_file_session,
    build_model_settings,
    build_run_config,
    build_state_envelope,
    restore_run_state,
    validate_state_envelope,
)
from .reviewer import AttestedReview, build_reviewer_tool
from .system_prompt import build_system_prompt
from .stdio_protocol import (
    PROTOCOL_VERSION,
    Frame,
    FrameKind,
    FrameReader,
    FrameWriter,
    JsonValue,
    ProtocolError,
)
from .stdio_invocation import (
    AgentTransportError,
    Invocation,
    interruption_arguments,
    interruption_call_id,
    interruption_operation_id,
    operation_id,
    parse_invocation,
    require_exact_fields,
    require_string,
    single_interruption,
)
from .tool_contract import ToolRequest, ToolResponse, build_function_tools


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
            invocation = parse_invocation(frame)
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
            if any(
                operation_id(operation) == "workflow_candidate_get"
                for operation in invocation.operations
            ):
                reviewer_tool = build_reviewer_tool(
                    invocation.operations,
                    lambda request: self._invoke_tool(invocation.invocation_id, request),
                    self._model,
                    lambda review: self._submit_review(invocation.invocation_id, review),
                )
                tools.append(reviewer_tool)
            agent: Agent[Any] = Agent(
                name=AGENT_NAME,
                instructions=build_system_prompt(),
                model=self._model,
                model_settings=build_model_settings(),
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
                max_turns=SDK_MAX_TURNS,
            )
            async for event in result.stream_events():
                self._emit_stream_event(invocation.invocation_id, event)
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
        interruption = single_interruption(interruptions)
        call_id = interruption_call_id(interruption)
        frame = await self._read_frame()
        if frame.kind is not FrameKind.APPROVAL_RESPONSE:
            raise AgentTransportError(
                "unexpected_frame", "expected approval_response after resume invoke"
            )
        require_exact_fields(
            frame.payload,
            {"invocation_id", "call_id", "approved"},
            "approval_response",
        )
        response_invocation = require_string(frame.payload, "invocation_id")
        response_call_id = require_string(frame.payload, "call_id")
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
        interruption = single_interruption(result.interruptions)
        interrupted_operation_id = interruption_operation_id(interruption)
        call_id = interruption_call_id(interruption)
        arguments_json = interruption_arguments(interruption)
        operation_ids = {
            operation_id(operation) for operation in invocation.operations
        }
        if interrupted_operation_id not in operation_ids:
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
                "operation_id": interrupted_operation_id,
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
        require_exact_fields(
            frame.payload,
            {"invocation_id", "call_id", "output_json"},
            "tool_response",
        )
        response_invocation = require_string(frame.payload, "invocation_id")
        call_id = require_string(frame.payload, "call_id")
        output_json = require_string(frame.payload, "output_json")
        if response_invocation != invocation_id or call_id != request.call_id:
            raise AgentTransportError(
                "correlation_mismatch", "tool_response does not match the pending call"
            )
        return ToolResponse(call_id=call_id, output_json=output_json)

    async def _submit_review(
        self, invocation_id: str, review: AttestedReview
    ) -> str:
        self._write(
            FrameKind.REVIEW_SUBMIT,
            {
                "invocation_id": invocation_id,
                **cast(dict[str, JsonValue], review.model_dump(mode="json")),
            },
        )
        frame = await self._read_frame()
        if frame.kind is not FrameKind.REVIEW_RESPONSE:
            raise AgentTransportError(
                "unexpected_frame", "expected review_response after review_submit"
            )
        require_exact_fields(
            frame.payload,
            {"invocation_id", "candidate_id", "review_receipt_id"},
            "review_response",
        )
        if require_string(frame.payload, "invocation_id") != invocation_id:
            raise AgentTransportError(
                "correlation_mismatch", "review_response invocation mismatch"
            )
        if require_string(frame.payload, "candidate_id") != review.candidate_id:
            raise AgentTransportError(
                "correlation_mismatch", "review_response candidate mismatch"
            )
        return require_string(frame.payload, "review_receipt_id")

    async def _read_frame(self) -> Frame:
        return await asyncio.to_thread(self._reader.read_frame)

    def _emit_stream_event(self, invocation_id: str, event: object) -> None:
        if not isinstance(event, RawResponsesStreamEvent):
            return
        model_dump = getattr(event.data, "model_dump", None)
        if not callable(model_dump):
            raise AgentTransportError(
                "invalid_stream_event", "Responses stream event data is not serializable"
            )
        try:
            data = model_dump(mode="json")
        except (TypeError, ValueError) as error:
            raise AgentTransportError(
                "invalid_stream_event", "Responses stream event data is not serializable"
            ) from error
        if not isinstance(data, dict):
            raise AgentTransportError(
                "invalid_stream_event", "Responses stream event data must be an object"
            )
        event_json = cast(dict[str, JsonValue], data)
        if not isinstance(event_json.get("type"), str):
            raise AgentTransportError(
                "invalid_stream_event", "Responses stream event data must include a type"
            )
        self._write(
            FrameKind.RESPONSES_EVENT,
            {"invocation_id": invocation_id, "event": event_json},
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


if __name__ == "__main__":
    run()
