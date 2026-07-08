"""FastAPI server entrypoint for the assistant sidecar."""

from __future__ import annotations

import asyncio
import os
import socket
from typing import Any

from .protocol import AssistantProtocol, ProtocolError


def run() -> None:
    try:
        import uvicorn
    except ModuleNotFoundError as error:
        raise RuntimeError(
            "Assistant server dependencies are not installed. Install the sidecar "
            "environment from assistant/requirements.txt before running the server."
        ) from error

    token = _required_env("OH_MY_DREAM_ASSISTANT_TOKEN")
    origins = set(filter(None, os.environ.get("OH_MY_DREAM_ALLOWED_ORIGINS", "").split(",")))
    port = _free_loopback_port()
    print(f"PORT={port}", flush=True)
    uvicorn.run(
        create_app(token=token, allowed_origins=origins),
        host="127.0.0.1",
        port=port,
        log_level="warning",
    )


def create_app(token: str, allowed_origins: set[str]):
    try:
        from fastapi import FastAPI, WebSocket, WebSocketDisconnect
    except ModuleNotFoundError as error:
        raise RuntimeError(
            "FastAPI is not installed. Install assistant/requirements.txt before running."
        ) from error

    app = FastAPI()

    async def websocket_handler(websocket: WebSocket) -> None:
        origin = websocket.headers.get("origin")
        if allowed_origins and origin not in allowed_origins:
            await websocket.close(code=1008)
            return
        await websocket.accept()
        protocol = AssistantProtocol(token=token)
        try:
            first_frame = await asyncio.wait_for(websocket.receive_json(), timeout=2.0)
            for frame in protocol.handle(_frame(first_frame)):
                await websocket.send_json(frame)
            if not protocol.authenticated:
                await websocket.close(code=1008)
                return
            while True:
                frame = _frame(await websocket.receive_json())
                if frame.get("type") == "user_message":
                    await websocket.send_json({"type": "token", "delta": "Assistant sidecar connected."})
                    await websocket.send_json({"type": "message_done"})
                else:
                    for response in protocol.handle(frame):
                        await websocket.send_json(response)
        except (TimeoutError, ProtocolError):
            await websocket.close(code=1008)
        except WebSocketDisconnect:
            return

    app.websocket("/")(websocket_handler)
    app.websocket("/ws")(websocket_handler)
    return app


def _frame(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ProtocolError("assistant frame must be an object")
    return value


def _required_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"{name} is required")
    return value


def _free_loopback_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])
