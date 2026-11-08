#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SELF_DIR="${BASH_SOURCE[0]%/*}"
if [ "$SELF_DIR" = "${BASH_SOURCE[0]}" ]; then SELF_DIR=.; fi
SDIR="$(cd -- "$SELF_DIR" && pwd)"
export BENCH_VERIFY_REPO_ROOT="${BENCH_VERIFY_REPO_ROOT:-$(cd "$SDIR/../../../.." && pwd)}"
export BENCH_PRESET_REPO_ROOT="$BENCH_VERIFY_REPO_ROOT"
export BENCH_PRESET_SKILL_DIR="$BENCH_VERIFY_REPO_ROOT/.claude/skills/adversarial-benchmark"
# The standalone Python verifier ignored argv; reuse the existing complete Rust suite.
exec bash "$SDIR/bench_preset.sh" --self-test
