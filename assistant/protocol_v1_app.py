"""OpenAI Agents SDK Runner over the strict Assistant protocol v1 channel."""

from __future__ import annotations

import asyncio
import json
import sys
from collections.abc import Mapping
from typing import Any, BinaryIO, cast

from agents import Agent, Model, Runner, RunState, SQLiteSession, Tool
from agents.stream_events import RawResponsesStreamEvent

from .protocol_v1 import TOOL_IDS, FrameKind, ProtocolError, ProtocolFrame
from .protocol_v1_io import ProtocolChannel
from .protocol_v1_reviewer import GET_CHANGE_TOOL_ID, review_workflow_change
from .protocol_v1_tools import build_protocol_tools
from .sdk_runtime import build_model_settings, build_run_config
from .system_prompt import build_system_prompt

AGENT_ID = "workflow_coauthor@1"
REVIEWER_AGENT_ID = "workflow_change_reviewer@1"
SDK_VERSION = "0.18.1"
CONTRACT_EPOCH = 1
MODEL_NAME = "gpt-5.4"
MAX_TURNS = 16


class ProtocolV1App:
    def __init__(
        self,
        reader: BinaryIO,
        writer: BinaryIO,
        *,
        model: Model | str | None = MODEL_NAME,
    ) -> None:
        self._channel = ProtocolChannel(reader, writer)
        self._model = model

    async def run_once(self) -> None:
        try:
            first = await self._channel.read()
            if first.kind is FrameKind.INVOCATION_START:
                await self._start(first)
            elif first.kind is FrameKind.CONTINUATION_RESUME:
                await self._resume(first)
            else:
                raise ProtocolError("first frame must start or resume an invocation")
        except Exception as error:
            self._fail(error)

    async def _start(self, frame: ProtocolFrame) -> None:
        payload = frame.payload
        context = _mapping(payload["trusted_context"], "trusted_context")
        contracts = _contracts(payload["tool_contracts"])
        self._channel.write(FrameKind.INVOCATION_ACCEPTED, {"agent_id": AGENT_ID})
        tools = self._tools(contracts)
        agent = self._agent(tools)
        session = SQLiteSession(cast(str, context["session_id"]))
        try:
            result = Runner.run_streamed(
                agent,
                _start_input(_mapping(payload["start"], "start")),
                session=session,
                run_config=build_run_config(),
                max_turns=MAX_TURNS,
            )
            await self._settle(result, contracts, cast(str, context["session_id"]))
        finally:
            session.close()

    async def _resume(self, frame: ProtocolFrame) -> None:
        payload = frame.payload
        envelope = _mapping(payload["envelope"], "envelope")
        state_json, contracts, session_id = _opaque_bundle(envelope)
        self._channel.write(FrameKind.INVOCATION_ACCEPTED, {"agent_id": AGENT_ID})
        tools = self._tools(contracts, payload["trusted_result"])
        agent = self._agent(tools)
        restored = await RunState.from_json(
            initial_agent=agent,
            state_json=state_json,
            context_override=None,
            strict_context=True,
        )
        interruptions = restored.get_interruptions()
        if len(interruptions) != 1:
            raise ProtocolError("resume requires exactly one pending approval")
        restored.approve(interruptions[0])
        session = SQLiteSession(session_id)
        try:
            result = Runner.run_streamed(
                agent,
                restored,
                session=session,
                run_config=build_run_config(),
                max_turns=MAX_TURNS,
            )
            await self._settle(result, contracts, session_id)
        finally:
            session.close()

    def _agent(self, tools: list[Any]) -> Agent[Any]:
        return Agent(
            name=AGENT_ID,
            instructions=build_system_prompt(),
            model=self._model,
            model_settings=build_model_settings(),
            tools=cast(list[Tool], tools),
        )

    def _tools(
        self,
        contracts: list[Mapping[str, Any]],
        trusted_resume_result: object | None = None,
    ) -> list[Any]:
        get_change = next(
            (contract for contract in contracts if contract.get("tool_id") == GET_CHANGE_TOOL_ID),
            None,
        )
        if get_change is None:
            raise ProtocolError("get-change contract is required")

        async def review(change_id: str) -> object:
            verdict = await review_workflow_change(
                change_id,
                get_change,
                self._channel,
                self._model,
            )
            return verdict.model_dump()

        return build_protocol_tools(
            contracts,
            self._channel,
            trusted_resume_result,
            review,
        )

    async def _settle(
        self,
        result: Any,
        contracts: list[Mapping[str, Any]],
        session_id: str,
    ) -> None:
        async for event in result.stream_events():
            self._emit_delta(event)
        if result.interruptions:
            if len(result.interruptions) != 1:
                raise ProtocolError("only one approval may be pending")
            envelope = _continuation_envelope(result, contracts, session_id)
            self._channel.write(
                FrameKind.CONTINUATION_ENVELOPE_READY,
                {"envelope": envelope},
            )
            self._channel.write(
                FrameKind.INVOCATION_COMPLETED,
                {"final_text": "Awaiting human approval"},
            )
            return
        final_output = result.final_output
        if not isinstance(final_output, str) or not final_output:
            raise ProtocolError("SDK final output must be bounded text")
        self._channel.write(FrameKind.INVOCATION_COMPLETED, {"final_text": final_output})

    def _emit_delta(self, event: object) -> None:
        if not isinstance(event, RawResponsesStreamEvent):
            return
        event_type = getattr(event.data, "type", None)
        text = getattr(event.data, "delta", None)
        if event_type == "response.output_text.delta" and isinstance(text, str) and text:
            self._channel.write(FrameKind.MODEL_OUTPUT_DELTA, {"text": text})

    def _fail(self, error: Exception) -> None:
        category = "ProtocolViolation" if isinstance(error, (ProtocolError, ValueError)) else "SdkFailure"
        try:
            self._channel.write(
                FrameKind.INVOCATION_FAILED,
                {"category": category, "safe_message": "Assistant invocation failed"},
            )
        except Exception:
            return


def _mapping(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ProtocolError(f"{label} must be an object")
    return cast(dict[str, Any], value)


def _contracts(value: object) -> list[Mapping[str, Any]]:
    if not isinstance(value, list) or len(value) != 11 or not all(isinstance(item, dict) for item in value):
        raise ProtocolError("exactly eleven tool contracts are required")
    contracts = cast(list[Mapping[str, Any]], value)
    expected_fields = {
        "tool_id",
        "description",
        "input_schema",
        "output_schema",
        "effect",
        "requires_human_approval",
    }
    if (
        {contract.get("tool_id") for contract in contracts} != TOOL_IDS
        or any(set(contract) != expected_fields for contract in contracts)
        or any(not isinstance(contract["description"], str) for contract in contracts)
        or any(not isinstance(contract["input_schema"], dict) for contract in contracts)
        or any(not isinstance(contract["output_schema"], dict) for contract in contracts)
        or any(
            contract["effect"]
            not in {
                "AuthoritativeRead",
                "AssistantStateMutation",
                "HumanApprovalRequest",
            }
            for contract in contracts
        )
        or any(
            contract["requires_human_approval"]
            != (contract["tool_id"] == "assistant.workflow.request_apply@1")
            for contract in contracts
        )
    ):
        raise ProtocolError("tool contracts do not match the closed catalog")
    return contracts


def _start_input(start: Mapping[str, Any]) -> str:
    if start.get("kind") == "UserMessage" and isinstance(start.get("message"), str):
        return cast(str, start["message"])
    if start.get("kind") == "RepairActivation":
        return json.dumps(
            {"repair_activation": start.get("exact_failed_run_facts")},
            separators=(",", ":"),
        )
    raise ProtocolError("invalid invocation start kind")


def _continuation_envelope(
    result: Any,
    contracts: list[Mapping[str, Any]],
    session_id: str,
) -> dict[str, object]:
    state = result.to_state().to_json(strict_context=True)
    if not isinstance(state, dict):
        raise ProtocolError("SDK continuation state must be an object")
    return {
        "protocol_version": 1,
        "contract_epoch": CONTRACT_EPOCH,
        "sdk_version": SDK_VERSION,
        "agent_id": AGENT_ID,
        "tool_ids": [contract["tool_id"] for contract in contracts],
        "opaque_state": json.dumps(
            {
                "sdk_state": state,
                "tool_contracts": contracts,
                "session_id": session_id,
            },
            separators=(",", ":"),
        ),
    }


def _opaque_bundle(
    envelope: Mapping[str, Any],
) -> tuple[dict[str, Any], list[Mapping[str, Any]], str]:
    if (
        envelope.get("protocol_version") != 1
        or envelope.get("contract_epoch") != CONTRACT_EPOCH
        or envelope.get("sdk_version") != SDK_VERSION
        or envelope.get("agent_id") != AGENT_ID
    ):
        raise ProtocolError("continuation metadata mismatch")
    bundle = _mapping(
        json.loads(cast(str, envelope["opaque_state"])),
        "opaque_state",
    )
    if set(bundle) != {"sdk_state", "tool_contracts", "session_id"}:
        raise ProtocolError("continuation state fields mismatch")
    contracts = _contracts(bundle["tool_contracts"])
    tool_ids = [contract["tool_id"] for contract in contracts]
    if tool_ids != envelope.get("tool_ids"):
        raise ProtocolError("continuation tool set mismatch")
    session_id = bundle["session_id"]
    if not isinstance(session_id, str) or not session_id:
        raise ProtocolError("continuation Session identity mismatch")
    return _mapping(bundle["sdk_state"], "sdk_state"), contracts, session_id


def main() -> None:
    asyncio.run(ProtocolV1App(sys.stdin.buffer, sys.stdout.buffer).run_once())


def run() -> None:
    """Production entrypoint shared by module and frozen executable."""
    main()


if __name__ == "__main__":
    run()
