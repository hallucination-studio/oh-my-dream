"""Build SDK function tools from Rust-generated operation contracts."""

from __future__ import annotations

from collections.abc import Awaitable, Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any, TypeAlias, cast

from agents import FunctionTool
from agents.tool_context import ToolContext


@dataclass(frozen=True)
class ToolRequest:
    """Opaque tool invocation sent to the product transport."""

    operation_id: str
    call_id: str
    arguments_json: str


@dataclass(frozen=True)
class ToolResponse:
    """Opaque tool result returned by the product transport."""

    call_id: str
    output_json: str


ToolInvoker: TypeAlias = Callable[[ToolRequest], Awaitable[ToolResponse]]


def build_function_tools(
    operations: Sequence[Mapping[str, Any]],
    invoker: ToolInvoker,
) -> list[FunctionTool]:
    """Build SDK tools without translating generated contract fields."""
    return [_build_function_tool(operation, invoker) for operation in operations]


def _build_function_tool(
    operation: Mapping[str, Any],
    invoker: ToolInvoker,
) -> FunctionTool:
    operation_id = cast(str, operation["id"])

    async def invoke(context: ToolContext[Any], arguments: str) -> str:
        response = await invoker(
            ToolRequest(
                operation_id=operation_id,
                call_id=context.tool_call_id,
                arguments_json=arguments,
            )
        )
        if response.call_id != context.tool_call_id:
            raise ValueError(
                "tool response call_id mismatch: "
                f"expected {context.tool_call_id!r}, got {response.call_id!r}"
            )
        return response.output_json

    return FunctionTool(
        name=operation_id,
        description=cast(str, operation["description"]),
        params_json_schema=cast(dict[str, Any], operation["input_schema"]),
        on_invoke_tool=invoke,
        strict_json_schema=cast(bool, operation["strict_json_schema"]),
        needs_approval=cast(bool, operation["needs_approval"]),
    )
