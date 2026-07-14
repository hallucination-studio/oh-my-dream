from __future__ import annotations

from typing import Any

import pytest

from assistant.tool_contract import ToolResponse, build_function_tools


def operation() -> dict[str, Any]:
    return {
        "id": "workflow_apply_reviewed_candidate",
        "version": 1,
        "description": "apply reviewed candidate",
        "input_schema": {
            "type": "object",
            "properties": {"review_receipt_id": {"type": "string"}},
            "required": ["review_receipt_id"],
            "additionalProperties": False,
        },
        "output_schema": {"type": "object"},
        "strict_json_schema": True,
        "needs_approval": True,
    }


@pytest.mark.asyncio
async def test_production_resolver_controls_sdk_interruption() -> None:
    async def invoke(request: Any) -> ToolResponse:
        return ToolResponse(call_id=request.call_id, output_json="{}")

    async def resolve(operation_id: str, arguments: dict[str, Any]) -> bool:
        return operation_id == "workflow_apply_reviewed_candidate" and arguments.get(
            "review_receipt_id"
        ) == "passed"

    tool = build_function_tools(
        [operation()], invoke, {"workflow_apply_reviewed_candidate": resolve}
    )[0]
    assert callable(tool.needs_approval)
    assert await tool.needs_approval(None, {"review_receipt_id": "passed"}, "call")
    assert not await tool.needs_approval(None, {"review_receipt_id": "forged"}, "call")


def test_default_builder_preserves_frozen_static_boolean() -> None:
    async def invoke(request: Any) -> ToolResponse:
        return ToolResponse(call_id=request.call_id, output_json="{}")

    tool = build_function_tools([operation()], invoke)[0]

    assert tool.needs_approval is True
