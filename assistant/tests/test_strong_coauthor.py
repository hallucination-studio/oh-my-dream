from __future__ import annotations

import json
import sqlite3
from pathlib import Path
from typing import Any

import pytest
from agents import Agent, FunctionTool, Runner

from assistant.sdk_runtime import build_run_config, drain_stream
from assistant.tests.sdk_runtime_fakes import ScriptedToolModel


@pytest.mark.asyncio
async def test_one_runner_call_owns_plan_patch_correction_and_review(tmp_path: Path) -> None:
    database = tmp_path / "plan.sqlite3"
    connection = sqlite3.connect(database)
    connection.execute("CREATE TABLE plan_items (id TEXT PRIMARY KEY, status TEXT NOT NULL)")
    connection.executemany(
        "INSERT INTO plan_items VALUES (?, 'pending')", [("shot-1",), ("shot-2",)]
    )
    connection.commit()
    events: list[tuple[str, dict[str, Any]]] = []

    async def invoke(name: str, arguments: str) -> str:
        payload = json.loads(arguments)
        events.append((name, payload))
        if name == "production_plan_update":
            connection.execute(
                "UPDATE plan_items SET status = ? WHERE id = ?",
                (payload["status"], payload["item_id"]),
            )
            connection.commit()
        if name == "workflow_evaluate_patch" and payload["patch"] == "invalid":
            return json.dumps({"ok": False, "error": {"code": "PATCH_INVALID"}})
        if name == "candidate_review" and payload["candidate_id"] == "candidate-1":
            return json.dumps({"verdict": "reject", "findings": ["missing second shot"]})
        if name == "candidate_review":
            return json.dumps({"verdict": "pass", "findings": []})
        return json.dumps({"ok": True})

    tool_names = {
        "production_plan_get",
        "production_plan_update",
        "workflow_prepare_patch",
        "workflow_evaluate_patch",
        "candidate_review",
    }
    tools = [_tool(name, invoke) for name in sorted(tool_names)]
    steps = [
        ("production_plan_get", "{}"),
        ("workflow_prepare_patch", '{"candidate_id":"candidate-1","patch":"shot-1"}'),
        ("production_plan_update", '{"item_id":"shot-1","status":"done"}'),
        ("workflow_evaluate_patch", '{"patch":"invalid"}'),
        ("workflow_evaluate_patch", '{"patch":"corrected"}'),
        ("workflow_prepare_patch", '{"candidate_id":"candidate-1","patch":"shot-2"}'),
        ("production_plan_update", '{"item_id":"shot-2","status":"done"}'),
        ("candidate_review", '{"candidate_id":"candidate-1"}'),
        ("workflow_prepare_patch", '{"candidate_id":"candidate-2","patch":"review-fix"}'),
        ("candidate_review", '{"candidate_id":"candidate-2"}'),
    ]
    model = ScriptedToolModel(steps)
    agent = Agent(name="Strong coauthor", model=model, tools=tools)

    result = Runner.run_streamed(
        agent,
        "Build both shots and correct rejected work.",
        max_turns=16,
        run_config=build_run_config(),
    )
    await drain_stream(result)

    statuses = dict(connection.execute("SELECT id, status FROM plan_items"))
    assert statuses == {"shot-1": "done", "shot-2": "done"}
    assert [name for name, _payload in events] == [name for name, _args in steps]
    assert result.final_output == "production turn complete"
    assert model.model_calls == len(steps) + 1


def _tool(name: str, invoke: Any) -> FunctionTool:
    async def call(_context: Any, arguments: str) -> str:
        return await invoke(name, arguments)

    return FunctionTool(
        name=name,
        description=f"Test environment capability {name}.",
        params_json_schema={"type": "object", "additionalProperties": True},
        on_invoke_tool=call,
        strict_json_schema=False,
    )
