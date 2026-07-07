#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace

cd ui
npm run typecheck
npm run test
