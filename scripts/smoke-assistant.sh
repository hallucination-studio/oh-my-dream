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

SESSION_PATH="$BUILD_ROOT/smoke-session.sqlite3"
INPUT_PATH="$BUILD_ROOT/smoke-input.ndjson"
OUTPUT_PATH="$BUILD_ROOT/smoke-output.ndjson"
"$VENV/bin/python" - "$SESSION_PATH" >"$INPUT_PATH" <<'PY'
import sys

from assistant.tests.agent_transport_fixture import encode_frames, operation
from assistant.stdio_protocol import FrameKind

session_path = sys.argv[1]
frames = [
    (
        FrameKind.INVOKE,
        {
            "invocation_id": "frozen-smoke",
            "session_id": "frozen-session",
            "session_path": session_path,
            "input": "Use the operation.",
            "operations": [operation("workspace_get_snapshot")],
            "state": None,
        },
    ),
    (
        FrameKind.TOOL_RESPONSE,
        {
            "invocation_id": "frozen-smoke",
            "call_id": "call-1",
            "output_json": '{"result":"smoke"}',
        },
    ),
]
sys.stdout.buffer.write(encode_frames(frames))
PY

"$BUILD_ROOT/smoke-dist/oh-my-dream-assistant-smoke$EXE_SUFFIX" \
  <"$INPUT_PATH" >"$OUTPUT_PATH" 2>"$BUILD_ROOT/smoke-stderr.log"

"$VENV/bin/python" - "$OUTPUT_PATH" "$BUILD_ROOT/smoke-stderr.log" <<'PY'
import sys

from assistant.stdio_protocol import FrameKind, FrameReader

with open(sys.argv[1], "rb") as stream:
    reader = FrameReader(stream)
    frames = []
    while True:
        try:
            frames.append(reader.read_frame())
        except Exception:
            break
assert frames[-1].kind is FrameKind.COMPLETED, [frame.kind for frame in frames]
assert "ASSISTANT_SMOKE_TRACING_DISABLED=1" in open(sys.argv[2]).read()
PY

printf '%s\n' "assistant frozen smoke passed"
