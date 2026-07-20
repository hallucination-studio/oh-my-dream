"""Local deterministic OpenAI-compatible HTTP server for integration tests."""

from __future__ import annotations

import json
import sys
import threading
import time
from contextlib import AbstractContextManager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


class LocalOpenAiContractServer(AbstractContextManager["LocalOpenAiContractServer"]):
    def __init__(self) -> None:
        self._server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)

    @property
    def origin(self) -> str:
        host, port = self._server.server_address
        return f"http://{host}:{port}"

    def start(self) -> "LocalOpenAiContractServer":
        self._thread.start()
        return self

    def __enter__(self) -> "LocalOpenAiContractServer":
        return self.start()

    def __exit__(self, *_args: object) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=2)


class _Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self) -> None:
        if not self._authorized():
            return
        if self.path == "/timeout/v1/models":
            time.sleep(0.2)
        if self.path == "/malformed/v1/models":
            self._json(200, {"object": "list", "data": [_model("bad\nmodel")]})
            return
        if self.path.endswith("/v1/models"):
            self._json(200, {"object": "list", "data": [_model("model-b"), _model("model-a")]})
            return
        self._json(404, {"error": {"message": "not found"}})

    def do_POST(self) -> None:
        if not self._authorized():
            return
        length = int(self.headers.get("content-length", "0"))
        try:
            body = json.loads(self.rfile.read(length))
        except json.JSONDecodeError:
            self._json(400, {"error": {"message": "invalid JSON"}})
            return
        if not self.path.endswith("/v1/responses"):
            self._json(404, {"error": {"message": "not found"}})
            return
        if body.get("model") == "missing-model":
            self._json(404, {"error": {"message": "model not found"}})
            return
        if body.get("stream") is True:
            self._stream_runtime_response(str(body.get("model")))
            return
        if body.get("model") == "incompatible-model":
            self._json(200, _text_response("incompatible-model", "No tool call"))
            return
        self._json(200, _compatibility_response(str(body.get("model"))))

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def _authorized(self) -> bool:
        if self.headers.get("authorization") == "Bearer rejected-key":
            self._json(401, {"error": {"message": "invalid key"}})
            return False
        return True

    def _json(self, status: int, value: dict[str, Any]) -> None:
        encoded = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _stream_runtime_response(self, model_id: str) -> None:
        event = {
            "type": "response.completed",
            "sequence_number": 0,
            "response": _text_response(model_id, "Local Assistant response"),
        }
        encoded = f"event: response.completed\ndata: {json.dumps(event, separators=(',', ':'))}\n\n".encode()
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


def _model(model_id: str) -> dict[str, object]:
    return {"id": model_id, "object": "model", "created": 0, "owned_by": "local-test"}


def _compatibility_response(model_id: str) -> dict[str, Any]:
    response = _response_base(model_id)
    response["output"] = [{
        "arguments": "{}",
        "call_id": "call_compatibility",
        "name": "assistant_provider_compatibility",
        "type": "function_call",
        "status": "completed",
    }]
    return response


def _text_response(model_id: str, text: str) -> dict[str, Any]:
    response = _response_base(model_id)
    response["output"] = [{
        "id": "message_local",
        "content": [{"annotations": [], "text": text, "type": "output_text"}],
        "role": "assistant",
        "status": "completed",
        "type": "message",
    }]
    return response


def _response_base(model_id: str) -> dict[str, Any]:
    return {
        "id": "resp_local",
        "created_at": 0,
        "model": model_id,
        "object": "response",
        "output": [],
        "parallel_tool_calls": False,
        "tool_choice": "auto",
        "tools": [],
        "status": "completed",
    }


def main() -> None:
    with LocalOpenAiContractServer() as server:
        print(server.origin, flush=True)
        sys.stdin.buffer.read()


if __name__ == "__main__":
    main()
