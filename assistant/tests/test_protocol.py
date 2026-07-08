import pytest

from assistant.protocol import AssistantProtocol, ProtocolError


def test_protocol_requires_auth_before_other_frames():
    protocol = AssistantProtocol(token="secret")

    with pytest.raises(ProtocolError):
        protocol.handle({"type": "user_message", "text": "hi"})

    assert protocol.handle({"type": "auth", "token": "secret"}) == [{"type": "auth_ok"}]
    assert protocol.authenticated


def test_protocol_builds_tool_result_frames_from_executor():
    calls = []

    def execute(capability, args):
        calls.append((capability, args))
        return {"id": "n1"}

    protocol = AssistantProtocol(token="secret", execute=execute)
    protocol.handle({"type": "auth", "token": "secret"})

    frames = protocol.handle(
        {
            "type": "tool_call",
            "call_id": "call-1",
            "capability": "workflow.add_node",
            "args": {"node_type": "TextPrompt"},
        }
    )

    assert calls == [("workflow.add_node", {"node_type": "TextPrompt"})]
    assert frames == [{"type": "tool_result", "call_id": "call-1", "ok": True, "result": {"id": "n1"}}]


def test_protocol_reports_tool_errors_as_frames():
    def execute(_capability, _args):
        raise RuntimeError("bad args")

    protocol = AssistantProtocol(token="secret", execute=execute)
    protocol.handle({"type": "auth", "token": "secret"})

    frames = protocol.handle({"type": "tool_call", "call_id": "call-2", "capability": "missing", "args": {}})

    assert frames == [{"type": "tool_result", "call_id": "call-2", "ok": False, "error": "bad args"}]
