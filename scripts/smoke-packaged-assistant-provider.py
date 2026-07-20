"""Smoke the packaged Assistant against a local OpenAI-compatible provider."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPOSITORY_ROOT))

from assistant.protocol_v1 import TOOL_IDS
from assistant.tests.openai_contract_server import LocalOpenAiContractServer


SECRET = "packaged-runtime-secret"


def main() -> None:
    executable = Path(sys.argv[1]).resolve()
    with LocalOpenAiContractServer() as server:
        base_url = f"{server.origin}/valid/v1"
        _assert_model_discovery(executable, base_url)
        _assert_model_compatibility(executable, base_url)
        _assert_text_invocation(executable, base_url)
    print("assistant packaged provider smoke passed")


def _assert_model_discovery(executable: Path, base_url: str) -> None:
    environment = os.environ.copy()
    environment.update(
        {
            "OH_MY_DREAM_ASSISTANT_MODE": "provider_control",
            "OH_MY_DREAM_ASSISTANT_PROVIDER_ACTION": "list_models",
            "OH_MY_DREAM_ASSISTANT_PROVIDER_BASE_URL": base_url,
            "OH_MY_DREAM_ASSISTANT_PROVIDER_API_KEY": SECRET,
        }
    )
    completed = subprocess.run(
        [executable],
        env=environment,
        check=True,
        capture_output=True,
        timeout=30,
    )
    assert json.loads(completed.stdout) == {
        "model_ids": ["model-a", "model-b"],
        "ok": True,
    }
    _assert_secret_absent(completed)


def _assert_model_compatibility(executable: Path, base_url: str) -> None:
    environment = os.environ.copy()
    environment.update(
        {
            "OH_MY_DREAM_ASSISTANT_MODE": "provider_control",
            "OH_MY_DREAM_ASSISTANT_PROVIDER_ACTION": "test_model",
            "OH_MY_DREAM_ASSISTANT_PROVIDER_BASE_URL": base_url,
            "OH_MY_DREAM_ASSISTANT_PROVIDER_API_KEY": SECRET,
            "OH_MY_DREAM_ASSISTANT_PROVIDER_MODEL_ID": "model-a",
        }
    )
    completed = subprocess.run(
        [executable],
        env=environment,
        check=True,
        capture_output=True,
        timeout=30,
    )
    assert json.loads(completed.stdout) == {"ok": True}
    _assert_secret_absent(completed)


def _assert_text_invocation(executable: Path, base_url: str) -> None:
    environment = os.environ.copy()
    environment.update(
        {
            "OMD_ASSISTANT_BASE_URL": base_url,
            "OMD_ASSISTANT_MODEL": "model-a",
            "OMD_ASSISTANT_API_KEY": SECRET,
        }
    )
    completed = subprocess.run(
        [executable],
        env=environment,
        input=_invocation_start(),
        check=True,
        capture_output=True,
        timeout=30,
    )
    frames = [json.loads(line) for line in completed.stdout.splitlines()]
    assert frames[0]["kind"] == "InvocationAccepted"
    assert frames[-1]["kind"] == "InvocationCompleted"
    assert frames[-1]["payload"]["final_text"] == "Local Assistant response"
    _assert_secret_absent(completed)


def _assert_secret_absent(completed: subprocess.CompletedProcess[bytes]) -> None:
    assert SECRET.encode() not in completed.stdout
    assert SECRET.encode() not in completed.stderr


def _invocation_start() -> bytes:
    contracts = [_tool_contract(tool_id) for tool_id in sorted(TOOL_IDS)]
    frame = {
        "protocol_version": 1,
        "invocation_id": "03000000-0000-4000-8000-000000000003",
        "direction_sequence": 1,
        "kind": "InvocationStart",
        "payload": {
            "start": {"kind": "UserMessage", "message": "Reply with text."},
            "trusted_context": {
                "project_id": "01000000-0000-4000-8000-000000000001",
                "session_id": "02000000-0000-4000-8000-000000000002",
                "workspace_snapshot": {},
            },
            "tool_contracts": contracts,
            "budgets": {
                "maximum_frame_bytes": 8_388_608,
                "maximum_events": 512,
                "maximum_tool_calls": 64,
                "maximum_model_turns": 16,
                "maximum_direction_bytes": 16_777_216,
                "deadline_ms": 600_000,
            },
        },
    }
    return json.dumps(frame, separators=(",", ":")).encode() + b"\n"


def _tool_contract(tool_id: str) -> dict[str, object]:
    requires_approval = tool_id == "assistant.workflow.request_apply@1"
    return {
        "tool_id": tool_id,
        "description": "Exact Rust tool.",
        "input_schema": {
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": False,
        },
        "output_schema": {},
        "effect": "HumanApprovalRequest" if requires_approval else "AuthoritativeRead",
        "requires_human_approval": requires_approval,
    }


if __name__ == "__main__":
    main()
