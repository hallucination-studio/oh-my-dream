from __future__ import annotations

import asyncio
import importlib.util
import io
from dataclasses import FrozenInstanceError
from pathlib import Path
import socket
import sys
import unittest
from collections.abc import Callable
from typing import Any, cast
from unittest.mock import patch

from assistant.stdio_protocol import (
    MAX_FRAME_BYTES,
    MAX_SAFE_JSON_NUMBER,
    Frame,
    FrameKind,
    FrameReader,
    FrameWriter,
    JsonValue,
    ProtocolError,
    ProtocolErrorCode,
)
from assistant.tests.stdio_protocol_fakes import RecordingWriter, frame_bytes
from assistant.tests.stdio_protocol_resilience_cases import (
    JsonDepthTests,
    WriterFailureTests,
)


class FrameCodecTests(unittest.TestCase):
    def test_frame_boundary_is_typed_and_immutable(self) -> None:
        frame = Frame(1, 0, FrameKind.INVOKE, {})

        self.assertIsInstance(frame.kind, FrameKind)
        with self.assertRaises(FrozenInstanceError):
            setattr(frame, "sequence", 1)

    def test_round_trips_every_supported_kind(self) -> None:
        output = RecordingWriter()
        writer = FrameWriter(output)
        expected = []

        for sequence, kind in enumerate(FrameKind):
            frame = Frame(
                protocol_version=1,
                sequence=sequence,
                kind=kind,
                payload={
                    "wire_kind": kind.value,
                    "nested": {"items": [1, True, None, "cafe"]},
                },
            )
            expected.append(frame)
            writer.write_frame(frame)

        reader = FrameReader(io.BytesIO(output.bytes))
        actual = [reader.read_frame() for _ in expected]

        self.assertEqual(actual, expected)
        self.assertEqual(
            [kind.value for kind in FrameKind],
            [
                "invoke",
                "responses_event",
                "tool_request",
                "tool_response",
                "approval_request",
                "approval_response",
                "cancel",
                "snapshot",
                "completed",
                "error",
            ],
        )
        self.assertEqual(output.flush_count, len(expected))

    def test_writes_exact_utf8_compact_json_bytes(self) -> None:
        output = RecordingWriter()
        writer = FrameWriter(output)

        writer.write_frame(
            Frame(
                protocol_version=1,
                sequence=0,
                kind=FrameKind.INVOKE,
                payload={"prompt": "caf\u00e9", "options": [1, True, None]},
            )
        )

        self.assertEqual(
            output.bytes,
            b'{"protocol_version":1,"sequence":0,"kind":"invoke",'
            b'"payload":{"prompt":"caf\xc3\xa9","options":[1,true,null]}}\n',
        )
        self.assertEqual(output.chunks, [output.bytes])
        self.assertEqual(output.flush_count, 1)

    def test_reader_and_writer_sequences_progress_independently(self) -> None:
        output = RecordingWriter()
        writer = FrameWriter(output)

        for sequence in range(3):
            writer.write_frame(
                Frame(1, sequence, FrameKind.RESPONSES_EVENT, {"event": {"type": "response.created"}})
            )

        reader = FrameReader(io.BytesIO(output.bytes))
        self.assertEqual([reader.read_frame().sequence for _ in range(3)], [0, 1, 2])
        self.assertEqual(writer.next_sequence, 3)
        self.assertEqual(reader.next_sequence, 3)

    def test_rejects_malformed_json_and_utf8(self) -> None:
        cases = [
            (b'{"protocol_version":1,}\n', ProtocolErrorCode.INVALID_JSON),
            (b"\xff\n", ProtocolErrorCode.INVALID_UTF8),
            (frame_bytes(0, payload='{"value":NaN}'), ProtocolErrorCode.INVALID_JSON),
        ]

        for data, code in cases:
            with self.subTest(code=code, data=data):
                self.assert_decode_error(data, code)

    def test_accepts_safe_number_boundaries_and_normal_fractional_values(self) -> None:
        payload: dict[str, JsonValue] = {
            "positive_int": MAX_SAFE_JSON_NUMBER,
            "negative_int": -MAX_SAFE_JSON_NUMBER,
            "positive_float": float(MAX_SAFE_JSON_NUMBER),
            "negative_float": -float(MAX_SAFE_JSON_NUMBER),
            "fractional": 123.625,
        }
        output = RecordingWriter()

        FrameWriter(output).write_frame(Frame(1, 0, FrameKind.INVOKE, payload))
        decoded = FrameReader(io.BytesIO(output.bytes)).read_frame()

        self.assertEqual(decoded.payload, payload)
        self.assertIsInstance(decoded.payload["positive_float"], float)
        self.assertEqual(decoded.payload["fractional"], 123.625)

    def test_rejects_out_of_domain_numbers_before_dispatch(self) -> None:
        invalid_numbers = [
            str(MAX_SAFE_JSON_NUMBER + 1),
            "-999999999999999999999999999999999999999999999999",
            "1e100",
        ]

        for number in invalid_numbers:
            with self.subTest(number=number):
                calls: list[Frame] = []
                reader = FrameReader(
                    io.BytesIO(frame_bytes(0, payload=f'{{"value":{number}}}'))
                )

                with self.assertRaises(ProtocolError) as raised:
                    reader.read_and_dispatch(calls.append)

                self.assertEqual(raised.exception.code, ProtocolErrorCode.INVALID_JSON)
                self.assertEqual(calls, [])

    def test_rejects_lone_surrogate_payload_and_key_before_dispatch(self) -> None:
        payloads = ['{"value":"\\uD800"}', '{"\\uD800":"value"}']

        for payload in payloads:
            with self.subTest(payload=payload):
                calls: list[Frame] = []
                reader = FrameReader(io.BytesIO(frame_bytes(0, payload=payload)))

                with self.assertRaises(ProtocolError) as raised:
                    reader.read_and_dispatch(calls.append)

                self.assertEqual(raised.exception.code, ProtocolErrorCode.INVALID_JSON)
                self.assertEqual(calls, [])

    def test_rejects_unknown_kind_and_version(self) -> None:
        cases = [
            (frame_bytes(0, kind="future_kind"), ProtocolErrorCode.UNKNOWN_KIND),
            (
                b'{"protocol_version":2,"sequence":0,"kind":"invoke","payload":{}}\n',
                ProtocolErrorCode.UNSUPPORTED_VERSION,
            ),
        ]

        for data, code in cases:
            with self.subTest(code=code):
                self.assert_decode_error(data, code)

    def test_rejects_extra_missing_or_duplicate_top_level_fields(self) -> None:
        cases = [
            b'{"protocol_version":1,"sequence":0,"kind":"invoke","payload":{},"extra":1}\n',
            b'{"protocol_version":1,"sequence":0,"kind":"invoke"}\n',
            b'{"protocol_version":1,"sequence":0,"sequence":0,"kind":"invoke","payload":{}}\n',
        ]

        for data in cases:
            with self.subTest(data=data):
                self.assert_decode_error(data, ProtocolErrorCode.INVALID_FIELDS)

    def test_rejects_invalid_field_types_and_payload_shape(self) -> None:
        cases = [
            b'{"protocol_version":true,"sequence":0,"kind":"invoke","payload":{}}\n',
            b'{"protocol_version":1,"sequence":true,"kind":"invoke","payload":{}}\n',
            b'{"protocol_version":1,"sequence":-1,"kind":"invoke","payload":{}}\n',
            b'{"protocol_version":1,"sequence":0,"kind":1,"payload":{}}\n',
            b'{"protocol_version":1,"sequence":0,"kind":"invoke","payload":[]}\n',
        ]

        for data in cases:
            with self.subTest(data=data):
                self.assert_decode_error(data, ProtocolErrorCode.INVALID_FRAME)

    def test_rejects_oversized_input_before_json_parsing(self) -> None:
        data = b"{" + (b"x" * MAX_FRAME_BYTES) + b"\n"

        self.assert_decode_error(data, ProtocolErrorCode.FRAME_TOO_LARGE)

    def test_accepts_exact_limit_and_rejects_oversized_output_before_writing(self) -> None:
        empty_frame = frame_bytes(0, payload='{"data":""}')
        exact_payload = "x" * (MAX_FRAME_BYTES - len(empty_frame))
        exact_frame = Frame(1, 0, FrameKind.INVOKE, {"data": exact_payload})
        output = RecordingWriter()

        FrameWriter(output).write_frame(exact_frame)

        self.assertEqual(len(output.bytes), MAX_FRAME_BYTES)
        self.assertEqual(FrameReader(io.BytesIO(output.bytes)).read_frame(), exact_frame)

        oversized_output = RecordingWriter()
        oversized_frame = Frame(1, 0, FrameKind.INVOKE, {"data": exact_payload + "x"})
        with self.assertRaises(ProtocolError) as raised:
            FrameWriter(oversized_output).write_frame(oversized_frame)

        self.assertEqual(raised.exception.code, ProtocolErrorCode.FRAME_TOO_LARGE)
        self.assertEqual(oversized_output.bytes, b"")
        self.assertEqual(oversized_output.flush_count, 0)

    def test_rejects_clean_eof_and_partial_line(self) -> None:
        cases = [
            (b"", ProtocolErrorCode.UNEXPECTED_EOF),
            (frame_bytes(0).removesuffix(b"\n"), ProtocolErrorCode.PARTIAL_FRAME),
        ]

        for data, code in cases:
            with self.subTest(code=code):
                self.assert_decode_error(data, code)

    def test_rejects_sequence_gap_duplicate_and_out_of_order(self) -> None:
        cases = [
            (frame_bytes(1), 0, 1),
            (frame_bytes(0) + frame_bytes(0), 1, 0),
            (frame_bytes(0) + frame_bytes(1) + frame_bytes(0), 2, 0),
        ]

        for data, valid_prefix_count, actual_sequence in cases:
            with self.subTest(data=data):
                reader = FrameReader(io.BytesIO(data))
                for _ in range(valid_prefix_count):
                    reader.read_frame()

                with self.assertRaises(ProtocolError) as raised:
                    reader.read_frame()

                self.assertEqual(raised.exception.code, ProtocolErrorCode.INVALID_SEQUENCE)
                self.assertEqual(raised.exception.details["expected"], valid_prefix_count)
                self.assertEqual(raised.exception.details["actual"], actual_sequence)

    def test_decode_error_poisoning_prevents_operation_dispatch(self) -> None:
        invalid_frames = [
            b'{not-json}\n',
            b"\xff\n",
            frame_bytes(0, kind="unknown"),
            b'{"protocol_version":2,"sequence":0,"kind":"invoke","payload":{}}\n',
            b'{"protocol_version":1,"sequence":0,"kind":"invoke"}\n',
            b"{" + (b"x" * MAX_FRAME_BYTES) + b"\n",
            b"",
            frame_bytes(0).removesuffix(b"\n"),
            frame_bytes(1),
        ]

        for data in invalid_frames:
            with self.subTest(data=data[:80]):
                calls: list[Frame] = []
                reader = FrameReader(io.BytesIO(data))
                operation: Callable[[Frame], None] = calls.append

                with self.assertRaises(ProtocolError):
                    reader.read_and_dispatch(operation)
                with self.assertRaises(ProtocolError) as raised_again:
                    reader.read_and_dispatch(operation)

                self.assertEqual(calls, [])
                self.assertEqual(raised_again.exception.code, ProtocolErrorCode.DECODER_FAILED)

    def test_encoding_rejects_invalid_frames_before_writing(self) -> None:
        invalid_frames = [
            Frame(2, 0, FrameKind.INVOKE, {}),
            Frame(1, 1, FrameKind.INVOKE, {}),
            Frame(1, 0, cast(FrameKind, "unknown"), {}),
            Frame(1, 0, FrameKind.INVOKE, cast(dict[str, Any], [])),
            Frame(
                1,
                0,
                FrameKind.INVOKE,
                cast(dict[str, Any], {"value": object()}),
            ),
            Frame(1, 0, FrameKind.INVOKE, {"value": float("nan")}),
            Frame(1, 0, FrameKind.INVOKE, {"value": MAX_SAFE_JSON_NUMBER + 1}),
            Frame(
                1,
                0,
                FrameKind.INVOKE,
                {"value": -999999999999999999999999999999999999999999999999},
            ),
            Frame(1, 0, FrameKind.INVOKE, {"value": 1e100}),
            Frame(1, 0, FrameKind.INVOKE, {"value": "\ud800"}),
            Frame(1, 0, FrameKind.INVOKE, {"\ud800": "value"}),
        ]

        for frame in invalid_frames:
            with self.subTest(frame=frame):
                output = RecordingWriter()
                writer = FrameWriter(output)

                with self.assertRaises(ProtocolError):
                    writer.write_frame(frame)

                self.assertEqual(output.bytes, b"")
                self.assertEqual(output.flush_count, 0)

    def test_module_opens_no_socket_or_listener(self) -> None:
        blocked = AssertionError("network API touched")
        module_name = "assistant._stdio_protocol_no_network_test"
        module_path = Path(__file__).resolve().parents[1] / "stdio_protocol.py"
        spec = importlib.util.spec_from_file_location(module_name, module_path)
        if spec is None or spec.loader is None:
            self.fail("could not load stdio_protocol module spec")
        module = importlib.util.module_from_spec(spec)
        with (
            patch.object(socket, "socket", side_effect=blocked),
            patch.object(socket, "create_connection", side_effect=blocked),
            patch.object(socket, "create_server", side_effect=blocked),
            patch.object(asyncio, "start_server", side_effect=blocked),
        ):
            sys.modules[module_name] = module
            try:
                spec.loader.exec_module(module)
            finally:
                del sys.modules[module_name]

    def assert_decode_error(self, data: bytes, code: ProtocolErrorCode) -> None:
        reader = FrameReader(io.BytesIO(data))

        with self.assertRaises(ProtocolError) as raised:
            reader.read_frame()

        self.assertEqual(raised.exception.code, code)
        self.assertIsInstance(raised.exception.message, str)
        self.assertIsInstance(raised.exception.details, dict)


if __name__ == "__main__":
    unittest.main()
