"""OpenAI Agents SDK tools backed only by Rust protocol calls."""

from __future__ import annotations

import json
from collections.abc import Awaitable, Callable, Mapping, Sequence
from typing import Any, cast

from agents import FunctionTool
from agents.tool_context import ToolContext

from .protocol_v1 import FrameKind
from .protocol_v1_io import ProtocolChannel

REQUEST_APPLY_TOOL_ID = "assistant.workflow.request_apply@1"
PROPOSE_CHANGE_TOOL_ID = "assistant.workflow.propose_change@1"
ReviewCallback = Callable[[str], Awaitable[object]]


def build_protocol_tools(
    contracts: Sequence[Mapping[str, Any]],
    channel: ProtocolChannel,
    trusted_resume_result: object | None = None,
    review: ReviewCallback | None = None,
) -> list[FunctionTool]:
    """Build exactly the Rust-provided tools without provider-side authority."""
    resume_result = [trusted_resume_result]
    review_state: dict[str, object] = {}
    return [
        _build_tool(contract, channel, resume_result, review, review_state)
        for contract in contracts
    ]


def _build_tool(
    contract: Mapping[str, Any],
    channel: ProtocolChannel,
    resume_result: list[object | None],
    review: ReviewCallback | None,
    review_state: dict[str, object],
) -> FunctionTool:
    tool_id = cast(str, contract["tool_id"])

    async def invoke(context: ToolContext[Any], arguments_json: str) -> str:
        if tool_id == REQUEST_APPLY_TOOL_ID and resume_result[0] is not None:
            return json.dumps(resume_result[0], separators=(",", ":"))
        if tool_id == REQUEST_APPLY_TOOL_ID and review_state.get("verdict") != "Pass":
            raise ValueError("request_apply requires an exact passing Reviewer verdict")
        arguments = json.loads(arguments_json)
        if not isinstance(arguments, dict):
            raise ValueError("tool arguments must be an object")
        channel.write(
            FrameKind.TOOL_CALL,
            {"call_id": context.tool_call_id, "tool_id": tool_id, "arguments": arguments},
        )
        response = await channel.read()
        if response.kind is not FrameKind.TOOL_RESULT:
            raise ValueError("expected ToolResult")
        payload = response.payload
        if payload["call_id"] != context.tool_call_id or payload["tool_id"] != tool_id:
            raise ValueError("ToolResult correlation mismatch")
        result = payload["result"]
        if tool_id == PROPOSE_CHANGE_TOOL_ID and review is not None:
            change_id = result.get("change_id") if isinstance(result, dict) else None
            if not isinstance(change_id, str):
                raise ValueError("proposal result omitted change_id")
            review_result = await review(change_id)
            if isinstance(review_result, Mapping):
                review_state.update(review_result)
        return json.dumps(result, separators=(",", ":"))

    async def needs_approval(
        _context: object,
        _arguments: dict[str, Any],
        _call_id: str,
    ) -> bool:
        return review_state.get("verdict") == "Pass"

    return FunctionTool(
        name=tool_id,
        description=cast(str, contract["description"]),
        params_json_schema=cast(dict[str, Any], contract["input_schema"]),
        on_invoke_tool=invoke,
        strict_json_schema=False,
        needs_approval=(
            needs_approval
            if tool_id == REQUEST_APPLY_TOOL_ID and resume_result[0] is None
            else cast(bool, contract["requires_human_approval"])
        ),
    )
