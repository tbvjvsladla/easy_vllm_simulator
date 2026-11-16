# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-11-16 --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_jq .claude/policies/arch_variant_ledger.json '.schema_version = 9 | ._x = 3 | .extra = 1 | .source_build_variants._note = 4 | .source_build_variants._bad = "x" | .source_build_variants.notobj = 5' || return 1
fx_commit || return 1
}
