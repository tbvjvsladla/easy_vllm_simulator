# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-11-16 --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_jq .claude/policies/arch_variant_ledger.json '"deepseek-v4-flash-sm12x-fork41834-0.27.0" as $k | .source_build_variants[$k].vllm_repo = "https://github.com/vllm-project/vllm.git" | .source_build_variants[$k].vllm_ref = "main" | .source_build_variants[$k].architecture = "gpu" | .source_build_variants[$k].port_manifest = {}' || return 1
fx_commit || return 1
}
