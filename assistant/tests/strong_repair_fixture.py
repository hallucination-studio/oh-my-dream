from __future__ import annotations

import json
from typing import Any

from agents import FunctionTool


class RepairEnvironment:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []
        self.applied: list[str] = []
        self.runs: list[str] = []

    async def invoke(self, name: str, arguments: str) -> str:
        payload = json.loads(arguments)
        self.calls.append((name, payload))
        if name == "candidate_review" and payload["candidate_id"] == "repair-1":
            return json.dumps({"verdict": "reject", "findings": ["repair is incomplete"]})
        if name == "candidate_review":
            return json.dumps({"verdict": "pass", "findings": []})
        if name == "workflow_apply_reviewed_candidate":
            self.applied.append(payload["review_receipt_id"])
            return json.dumps({"workflow_head": {"revision": 2}, "deduplicated": False})
        if name == "workflow_run":
            self.runs.append(payload["run_id"])
            return json.dumps({"status": "succeeded", "run_id": payload["run_id"]})
        return json.dumps({"ok": True})

    def tools(self) -> list[FunctionTool]:
        names = {
            "workspace_snapshot",
            "workflow_prepare_patch",
            "candidate_review",
            "workflow_apply_reviewed_candidate",
            "workflow_run",
        }
        return [self._tool(name) for name in sorted(names)]

    def _tool(self, name: str) -> FunctionTool:
        async def call(_context: Any, arguments: str) -> str:
            return await self.invoke(name, arguments)

        return FunctionTool(
            name=name,
            description=f"Repair environment capability {name}.",
            params_json_schema={"type": "object", "additionalProperties": True},
            on_invoke_tool=call,
            needs_approval=name == "workflow_apply_reviewed_candidate",
            strict_json_schema=False,
        )


REPAIR_STEPS = [
    ("workspace_snapshot", "{}"),
    ("workflow_prepare_patch", '{"candidate_id":"repair-1","patch":"first repair"}'),
    ("candidate_review", '{"candidate_id":"repair-1"}'),
    ("workflow_prepare_patch", '{"candidate_id":"repair-2","patch":"revised repair"}'),
    ("candidate_review", '{"candidate_id":"repair-2"}'),
    ("workflow_apply_reviewed_candidate", '{"review_receipt_id":"repair-pass"}'),
    ("workflow_run", '{"run_id":"assistant-run-action-2"}'),
]
