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


class MultiStepToolModel(Model):
    """Request one tool step per model turn, then finish."""

    def __init__(self, steps: list[str]) -> None:
        self.steps = steps
        self.model_calls = 0
        self.requested_steps: list[str] = []

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
        self.model_calls += 1
        completed_steps = 0
        if isinstance(input, list):
            completed_steps = sum(
                (
                    item.get("type")
                    if isinstance(item, dict)
                    else getattr(item, "type", None)
                )
                == "function_call_output"
                for item in input
            )
        if completed_steps < len(self.steps):
            step = self.steps[completed_steps]
            self.requested_steps.append(step)
            output: list[Any] = [
                ResponseFunctionToolCall(
                    arguments=f'{{"value":"{step}"}}',
                    call_id=f"call-{completed_steps + 1}",
                    name="echo_value",
                    type="function_call",
                    status="completed",
                )
            ]
            response_id = f"response-tool-{completed_steps + 1}"
        else:
            output = [
                ResponseOutputMessage(
                    id="message-production-complete",
                    content=[
                        ResponseOutputText(
                            annotations=[],
                            text="production turn complete",
                            type="output_text",
                        )
                    ],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
            response_id = "response-production-complete"
        yield ResponseCompletedEvent(
            response=_response(output, response_id),
            sequence_number=0,
            type="response.completed",
        )


class ScriptedToolModel(MultiStepToolModel):
    """Request exact named tools with exact JSON arguments."""

    def __init__(
        self,
        steps: list[tuple[str, str]],
        required_previous_outputs: dict[int, str] | None = None,
        initial_completed_outputs: int = 0,
        capture_initial_completed_outputs: bool = False,
        track_requested_steps: bool = False,
    ) -> None:
        super().__init__([name for name, _arguments in steps])
        self.scripted_steps = steps
        self.required_previous_outputs = required_previous_outputs or {}
        self.initial_completed_outputs = initial_completed_outputs
        self.capture_initial_completed_outputs = capture_initial_completed_outputs
        self.captured_completed_outputs: int | None = None
        self.track_requested_steps = track_requested_steps

    async def stream_response(self, *args: Any, **kwargs: Any) -> AsyncIterator[ResponseStreamEvent]:
        input_value = args[1] if len(args) > 1 else kwargs["input"]
        self.model_calls += 1
        completed_steps = 0
        if isinstance(input_value, list):
            completed_steps = sum(
                (item.get("type") if isinstance(item, dict) else getattr(item, "type", None))
                == "function_call_output"
                for item in input_value
            )
        if self.capture_initial_completed_outputs and self.captured_completed_outputs is None:
            self.captured_completed_outputs = completed_steps
        offset = self.captured_completed_outputs or self.initial_completed_outputs
        completed_steps = max(0, completed_steps - offset)
        if self.track_requested_steps:
            completed_steps = len(self.requested_steps)
        required = self.required_previous_outputs.get(completed_steps)
        if required is not None:
            outputs = [
                item.get("output") if isinstance(item, dict) else getattr(item, "output", None)
                for item in input_value
                if (item.get("type") if isinstance(item, dict) else getattr(item, "type", None))
                == "function_call_output"
            ]
            if not outputs or required not in str(outputs[-1]):
                raise AssertionError(f"expected previous tool output containing {required!r}")
        if completed_steps < len(self.scripted_steps):
            name, arguments = self.scripted_steps[completed_steps]
            self.requested_steps.append(name)
            output: list[Any] = [
                ResponseFunctionToolCall(
                    arguments=arguments,
                    call_id=f"scripted-call-{completed_steps + 1}",
                    name=name,
                    type="function_call",
                    status="completed",
                )
            ]
            response_id = f"scripted-tool-{completed_steps + 1}"
        else:
            output = [
                ResponseOutputMessage(
                    id="scripted-complete",
                    content=[ResponseOutputText(annotations=[], text="production turn complete", type="output_text")],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
            response_id = "scripted-complete"
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
