#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_ROOT="${ASSISTANT_BUILD_ROOT:-$PROJECT_ROOT/.build/assistant}"
VENV="$BUILD_ROOT/venv"
PYTHON="${PYTHON:-python3}"
TARGET="${TAURI_ENV_TARGET_TRIPLE:-${TARGET:-$(rustc -vV | awk '/^host:/ {print $2}')}}"
EXE_SUFFIX="${EXE_SUFFIX:-}"
export PIP_CACHE_DIR="$BUILD_ROOT/pip-cache"
export PYINSTALLER_CONFIG_DIR="$BUILD_ROOT/pyinstaller-config"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) EXE_SUFFIX=".exe" ;;
esac

mkdir -p "$BUILD_ROOT"
if [ ! -x "$VENV/bin/python" ]; then
  "$PYTHON" -m venv "$VENV"
fi

"$VENV/bin/python" -m pip install --disable-pip-version-check --upgrade \
  -r "$PROJECT_ROOT/assistant/requirements-stdio.txt"

rm -rf "$BUILD_ROOT/work" "$BUILD_ROOT/dist"
mkdir -p "$PROJECT_ROOT/src-tauri/binaries"
"$VENV/bin/python" -m PyInstaller \
  --clean \
  --noconfirm \
  --distpath "$BUILD_ROOT/dist" \
  --workpath "$BUILD_ROOT/work" \
  "$PROJECT_ROOT/assistant/assistant.spec"

SOURCE="$BUILD_ROOT/dist/oh-my-dream-assistant$EXE_SUFFIX"
DEST="$PROJECT_ROOT/src-tauri/binaries/oh-my-dream-assistant-$TARGET$EXE_SUFFIX"
rm -f "$DEST"
cp "$SOURCE" "$DEST"
chmod +x "$DEST"
printf '%s\n' "$DEST"
