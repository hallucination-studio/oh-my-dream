"""Strict framed NDJSON transport for the assistant sidecar."""

from __future__ import annotations

import json
import math
from collections.abc import Callable
from dataclasses import dataclass
from enum import Enum
from typing import Protocol, TypeAlias, TypeVar, cast


PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 1_048_576
MAX_SAFE_JSON_NUMBER = 9_007_199_254_740_991
MAX_JSON_DEPTH = 64
_FRAME_FIELDS = frozenset({"protocol_version", "sequence", "kind", "payload"})

JsonScalar: TypeAlias = None | bool | int | float | str
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
DispatchResult = TypeVar("DispatchResult")


class FrameKind(str, Enum):
    INVOKE = "invoke"
    ASSISTANT_TOKEN = "assistant_token"
    ASSISTANT_MESSAGE = "assistant_message"
    TOOL_REQUEST = "tool_request"
    TOOL_RESPONSE = "tool_response"
    APPROVAL_REQUEST = "approval_request"
    APPROVAL_RESPONSE = "approval_response"
    CANCEL = "cancel"
    SNAPSHOT = "snapshot"
    COMPLETED = "completed"
    ERROR = "error"


@dataclass(frozen=True, slots=True)
class Frame:
    protocol_version: int
    sequence: int
    kind: FrameKind
    payload: dict[str, JsonValue]


class ProtocolErrorCode(str, Enum):
    UNEXPECTED_EOF = "unexpected_eof"
    PARTIAL_FRAME = "partial_frame"
    FRAME_TOO_LARGE = "frame_too_large"
    INVALID_UTF8 = "invalid_utf8"
    INVALID_JSON = "invalid_json"
    INVALID_FIELDS = "invalid_fields"
    INVALID_FRAME = "invalid_frame"
    UNSUPPORTED_VERSION = "unsupported_version"
    UNKNOWN_KIND = "unknown_kind"
    INVALID_SEQUENCE = "invalid_sequence"
    DECODER_FAILED = "decoder_failed"
    WRITER_FAILED = "writer_failed"
    IO_ERROR = "io_error"


class ProtocolError(Exception):
    def __init__(
        self,
        code: ProtocolErrorCode,
        message: str,
        details: dict[str, object] | None = None,
    ) -> None:
        self.code = code
        self.message = message
        self.details = details or {}
        super().__init__(f"{code.value}: {message}")


class BinaryLineReader(Protocol):
    def readline(self, size: int = -1, /) -> bytes:
        ...


class BinaryWriter(Protocol):
    def write(self, data: bytes, /) -> int:
        ...

    def flush(self) -> None:
        ...


class FrameReader:
    def __init__(self, stream: BinaryLineReader) -> None:
        self._stream = stream
        self._next_sequence = 0
        self._failed = False

    @property
    def next_sequence(self) -> int:
        return self._next_sequence

    def read_frame(self) -> Frame:
        if self._failed:
            raise ProtocolError(
                ProtocolErrorCode.DECODER_FAILED,
                "decoder cannot continue after a protocol error",
            )
        try:
            frame = self._read_frame()
        except ProtocolError:
            self._failed = True
            raise
        self._next_sequence += 1
        return frame

    def read_and_dispatch(self, operation: Callable[[Frame], DispatchResult]) -> DispatchResult:
        return operation(self.read_frame())

    def _read_frame(self) -> Frame:
        try:
            encoded = self._stream.readline(MAX_FRAME_BYTES + 1)
        except (OSError, ValueError) as error:
            raise ProtocolError(
                ProtocolErrorCode.IO_ERROR,
                "failed to read protocol input",
            ) from error
        if encoded == b"":
            raise ProtocolError(
                ProtocolErrorCode.UNEXPECTED_EOF,
                "expected a protocol frame before EOF",
            )
        if len(encoded) > MAX_FRAME_BYTES:
            raise ProtocolError(
                ProtocolErrorCode.FRAME_TOO_LARGE,
                "encoded frame exceeds the 1 MiB limit",
                {"maximum": MAX_FRAME_BYTES, "actual_at_least": len(encoded)},
            )
        if not encoded.endswith(b"\n"):
            raise ProtocolError(
                ProtocolErrorCode.PARTIAL_FRAME,
                "protocol frame ended without a newline",
            )

        decoded = _decode_json_object(encoded[:-1])
        frame = _frame_from_object(decoded)
        if frame.sequence != self._next_sequence:
            raise ProtocolError(
                ProtocolErrorCode.INVALID_SEQUENCE,
                "frame sequence is not contiguous",
                {"expected": self._next_sequence, "actual": frame.sequence},
            )
        return frame


class FrameWriter:
    def __init__(self, stream: BinaryWriter) -> None:
        self._stream = stream
        self._next_sequence = 0
        self._failed = False

    @property
    def next_sequence(self) -> int:
        return self._next_sequence

    def write_frame(self, frame: Frame) -> None:
        if self._failed:
            raise ProtocolError(
                ProtocolErrorCode.WRITER_FAILED,
                "writer cannot continue after an output failure",
            )
        encoded = _encode_frame(frame, self._next_sequence)
        try:
            written = self._stream.write(encoded)
            if written != len(encoded):
                self._failed = True
                raise ProtocolError(
                    ProtocolErrorCode.IO_ERROR,
                    "protocol output accepted only part of a frame",
                    {"expected": len(encoded), "actual": written},
                )
            self._stream.flush()
        except ProtocolError:
            raise
        except (OSError, ValueError) as error:
            self._failed = True
            raise ProtocolError(
                ProtocolErrorCode.IO_ERROR,
                "failed to write protocol output",
            ) from error
        self._next_sequence += 1


class _DuplicateKeyError(ValueError):
    def __init__(self, key: str) -> None:
        self.key = key
        super().__init__(key)


def _decode_json_object(encoded: bytes) -> dict[str, object]:
    try:
        text = encoded.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_UTF8,
            "protocol frame is not valid UTF-8",
        ) from error

    try:
        value = json.loads(
            text,
            object_pairs_hook=_object_without_duplicates,
            parse_constant=_reject_json_constant,
        )
    except _DuplicateKeyError as error:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FIELDS,
            "JSON objects must not contain duplicate keys",
            {"key": error.key},
        ) from error
    except (json.JSONDecodeError, ValueError, RecursionError) as error:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_JSON,
            "protocol frame is not valid JSON",
        ) from error

    try:
        _validate_json_value(value)
    except (TypeError, ValueError, UnicodeEncodeError, RecursionError) as error:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_JSON,
            "protocol frame contains an invalid JSON value",
        ) from error
    if not isinstance(value, dict):
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "protocol frame must be a JSON object",
        )
    return cast(dict[str, object], value)


def _frame_from_object(value: dict[str, object]) -> Frame:
    fields = set(value)
    if fields != _FRAME_FIELDS:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FIELDS,
            "protocol frame must contain exactly the version 1 fields",
            {
                "missing": sorted(_FRAME_FIELDS - fields),
                "extra": sorted(fields - _FRAME_FIELDS),
            },
        )
    protocol_version = value["protocol_version"]
    sequence = value["sequence"]
    kind = value["kind"]
    payload = value["payload"]
    _validate_version(protocol_version)
    _validate_sequence_value(sequence)
    frame_kind = _validate_kind(kind)
    if not isinstance(payload, dict):
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "frame payload must be a JSON object",
        )
    return Frame(
        protocol_version=cast(int, protocol_version),
        sequence=cast(int, sequence),
        kind=frame_kind,
        payload=cast(dict[str, JsonValue], payload),
    )


def _encode_frame(frame: Frame, expected_sequence: int) -> bytes:
    _validate_version(frame.protocol_version)
    _validate_sequence_value(frame.sequence)
    frame_kind = _validate_kind(frame.kind)
    if frame.sequence != expected_sequence:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_SEQUENCE,
            "frame sequence is not contiguous",
            {"expected": expected_sequence, "actual": frame.sequence},
        )
    if not isinstance(frame.payload, dict):
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "frame payload must be a JSON object",
        )
    try:
        _validate_json_value(frame.payload, 1)
        text = json.dumps(
            {
                "protocol_version": frame.protocol_version,
                "sequence": frame.sequence,
                "kind": frame_kind.value,
                "payload": frame.payload,
            },
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
        )
        encoded = text.encode("utf-8") + b"\n"
    except (TypeError, ValueError, UnicodeEncodeError, RecursionError) as error:
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "frame cannot be encoded as strict UTF-8 JSON",
        ) from error
    if len(encoded) > MAX_FRAME_BYTES:
        raise ProtocolError(
            ProtocolErrorCode.FRAME_TOO_LARGE,
            "encoded frame exceeds the 1 MiB limit",
            {"maximum": MAX_FRAME_BYTES, "actual": len(encoded)},
        )
    return encoded


def _validate_version(value: object) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "protocol_version must be an integer",
        )
    if value != PROTOCOL_VERSION:
        raise ProtocolError(
            ProtocolErrorCode.UNSUPPORTED_VERSION,
            "unsupported protocol version",
            {"expected": PROTOCOL_VERSION, "actual": value},
        )


def _validate_sequence_value(value: object) -> None:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < 0
        or value > MAX_SAFE_JSON_NUMBER
    ):
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "sequence must be a non-negative safe JSON integer",
        )


def _validate_kind(value: object) -> FrameKind:
    if isinstance(value, FrameKind):
        return value
    if not isinstance(value, str):
        raise ProtocolError(
            ProtocolErrorCode.INVALID_FRAME,
            "kind must be a string",
        )
    try:
        return FrameKind(value)
    except ValueError as error:
        raise ProtocolError(
            ProtocolErrorCode.UNKNOWN_KIND,
            "unsupported frame kind",
            {"actual": value},
        ) from error


def _validate_json_value(value: object, depth: int = 0) -> None:
    if isinstance(value, (list, dict)) and depth > MAX_JSON_DEPTH:
        raise ValueError("JSON container exceeds the shared maximum depth")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, (int, float)):
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError("JSON numbers must be finite")
        if abs(value) > MAX_SAFE_JSON_NUMBER:
            raise ValueError("JSON number exceeds the shared safe range")
        return
    if isinstance(value, str):
        _validate_utf8(value)
        return
    if isinstance(value, list):
        for item in value:
            _validate_json_value(item, depth + 1)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError("JSON object keys must be strings")
            _validate_utf8(key)
            _validate_json_value(item, depth + 1)
        return
    raise TypeError("unsupported JSON value")


def _validate_utf8(value: str) -> None:
    value.encode("utf-8")


def _object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateKeyError(key)
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise ValueError(f"invalid JSON constant: {value}")
