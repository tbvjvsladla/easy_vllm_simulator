# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-11-16 --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
sed -i '/^ *SM12X_PORT:/d' "$ROOT/r/.claude/skills/upstream-version-watch/templates/docker-compose.multi.template.yaml" || return 1
fx_commit || return 1
}
