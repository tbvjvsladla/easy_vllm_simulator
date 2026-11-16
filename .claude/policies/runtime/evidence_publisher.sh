#!/bin/bash
# `evidence_publisher.py init|append-raw|set-narrative|record-capacity-rejection|publish-benchmark|publish-lite-report|set-promotion-target|finalize|--self-test` through the node-local Rust executable (crate .claude/rust/policy-publisher).
# Installation and build are explicit operations: a missing executable is exit 2 with guidance, never a Python start.
# argv[0] is the original script path so the program locates the repo/schemas exactly as `Path(__file__)` did and its
# argparse-style usage texts name `evidence_publisher.py`.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${EVIDENCE_PUBLISHER_BIN:-$SDIR/../../rust/policy-publisher/target/release/evidence_publisher}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[evidence_publisher] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[evidence_publisher] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$(cd "$SDIR/../../.." && pwd -P)/.claude/skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[evidence_publisher] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec -a "$SDIR/evidence_publisher.py" "$BIN" "$@"
