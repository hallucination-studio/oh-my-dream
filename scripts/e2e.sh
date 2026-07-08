#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace
python3 -m pytest assistant/tests

cd ui
npm run typecheck
npm run test
