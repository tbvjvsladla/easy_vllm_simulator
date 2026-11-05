#!/bin/bash
# Node-local Rust executable; installation and build are explicit operations.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${RESOLVE_TOPOLOGY_BIN:-$SDIR/../rust/target/release/resolve_topology}"
if [ ! -x "$BIN" ]; then
  echo '[resolve_topology] ERROR Rust 실행파일이 없다. Terraform 설치·검사 뒤 명시 빌드가 필요하다.' >&2
  echo "  bash "$SDIR/../../terraforming_node/scripts/rust_bootstrap.sh" install" >&2
  echo "  bash "$SDIR/build_rust.sh" --offline" >&2
  exit 2
fi
export RESOLVE_TOPOLOGY_REPO_ROOT="${RESOLVE_TOPOLOGY_REPO_ROOT:-$(cd "$SDIR/../../../.." && pwd)}"
export RESOLVE_TOPOLOGY_SCRIPTS_DIR="${RESOLVE_TOPOLOGY_SCRIPTS_DIR:-$SDIR}"
export RESOLVE_TOPOLOGY_RUNTIME_DIR="${RESOLVE_TOPOLOGY_RUNTIME_DIR:-$SDIR/../rust/target/release}"
export RESOLVE_TOPOLOGY_PYTHON="${RESOLVE_TOPOLOGY_PYTHON:-python3}"
exec "$BIN" "$@"
