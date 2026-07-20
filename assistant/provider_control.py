"""One-shot, secret-safe control entrypoint for Assistant provider settings."""

from __future__ import annotations

import asyncio
import json
import os
import sys
from collections.abc import Mapping
from typing import TextIO

from agents import set_tracing_disabled
from openai import (
    APIConnectionError,
    APIStatusError,
    APITimeoutError,
    AuthenticationError,
    BadRequestError,
    NotFoundError,
)

from .openai_provider import (
    ProviderCompatibilityError,
    ProviderResponseError,
    build_openai_client,
    list_model_ids,
    probe_model_compatibility,
)


set_tracing_disabled(True)

ACTION_ENV = "OH_MY_DREAM_ASSISTANT_PROVIDER_ACTION"
BASE_URL_ENV = "OH_MY_DREAM_ASSISTANT_PROVIDER_BASE_URL"
API_KEY_ENV = "OH_MY_DREAM_ASSISTANT_PROVIDER_API_KEY"
MODEL_ID_ENV = "OH_MY_DREAM_ASSISTANT_PROVIDER_MODEL_ID"


async def run_provider_control(environment: Mapping[str, str], output: TextIO) -> int:
    """Run one validated provider operation and emit one safe JSON result."""
    action = environment.get(ACTION_ENV)
    base_url = environment.get(BASE_URL_ENV)
    api_key = environment.get(API_KEY_ENV)
    model_id = environment.get(MODEL_ID_ENV)
    if not _valid_input(action, base_url, api_key, model_id):
        _write(output, {"ok": False, "error": "invalid_control_input"})
        return 0

    client = build_openai_client(base_url, api_key)
    try:
        if action == "list_models":
            model_ids = await list_model_ids(client)
            result: dict[str, object] = {"ok": True, "model_ids": model_ids}
        else:
            await probe_model_compatibility(client, model_id)
            result = {"ok": True}
    except Exception as error:  # All external errors cross one closed safe boundary.
        result = {"ok": False, "error": _safe_error_code(action, error)}
    finally:
        await client.close()
    _write(output, result)
    return 0


def run() -> None:
    """Run the process entrypoint against inherited environment and stdout."""
    raise SystemExit(asyncio.run(run_provider_control(os.environ, sys.stdout)))


def _valid_input(
    action: str | None,
    base_url: str | None,
    api_key: str | None,
    model_id: str | None,
) -> bool:
    if action not in {"list_models", "test_model"}:
        return False
    if not _bounded_text(base_url, 2048) or not _bounded_text(api_key, 16 * 1024):
        return False
    return action == "list_models" or _bounded_text(model_id, 256)


def _bounded_text(value: str | None, maximum_bytes: int) -> bool:
    return (
        isinstance(value, str)
        and bool(value)
        and len(value.encode("utf-8")) <= maximum_bytes
        and not any(character.isascii() and not character.isprintable() for character in value)
    )


def _safe_error_code(action: str, error: Exception) -> str:
    if isinstance(error, AuthenticationError):
        return "authentication_rejected"
    if isinstance(error, APITimeoutError):
        return "provider_timed_out"
    if isinstance(error, APIConnectionError):
        return "provider_unreachable"
    if isinstance(error, ProviderResponseError):
        return "invalid_models_response"
    if isinstance(error, ProviderCompatibilityError):
        return "missing_function_tool_behavior"
    if isinstance(error, (NotFoundError, BadRequestError)) and action == "test_model":
        return "selected_model_rejected"
    if isinstance(error, APIStatusError):
        return (
            "models_endpoint_unavailable"
            if action == "list_models"
            else "responses_endpoint_unavailable"
        )
    return "provider_control_failed"


def _write(output: TextIO, value: Mapping[str, object]) -> None:
    output.write(json.dumps(value, separators=(",", ":"), sort_keys=True))
    output.write("\n")
    output.flush()


if __name__ == "__main__":
    run()
