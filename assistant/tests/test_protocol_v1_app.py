from __future__ import annotations

import io
import json
from collections.abc import AsyncIterator
from types import SimpleNamespace
from typing import Any, cast

import pytest
from agents import (
    Agent,
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

from assistant.protocol_v1_app import (
    ContinuationIncompatibleError,
    ProtocolV1App,
    _opaque_bundle,
    _route_fingerprint,
    _run_production,
)
from assistant.protocol_v1 import FrameKind
from assistant.protocol_v1_reviewer import ReviewerOutput, review_workflow_change

INVOCATION_ID = "03000000-0000-4000-8000-000000000003"
TOOL_IDS = [
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


@pytest.mark.asyncio
async def test_production_runtime_uses_configured_responses_model(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, object] = {}

    class ClientFake:
        def __init__(self, **kwargs: object) -> None:
            captured["client_kwargs"] = kwargs

        async def __aenter__(self) -> ClientFake:
            captured["entered"] = True
            return self

        async def __aexit__(self, *_args: object) -> None:
            captured["closed"] = True

    class ResponsesModelFake:
        def __init__(self, model: str, openai_client: object) -> None:
            captured["model_id"] = model
            captured["client"] = openai_client

    async def run_once_fake(self: ProtocolV1App) -> None:
        captured["app_model"] = self._model
        captured["route_fingerprint"] = self._route_fingerprint

    monkeypatch.setenv("OMD_ASSISTANT_BASE_URL", "http://localhost:11434/v1")
    monkeypatch.setenv("OMD_ASSISTANT_MODEL", "local-text-model")
    monkeypatch.setenv("OMD_ASSISTANT_API_KEY", "secret-value")
    monkeypatch.setattr("assistant.protocol_v1_app.AsyncOpenAI", ClientFake)
    monkeypatch.setattr("assistant.protocol_v1_app.OpenAIResponsesModel", ResponsesModelFake)
    monkeypatch.setattr(ProtocolV1App, "run_once", run_once_fake)

    await _run_production(io.BytesIO(), io.BytesIO())

    assert captured["client_kwargs"] == {
        "base_url": "http://localhost:11434/v1",
        "api_key": "secret-value",
    }
    assert captured["model_id"] == "local-text-model"
    assert captured["client"] is not None
    assert captured["app_model"] is not None
    assert captured["route_fingerprint"] == _route_fingerprint(
        "http://localhost:11434/v1",
        "local-text-model",
    )
    assert captured["entered"] is True
    assert captured["closed"] is True


def test_route_fingerprint_excludes_api_key_and_binds_endpoint_and_model() -> None:
    original = _route_fingerprint("http://localhost:11434/v1", "text-model")

    assert original == _route_fingerprint("http://localhost:11434/v1", "text-model")
    assert original != _route_fingerprint("http://localhost:11435/v1", "text-model")
    assert original != _route_fingerprint("http://localhost:11434/v1", "other-model")


def test_continuation_rejects_a_changed_model_route() -> None:
    envelope = {
        "protocol_version": 1,
        "contract_epoch": 2,
        "sdk_version": "0.18.1",
        "agent_id": "workflow_coauthor@1",
        "tool_ids": TOOL_IDS,
        "route_fingerprint": _route_fingerprint("http://localhost:11434/v1", "text-model"),
        "opaque_state": json.dumps(
            {
                "sdk_state": {},
                "tool_contracts": contracts(),
                "session_id": "session-1",
            },
            separators=(",", ":"),
        ),
    }

    with pytest.raises(ContinuationIncompatibleError, match="continuation metadata mismatch"):
        _opaque_bundle(
            envelope,
            _route_fingerprint("http://localhost:11434/v1", "other-model"),
        )


class OneToolModel(Model):
    def __init__(self, tool_id: str) -> None:
        self.tool_id = tool_id

    async def get_response(self, *args: Any, **kwargs: Any) -> ModelResponse:
        raise AssertionError("streaming is required")

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
        if self._complete(input):
            output: list[Any] = [
                ResponseOutputMessage(
                    id="message-1",
                    content=[
                        ResponseOutputText(
                            annotations=[],
                            text="done",
                            type="output_text",
                        )
                    ],
                    role="assistant",
                    status="completed",
                    type="message",
                )
            ]
        else:
            output = [
                ResponseFunctionToolCall(
                    arguments="{}",
                    call_id="call-1",
                    name=self.tool_id,
                    type="function_call",
                    status="completed",
                )
            ]
        response = Response(
            id="response-1",
            created_at=0.0,
            model="deterministic",
            object="response",
            output=output,
            parallel_tool_calls=False,
            tool_choice="auto",
            tools=[],
        )
        yield ResponseCompletedEvent(
            response=response,
            sequence_number=0,
            type="response.completed",
        )

    def _complete(self, input: str | list[TResponseInputItem]) -> bool:
        return isinstance(input, list) and any(
            (item.get("type") if isinstance(item, dict) else getattr(item, "type", None))
            == "function_call_output"
            for item in input
        )


class ProposalApprovalModel(OneToolModel):
    def __init__(self, tool_id: str) -> None:
        super().__init__(tool_id)
        self._approval_requested = False
        self._complete_now = False

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
        output_count = self._output_count(input)
        self._complete_now = self._approval_requested and output_count > 0
        if output_count == 0:
            self.tool_id = TOOL_IDS[-3]
        elif not self._complete_now:
            self.tool_id = TOOL_IDS[-1]
            self._approval_requested = True
        async for event in super().stream_response(
            system_instructions,
            input,
            model_settings,
            tools,
            output_schema,
            handoffs,
            tracing,
            previous_response_id=previous_response_id,
            conversation_id=conversation_id,
            prompt=prompt,
        ):
            yield event

    def _complete(self, input: str | list[TResponseInputItem]) -> bool:
        return self._complete_now

    @staticmethod
    def _output_count(input: str | list[TResponseInputItem]) -> int:
        return (
            sum(
                1
                for item in input
                if (item.get("type") if isinstance(item, dict) else getattr(item, "type", None))
                == "function_call_output"
            )
            if isinstance(input, list)
            else 0
        )


@pytest.mark.asyncio
async def test_sdk_runner_round_trips_one_exact_tool_and_completes() -> None:
    input_stream = io.BytesIO(
        start_line()
        + rust_line(
            2,
            "ToolResult",
            {
                "call_id": "call-1",
                "tool_id": TOOL_IDS[0],
                "result": {"snapshot": {}},
            },
        )
    )
    output = io.BytesIO()

    await ProtocolV1App(input_stream, output, model=OneToolModel(TOOL_IDS[0])).run_once()

    frames = output_frames(output)
    assert [frame["kind"] for frame in frames] == [
        "InvocationAccepted",
        "ToolCall",
        "InvocationCompleted",
    ]
    assert frames[1]["payload"]["tool_id"] == TOOL_IDS[0]
    assert frames[2]["payload"]["final_text"] == "done"


@pytest.mark.asyncio
async def test_approval_resume_uses_trusted_result_without_replaying_tool_call(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def passing_review(*_args: object, **_kwargs: object) -> ReviewerOutput:
        return ReviewerOutput(verdict="Pass", prose="Coherent.")

    monkeypatch.setattr(
        "assistant.protocol_v1_app.review_workflow_change",
        passing_review,
    )
    model = ProposalApprovalModel(TOOL_IDS[-3])
    paused_output = io.BytesIO()
    proposed = rust_line(
        2,
        "ToolResult",
        {
            "call_id": "call-1",
            "tool_id": TOOL_IDS[-3],
            "result": {
                "change_id": INVOCATION_ID,
                "mutation_digest_hex": "a" * 64,
            },
        },
    )
    await ProtocolV1App(io.BytesIO(start_line() + proposed), paused_output, model=model).run_once()
    paused = output_frames(paused_output)
    envelope = next(
        frame["payload"]["envelope"]
        for frame in paused
        if frame["kind"] == "ContinuationEnvelopeReady"
    )

    resumed_output = io.BytesIO()
    resume = rust_line(
        1,
        "ContinuationResume",
        {"envelope": envelope, "trusted_result": {"applied": True}},
    )
    await ProtocolV1App(io.BytesIO(resume), resumed_output, model=model).run_once()

    resumed = output_frames(resumed_output)
    assert resumed[0]["kind"] == "InvocationAccepted"
    assert resumed[-1]["kind"] == "InvocationCompleted"
    assert all(frame["kind"] != "ToolCall" for frame in resumed)


@pytest.mark.asyncio
async def test_reviewer_has_only_exact_read_tool_and_emits_attested_verdict(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, object] = {}

    async def fake_run(agent: object, *_args: object, **_kwargs: object) -> object:
        captured["agent"] = agent
        return SimpleNamespace(
            new_items=[
                SimpleNamespace(
                    type="tool_call_output_item",
                    output=json.dumps(
                        {
                            "change_id": INVOCATION_ID,
                            "mutation_digest_hex": "a" * 64,
                        }
                    ),
                )
            ],
            final_output=ReviewerOutput(verdict="Pass", prose="Coherent."),
        )

    monkeypatch.setattr("assistant.protocol_v1_reviewer.Runner.run", fake_run)
    channel = RecordingChannel()
    get_change = next(
        contract for contract in contracts() if contract["tool_id"] == TOOL_IDS[-2]
    )

    await review_workflow_change(INVOCATION_ID, get_change, channel, None)

    agent = cast(Agent[Any], captured["agent"])
    assert [tool.name for tool in agent.tools] == [TOOL_IDS[-2]]
    assert channel.writes == [
        (
            "ReviewerVerdict",
            {
                "change_id": INVOCATION_ID,
                "mutation_digest": "a" * 64,
                "verdict": "Pass",
                "prose": "Coherent.",
            },
        )
    ]


def start_line() -> bytes:
    return rust_line(
        1,
        "InvocationStart",
        {
            "start": {"kind": "UserMessage", "message": "Create a scene"},
            "trusted_context": {
                "project_id": "01000000-0000-4000-8000-000000000001",
                "session_id": "02000000-0000-4000-8000-000000000002",
                "workspace_snapshot": {},
            },
            "tool_contracts": contracts(),
            "budgets": {
                "maximum_frame_bytes": 8388608,
                "maximum_events": 512,
                "maximum_tool_calls": 64,
                "maximum_model_turns": 16,
                "maximum_direction_bytes": 16777216,
                "deadline_ms": 600000,
            },
        },
    )


def contracts() -> list[dict[str, object]]:
    return [
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
            "effect": "HumanApprovalRequest" if tool_id == TOOL_IDS[-1] else "AuthoritativeRead",
            "requires_human_approval": tool_id == TOOL_IDS[-1],
        }
        for tool_id in TOOL_IDS
    ]


def rust_line(sequence: int, kind: str, payload: dict[str, object]) -> bytes:
    return (
        json.dumps(
            {
                "protocol_version": 1,
                "invocation_id": INVOCATION_ID,
                "direction_sequence": sequence,
                "kind": kind,
                "payload": payload,
            },
            separators=(",", ":"),
        ).encode()
        + b"\n"
    )


def output_frames(stream: io.BytesIO) -> list[dict[str, Any]]:
    return [json.loads(line) for line in stream.getvalue().splitlines()]


class RecordingChannel:
    def __init__(self) -> None:
        self.writes: list[tuple[str, dict[str, object]]] = []

    def write(self, kind: FrameKind, payload: dict[str, object]) -> None:
        self.writes.append((kind.value, payload))
