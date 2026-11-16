# shellcheck shell=bash
# stage 1 exit 2 propagates; stdout order = child JSON then buffered harness lines
PROG_KIND=harness_verify
ARGS=(--as-of 2026-01-15 --registry "$ROOT/nope.json")
