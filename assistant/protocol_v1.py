"""Strict Assistant protocol version 1 schemas shared with the Rust runner."""

from __future__ import annotations

import json
from dataclasses import dataclass
from enum import Enum
from typing import TypeAlias, cast

PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 8 * 1024 * 1024
MAX_JSON_DEPTH = 32
MAX_INBOUND_EVENTS = 512
MAX_TOOL_CALLS = 64
FRAME_FIELDS = frozenset(
    {"protocol_version", "invocation_id", "direction_sequence", "kind", "payload"}
)

JsonValue: TypeAlias = None | bool | int | float | str | list["JsonValue"] | dict[str, "JsonValue"]


class ProtocolDirection(str, Enum):
    RUST_TO_PYTHON = "rust_to_python"
    PYTHON_TO_RUST = "python_to_rust"


class ProtocolError(ValueError):
    """Fail-closed protocol validation error."""


class FrameKind(str, Enum):
    INVOCATION_START = "InvocationStart"
    TOOL_RESULT = "ToolResult"
    CONTINUATION_RESUME = "ContinuationResume"
    INVOCATION_CANCEL = "InvocationCancel"
    INVOCATION_ACCEPTED = "InvocationAccepted"
    MODEL_OUTPUT_DELTA = "ModelOutputDelta"
    TOOL_CALL = "ToolCall"
    REVIEWER_VERDICT = "ReviewerVerdict"
    CONTINUATION_ENVELOPE_READY = "ContinuationEnvelopeReady"
    INVOCATION_COMPLETED = "InvocationCompleted"
    INVOCATION_FAILED = "InvocationFailed"


RUST_KINDS = frozenset(
    {
        FrameKind.INVOCATION_START,
        FrameKind.TOOL_RESULT,
        FrameKind.CONTINUATION_RESUME,
        FrameKind.INVOCATION_CANCEL,
    }
)

PAYLOAD_FIELDS = {
    FrameKind.INVOCATION_START: frozenset(
        {"start", "trusted_context", "tool_contracts", "budgets"}
    ),
    FrameKind.TOOL_RESULT: frozenset({"call_id", "tool_id", "result"}),
    FrameKind.CONTINUATION_RESUME: frozenset({"envelope", "trusted_result"}),
    FrameKind.INVOCATION_CANCEL: frozenset({"reason"}),
    FrameKind.INVOCATION_ACCEPTED: frozenset({"agent_id"}),
    FrameKind.MODEL_OUTPUT_DELTA: frozenset({"text"}),
    FrameKind.TOOL_CALL: frozenset({"call_id", "tool_id", "arguments"}),
    FrameKind.REVIEWER_VERDICT: frozenset(
        {"change_id", "mutation_digest", "verdict", "prose"}
    ),
    FrameKind.CONTINUATION_ENVELOPE_READY: frozenset({"envelope"}),
    FrameKind.INVOCATION_COMPLETED: frozenset({"final_text"}),
    FrameKind.INVOCATION_FAILED: frozenset({"category", "safe_message"}),
}
TOOL_IDS = frozenset(
    {
        "assistant.workspace.get_snapshot@1",
        "assistant.node_capability.list@1",
        "assistant.node_capability.describe@1",
        "assistant.production_plan.get@1",
        "assistant.production_plan.create@1",
        "assistant.production_plan.replace@1",
        "assistant.production_plan.update_item@1",
        "assistant.workflow.evaluate_mutation@1",
        "assistant.workflow.propose_change@1",
        "assistant.workflow.get_change@1",
        "assistant.workflow.request_apply@1",
    }
)


@dataclass(frozen=True, slots=True)
class ProtocolFrame:
    protocol_version: int
    invocation_id: str
    direction_sequence: int
    kind: FrameKind
    payload: dict[str, JsonValue]


class ProtocolDecoder:
    def __init__(self, direction: ProtocolDirection) -> None:
        self._direction = direction
        self._next_sequence = 1
        self._events = 0
        self._tool_calls = 0
        self._call_ids: set[str] = set()
        self._terminal = False

    def decode(self, encoded: bytes) -> ProtocolFrame:
        if len(encoded) > MAX_FRAME_BYTES:
            raise ProtocolError("frame too large")
        if not encoded.endswith(b"\n"):
            raise ProtocolError("partial frame")
        if self._terminal:
            raise ProtocolError("frame after terminal")
        value = _decode_json(encoded[:-1])
        frame = _validate_frame(value, self._direction, self._next_sequence)
        self._record(frame)
        self._next_sequence += 1
        return frame

    def _record(self, frame: ProtocolFrame) -> None:
        self._events += 1
        if self._events > MAX_INBOUND_EVENTS:
            raise ProtocolError("event budget exceeded")
        if frame.kind is FrameKind.TOOL_CALL:
            self._tool_calls += 1
            call_id = frame.payload["call_id"]
            if (
                self._tool_calls > MAX_TOOL_CALLS
                or not isinstance(call_id, str)
                or not 0 < len(call_id) <= 128
                or call_id in self._call_ids
            ):
                raise ProtocolError("invalid or duplicate call ID")
            self._call_ids.add(call_id)
        self._terminal = frame.kind in {
            FrameKind.INVOCATION_COMPLETED,
            FrameKind.INVOCATION_FAILED,
        }


def _decode_json(encoded: bytes) -> dict[str, object]:
    try:
        value = json.loads(
            encoded.decode("utf-8"),
            object_pairs_hook=_without_duplicate_keys,
            parse_constant=lambda value: (_ for _ in ()).throw(
                ProtocolError(f"invalid number {value}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ProtocolError("invalid JSON") from error
    if not isinstance(value, dict):
        raise ProtocolError("frame must be an object")
    if _json_depth(value) > MAX_JSON_DEPTH:
        raise ProtocolError("JSON too deep")
    return cast(dict[str, object], value)


def _without_duplicate_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ProtocolError("duplicate object key")
        value[key] = item
    return value


def _validate_frame(
    value: dict[str, object],
    direction: ProtocolDirection,
    expected_sequence: int,
) -> ProtocolFrame:
    if set(value) != FRAME_FIELDS:
        raise ProtocolError("invalid frame fields")
    version = value["protocol_version"]
    invocation_id = value["invocation_id"]
    sequence = value["direction_sequence"]
    payload = value["payload"]
    if version != PROTOCOL_VERSION or isinstance(version, bool):
        raise ProtocolError("invalid protocol version")
    if (
        not isinstance(invocation_id, str)
        or len(invocation_id) != 36
        or not invocation_id.isascii()
        or sequence != expected_sequence
        or isinstance(sequence, bool)
    ):
        raise ProtocolError("invalid identity or sequence")
    try:
        kind = FrameKind(value["kind"])
    except (TypeError, ValueError) as error:
        raise ProtocolError("unknown frame kind") from error
    if not isinstance(payload, dict) or set(payload) != PAYLOAD_FIELDS[kind]:
        raise ProtocolError("invalid payload fields")
    _validate_payload(kind, payload)
    rust_frame = kind in RUST_KINDS
    if rust_frame != (direction is ProtocolDirection.RUST_TO_PYTHON):
        raise ProtocolError("wrong frame direction")
    return ProtocolFrame(version, invocation_id, sequence, kind, cast(dict[str, JsonValue], payload))


def _validate_payload(kind: FrameKind, payload: dict[str, object]) -> None:
    if kind is FrameKind.INVOCATION_ACCEPTED and payload["agent_id"] not in {
        "workflow_coauthor@1",
        "workflow_change_reviewer@1",
    }:
        raise ProtocolError("invalid agent identity")
    if kind in {FrameKind.TOOL_CALL, FrameKind.TOOL_RESULT}:
        call_id = payload["call_id"]
        if (
            not isinstance(call_id, str)
            or not 0 < len(call_id) <= 128
            or payload["tool_id"] not in TOOL_IDS
        ):
            raise ProtocolError("invalid tool call")
    if kind is FrameKind.INVOCATION_CANCEL and payload["reason"] not in {
        "Deadline",
        "ProcessShutdown",
    }:
        raise ProtocolError("invalid cancel reason")
    if kind is FrameKind.INVOCATION_START:
        _validate_start_payload(payload)


def _validate_start_payload(payload: dict[str, object]) -> None:
    start = payload["start"]
    context = payload["trusted_context"]
    contracts = payload["tool_contracts"]
    budgets = payload["budgets"]
    if not isinstance(start, dict) or start.get("kind") not in {
        "UserMessage",
        "RepairActivation",
    }:
        raise ProtocolError("invalid invocation start")
    if not isinstance(context, dict) or set(context) != {
        "project_id",
        "session_id",
        "workspace_snapshot",
    }:
        raise ProtocolError("invalid trusted context")
    if (
        not isinstance(contracts, list)
        or len(contracts) != 11
        or {item.get("tool_id") for item in contracts if isinstance(item, dict)} != TOOL_IDS
    ):
        raise ProtocolError("invalid tool contracts")
    expected_budgets = {
        "maximum_frame_bytes": MAX_FRAME_BYTES,
        "maximum_events": MAX_INBOUND_EVENTS,
        "maximum_tool_calls": MAX_TOOL_CALLS,
        "maximum_model_turns": 16,
        "maximum_direction_bytes": 16 * 1024 * 1024,
        "deadline_ms": 600_000,
    }
    if budgets != expected_budgets:
        raise ProtocolError("invalid invocation budgets")


def _json_depth(value: object, depth: int = 0) -> int:
    if isinstance(value, dict):
        return max((_json_depth(item, depth + 1) for item in value.values()), default=depth + 1)
    if isinstance(value, list):
        return max((_json_depth(item, depth + 1) for item in value), default=depth + 1)
    return depth
