# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
sed -i 's/체크포인트-교체/X/; s/단일 변종-트랙/X/' "$ROOT/r/.claude/rules/workflow.md" "$ROOT/r/.claude/skills/upstream-version-watch/SKILL.md" || return 1
fx_commit || return 1
}
