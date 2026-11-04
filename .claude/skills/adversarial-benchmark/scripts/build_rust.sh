#!/bin/bash
# Explicit node-local application build; normal evaluation never installs or builds.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
CRATE="$SDIR/../rust"
BOOTSTRAP="$REPO/.claude/skills/terraforming_node/scripts/rust_bootstrap.sh"
runtime_args=()
cargo_args=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --offline) cargo_args+=(--offline); shift ;;
    --runtime-home)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        echo '[build_rust] ERROR --runtime-home 에 경로가 필요하다' >&2; exit 2
      fi
      runtime_args+=(--runtime-home "$2"); shift 2 ;;
    --help|-h)
      echo '사용법: bash build_rust.sh [--offline] [--runtime-home DIR]'
      echo 'Terraform install/check 뒤 Cargo.lock 으로 현재 노드용 release 실행파일을 빌드한다.'
      exit 0 ;;
    *) printf '[build_rust] ERROR 알 수 없는 인자: %s\n' "$1" >&2; exit 2 ;;
  esac
done
bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo build --release --locked \
  --manifest-path "$CRATE/Cargo.toml" --target-dir "$CRATE/target" "${cargo_args[@]}"
bash "$SDIR/sweep_stop.sh" --self-test
