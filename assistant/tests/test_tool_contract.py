from __future__ import annotations

import json
import os
from pathlib import Path
import socket
from typing import Any
import unittest
from unittest.mock import patch

from agents.tool_context import ToolContext

from assistant.tool_contract import ToolRequest, ToolResponse, build_function_tools


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_PATH = REPOSITORY_ROOT / "ui/src/__fixtures__/assistant_operations.json"


class RecordingInvoker:
    def __init__(self, output_json: str) -> None:
        self.output_json = output_json
        self.requests: list[ToolRequest] = []

    async def __call__(self, request: ToolRequest) -> ToolResponse:
        self.requests.append(request)
        return ToolResponse(call_id=request.call_id, output_json=self.output_json)


def load_operations() -> list[dict[str, Any]]:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    operations = fixture["operations"]
    if not isinstance(operations, list):
        raise TypeError("assistant operation fixture must contain an operations list")
    return operations


class FunctionToolContractTests(unittest.IsolatedAsyncioTestCase):
    async def test_builds_fixture_tools_and_preserves_opaque_json(self) -> None:
        operations = load_operations()
        arguments_by_id = {
            "workspace_get_snapshot": "{}",
            "workflow_apply_patch": (
                '{ "params": { "position": 2, "type": "image" }, '
                '"expected_revision": 7 }'
            ),
            "workflow_evaluate_patch": '{"expected_revision":7,"operations":[]}',
            "proposal_execute": '{\n  "proposal_id": "proposal-42"\n}',
            "capability_search": '{"query":"three-shot video","kinds":null}',
            "capability_describe": (
                '{"refs":[{"id":"ImageToVideo","version":"1.0"}]}'
            ),
        }
        output_by_id = {
            "workspace_get_snapshot": '{ "result" : "snapshot" }',
            "workflow_apply_patch": '{"result":"patched", "revision": 8}',
            "workflow_evaluate_patch": '{"changed":false,"readiness_blockers":[]}',
            "proposal_execute": '{\n "result": "started"\n}',
            "capability_search": '{"capabilities":[]}',
            "capability_describe": '{"capabilities":[]}',
        }
        invokers = {
            operation["id"]: RecordingInvoker(output_by_id[operation["id"]])
            for operation in operations
        }

        async def invoke(request: ToolRequest) -> ToolResponse:
            return await invokers[request.operation_id](request)

        with (
            patch.dict(os.environ, {}, clear=True),
            patch.object(
                socket.socket,
                "connect",
                side_effect=AssertionError("tool construction must not use the network"),
            ),
        ):
            tools = build_function_tools(operations, invoke)
            for operation, tool in zip(operations, tools, strict=True):
                operation_id = operation["id"]
                call_id = f"call-for-{operation_id}"
                arguments_json = arguments_by_id[operation_id]
                context = ToolContext(
                    context=None,
                    tool_name=operation_id,
                    tool_call_id=call_id,
                    tool_arguments=arguments_json,
                )

                output_json = await tool.on_invoke_tool(context, arguments_json)

                self.assertEqual(tool.name, operation_id)
                self.assertEqual(tool.description, operation["description"])
                self.assertEqual(tool.params_json_schema, operation["input_schema"])
                self.assertIs(
                    tool.strict_json_schema,
                    operation["strict_json_schema"],
                )
                self.assertIs(tool.needs_approval, operation["needs_approval"])
                self.assertEqual(output_json, output_by_id[operation_id])
                self.assertEqual(
                    invokers[operation_id].requests,
                    [
                        ToolRequest(
                            operation_id=operation_id,
                            call_id=call_id,
                            arguments_json=arguments_json,
                        )
                    ],
                )

        self.assertEqual(
            [tool.name for tool in tools],
            [operation["id"] for operation in operations],
        )

    async def test_rejects_transport_response_for_another_call(self) -> None:
        operation = load_operations()[0]

        async def invoke(_request: ToolRequest) -> ToolResponse:
            return ToolResponse(call_id="different-call", output_json='{"result":"unused"}')

        tool = build_function_tools([operation], invoke)[0]
        context = ToolContext(
            context=None,
            tool_name=operation["id"],
            tool_call_id="expected-call",
            tool_arguments="{}",
        )

        with self.assertRaisesRegex(ValueError, "different-call"):
            await tool.on_invoke_tool(
                context,
                "{}",
            )


if __name__ == "__main__":
    unittest.main()
