# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/reg.json" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
: >"$ROOT/r/reg.json" || return 1
}
