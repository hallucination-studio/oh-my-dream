from pathlib import Path

import pytest

from assistant.protocol_v1 import (
    FrameKind,
    ProtocolDecoder,
    ProtocolDirection,
    ProtocolError,
)

FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "crates"
    / "assistant"
    / "tests"
    / "fixtures"
    / "protocol_v1_valid.ndjson"
)


def test_rust_fixture_decodes_with_exact_one_based_frames() -> None:
    decoder = ProtocolDecoder(ProtocolDirection.PYTHON_TO_RUST)
    frames = [decoder.decode(line) for line in FIXTURE.read_bytes().splitlines(keepends=True)]

    assert [frame.direction_sequence for frame in frames] == [1, 2, 3]
    assert frames[0].kind is FrameKind.INVOCATION_ACCEPTED
    assert frames[-1].kind is FrameKind.INVOCATION_COMPLETED


def test_duplicate_and_unknown_fields_fail_closed() -> None:
    decoder = ProtocolDecoder(ProtocolDirection.PYTHON_TO_RUST)
    duplicate = (
        b'{"protocol_version":1,"invocation_id":"03000000-0000-4000-8000-000000000003",'
        b'"direction_sequence":1,"direction_sequence":1,"kind":"InvocationAccepted",'
        b'"payload":{"agent_id":"workflow_coauthor@1"}}\n'
    )
    with pytest.raises(ProtocolError):
        decoder.decode(duplicate)

    unknown = (
        b'{"protocol_version":1,"invocation_id":"03000000-0000-4000-8000-000000000003",'
        b'"direction_sequence":1,"kind":"InvocationAccepted",'
        b'"payload":{"agent_id":"workflow_coauthor@1","extra":true}}\n'
    )
    with pytest.raises(ProtocolError):
        ProtocolDecoder(ProtocolDirection.PYTHON_TO_RUST).decode(unknown)
