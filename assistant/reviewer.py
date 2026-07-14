"""Read-only Reviewer Agent with attested Rust candidate evidence."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any, Literal

from agents import Agent, FunctionTool, Model
from pydantic import BaseModel, ConfigDict, Field

from .sdk_runtime import SDK_MAX_TURNS, build_model_settings
from .tool_contract import ToolInvoker, build_function_tools


REVIEWER_VERSION = "workflow-reviewer-v1"


class ReviewRequest(BaseModel):
    """Only input the main Assistant may provide to the Reviewer."""

    model_config = ConfigDict(extra="forbid")
    candidate_id: str = Field(min_length=1, max_length=160)


class ReviewerVerdict(BaseModel):
    """Typed creative and structural review result."""

    model_config = ConfigDict(extra="forbid")
    verdict: Literal["pass", "reject"]
    summary: str = Field(min_length=1, max_length=4096)
    findings: list[str] = Field(max_length=32)


class AttestedReview(BaseModel):
    """Wrapper-authored result returned to the main Assistant."""

    model_config = ConfigDict(extra="forbid")
    candidate_id: str
    candidate_digest: str
    reviewer_version: str
    verdict: Literal["pass", "reject"]
    summary: str
    findings: list[str]
    evidence_hash: str


def build_reviewer_tool(
    operations: Sequence[Mapping[str, Any]],
    invoker: ToolInvoker,
    model: Model | str | None,
) -> FunctionTool:
    """Build an Agent-as-tool reviewer over the exact candidate read operation."""
    candidate_operation = next(
        (operation for operation in operations if operation.get("id") == "workflow_candidate_get"),
        None,
    )
    if candidate_operation is None:
        raise ValueError("workflow_candidate_get is required for Reviewer Agent")
    candidate_tool = build_function_tools([candidate_operation], invoker)[0]
    reviewer: Agent[Any] = Agent(
        name="workflow_reviewer",
        instructions=(
            "Review exactly the requested Workflow candidate. First call "
            "workflow_candidate_get with its candidate_id. Judge structural coherence, "
            "readiness blockers, and whether it honestly advances the requested production. "
            "Return reject with actionable findings or pass. Never invent candidate evidence."
        ),
        model=model,
        model_settings=build_model_settings(),
        tools=[candidate_tool],
        output_type=ReviewerVerdict,
    )

    async def extract(result: Any) -> str:
        return attest_review_result(result).model_dump_json()

    return reviewer.as_tool(
        tool_name="review_workflow_candidate",
        tool_description="Review one exact immutable Workflow candidate by candidate_id.",
        parameters=ReviewRequest,
        input_builder=lambda options: json.dumps(options["params"]),
        custom_output_extractor=extract,
        max_turns=SDK_MAX_TURNS,
    )


def attest_review_result(result: Any) -> AttestedReview:
    """Require one matching Rust fetch and derive evidence from its exact response."""
    invocation = getattr(result, "agent_tool_invocation", None)
    arguments = getattr(invocation, "tool_arguments", None)
    if not isinstance(arguments, str):
        raise ValueError("Reviewer result omitted structured invocation arguments")
    request = ReviewRequest.model_validate_json(arguments)
    outputs = [
        item.output
        for item in getattr(result, "new_items", [])
        if getattr(item, "type", None) == "tool_call_output_item"
        and isinstance(getattr(item, "output", None), str)
    ]
    if len(outputs) != 1:
        raise ValueError("Reviewer must perform exactly one Rust candidate fetch")
    evidence_json = outputs[0]
    evidence = json.loads(evidence_json)
    if not isinstance(evidence, dict):
        raise ValueError("candidate evidence must be a JSON object")
    candidate_id = evidence.get("candidate_id")
    digest = evidence.get("digest")
    if candidate_id != request.candidate_id or not isinstance(digest, str):
        raise ValueError("Reviewer evidence does not match the requested candidate")
    verdict = getattr(result, "final_output", None)
    if not isinstance(verdict, ReviewerVerdict):
        raise ValueError("Reviewer omitted its typed verdict")
    return AttestedReview(
        candidate_id=request.candidate_id,
        candidate_digest=digest,
        reviewer_version=REVIEWER_VERSION,
        verdict=verdict.verdict,
        summary=verdict.summary,
        findings=verdict.findings,
        evidence_hash=f"sha256:{hashlib.sha256(evidence_json.encode()).hexdigest()}",
    )
