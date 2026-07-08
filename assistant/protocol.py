"""Assistant WebSocket frame handling.

The server layer is intentionally thin: parse JSON, pass dictionaries here, and
write returned dictionaries back to the socket.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any


class ProtocolError(Exception):
    """Raised when a frame violates the assistant protocol."""


ToolExecutor = Callable[[str, dict[str, Any]], Any]


class AssistantProtocol:
    def __init__(self, token: str, execute: ToolExecutor | None = None) -> None:
        self._token = token
        self._execute = execute or self._missing_executor
        self.authenticated = False

    def handle(self, frame: dict[str, Any]) -> list[dict[str, Any]]:
        frame_type = self._string(frame, "type")
        if not self.authenticated:
            if frame_type != "auth":
                raise ProtocolError("first assistant frame must be auth")
            return self._handle_auth(frame)
        if frame_type == "tool_call":
            return [self._handle_tool_call(frame)]
        if frame_type == "cancel":
            return [{"type": "status", "text": "cancelled"}]
        if frame_type in {"client_ready", "user_message", "tool_result", "confirm_result"}:
            return []
        raise ProtocolError(f"unsupported assistant frame `{frame_type}`")

    def _handle_auth(self, frame: dict[str, Any]) -> list[dict[str, Any]]:
        if self._string(frame, "token") != self._token:
            return [{"type": "auth_err", "reason": "invalid token"}]
        self.authenticated = True
        return [{"type": "auth_ok"}]

    def _handle_tool_call(self, frame: dict[str, Any]) -> dict[str, Any]:
        call_id = self._string(frame, "call_id")
        try:
            result = self._execute(self._string(frame, "capability"), self._object(frame, "args"))
            return {"type": "tool_result", "call_id": call_id, "ok": True, "result": result}
        except Exception as error:
            return {"type": "tool_result", "call_id": call_id, "ok": False, "error": str(error)}

    def _missing_executor(self, _capability: str, _args: dict[str, Any]) -> Any:
        raise ProtocolError("no tool executor configured")

    def _string(self, frame: dict[str, Any], field: str) -> str:
        value = frame.get(field)
        if not isinstance(value, str):
            raise ProtocolError(f"frame field `{field}` must be a string")
        return value

    def _object(self, frame: dict[str, Any], field: str) -> dict[str, Any]:
        value = frame.get(field)
        if not isinstance(value, dict):
            raise ProtocolError(f"frame field `{field}` must be an object")
        return value
