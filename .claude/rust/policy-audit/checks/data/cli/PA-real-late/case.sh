# shellcheck shell=bash
# clocks run out
PROG_KIND=policy_registry
ARGS=(verify --as-of 2027-06-01 --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_commit || return 1
}
