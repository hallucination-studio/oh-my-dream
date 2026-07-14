"""Deterministic Responses model for the M3 paragraph-to-Workflow proof."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator
from typing import Any

from agents import (
    AgentOutputSchemaBase,
    Handoff,
    Model,
    ModelResponse,
    ModelSettings,
    ModelTracing,
    Tool,
    TResponseInputItem,
)
from openai.types.responses import (
    Response,
    ResponseCompletedEvent,
    ResponseFunctionToolCall,
    ResponseOutputMessage,
    ResponseOutputText,
    ResponseStreamEvent,
)
from openai.types.responses.response_prompt_param import ResponsePromptParam


SHOT_PROMPTS = (
    "sunrise over a city",
    "a cyclist crossing a bridge",
    "coffee steaming by a window",
)


def _response(output: list[Any], response_id: str) -> Response:
    return Response(
        id=response_id,
        created_at=0.0,
        model="coauthor-fixture",
        object="response",
        output=output,
        parallel_tool_calls=False,
        tool_choice="auto",
        tools=[],
    )


def patch_arguments() -> str:
    """Return one atomic patch for the documented three-shot acceptance prompt."""
    operations: list[dict[str, Any]] = []
    for index, prompt in enumerate(SHOT_PROMPTS, start=1):
        operations.extend(
            [
                {
                    "op": "add_node",
                    "alias": f"prompt-{index}",
                    "capability": {"id": "TextPrompt", "version": "1.0"},
                    "params": {"text": prompt},
                    "position": [index * 360.0, 40.0],
                },
                {
                    "op": "add_node",
                    "alias": f"image-{index}",
                    "capability": {"id": "TextToImage", "version": "1.0"},
                    "params": {"model": "mock-image"},
                    "position": [index * 360.0, 180.0],
                },
                {
                    "op": "add_node",
                    "alias": f"video-{index}",
                    "capability": {"id": "ImageToVideo", "version": "1.0"},
                    "params": {"model": "mock-video", "duration": 4.0},
                    "position": [index * 360.0, 320.0],
                },
            ]
        )
        operations.extend(
            [
                {
                    "op": "set_input",
                    "node": {"kind": "alias", "alias": f"image-{index}"},
                    "input": "prompt",
                    "binding": {
                        "kind": "single",
                        "source": {"kind": "alias", "alias": f"prompt-{index}"},
                    },
                },
                {
                    "op": "set_input",
                    "node": {"kind": "alias", "alias": f"video-{index}"},
                    "input": "image",
                    "binding": {
                        "kind": "single",
                        "source": {"kind": "alias", "alias": f"image-{index}"},
                    },
                },
            ]
        )
    operations.extend(
        [
            {
                "op": "add_node",
                "alias": "concat",
                "capability": {"id": "VideoConcat", "version": "1.0"},
                "params": {},
                "position": [720.0, 500.0],
            },
            {
                "op": "set_input",
                "node": {"kind": "alias", "alias": "concat"},
                "input": "clips",
                "binding": {
                    "kind": "ordered_many",
                    "sources": [
                        {"kind": "alias", "alias": "video-1"},
                        {"kind": "alias", "alias": "video-2"},
                        {"kind": "alias", "alias": "video-3"},
                    ],
                },
            },
        ]
    )
    return json.dumps(
        {"expected_revision": None, "operations": operations},
        separators=(",", ":"),
    )


COAUTHOR_CALLS = (
    ("workspace_get_snapshot", "{}", "snapshot-call"),
    ("capability_search", '{"query":"video","kinds":null}', "search-video-call"),
    (
        "capability_search",
        '{"query":"prompt","kinds":null}',
        "search-prompt-call",
    ),
    (
        "capability_describe",
        '{"refs":[{"id":"ImageToVideo","version":"1.0"},{"id":"VideoConcat","version":"1.0"},{"id":"TextToImage","version":"1.0"}]}',
        "describe-video-call",
    ),
    (
        "capability_describe",
        '{"refs":[{"id":"TextPrompt","version":"1.0"}]}',
        "describe-prompt-call",
    ),
    ("workflow_apply_patch", patch_arguments(), "patch-call"),
)


class CoauthorModel(Model):
    """Follow the fixed M3 discovery protocol, then apply one Workflow patch."""

    def __init__(self) -> None:
        self.inputs: list[str | list[TResponseInputItem]] = []
        self.system_instructions: list[str | None] = []
        self.tool_names: list[str] = []

    async def get_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> ModelResponse:
        raise AssertionError("streamed runs must use stream_response")

    async def stream_response(
        self,
        system_instructions: str | None,
        input: str | list[TResponseInputItem],
        model_settings: ModelSettings,
        tools: list[Tool],
        output_schema: AgentOutputSchemaBase | None,
        handoffs: list[Handoff],
        tracing: ModelTracing,
        *,
        previous_response_id: str | None,
        conversation_id: str | None,
        prompt: ResponsePromptParam | None,
    ) -> AsyncIterator[ResponseStreamEvent]:
        self.inputs.append(input)
        self.system_instructions.append(system_instructions)
        self.tool_names = [tool.name for tool in tools]
        completed_calls = 0
        if isinstance(input, list):
            completed_calls = sum(
                (
                    item.get("type")
                    if isinstance(item, dict)
                    else getattr(item, "type", None)
                )
                == "function_call_output"
                for item in input
            )
        if completed_calls < len(COAUTHOR_CALLS):
            operation_id, arguments_json, call_id = COAUTHOR_CALLS[completed_calls]
            output: list[Any] = [
                ResponseFunctionToolCall(
                    arguments=arguments_json,
                    call_id=call_id,
                    name=operation_id,
                    type="function_call",
                    status="completed",
                )
            ]
            response_id = f"response-coauthor-{completed_calls + 1}"
        else:
            output = [
                ResponseOutputMessage(
                    id="message-coauthor-final",
                    content=[
                        ResponseOutputText(
                            annotations=[],
                            text="Workflow created.",
                            type="output_text",
                        )
                    ],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
            response_id = "response-coauthor-final"
        yield ResponseCompletedEvent(
            response=_response(output, response_id),
            sequence_number=0,
            type="response.completed",
        )
