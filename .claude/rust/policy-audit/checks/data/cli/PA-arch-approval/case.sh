# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
f=$(ls "$ROOT/r/.claude/policies/arch_variant_evidence"/*approval*.json | head -1) || return 1
jq '.approved_by = "someone" | .result = "FAIL"' "$f" >"$f.n" && mv "$f.n" "$f" || return 1
fx_commit || return 1
}
