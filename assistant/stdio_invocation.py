"""Validated invocation and approval values for the stdio SDK boundary."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any, cast

from .stdio_protocol import Frame, FrameKind


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


def parse_invocation(frame: Frame) -> Invocation:
    if frame.kind is not FrameKind.INVOKE:
        raise AgentTransportError("unexpected_frame", "first frame must be invoke")
    require_exact_fields(
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
        invocation_id=require_string(frame.payload, "invocation_id"),
        session_id=require_string(frame.payload, "session_id"),
        session_path=require_string(frame.payload, "session_path"),
        input=input_value,
        operations=cast(list[Mapping[str, Any]], operations_value),
        state=cast(Mapping[str, Any] | None, state_value),
    )


def require_exact_fields(
    payload: Mapping[str, object], expected: set[str], label: str
) -> None:
    if set(payload) != expected:
        raise AgentTransportError(
            "invalid_payload", f"{label} payload fields do not match the contract"
        )


def require_string(payload: Mapping[str, object], key: str) -> str:
    value = payload[key]
    if not isinstance(value, str):
        raise AgentTransportError("invalid_payload", f"{key} must be a string")
    return value


def operation_id(operation: Mapping[str, Any]) -> str:
    value = operation.get("id")
    if not isinstance(value, str):
        raise AgentTransportError("invalid_operations", "operation id must be a string")
    return value


def single_interruption(interruptions: list[Any]) -> Any:
    if len(interruptions) != 1:
        raise AgentTransportError(
            "invalid_interruption", "exactly one pending interruption is required"
        )
    return interruptions[0]


def interruption_operation_id(interruption: Any) -> str:
    operation_id = interruption.tool_name
    if not isinstance(operation_id, str):
        raise AgentTransportError(
            "invalid_interruption", "interruption tool_name must be a string"
        )
    return operation_id


def interruption_call_id(interruption: Any) -> str:
    call_id = interruption.call_id
    if not isinstance(call_id, str):
        raise AgentTransportError(
            "invalid_interruption", "interruption call_id must be a string"
        )
    return call_id


def interruption_arguments(interruption: Any) -> str:
    arguments = interruption.arguments
    if not isinstance(arguments, str):
        raise AgentTransportError(
            "invalid_interruption", "interruption arguments must be a string"
        )
    return arguments
