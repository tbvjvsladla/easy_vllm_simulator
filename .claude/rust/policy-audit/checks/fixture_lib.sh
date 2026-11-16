# Fixture helpers sourced by checks/data/cli/*/case.sh (bash; runs inside run_case.sh with ROOT and REPO set).
# All fixtures are git repos under $ROOT/r built from commit $FX_REV's blobs or from literal text, so a golden never
# depends on the live working tree.
FX_REV=b1f3563e

fx_init() {
  mkdir -p "$ROOT/r"
  git init -q -b single-node "$ROOT/r"
}

fx_commit() {
  git -C "$ROOT/r" add -A
  git -C "$ROOT/r" commit -q -m fixture
}

# Extract repo-relative paths of $FX_REV into $ROOT/r.
fx_from_rev() {
  git -C "$REPO" archive -o "$ROOT/sub.tar" "$FX_REV" -- "$@"
  tar -x -C "$ROOT/r" -f "$ROOT/sub.tar"
  rm -f -- "$ROOT/sub.tar"
}

# The real registry with everything its verification reads (evidence files, arch ledger, procedures) at $FX_REV.
fx_real() {
  fx_init
  fx_from_rev .claude/policies/registry.yaml .claude/policies/claim_bindings.json \
    .claude/schemas/policy-registry.schema.json .claude/policies/arch_variant_ledger.json \
    .claude/policies/arch_variant_evidence .claude/policies/provenance \
    .claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template \
    .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh \
    .claude/skills/upstream-version-watch/scripts/slave_forward.py \
    .claude/skills/upstream-version-watch/rust/runtime/src/slave_forward.rs \
    .claude/skills/upstream-version-watch/scripts/render_dockerfile.py \
    .claude/skills/upstream-version-watch/templates/docker-compose.multi.template.yaml \
    .claude/rules/workflow.md .claude/skills/upstream-version-watch/SKILL.md \
    .claude/policies/runtime/policy_registry.py
  local -a ev
  mapfile -t ev < <( { jq -r '.policies[].evidence[].path' "$ROOT/r/.claude/policies/registry.yaml"
                       jq -r '.bindings[][].path' "$ROOT/r/.claude/policies/claim_bindings.json"; } | sort -u)
  fx_from_rev "${ev[@]}"
  fx_dedicated_rust
}

# $FX_REV cites each clause's dedicated predicate in the retired `claim_predicates.py`; the validator now requires it
# in the Rust port (`.claude/rust/policy-predicates/src/claims*.rs`). Carry those tuples to one fixture Rust file that
# defines exactly the cited ids, so every other verdict of the real registry stays the one the original produced.
fx_dedicated_rust() {
  local old=.claude/policies/predicates/claim_predicates.py
  local new=.claude/rust/policy-predicates/src/claims_fx.rs
  mkdir -p "$ROOT/r/${new%/*}"
  { jq -r --arg o "$old" '.policies[].evidence[] | select(.path == $o) | .assertion_ids[]' "$ROOT/r/.claude/policies/registry.yaml"
    jq -r --arg o "$old" '.bindings[][] | select(.path == $o) | .assertion_ids[]' "$ROOT/r/.claude/policies/claim_bindings.json"
  } | sort -u | sed 's/^\(.*\)$/pub fn \1() {}/' >"$ROOT/r/$new"
  fx_jq .claude/policies/registry.yaml "(.policies[].evidence[] | select(.path == \"$old\") | .path) |= \"$new\""
  fx_jq .claude/policies/claim_bindings.json "(.bindings[][] | select(.path == \"$old\") | .path) |= \"$new\""
}

# jq-edit a fixture file in place: fx_jq FILE FILTER
fx_jq() {
  local f="$ROOT/r/$1"
  jq --indent 2 "$2" "$f" >"$f.new" && mv -- "$f.new" "$f"
}

# A synthetic repo: the real schema plus small evidence files of every kind.
fx_synth() {
  fx_init
  fx_from_rev .claude/schemas/policy-registry.schema.json
  mkdir -p "$ROOT/r/ev" "$ROOT/r/.claude/policies"
  cat >"$ROOT/r/ev/a.py" <<'PY'
import unittest
X = 1
Y: int = 2
A = B = 3
a, b = 4, 5
def alpha():
    return "def hidden(): pass"
async def beta():
    pass
class K(unittest.TestCase):
    def test_one(self):
        pass
    def helper(self):
        pass
class Plain:
    def test_two(self):
        pass
PY
  cat >"$ROOT/r/ev/b.sh" <<'SH'
#!/bin/bash
KEY_OK=1
do_thing() { :; }
# RETIRED_VAR=1
case "$1" in --topology) ;; esac
SH
  printf '# Heading One\n\nprose\n## Second heading\n' >"$ROOT/r/ev/c.md"
  printf 'top_key: 1\nblock: |\n  hidden_key: 2\nother.key: x # trailing\n' >"$ROOT/r/ev/d.yaml"
  printf '{"alpha_key": 1, "beta_key": {"inner": 2}}\n' >"$ROOT/r/ev/e.json"
  printf 'export function tsFn() {}\n/* const hiddenTs = 1 */\nconst tsConst = 2\n' >"$ROOT/r/ev/f.ts"
  printf 'pub fn rs_fn() {}\nconst RS_CONST: u8 = 1;\n    fn indented() {}\n' >"$ROOT/r/ev/g.rs"
  printf 'plain text\n' >"$ROOT/r/ev/h.txt"
  printf '{"schema_version": 1, "bindings": {}}\n' >"$ROOT/r/.claude/policies/claim_bindings.json"
}
