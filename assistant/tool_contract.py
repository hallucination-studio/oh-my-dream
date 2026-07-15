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
ApprovalResolver: TypeAlias = Callable[[str, dict[str, Any]], Awaitable[bool]]


def build_function_tools(
    operations: Sequence[Mapping[str, Any]],
    invoker: ToolInvoker,
    approval_resolvers: Mapping[str, ApprovalResolver] | None = None,
) -> list[FunctionTool]:
    """Build SDK tools without translating generated contract fields."""
    return [
        _build_function_tool(operation, invoker, approval_resolvers)
        for operation in operations
    ]


def _build_function_tool(
    operation: Mapping[str, Any],
    invoker: ToolInvoker,
    approval_resolvers: Mapping[str, ApprovalResolver] | None,
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

    needs_approval: bool | Callable[..., Awaitable[bool]] = cast(
        bool, operation["needs_approval"]
    )
    approval_resolver = (
        approval_resolvers.get(operation_id) if approval_resolvers is not None else None
    )
    if needs_approval and approval_resolver is not None:
        async def resolve(_context: Any, arguments: dict[str, Any], _call_id: str) -> bool:
            return await approval_resolver(operation_id, arguments)

        needs_approval = resolve

    return FunctionTool(
        name=operation_id,
        description=cast(str, operation["description"]),
        params_json_schema=cast(dict[str, Any], operation["input_schema"]),
        on_invoke_tool=invoke,
        strict_json_schema=cast(bool, operation["strict_json_schema"]),
        needs_approval=needs_approval,
    )
