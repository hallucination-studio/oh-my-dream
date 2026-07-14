from __future__ import annotations

import json

import pytest
from agents import Agent, Runner

from assistant.sdk_runtime import build_run_config, drain_stream, restore_run_state
from assistant.tests.sdk_runtime_fakes import ScriptedToolModel
from assistant.tests.strong_repair_fixture import REPAIR_STEPS, RepairEnvironment


@pytest.mark.asyncio
async def test_failure_activation_drives_reviewed_repair_and_second_approval() -> None:
    activation = {
        "kind": "workflow_run_failed",
        "project_id": "project",
        "session_id": "project:project",
        "run_id": "assistant-run-action-1",
        "workflow_revision": 1,
        "reason": "provider outage",
    }
    environment = RepairEnvironment()
    model = ScriptedToolModel(
        REPAIR_STEPS,
        required_previous_outputs={
            3: '"verdict": "reject"',
            5: '"verdict": "pass"',
        },
    )
    agent = Agent(name="Strong repairer", model=model, tools=environment.tools())

    paused = Runner.run_streamed(
        agent,
        json.dumps(activation),
        context={"project_id": "project", "session_id": "project:project"},
        max_turns=12,
        run_config=build_run_config(),
    )
    await drain_stream(paused)

    assert len(paused.interruptions) == 1
    assert environment.applied == []
    assert environment.runs == []
    state_json = json.loads(json.dumps(paused.to_state().to_json(strict_context=True)))
    restored = await restore_run_state(
        agent,
        state_json,
        context_override={"project_id": "project", "session_id": "project:project"},
    )
    restored.approve(restored.get_interruptions()[0])

    resumed = Runner.run_streamed(
        agent,
        restored,
        max_turns=12,
        run_config=build_run_config(),
    )
    await drain_stream(resumed)

    assert environment.applied == ["repair-pass"]
    assert environment.runs == ["assistant-run-action-2"]
    assert resumed.final_output == "production turn complete"
