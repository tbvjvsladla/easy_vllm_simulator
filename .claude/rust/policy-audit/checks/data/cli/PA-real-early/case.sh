# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-07-01 --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_commit || return 1
}
