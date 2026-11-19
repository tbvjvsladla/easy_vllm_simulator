# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
sed -i 's/--cell-env "$EF"/--cell-env "$EFC"/' "$ROOT/r/.claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh" || return 1
fx_commit || return 1
}
