# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(check-removal --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r" --policy-id HOST_SAFETY_LAYERED_DEFENSE)
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_commit || return 1
}
