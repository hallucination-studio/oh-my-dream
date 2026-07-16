"""Read-only workflow-change Reviewer for Assistant protocol version 1."""

from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any, Literal, Protocol, cast

from agents import Agent, Model, Runner
from pydantic import BaseModel, ConfigDict, Field

from .protocol_v1 import FrameKind, ProtocolError
from .protocol_v1_io import ProtocolChannel
from .protocol_v1_tools import build_protocol_tools
from .sdk_runtime import build_model_settings, build_run_config

REVIEWER_AGENT_ID = "workflow_change_reviewer@1"
GET_CHANGE_TOOL_ID = "assistant.workflow.get_change@1"


class ReviewerOutput(BaseModel):
    model_config = ConfigDict(extra="forbid")
    verdict: Literal["Pass", "Reject"]
    prose: str = Field(min_length=1, max_length=65536)


class ReviewerChannel(Protocol):
    def write(self, kind: FrameKind, payload: dict[str, object]) -> None:
        ...


async def review_workflow_change(
    change_id: str,
    get_change_contract: Mapping[str, Any],
    channel: ReviewerChannel,
    model: Model | str | None,
) -> ReviewerOutput:
    """Fetch exact Rust evidence once, emit an attested closed Reviewer verdict."""
    if get_change_contract.get("tool_id") != GET_CHANGE_TOOL_ID:
        raise ProtocolError("Reviewer requires the exact get-change tool")
    tool = build_protocol_tools([get_change_contract], cast(ProtocolChannel, channel))[0]
    reviewer: Agent[Any] = Agent(
        name=REVIEWER_AGENT_ID,
        instructions=(
            "Review exactly one immutable Workflow change. Call "
            "assistant.workflow.get_change@1 exactly once with the supplied change_id. "
            "Return Pass only when the change is coherent and ready; otherwise Reject. "
            "Never call a mutation, approval, apply, or Run tool."
        ),
        model=model,
        model_settings=build_model_settings(),
        tools=[tool],
        output_type=ReviewerOutput,
    )
    result = await Runner.run(
        reviewer,
        json.dumps({"change_id": change_id}, separators=(",", ":")),
        run_config=build_run_config(),
        max_turns=16,
    )
    evidence = _exact_evidence(result, change_id)
    verdict = result.final_output
    if not isinstance(verdict, ReviewerOutput):
        raise ProtocolError("Reviewer omitted its typed verdict")
    channel.write(
        FrameKind.REVIEWER_VERDICT,
        {
            "change_id": change_id,
            "mutation_digest": evidence["mutation_digest_hex"],
            "verdict": verdict.verdict,
            "prose": verdict.prose,
        },
    )
    return verdict


def _exact_evidence(result: Any, change_id: str) -> dict[str, Any]:
    outputs = [
        item.output
        for item in getattr(result, "new_items", [])
        if getattr(item, "type", None) == "tool_call_output_item"
        and isinstance(getattr(item, "output", None), str)
    ]
    if len(outputs) != 1:
        raise ProtocolError("Reviewer must fetch exact evidence once")
    evidence = json.loads(outputs[0])
    if (
        not isinstance(evidence, dict)
        or evidence.get("change_id") != change_id
        or not isinstance(evidence.get("mutation_digest_hex"), str)
    ):
        raise ProtocolError("Reviewer evidence mismatch")
    return cast(dict[str, Any], evidence)
