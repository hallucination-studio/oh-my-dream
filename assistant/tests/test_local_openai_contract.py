from __future__ import annotations

import io
import json

import pytest
from openai import APIStatusError, APITimeoutError, AsyncOpenAI

from assistant.openai_provider import (
    ProviderCompatibilityError,
    ProviderResponseError,
    build_openai_client,
    list_model_ids,
    probe_model_compatibility,
)
from assistant.protocol_v1_app import _run_production
from assistant.tests.openai_contract_server import LocalOpenAiContractServer


@pytest.mark.asyncio
async def test_local_server_exercises_models_responses_failures_and_runtime(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    with LocalOpenAiContractServer() as server:
        client = build_openai_client(f"{server.origin}/valid/v1", "test-key")
        try:
            assert await list_model_ids(client) == ["model-a", "model-b"]
            await probe_model_compatibility(client, "model-a")
            with pytest.raises(APIStatusError):
                await probe_model_compatibility(client, "missing-model")
            with pytest.raises(ProviderCompatibilityError):
                await probe_model_compatibility(client, "incompatible-model")
        finally:
            await client.close()

        rejected = build_openai_client(f"{server.origin}/valid/v1", "rejected-key")
        try:
            with pytest.raises(APIStatusError):
                await list_model_ids(rejected)
        finally:
            await rejected.close()

        malformed = build_openai_client(f"{server.origin}/malformed/v1", "test-key")
        try:
            with pytest.raises(ProviderResponseError):
                await list_model_ids(malformed)
        finally:
            await malformed.close()

        timeout = AsyncOpenAI(
            base_url=f"{server.origin}/timeout/v1",
            api_key="test-key",
            timeout=0.03,
            max_retries=0,
        )
        try:
            with pytest.raises(APITimeoutError):
                await timeout.models.list()
        finally:
            await timeout.close()

        monkeypatch.setenv("OMD_ASSISTANT_BASE_URL", f"{server.origin}/valid/v1")
        monkeypatch.setenv("OMD_ASSISTANT_MODEL", "model-a")
        monkeypatch.setenv("OMD_ASSISTANT_API_KEY", "runtime-secret")
        output = io.BytesIO()
        await _run_production(io.BytesIO(_start_line()), output)

    frames = [json.loads(line) for line in output.getvalue().splitlines()]
    assert frames[0]["kind"] == "InvocationAccepted"
    assert frames[-1]["kind"] == "InvocationCompleted"
    assert frames[-1]["payload"]["final_text"] == "Local Assistant response"
    assert "runtime-secret" not in output.getvalue().decode()


def _start_line() -> bytes:
    tool_ids = [
        "assistant.workspace.get_snapshot@1",
        "assistant.node_capability.list@1",
        "assistant.node_capability.describe@1",
        "assistant.production_plan.get@1",
        "assistant.production_plan.create@1",
        "assistant.production_plan.replace@1",
        "assistant.production_plan.update_item@1",
        "assistant.workflow.evaluate_mutation@1",
        "assistant.workflow.propose_change@1",
        "assistant.workflow.get_change@1",
        "assistant.workflow.request_apply@1",
    ]
    payload = {
        "start": {"kind": "UserMessage", "message": "Reply with text."},
        "trusted_context": {
            "project_id": "01000000-0000-4000-8000-000000000001",
            "session_id": "02000000-0000-4000-8000-000000000002",
            "workspace_snapshot": {},
        },
        "tool_contracts": [
            {
                "tool_id": tool_id,
                "description": "Exact Rust tool.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": False,
                },
                "output_schema": {},
                "effect": "HumanApprovalRequest"
                if tool_id == "assistant.workflow.request_apply@1"
                else "AuthoritativeRead",
                "requires_human_approval": tool_id == "assistant.workflow.request_apply@1",
            }
            for tool_id in tool_ids
        ],
        "budgets": {
            "maximum_frame_bytes": 8388608,
            "maximum_events": 512,
            "maximum_tool_calls": 64,
            "maximum_model_turns": 16,
            "maximum_direction_bytes": 16777216,
            "deadline_ms": 600000,
        },
    }
    return (
        json.dumps(
            {
                "protocol_version": 1,
                "invocation_id": "03000000-0000-4000-8000-000000000003",
                "direction_sequence": 1,
                "kind": "InvocationStart",
                "payload": payload,
            },
            separators=(",", ":"),
        ).encode()
        + b"\n"
    )
