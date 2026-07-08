#!/bin/bash
# Start the assistant sidecar with configuration from .env

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_ROOT/.env"

if [ ! -f "$ENV_FILE" ]; then
  echo "Error: $ENV_FILE not found"
  echo "Copy .env.example to .env and update with your configuration"
  exit 1
fi

# Load .env into environment
set -a
source "$ENV_FILE"
set +a

echo "Starting oh-my-dream assistant sidecar..."
echo "Config:"
echo "  Enabled: $OMD_ASSISTANT_ENABLED"
echo "  Base URL: $OMD_ASSISTANT_BASE_URL"
echo "  Model: $OMD_ASSISTANT_MODEL"
echo "  Temperature: $OMD_ASSISTANT_TEMPERATURE"
echo "  Max Tool Iters: $OMD_ASSISTANT_MAX_TOOL_ITERS"
echo "  Developer Mode: $OMD_ASSISTANT_DEVELOPER_MODE"
if [ -n "$OMD_ASSISTANT_ENABLED_SKILLS" ]; then
  echo "  Enabled Skills: $OMD_ASSISTANT_ENABLED_SKILLS"
fi
echo ""

cd "$PROJECT_ROOT/assistant"
python -m assistant
