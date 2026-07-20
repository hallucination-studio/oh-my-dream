from __future__ import annotations

import json
import unittest

import httpx

from assistant.openai_provider import (
    ProviderCompatibilityError,
    ProviderResponseError,
    build_openai_client,
    list_model_ids,
    probe_model_compatibility,
)


class OpenAiProviderTests(unittest.IsolatedAsyncioTestCase):
    async def test_lists_sorted_unique_bounded_model_ids_with_explicit_client(self) -> None:
        async def handler(request: httpx.Request) -> httpx.Response:
            self.assertEqual(request.method, "GET")
            self.assertEqual(request.url.path, "/v1/models")
            self.assertEqual(request.headers["authorization"], "Bearer test-key")
            return httpx.Response(
                200,
                json={
                    "object": "list",
                    "data": [
                        model("zeta"),
                        model("alpha"),
                        model("alpha"),
                    ],
                },
            )

        client = build_openai_client(
            "http://provider.test/v1",
            "test-key",
            http_client=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
        )
        try:
            self.assertEqual(await list_model_ids(client), ["alpha", "zeta"])
        finally:
            await client.close()

    async def test_rejects_invalid_model_ids_from_untrusted_models_response(self) -> None:
        async def handler(_request: httpx.Request) -> httpx.Response:
            return httpx.Response(
                200,
                json={"object": "list", "data": [model("bad\nmodel")]},
            )

        client = build_openai_client(
            "https://provider.test/v1",
            "test-key",
            http_client=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
        )
        try:
            with self.assertRaises(ProviderResponseError):
                await list_model_ids(client)
        finally:
            await client.close()

    async def test_compatibility_requires_exact_no_argument_function_call(self) -> None:
        async def handler(request: httpx.Request) -> httpx.Response:
            self.assertEqual(request.method, "POST")
            self.assertEqual(request.url.path, "/v1/responses")
            body = json.loads(request.content)
            self.assertEqual(body["model"], "model-a")
            self.assertFalse(body["store"])
            self.assertFalse(body["parallel_tool_calls"])
            self.assertEqual(
                body["tool_choice"],
                {"type": "function", "name": "assistant_provider_compatibility"},
            )
            self.assertEqual(
                body["tools"],
                [
                    {
                        "type": "function",
                        "name": "assistant_provider_compatibility",
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
            )
            return response_with_call("assistant_provider_compatibility", "{}")

        client = build_openai_client(
            "https://provider.test/v1",
            "test-key",
            http_client=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
        )
        try:
            await probe_model_compatibility(client, "model-a")
        finally:
            await client.close()

    async def test_rejects_wrong_function_name_or_nonempty_arguments(self) -> None:
        responses = iter(
            [
                response_with_call("wrong_function", "{}"),
                response_with_call(
                    "assistant_provider_compatibility", '{"unexpected":true}'
                ),
            ]
        )

        async def handler(_request: httpx.Request) -> httpx.Response:
            return next(responses)

        client = build_openai_client(
            "https://provider.test/v1",
            "test-key",
            http_client=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
        )
        try:
            with self.assertRaises(ProviderCompatibilityError):
                await probe_model_compatibility(client, "model-a")
            with self.assertRaises(ProviderCompatibilityError):
                await probe_model_compatibility(client, "model-a")
        finally:
            await client.close()


def model(model_id: str) -> dict[str, object]:
    return {"id": model_id, "object": "model", "created": 0, "owned_by": "test"}


def response_with_call(name: str, arguments: str) -> httpx.Response:
    return httpx.Response(
        200,
        json={
            "id": "resp_test",
            "created_at": 0,
            "model": "model-a",
            "object": "response",
            "output": [
                {
                    "arguments": arguments,
                    "call_id": "call_test",
                    "name": name,
                    "type": "function_call",
                }
            ],
            "parallel_tool_calls": False,
            "tool_choice": {
                "type": "function",
                "name": "assistant_provider_compatibility",
            },
            "tools": [],
        },
    )


if __name__ == "__main__":
    unittest.main()
