"""Bounded binary stdio channel for Assistant protocol version 1."""

from __future__ import annotations

import asyncio
from typing import BinaryIO, cast

from .protocol_v1 import (
    MAX_FRAME_BYTES,
    PROTOCOL_VERSION,
    FrameKind,
    ProtocolDecoder,
    ProtocolDirection,
    ProtocolError,
    ProtocolFrame,
    JsonValue,
    encode_frame,
)


class ProtocolChannel:
    def __init__(self, reader: BinaryIO, writer: BinaryIO) -> None:
        self._reader = reader
        self._writer = writer
        self._decoder = ProtocolDecoder(ProtocolDirection.RUST_TO_PYTHON)
        self._invocation_id: str | None = None
        self._next_output_sequence = 1

    async def read(self) -> ProtocolFrame:
        encoded = await asyncio.to_thread(self._reader.readline, MAX_FRAME_BYTES + 1)
        frame = self._decoder.decode(encoded)
        if self._invocation_id is None:
            self._invocation_id = frame.invocation_id
        elif frame.invocation_id != self._invocation_id:
            raise ProtocolError("invocation identity mismatch")
        return frame

    def write(self, kind: FrameKind, payload: dict[str, object]) -> None:
        if self._invocation_id is None:
            raise ProtocolError("cannot write before invocation identity is known")
        frame = ProtocolFrame(
            protocol_version=PROTOCOL_VERSION,
            invocation_id=self._invocation_id,
            direction_sequence=self._next_output_sequence,
            kind=kind,
            payload=cast(dict[str, JsonValue], payload),
        )
        encoded = encode_frame(frame, ProtocolDirection.PYTHON_TO_RUST)
        written = self._writer.write(encoded)
        if written != len(encoded):
            raise ProtocolError("partial protocol write")
        self._writer.flush()
        self._next_output_sequence += 1
