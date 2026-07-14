from __future__ import annotations

import json
from types import SimpleNamespace

import pytest

from assistant.reviewer import ReviewerVerdict, attest_review_result, build_reviewer_tool


def result(*, candidate_id: str, evidence: list[str], verdict: str = "pass") -> object:
    return SimpleNamespace(
        agent_tool_invocation=SimpleNamespace(
            tool_arguments=json.dumps({"candidate_id": candidate_id})
        ),
        new_items=[
            SimpleNamespace(type="tool_call_output_item", output=output)
            for output in evidence
        ],
        final_output=ReviewerVerdict(
            verdict=verdict,
            summary="Candidate is coherent.",
            findings=[],
        ),
    )


def test_pass_is_attested_from_the_exact_rust_fetch() -> None:
    evidence = json.dumps({"candidate_id": "candidate-1", "digest": "sha256:abc"})

    review = attest_review_result(
        result(candidate_id="candidate-1", evidence=[evidence])
    )

    assert review.candidate_digest == "sha256:abc"
    assert review.evidence_hash.startswith("sha256:")
    assert review.verdict == "pass"


def test_main_agent_reviewer_tool_accepts_only_candidate_id() -> None:
    async def invoke(_request: object) -> object:
        raise AssertionError("schema construction must not invoke Rust")

    tool = build_reviewer_tool(
        [
            {
                "id": "workflow_candidate_get",
                "description": "read candidate",
                "input_schema": {
                    "type": "object",
                    "properties": {"candidate_id": {"type": "string"}},
                    "required": ["candidate_id"],
                    "additionalProperties": False,
                },
                "strict_json_schema": True,
                "needs_approval": False,
            }
        ],
        invoke,  # type: ignore[arg-type]
        None,
    )

    assert set(tool.params_json_schema["properties"]) == {"candidate_id"}
    assert tool.params_json_schema["additionalProperties"] is False


@pytest.mark.parametrize(
    "evidence",
    [
        [],
        [json.dumps({"candidate_id": "other", "digest": "sha256:abc"})],
        [
            json.dumps({"candidate_id": "candidate-1", "digest": "sha256:abc"}),
            json.dumps({"candidate_id": "candidate-1", "digest": "sha256:abc"}),
        ],
    ],
)
def test_pass_without_one_matching_fetch_is_rejected(evidence: list[str]) -> None:
    with pytest.raises(ValueError):
        attest_review_result(result(candidate_id="candidate-1", evidence=evidence))
