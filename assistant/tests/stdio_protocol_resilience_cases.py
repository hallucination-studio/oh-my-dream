from __future__ import annotations

import io
import json
import unittest

from assistant.stdio_protocol import (
    MAX_JSON_DEPTH,
    Frame,
    FrameKind,
    FrameReader,
    FrameWriter,
    JsonValue,
    ProtocolError,
    ProtocolErrorCode,
)
from assistant.tests.stdio_protocol_fakes import RecordingWriter, nested_payload_at_depth


def encoded_frame(payload: dict[str, JsonValue]) -> bytes:
    return (
        json.dumps(
            {
                "protocol_version": 1,
                "sequence": 0,
                "kind": "invoke",
                "payload": payload,
            },
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
        ).encode("utf-8")
        + b"\n"
    )


class WriterFailureTests(unittest.TestCase):
    def test_short_write_permanently_rejects_retry_without_advancing_sequence(self) -> None:
        output = RecordingWriter(short_write_bytes=12)
        writer = FrameWriter(output)
        frame = Frame(1, 0, FrameKind.INVOKE, {"prompt": "draw"})

        with self.assertRaises(ProtocolError) as first:
            writer.write_frame(frame)
        with self.assertRaises(ProtocolError) as retry:
            writer.write_frame(frame)

        self.assertEqual(first.exception.code, ProtocolErrorCode.IO_ERROR)
        self.assertEqual(retry.exception.code, ProtocolErrorCode.WRITER_FAILED)
        self.assertEqual(writer.next_sequence, 0)
        self.assertEqual(output.write_count, 1)
        self.assertEqual(output.flush_count, 0)
        self.assertEqual(len(output.bytes), 12)

    def test_flush_failure_permanently_rejects_retry_without_advancing_sequence(self) -> None:
        output = RecordingWriter(fail_flush=True)
        writer = FrameWriter(output)
        frame = Frame(1, 0, FrameKind.INVOKE, {"prompt": "draw"})

        with self.assertRaises(ProtocolError) as first:
            writer.write_frame(frame)
        bytes_after_failure = output.bytes
        with self.assertRaises(ProtocolError) as retry:
            writer.write_frame(frame)

        self.assertEqual(first.exception.code, ProtocolErrorCode.IO_ERROR)
        self.assertEqual(retry.exception.code, ProtocolErrorCode.WRITER_FAILED)
        self.assertEqual(writer.next_sequence, 0)
        self.assertEqual(output.write_count, 1)
        self.assertEqual(output.flush_count, 1)
        self.assertEqual(output.bytes, bytes_after_failure)

    def test_validation_error_does_not_poison_writer(self) -> None:
        output = RecordingWriter()
        writer = FrameWriter(output)

        with self.assertRaises(ProtocolError) as invalid:
            writer.write_frame(Frame(1, 1, FrameKind.INVOKE, {}))
        writer.write_frame(Frame(1, 0, FrameKind.INVOKE, {}))

        self.assertEqual(invalid.exception.code, ProtocolErrorCode.INVALID_SEQUENCE)
        self.assertEqual(writer.next_sequence, 1)
        self.assertEqual(output.write_count, 1)
        self.assertEqual(output.flush_count, 1)


class JsonDepthTests(unittest.TestCase):
    def test_shared_maximum_depth_is_64(self) -> None:
        self.assertEqual(MAX_JSON_DEPTH, 64)

    def test_depth_64_container_with_depth_65_scalar_is_accepted(self) -> None:
        payload = nested_payload_at_depth(MAX_JSON_DEPTH)

        decoded = FrameReader(io.BytesIO(encoded_frame(payload))).read_frame()
        output = RecordingWriter()
        FrameWriter(output).write_frame(Frame(1, 0, FrameKind.INVOKE, payload))

        self.assertEqual(decoded.payload, payload)
        self.assertGreater(len(output.bytes), 0)
        self.assertEqual(output.flush_count, 1)

    def test_depth_65_decode_fails_before_dispatch(self) -> None:
        payload = nested_payload_at_depth(MAX_JSON_DEPTH + 1)
        calls: list[Frame] = []
        reader = FrameReader(io.BytesIO(encoded_frame(payload)))

        with self.assertRaises(ProtocolError) as raised:
            reader.read_and_dispatch(calls.append)

        self.assertEqual(raised.exception.code, ProtocolErrorCode.INVALID_JSON)
        self.assertEqual(calls, [])

    def test_depth_65_encode_fails_before_write(self) -> None:
        payload = nested_payload_at_depth(MAX_JSON_DEPTH + 1)
        output = RecordingWriter()

        with self.assertRaises(ProtocolError) as raised:
            FrameWriter(output).write_frame(Frame(1, 0, FrameKind.INVOKE, payload))

        self.assertEqual(raised.exception.code, ProtocolErrorCode.INVALID_FRAME)
        self.assertEqual(output.write_count, 0)
        self.assertEqual(output.flush_count, 0)
        self.assertEqual(output.bytes, b"")
