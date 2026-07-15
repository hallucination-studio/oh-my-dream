from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from agents import Agent, FunctionTool, Runner

from assistant.sdk_runtime import build_file_session, build_run_config, drain_stream, restore_run_state
from assistant.tests.sdk_runtime_fakes import ScriptedToolModel


@pytest.mark.asyncio
async def test_strong_mvp_e2e_uses_two_sdk_turns_and_two_exact_approvals(tmp_path: Path) -> None:
    environment = StrongMvpEnvironment()
    session = build_file_session("project:project", tmp_path / "strong-mvp.sqlite3")
    initial_steps = [
        ("production_plan_create", '{"items":["shot-1","shot-2"]}'),
        ("production_plan_update_item", '{"item_id":"shot-1","action":"complete"}'),
        ("workflow_evaluate_patch", '{"patch":"invalid"}'),
        ("workflow_evaluate_patch", '{"patch":"corrected"}'),
        ("workflow_prepare_patch", '{"candidate_id":"build-1","patch":"two shots"}'),
        ("production_plan_update_item", '{"item_id":"shot-2","action":"complete"}'),
        ("candidate_review", '{"candidate_id":"build-1"}'),
        ("workflow_prepare_patch", '{"candidate_id":"build-2","patch":"review revision"}'),
        ("candidate_review", '{"candidate_id":"build-2"}'),
        ("workflow_apply_reviewed_candidate", '{"review_receipt_id":"build-pass"}'),
        ("workflow_run", '{"run_id":"assistant-run-build"}'),
    ]
    initial_model = ScriptedToolModel(
        initial_steps,
        required_previous_outputs={3: "PATCH_INVALID", 7: '"verdict": "reject"'},
    )
    initial_agent = Agent(name="Strong coauthor", model=initial_model, tools=environment.tools())
    first = Runner.run_streamed(
        initial_agent,
        "Build the complete two-shot production.",
        session=session,
        max_turns=16,
        run_config=build_run_config(),
    )
    await drain_stream(first)
    assert len(first.interruptions) == 1
    assert environment.applied == []
    first_resumed = await approve_and_resume(initial_agent, first, session)
    assert first_resumed.final_output == "production turn complete"
    assert environment.applied == ["build-pass"]
    assert environment.activations == ["provider outage"]

    repair_steps = [
        ("workspace_snapshot", "{}"),
        ("workflow_prepare_patch", '{"candidate_id":"repair-1","patch":"first repair"}'),
        ("candidate_review", '{"candidate_id":"repair-1"}'),
        ("workflow_prepare_patch", '{"candidate_id":"repair-2","patch":"revised repair"}'),
        ("candidate_review", '{"candidate_id":"repair-2"}'),
        ("workflow_apply_reviewed_candidate", '{"review_receipt_id":"repair-pass"}'),
        ("workflow_run", '{"run_id":"assistant-run-repair"}'),
    ]
    repair_model = ScriptedToolModel(
        repair_steps,
        required_previous_outputs={3: '"verdict": "reject"', 5: '"verdict": "pass"'},
        track_requested_steps=True,
    )
    repair_agent = Agent(name="Strong repairer", model=repair_model, tools=environment.tools())
    repair = Runner.run_streamed(
        repair_agent,
        json.dumps({"kind": "workflow_run_failed", "reason": environment.activations[-1]}),
        session=session,
        max_turns=12,
        run_config=build_run_config(),
    )
    await drain_stream(repair)
    assert len(repair.interruptions) == 1
    repaired = await approve_and_resume(repair_agent, repair, session)
    session.close()

    assert repaired.final_output == "production turn complete"
    assert environment.applied == ["build-pass", "repair-pass"]
    assert environment.runs == ["assistant-run-build", "assistant-run-repair"]
    assert environment.succeeded


async def approve_and_resume(agent: Agent[Any], paused: Any, session: Any) -> Any:
    state = json.loads(json.dumps(paused.to_state().to_json(strict_context=True)))
    restored = await restore_run_state(agent, state, context_override={})
    restored.approve(restored.get_interruptions()[0])
    resumed = Runner.run_streamed(agent, restored, session=session, run_config=build_run_config())
    await drain_stream(resumed)
    return resumed


class StrongMvpEnvironment:
    def __init__(self) -> None:
        self.applied: list[str] = []
        self.runs: list[str] = []
        self.activations: list[str] = []
        self.succeeded = False

    def tools(self) -> list[FunctionTool]:
        names = {
            "production_plan_create", "production_plan_update_item", "workspace_snapshot",
            "workflow_evaluate_patch", "workflow_prepare_patch", "candidate_review",
            "workflow_apply_reviewed_candidate", "workflow_run",
        }
        return [self.tool(name) for name in sorted(names)]

    def tool(self, name: str) -> FunctionTool:
        async def call(_context: Any, arguments: str) -> str:
            payload = json.loads(arguments)
            if name == "workflow_evaluate_patch" and payload["patch"] == "invalid":
                return json.dumps({"ok": False, "error": {"code": "PATCH_INVALID"}})
            if name == "candidate_review" and payload["candidate_id"] in {"build-1", "repair-1"}:
                return json.dumps({"verdict": "reject", "findings": ["revision required"]})
            if name == "candidate_review":
                return json.dumps({"verdict": "pass", "findings": []})
            if name == "workflow_apply_reviewed_candidate":
                self.applied.append(payload["review_receipt_id"])
            if name == "workflow_run":
                self.runs.append(payload["run_id"])
                if payload["run_id"] == "assistant-run-build":
                    self.activations.append("provider outage")
                    return json.dumps({"status": "failed", "reason": "provider outage"})
                self.succeeded = True
                return json.dumps({"status": "succeeded"})
            return json.dumps({"ok": True})

        return FunctionTool(
            name=name,
            description=f"Strong MVP environment capability {name}.",
            params_json_schema={"type": "object", "additionalProperties": True},
            on_invoke_tool=call,
            needs_approval=name == "workflow_apply_reviewed_candidate",
            strict_json_schema=False,
        )
