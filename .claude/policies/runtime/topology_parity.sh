#!/bin/bash
# Direct normal ELF startup; no install/build/download or Python business fallback.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
BINARY="${TOPOLOGY_PARITY_BIN:-$SDIR/../../skills/adversarial-benchmark/rust/target/release/topology_parity}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[topology_parity] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BINARY" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[topology_parity] ERROR normal Rust ELF invalid: %s\n' "$BINARY" >&2
  exit 2
fi
exec -a "$SDIR/topology_parity.sh" "$BINARY" "$@"
