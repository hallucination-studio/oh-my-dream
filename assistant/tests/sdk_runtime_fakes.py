"""Deterministic public-SDK test models for runtime characterization."""

from __future__ import annotations

from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Any

from agents import (
    Agent,
    AgentOutputSchemaBase,
    FunctionTool,
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


@dataclass(frozen=True)
class NonMappingContext:
    project_id: str


def build_echo_agent(
    *,
    name: str,
    model: Model,
    on_invoke_tool: Any,
    needs_approval: bool = False,
) -> tuple[Agent[Any], FunctionTool]:
    tool = FunctionTool(
        name="echo_value",
        description="Echo one value.",
        params_json_schema={
            "type": "object",
            "properties": {"value": {"type": "string"}},
            "required": ["value"],
            "additionalProperties": False,
        },
        on_invoke_tool=on_invoke_tool,
        needs_approval=needs_approval,
    )
    return Agent(name=name, model=model, tools=[tool]), tool


def _response(output: list[Any], response_id: str) -> Response:
    return Response(
        id=response_id,
        created_at=0.0,
        model="deterministic-fake",
        object="response",
        output=output,
        parallel_tool_calls=False,
        tool_choice="auto",
        tools=[],
    )


class DeterministicToolModel(Model):
    def __init__(self) -> None:
        self.inputs: list[str | list[TResponseInputItem]] = []

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
        has_tool_output = isinstance(input, list) and any(
            (
                item.get("type") if isinstance(item, dict) else getattr(item, "type", None)
            )
            == "function_call_output"
            for item in input
        )
        output: list[Any]
        if has_tool_output:
            output = [
                ResponseOutputMessage(
                    id="message-1",
                    content=[
                        ResponseOutputText(
                            annotations=[],
                            text="tool completed",
                            type="output_text",
                        )
                    ],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
            response_id = "response-final"
        else:
            output = [
                ResponseFunctionToolCall(
                    arguments='{"value":"canvas"}',
                    call_id="call-1",
                    name="echo_value",
                    type="function_call",
                    status="completed",
                )
            ]
            response_id = "response-tool-call"
        yield ResponseCompletedEvent(
            response=_response(output, response_id),
            sequence_number=0,
            type="response.completed",
        )


class RecordingFinalModel(Model):
    def __init__(self) -> None:
        self.inputs: list[str | list[TResponseInputItem]] = []

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
        message = ResponseOutputMessage(
            id="message-recorded",
            content=[ResponseOutputText(annotations=[], text="recorded", type="output_text")],
            role="assistant",
            status="completed",
            type="message",
        )
        yield ResponseCompletedEvent(
            response=_response([message], "response-recorded"),
            sequence_number=0,
            type="response.completed",
        )
