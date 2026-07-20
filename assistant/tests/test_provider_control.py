from __future__ import annotations

import io
import json
import unittest
from unittest.mock import patch

import httpx

from assistant.openai_provider import build_openai_client
from assistant.provider_control import run_provider_control


class ProviderControlTests(unittest.IsolatedAsyncioTestCase):
    async def test_model_list_writes_one_bounded_success_object(self) -> None:
        async def handler(_request: httpx.Request) -> httpx.Response:
            return httpx.Response(
                200,
                json={
                    "object": "list",
                    "data": [
                        {"id": "model-b", "object": "model", "created": 0, "owned_by": "test"},
                        {"id": "model-a", "object": "model", "created": 0, "owned_by": "test"},
                    ],
                },
            )

        client = client_with(handler)
        output = io.StringIO()
        with patch("assistant.provider_control.build_openai_client", return_value=client):
            exit_code = await run_provider_control(environment("list_models"), output)

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            json.loads(output.getvalue()),
            {"ok": True, "model_ids": ["model-a", "model-b"]},
        )

    async def test_authentication_failure_never_exposes_provider_body_or_key(self) -> None:
        async def handler(_request: httpx.Request) -> httpx.Response:
            return httpx.Response(401, json={"error": "body-secret"})

        client = client_with(handler)
        output = io.StringIO()
        with patch("assistant.provider_control.build_openai_client", return_value=client):
            exit_code = await run_provider_control(
                environment("list_models", api_key="api-key-secret"), output
            )

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            json.loads(output.getvalue()),
            {"ok": False, "error": "authentication_rejected"},
        )
        self.assertNotIn("body-secret", output.getvalue())
        self.assertNotIn("api-key-secret", output.getvalue())

    async def test_model_test_requires_model_input_and_reports_compatibility(self) -> None:
        output = io.StringIO()
        exit_code = await run_provider_control(environment("test_model"), output)

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            json.loads(output.getvalue()),
            {"ok": False, "error": "invalid_control_input"},
        )


def client_with(handler):
    return build_openai_client(
        "https://provider.test/v1",
        "test-key",
        http_client=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
    )


def environment(action: str, api_key: str = "test-key") -> dict[str, str]:
    return {
        "OH_MY_DREAM_ASSISTANT_PROVIDER_ACTION": action,
        "OH_MY_DREAM_ASSISTANT_PROVIDER_BASE_URL": "https://provider.test/v1",
        "OH_MY_DREAM_ASSISTANT_PROVIDER_API_KEY": api_key,
    }


if __name__ == "__main__":
    unittest.main()
