"""Bounded OpenAI Responses provider operations for Desktop Assistant settings."""

from __future__ import annotations

import json
import unicodedata
from typing import Final

import httpx
from openai import AsyncOpenAI


MAX_MODEL_COUNT: Final = 10_000
MAX_MODEL_ID_BYTES: Final = 256
MAX_MODEL_ID_TOTAL_BYTES: Final = 1024 * 1024
COMPATIBILITY_TOOL_NAME: Final = "assistant_provider_compatibility"


class ProviderResponseError(ValueError):
    """A provider response violated the bounded public contract."""

    def __init__(self) -> None:
        super().__init__("Assistant provider response is invalid")


class ProviderCompatibilityError(ValueError):
    """The selected model did not make the exact required function call."""

    def __init__(self) -> None:
        super().__init__("Assistant provider model is incompatible")


def build_openai_client(
    base_url: str,
    api_key: str,
    *,
    http_client: httpx.AsyncClient | None = None,
) -> AsyncOpenAI:
    """Build an explicit non-retrying client for one bounded provider operation."""
    return AsyncOpenAI(
        base_url=base_url,
        api_key=api_key,
        timeout=15.0,
        max_retries=0,
        http_client=http_client,
        _strict_response_validation=True,
    )


async def list_model_ids(client: AsyncOpenAI) -> list[str]:
    """Return sorted unique model IDs from one Models response page."""
    page = await client.models.list()
    model_ids = [model.id for model in page.data]
    if len(model_ids) > MAX_MODEL_COUNT:
        raise ProviderResponseError()
    total_bytes = 0
    for model_id in model_ids:
        if not _valid_model_id(model_id):
            raise ProviderResponseError()
        total_bytes += len(model_id.encode("utf-8"))
        if total_bytes > MAX_MODEL_ID_TOTAL_BYTES:
            raise ProviderResponseError()
    return sorted(set(model_ids))


async def probe_model_compatibility(client: AsyncOpenAI, model_id: str) -> None:
    """Require one exact no-argument function call from the Responses API."""
    response = await client.responses.create(
        model=model_id,
        input=(
            "Call the assistant_provider_compatibility function now with an empty "
            "JSON object and do not produce any other function call."
        ),
        tools=[
            {
                "type": "function",
                "name": COMPATIBILITY_TOOL_NAME,
                "description": "Confirm Assistant function-tool compatibility.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": False,
                },
                "strict": True,
            }
        ],
        tool_choice={"type": "function", "name": COMPATIBILITY_TOOL_NAME},
        store=False,
        parallel_tool_calls=False,
        max_output_tokens=64,
    )
    calls = [item for item in response.output if item.type == "function_call"]
    if len(calls) != 1 or calls[0].name != COMPATIBILITY_TOOL_NAME:
        raise ProviderCompatibilityError()
    try:
        arguments = json.loads(calls[0].arguments)
    except (TypeError, json.JSONDecodeError) as error:
        raise ProviderCompatibilityError() from error
    if type(arguments) is not dict or arguments:
        raise ProviderCompatibilityError()


def _valid_model_id(value: object) -> bool:
    if not isinstance(value, str) or not value or value != value.strip():
        return False
    if len(value.encode("utf-8")) > MAX_MODEL_ID_BYTES:
        return False
    return not any(unicodedata.category(character) == "Cc" for character in value)
