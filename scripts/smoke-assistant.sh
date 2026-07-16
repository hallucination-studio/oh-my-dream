#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_ROOT="${ASSISTANT_BUILD_ROOT:-$PROJECT_ROOT/.build/assistant}"
VENV="$BUILD_ROOT/venv"
EXE_SUFFIX="${EXE_SUFFIX:-}"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) EXE_SUFFIX=".exe" ;;
esac

"$SCRIPT_DIR/build-assistant.sh" >/dev/null
rm -rf "$BUILD_ROOT/smoke-work" "$BUILD_ROOT/smoke-dist"
"$VENV/bin/python" -m PyInstaller \
  --clean \
  --noconfirm \
  --distpath "$BUILD_ROOT/smoke-dist" \
  --workpath "$BUILD_ROOT/smoke-work" \
  "$PROJECT_ROOT/assistant/smoke.spec" >/dev/null

INPUT_PATH="$BUILD_ROOT/smoke-input.ndjson"
OUTPUT_PATH="$BUILD_ROOT/smoke-output.ndjson"
"$VENV/bin/python" >"$INPUT_PATH" <<'PY'
import json
import sys

from assistant.protocol_v1 import TOOL_IDS

contracts = [
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
        "effect": "HumanApprovalRequest" if tool_id == "assistant.workflow.request_apply@1" else "AuthoritativeRead",
        "requires_human_approval": tool_id == "assistant.workflow.request_apply@1",
    }
    for tool_id in sorted(TOOL_IDS)
]
invocation_id = "03000000-0000-4000-8000-000000000003"
frames = [
    {
        "protocol_version": 1,
        "invocation_id": invocation_id,
        "direction_sequence": 1,
        "kind": "InvocationStart",
        "payload": {
            "start": {"kind": "UserMessage", "message": "Use the operation."},
            "trusted_context": {
                "project_id": "01000000-0000-4000-8000-000000000001",
                "session_id": "02000000-0000-4000-8000-000000000002",
                "workspace_snapshot": {},
            },
            "tool_contracts": contracts,
            "budgets": {
                "maximum_frame_bytes": 8388608,
                "maximum_events": 512,
                "maximum_tool_calls": 64,
                "maximum_model_turns": 16,
                "maximum_direction_bytes": 16777216,
                "deadline_ms": 600000,
            },
        },
    },
    {
        "protocol_version": 1,
        "invocation_id": invocation_id,
        "direction_sequence": 2,
        "kind": "ToolResult",
        "payload": {
            "call_id": "call-1",
            "tool_id": "assistant.workspace.get_snapshot@1",
            "result": {"snapshot": {"smoke": True}},
        },
    },
]
for frame in frames:
    sys.stdout.write(json.dumps(frame, separators=(",", ":")) + "\n")
PY

"$BUILD_ROOT/smoke-dist/oh-my-dream-assistant-smoke$EXE_SUFFIX" \
  <"$INPUT_PATH" >"$OUTPUT_PATH" 2>"$BUILD_ROOT/smoke-stderr.log"

"$VENV/bin/python" - "$OUTPUT_PATH" "$BUILD_ROOT/smoke-stderr.log" <<'PY'
import json
import sys

with open(sys.argv[1], "rb") as stream:
    frames = [json.loads(line) for line in stream]
assert frames[-1]["kind"] == "InvocationCompleted", [frame["kind"] for frame in frames]
assert "ASSISTANT_SMOKE_TRACING_DISABLED=1" in open(sys.argv[2]).read()
PY

printf '%s\n' "assistant frozen smoke passed"
