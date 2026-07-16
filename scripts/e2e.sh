#!/usr/bin/env bash
set -euo pipefail

declare -a stage_names=()
declare -a stage_pids=()
result=0

start_stage() {
  local name="$1"
  shift
  echo "==> Starting ${name}"
  "$@" &
  stage_names+=("${name}")
  stage_pids+=("$!")
}

stop_stages() {
  for pid in "${stage_pids[@]}"; do
    kill "${pid}" 2>/dev/null || true
  done
}

wait_for_stages() {
  local index
  for index in "${!stage_pids[@]}"; do
    if wait "${stage_pids[${index}]}"; then
      echo "==> Passed: ${stage_names[${index}]}"
    else
      echo "==> Failed: ${stage_names[${index}]}" >&2
      result=1
    fi
  done
  stage_names=()
  stage_pids=()
}

trap stop_stages INT TERM

start_stage "Rust workspace tests" cargo test --workspace
start_stage "Assistant Python tests" python3 -m pytest assistant/tests
wait_for_stages

# Rust contract tests generate the fixtures consumed by both frontend gates.
start_stage "Frontend typecheck" npm --prefix ui run typecheck
start_stage "Frontend Vitest" npm --prefix ui run test
wait_for_stages

exit "${result}"
