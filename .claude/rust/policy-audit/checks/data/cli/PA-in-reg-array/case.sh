# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-01-15 --registry "$ROOT/r/reg.json" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
printf '[]' >"$ROOT/r/reg.json" || return 1
}
