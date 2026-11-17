#!/bin/bash
# Fixture repositories for the tripwire / lint / scanner goldens of this crate.
#
#   fixtures.sh list                    case names, one per line
#   fixtures.sh build <case> <dir>      build <dir> (must not exist) as the fixture of <case>
#
# The goldens checks/data/cli/<case>/{stdout,stderr,exit_code} are frozen: they were captured from the original Python
# programs, which are deleted (plan_26111615), so the capture mode is retired (plan_26111717).
# Each fixture is a minimal canonical repository (CLAUDE.md, rules/workflow.md, policies/registry.yaml, branch
# single-node) whose untracked overlay carries the programs the ORIGINAL loads from the checked root
# (antipattern_scan.py, topology_parity.py) and the wiki-desk forbidden-string scanner; one mutation per case
# drives exactly one tripwire branch. The cargo check builds the same fixture and runs the native binary with
# argv[0] = <fixture>/.claude/policies/runtime/<name>.sh (the launcher's own repository-root rule).
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
CHECKS="$(cd "${SELF%/*}" && pwd)"
REPO="$(cd "$CHECKS/../../../.." && pwd)"
# skill name assembled where a literal path would itself be a governed cross-reference (tripwire ⑩ / public surface)
TN=terraforming_node

CASES=(tw-pass tw-noncanon tw-bak-file tw-stray-branch tw-bad-tag tw-gitignore tw-digest tw-retired-prose
  tw-dup-cert tw-unkeyed-cert tw-pii tw-revived tw-surface tw-root-stray tw-tombstone tw-topology-header
  tw-vocabulary tw-sync-literal tw-skill-edge tw-router-anchor tw-literal-binding tw-literal-binding-rs tw-ledger-reasonless
  cl-text cl-json cl-noclaude cl-findings-text cl-findings-json ap-tripwire-staged ap-inventory
  arg-ap-help arg-ap-unknown arg-ap-novalue arg-ap-explicit arg-ap-prefix arg-ap-noaction arg-cl-help arg-cl-choice arg-cl-ambiguous arg-rs-help arg-rs-unknown)

g() { git -C "$D" "$@"; }

base() {
  D="$1"
  mkdir -p "$D"
  git init -q -b single-node "$D"
  mkdir -p "$D/.claude/rules" "$D/.claude/policies/runtime" "$D/.claude/hooks" "$D/.claude/skills/terraforming_node" \
    "$D/.claude/skills/wiki-desk/scripts" "$D/.claude/skills/upstream-version-watch/scripts"
  printf '# fixture constitution\n이 저장소는 골든 픽스처이며 토폴로지 중립 문장만 담는다.\n' > "$D/CLAUDE.md"
  printf '# workflow\n공통층 규칙 문장 하나.\n' > "$D/.claude/rules/workflow.md"
  printf '# strategy\n\n**topology: single** · layer: topology\n' > "$D/.claude/rules/strategy.topology.md"
  printf '{"schema_version": 2, "policies": [{"policy_id": "REAL_ONE"}]}\n' > "$D/.claude/policies/registry.yaml"
  printf 'target/\n' > "$D/.gitignore"
  cat > "$D/.claude/policies/root_registry.json" <<'JSON'
{"entries": [{"name": "CLAUDE.md", "git": "tracked"}, {"name": ".claude", "git": "tracked"},
 {"name": ".gitignore", "git": "tracked"}, {"name": "docs", "git": "either"}],
 "tombstones": [{"name": "tasks", "retired_utc": "2026-09-06", "successor": "campaigns/"}]}
JSON
  printf '{"edges": []}\n' > "$D/.claude/policies/base_skill_edges.json"
  printf '# tn\n## 2.7 노드\n## 4. 공개 표면 — 전수\n\n| 진입점 | 소유 |\n|---|---|\n| `scripts/x.py` | §1 |\n\n## 5. 금지\n' \
    > "$D/.claude/skills/terraforming_node/SKILL.md"
  printf '#!/bin/bash\n# fixture hook\n' > "$D/.claude/hooks/pre-commit"
  printf '#!/bin/bash\nspec=":(exclude)*.topology.md"\n' > "$D/.claude/skills/upstream-version-watch/scripts/sync_branches.sh"
  g add -A
  g -c user.name=fixture -c user.email=fixture@example.invalid commit -qm fixture
  # untracked overlay: what the original loads from the checked root + the scanner both read
  cp "$REPO/.claude/skills/wiki-desk/scripts/scan-forbidden-strings.ts" "$D/.claude/skills/wiki-desk/scripts/"
  for f in antipattern_scan.py topology_parity.py; do
    if [ -f "$REPO/.claude/policies/runtime/$f" ]; then cp "$REPO/.claude/policies/runtime/$f" "$D/.claude/policies/runtime/"; fi
  done
  printf 'scan-forbidden-strings.ts\n' > "$D/.claude/skills/wiki-desk/scripts/.gitignore"
  printf '*.py\n' > "$D/.claude/policies/runtime/.gitignore"
}

commit_all() { g add -A; g -c user.name=fixture -c user.email=fixture@example.invalid commit -qm mutation; }

# constitution_lint findings: 22 shared sentences (the >20 truncation), a sentence in three files, contract lines,
# noise lines, ghost / declared / clause-suffixed citations, skill docs and references, a dated narrative line.
findings() {
  local i shared=""
  for i in $(seq 1 22); do shared+="공유 문장 번호 ${i} 은 두 헌법 파일에 똑같이 적혀 있어 중복으로 잡혀야 하는 긴 문장이다.\n"; done
  local three='세 파일에 같은 `규칙` 이 *적혀* 있으면   공백과 기호를 정규화한 뒤에도 같은 문장으로 묶인다.'
  printf "# c\n${shared}${three}\n**topology: single** 같은 계약 줄도 마흔 글자를 넘기도록 길게 늘여 적은 줄이다\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\npolicy:REAL_ONE 과 policy:GHOST_ONE 과 policy:REAL_ONE.C2 와 policy:ABC 를 쓴다.\n" > "$D/CLAUDE.md"
  printf "# w\n${shared}${three}\n" > "$D/.claude/rules/workflow.md"
  printf "# d\n${three}\npolicy:GHOST_TWO\n" > "$D/.claude/rules/docs.md"
  printf '# strategy\n\n**topology: single** 같은 계약 줄도 마흔 글자를 넘기도록 길게 늘여 적은 줄이다\n' > "$D/.claude/rules/strategy.topology.md"
  mkdir -p "$D/.claude/skills/terraforming_node/references" "$D/.claude/skills/other/references/sub"
  printf '# tn\n(2026-09-30 신설) 날짜 서사 줄\n(2026-10-01 · plan 개정) 또 한 줄\npolicy:GHOST_ONE\n## 4. 공개 표면 — 전수\n\n| 진입점 | 소유 |\n|---|---|\n| `scripts/x.py` | §1 |\n' \
    > "$D/.claude/skills/terraforming_node/SKILL.md"
  printf 'a\nb\nc\n' > "$D/.claude/skills/$TN/references/r1.md"
  printf 'policy:GHOST_THREE\n' > "$D/.claude/skills/other/references/sub/r2.md"
  printf 'policy:GHOST_ONE\n' > "$D/.claude/skills/other/SKILL.md"
  commit_all
}

build() {
  local case="$1" D="$2"
  base "$D"
  # the overlay ignore files are fixture plumbing: track them so the tree stays clean
  commit_all
  case "$case" in
    tw-pass|cl-text|cl-json|arg-*) ;;
    tw-noncanon) rm "$D/.claude/policies/registry.yaml" ;;
    tw-bak-file) printf 'x\n' > "$D/notes.bak" ;;
    tw-stray-branch) g branch backup-before-edit ;;
    tw-bad-tag) g tag v1.0 ;;
    tw-gitignore) printf '*.bak\n' >> "$D/.gitignore"; commit_all ;;
    tw-digest)
      local sha; sha="$(g rev-parse HEAD:CLAUDE.md)"
      printf '{"integrity": "%s"}\n' "$sha" > "$D/.claude/policies/integrity.json"; commit_all ;;
    tw-retired-prose) printf '# note\nthe tracked_index ledger returns\n' > "$D/.claude/note.md"; commit_all ;;
    tw-dup-cert|tw-unkeyed-cert)
      mkdir -p "$D/docs/benchmark"
      local c='schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: m\ngpu_model: "NVIDIA GB10"\nvllm_version: 0.18.0\nquantization: mxfp4\ntopology: single\ntensor_parallel_size: 1\n'
      if [ "$case" = tw-dup-cert ]; then
        printf "${c}measured_utc: \"2026-09-03T12:00:00Z\"\n" > "$D/docs/benchmark/benchmark_a.yaml"
        printf "${c}measured_utc: \"2026-09-03T12:00:00Z\"\n" > "$D/docs/benchmark/benchmark_b.yaml"
      else
        printf "$c" > "$D/docs/benchmark/benchmark_a.yaml"
      fi
      commit_all ;;
    tw-pii)
      # a literal term from the scanner's untracked terms file (a generic pattern would put a private address into
      # the tracked golden, which the repository's own tripwire ⑤ then flags)
      printf 'fixture-operator-host\n' > "$D/.claude/pii_terms.txt"
      printf 'login fixture-operator-host\n' > "$D/.claude/notes.txt"; commit_all ;;
    tw-revived)
      mkdir -p "$D/.claude/skills/terraforming_node/scripts"
      printf 'GRADES = {"S": 1}\n' > "$D/.claude/skills/terraforming_node/scripts/turn_budget.py"
      g add -- .claude/skills/terraforming_node/scripts/turn_budget.py ;;
    # the reference is assembled: a literal skill-script path in this tracked file would itself be an off-surface call
    tw-surface) tn=terraforming_node; printf 'see .claude/skills/%s/scripts/zzz.py\n' "$tn" > "$D/.claude/cite.txt"; commit_all ;;
    tw-root-stray) printf 'x\n' > "$D/stray.txt"; commit_all ;;
    tw-tombstone)
      mkdir -p "$D/tasks"; printf 'x\n' > "$D/tasks/t.txt"
      cat > "$D/.claude/policies/root_registry.json" <<'JSON'
{"entries": [{"name": "CLAUDE.md", "git": "tracked"}, {"name": ".claude", "git": "tracked"},
 {"name": ".gitignore", "git": "tracked"}, {"name": "tasks", "git": "either"}],
 "tombstones": [{"name": "tasks", "retired_utc": "2026-09-06", "successor": "campaigns/"}, {"name": "config*.yaml"}]}
JSON
      commit_all ;;
    tw-topology-header) printf '# strategy\n\n**topology: multi** · layer: topology\n' > "$D/.claude/rules/strategy.topology.md"; commit_all ;;
    tw-vocabulary) printf '# fixture constitution\n서브는 Ray 워커다.\n' > "$D/CLAUDE.md"; commit_all ;;
    tw-sync-literal) printf '#!/bin/bash\n# no exclude here\n' > "$D/.claude/skills/upstream-version-watch/scripts/sync_branches.sh"; commit_all ;;
    tw-skill-edge) printf 'P = ".claude/skills/s1/scripts/x.py"\n' > "$D/.claude/policies/a.json"; commit_all ;;
    tw-router-anchor) printf '`%s` SKILL.md §9.9 · %s/references/zz.md\n' "$TN" "$TN" > "$D/.claude/cite.md"; commit_all ;;
    tw-literal-binding)
      printf "def f():\n    s = _read('.claude/skills/terraforming_node/SKILL.md')\n    _require('## 3.1 없는 절' in s)\n    _require('## 2.7 노드' not in s)\n" \
        > "$D/.claude/policies/runtime/p_check.py"
      # plan_26111615 pass 3: a harness .py is no longer a base→skill edge source (tripwire ⑨ reads the launchers and the
      # Rust policy sources), so this fixture lists no edge; ⑩④ still judges the .py (golden captured from the original).
      printf '{"edges": []}\n' > "$D/.claude/policies/base_skill_edges.json"
      g add -f -- .claude/policies/runtime/p_check.py; commit_all ;;
    tw-literal-binding-rs)
      # The Rust form of the same binding (hand-maintained golden: the original never read Rust sources).
      mkdir -p "$D/.claude/rust/policy-fx/src"
      printf 'pub fn f() -> R {\n    let s = read(".claude/skills/%s/SKILL.md")?;\n    req(s.contains("## 3.1 없는 절"), "a")?;\n    req(!s.contains("## 2.7 노드"), "b")\n}\n' "$TN" \
        > "$D/.claude/rust/policy-fx/src/p_check.rs"
      printf '{"edges": [{"caller": ".claude/rust/policy-fx/src/p_check.rs", "callee": "%s/SKILL.md", "phase": "verification_plane", "reason": "fixture"}]}\n' "$TN" \
        > "$D/.claude/policies/base_skill_edges.json"
      commit_all ;;
    tw-ledger-reasonless)
      printf '{"edges": [{"caller": ".claude/policies/gone.py", "callee": "s9/x.py", "phase": "elsewhere", "reason": ""}]}\n' \
        > "$D/.claude/policies/base_skill_edges.json"; commit_all ;;
    cl-noclaude) rm "$D/CLAUDE.md"; commit_all ;;
    cl-findings-text|cl-findings-json) findings ;;
    ap-tripwire-staged)
      mkdir -p "$D/.claude/skills/upstream-version-watch/scripts"
      printf 'READY_MAX="${READY_MAX:-180}"\n# READY_MAX:-1 in a comment\n' > "$D/.claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh"
      g add -- .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh ;;
    ap-inventory)
      mkdir -p "$D/.claude/skills/x/scripts"
      printf 'X = 12345\nY = os.environ.get("A", 7)\nZ = os.getenv("B", "v")  # print\nV = "1.2.3 GB10 sonnet mock sha256 grade"\n' > "$D/.claude/skills/x/scripts/inv.py"
      printf 'A="${A:-4096}"\nB="${B:-10}"  # self-test\nnvcr.io/x:1.2.3\n' > "$D/.claude/skills/x/scripts/inv.sh"
      commit_all ;;
    *) echo "unknown case $case" >&2; exit 2 ;;
  esac
}

# argv of each case: <program> <args...> (program = runtime_selftest | antipattern_scan | constitution_lint)
argv_of() {
  case "$1" in
    tw-*) echo "runtime_selftest --tripwires-only" ;;
    cl-text|cl-noclaude|cl-findings-text) echo "constitution_lint --repo ." ;;
    cl-json|cl-findings-json) echo "constitution_lint --repo . --format json" ;;
    ap-tripwire-staged) echo "antipattern_scan --root . --tripwire" ;;
    ap-inventory) echo "antipattern_scan --root . --inventory inv.jsonl" ;;
    arg-ap-help) echo "antipattern_scan -h" ;;
    arg-ap-unknown) echo "antipattern_scan --bogus x" ;;
    arg-ap-novalue) echo "antipattern_scan --root" ;;
    arg-ap-explicit) echo "antipattern_scan --tripwire=1" ;;
    arg-ap-prefix) echo "antipattern_scan --tri --fil" ;;
    arg-ap-noaction) echo "antipattern_scan --root ." ;;
    arg-cl-help) echo "constitution_lint --help" ;;
    arg-cl-choice) echo "constitution_lint --format yaml" ;;
    arg-cl-ambiguous) echo "constitution_lint --re ." ;;
    arg-rs-help) echo "runtime_selftest -h" ;;
    arg-rs-unknown) echo "runtime_selftest --tripwires-only extra" ;;
  esac
}

case "${1:-}" in
  list) printf '%s\n' "${CASES[@]}" ;;
  build) build "$2" "$3" ;;
  argv) argv_of "$2" ;;
  *) echo "usage: fixtures.sh list|build <case> <dir>|argv <case>" >&2; exit 2 ;;
esac
