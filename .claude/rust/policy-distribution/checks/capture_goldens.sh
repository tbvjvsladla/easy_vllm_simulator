#!/bin/bash
# Re-capture the goldens of checks/pure_goldens.rs from the ORIGINAL verify_distribution.py (needs that .py and
# python3 3.12; the Rust checks never run Python). Inputs are frozen: the tree of commit $REV (git archive) plus the
# literal fixtures below; outputs land in checks/data/*.json.
#   bash .claude/rust/policy-distribution/checks/capture_goldens.sh [REV]
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
CHECKS="$(cd "${SELF%/*}" && pwd -P)"
REPO="$(cd "$CHECKS/../../../.." && pwd -P)"
REV="${1:-b1f3563e}"
ORIG="$REPO/.claude/policies/runtime/verify_distribution.py"
[ -f "$ORIG" ] || { echo "original $ORIG missing — goldens can only be re-captured while it exists" >&2; exit 2; }
work="$(mktemp -d "${TMPDIR:-/tmp}/vd-golden.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT
tree="$work/tree"
mkdir -p "$tree"
git -C "$REPO" archive "$REV" | tar -x -C "$tree"
git -C "$tree" init -q -b single-node
git -C "$tree" add -A
mkdir -p "$CHECKS/data"
python3 -B - "$ORIG" "$tree" "$CHECKS/data" "$REV" <<'PY'
import importlib.util, json, sys, tomllib, ast, datetime as dt
from pathlib import Path
orig, tree, out, rev = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4]
spec = importlib.util.spec_from_file_location("vd_original", orig)
vd = importlib.util.module_from_spec(spec); spec.loader.exec_module(vd)
vd.REPO = tree
vd._shipped_python_paths.__defaults__ = (tree,)   # default bound to the module's REPO at import time
PY_EXE = sys.executable

def norm(v):
    if isinstance(v, str):
        return v.replace(str(tree), "<REPO>").replace(PY_EXE, "<PY>")
    if isinstance(v, list):
        return [norm(x) for x in v]
    if isinstance(v, dict):
        return {k: norm(x) for k, x in v.items()}
    return v

def stub_run(name, argv, expected, cwd=None, env_overrides=None):
    return {"name": name, "ok": True, "rc": 0, "expected_rc": sorted(expected),
            "argv": norm([str(a) for a in argv]), "cwd": norm(str(cwd if cwd is not None else vd.REPO)), "env": env_overrides or {}}
real_run = vd._run
run_cases = [
    {"name": "rc3_crlf", "argv": ["sh", "-c", "printf 'a\\r\\nb\\rc'; printf 'e' >&2; exit 3"], "expected": [3, 0]},
    {"name": "tail_1000", "argv": ["sh", "-c", "i=0; while [ $i -lt 300 ]; do printf '가나다라'; i=$((i+1)); done"], "expected": [0]},
    {"name": "missing_program", "argv": ["no-such-program-xyz"], "expected": [0]},
    {"name": "killed", "argv": ["sh", "-c", "kill -9 $$"], "expected": [0]},
    {"name": "env_override", "argv": ["sh", "-c", "printf '%s|%s' \"$PYTHONDONTWRITEBYTECODE\" \"$X_OVR\""], "expected": [0], "env": {"X_OVR": "v"}},
    {"name": "missing_cwd", "argv": ["sh", "-c", "true"], "expected": [0], "cwd": "/nonexistent-vd-cwd"},
]
run_out = []
for c in run_cases:
    res = real_run(c["name"], c["argv"], set(c["expected"]), cwd=Path(c.get("cwd", str(tree))), env_overrides=c.get("env"))
    run_out.append({"case": c, "result": res})
(out / "run.json").write_text(json.dumps(run_out, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
vd._run = stub_run
vd._gitless_hint_match_checks = lambda: [{"name": "gitless_hint_match", "ok": True, "stub": True},
                                         {"name": "gitless_hint_match_reads_index", "ok": True, "stub": True}]
vd._terraform_flag_issued = lambda: False
report = vd.verify("2026-11-16")
(out / "verify_static.json").write_text(json.dumps({"rev": rev, "report": norm(report)}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

# The same verify() on a MUTATED copy (every static check's failing branch). The mutation list is shared with the Rust
# check through mutations.json (the Rust side applies the same edits to its own frozen tree).
import shutil, subprocess
mutations = [
    {"op": "append", "path": ".claude/skills/hint-publisher/scripts/hint.py", "text": "\nassert False\n"},
    {"op": "write", "path": ".claude/skills/hint-publisher/tests/foo.txt", "text": "x\n"},
    {"op": "replace", "path": ".claude/policies/claim_bindings.json", "old": ".claude/skills/terraforming_node/scripts/scan_node_original.py", "new": "scripts/scan_node_original.py", "count": 1},
    {"op": "delete", "path": ".agents/skills/wiki-desk/SKILL.md"},
    {"op": "replace", "path": ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh", "old": "REST_BRANCH=", "new": "REST_BRANCHX=", "count": 0},
    {"op": "replace", "path": ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh", "old": "retirement-scan", "new": "retirement-skan", "count": 0},
    {"op": "replace", "path": ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh", "old": "apply_overlay_tombstones", "new": "apply_overlay_tomb", "count": 0},
    {"op": "replace", "path": ".claude/skills/upstream-version-watch/scripts/sync_branches.sh", "old": "  scripts/hint_tag.py\n", "new": "", "count": 1},
    {"op": "append", "path": ".claude/skills/upstream-version-watch/assets/build_plane/model_inputs/envs/.env.hy3", "text": "SSH_USER=bob\nHOST=10.0.0.9 a@b.example /home/op/x spark-abcdef\n"},
    {"op": "write", "path": ".claude/skills/upstream-version-watch/assets/build_plane/model_inputs/configs/x_patch.py", "text": "x = 1\n"},
    {"op": "write", "path": ".claude/skills/upstream-version-watch/assets/build_plane/runtime_patches/a_patch.provenance.json", "text": "{}\n"},
    {"op": "write", "path": ".claude/skills/upstream-version-watch/assets/build_plane/runtime_patches/b_patch.py", "text": ""},
    {"op": "replace", "path": ".dockerignore", "old": ".codex/\n", "new": "", "count": 1},
    {"op": "write", "path": "scripts/keep.txt", "text": "x\n"},
    {"op": "append", "path": ".codex/config.toml", "text": "approval_policy = \"never\"\n"},
    {"op": "append", "path": "CLAUDE.md", "text": "\nsee scripts/smoke_clone.sh\n"},
    {"op": "write_bytes_hex", "path": ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh", "hex": "ff0a"},
    {"op": "write", "path": ".claude/policies/registry.yaml", "text": "{not json"},
    {"op": "replace", "path": ".claude/skills/terraforming_node/SKILL.md", "old": ".claude/skills/terraforming_node/scripts/host_safety/", "new": ".claude/skills/terraforming_node/" + "scripts/hs/", "count": 0},
]
(out / "mutations.json").write_text(json.dumps(mutations, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
mtree = Path(str(tree) + "-mut")
shutil.copytree(tree, mtree, symlinks=True)
for m in mutations:
    p = mtree / m["path"]
    if m["op"] == "append":
        p.write_text(p.read_text(encoding="utf-8") + m["text"], encoding="utf-8")
    elif m["op"] == "write":
        p.parent.mkdir(parents=True, exist_ok=True); p.write_text(m["text"], encoding="utf-8")
    elif m["op"] == "write_bytes_hex":
        p.write_bytes(bytes.fromhex(m["hex"]))
    elif m["op"] == "delete":
        p.unlink()
    elif m["op"] == "replace":
        t = p.read_text(encoding="utf-8")
        p.write_text(t.replace(m["old"], m["new"], m["count"]) if m["count"] else t.replace(m["old"], m["new"]), encoding="utf-8")
subprocess.run(["git", "-C", str(mtree), "add", "-A"], check=True)
vd.REPO = mtree
vd._shipped_python_paths.__defaults__ = (mtree,)
tree_saved, tree = tree, mtree
report = vd.verify("2026-11-16")
(out / "verify_static_mutated.json").write_text(json.dumps({"rev": rev, "report": norm(report)}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
tree = tree_saved
vd.REPO = tree
vd._shipped_python_paths.__defaults__ = (tree,)

# assert detector: every shipped .py of the frozen tree + snippets
fixtures = {
    "plain": "assert x\n",
    "nested": "def f():\n    if a:\n        assert b, 'm'\n    return 1\nassert c\n",
    "in_string": "s = 'assert x'\nt = \"\"\"\nassert y\n\"\"\"\n# assert z\n",
    "fstring_nested": "x = f\"{a['assert']} {b!r:>{w}} {{assert}}\"\nassert q\n",
    "fstring_triple": "y = f'''\n{v}\nassert no\n'''\nassert yes\n",
    "raw_bytes": "r = rb'\\'assert'\nassert z\n",
    "attribute_like": "self.asserted = 1\nassertion = 2\n",
    "inline": "if x: assert y; assert z\n",
    "continuation": "x = (1,\n     2)\nassert \\\n  x\n",
    "unicode_ident": "변수 = 1\nassert 변수\n",
    "nul": "a = 1\0\n",
    "unterminated": "s = 'abc\n",
}
asserts = {"files": {}, "snippets": {}}
for path in vd._shipped_python_paths(tree):
    rel = str(path.relative_to(tree))
    try:
        t = ast.parse(path.read_text(encoding="utf-8"), filename=rel)
        asserts["files"][rel] = sorted(n.lineno for n in ast.walk(t) if isinstance(n, ast.Assert))
    except (OSError, UnicodeError, SyntaxError) as exc:
        asserts["files"][rel] = f"{type(exc).__name__}"
for key, text in fixtures.items():
    try:
        t = ast.parse(text, filename=key)
        asserts["snippets"][key] = {"text": text, "lines": sorted(n.lineno for n in ast.walk(t) if isinstance(n, ast.Assert))}
    except SyntaxError as exc:
        asserts["snippets"][key] = {"text": text, "error": str(exc)}
(out / "asserts.json").write_text(json.dumps(asserts, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

# TOML subset
real = (tree / ".codex/config.toml").read_text(encoding="utf-8")
toml_cases = {
    "real": real,
    "legacy_prefix": 'sandbox_mode = "workspace-write"\n' + real,
    "retired": real.replace('":danger-full-access"', '"vllm-main"'),
    "custom": real + '\n[permissions.vllm-main]\nextends = ":workspace"\n',
    "dup_key": 'a = "x"\na = "y"\n',
    "dup_table": '[t]\nx = 1\n[t]\ny = 2\n',
    "bad_value": 'a = nope\n',
    "missing_eq": 'a "x"\n',
    "trailing": 'a = "x" b\n',
    "types": 'i = 1_000\nf = 3.5\nb = true\narr = [1, "two", [3]]\ninl = { k = "v", n.m = 2 }\nlit = \'c:\\\\p\'\nml = """\nl1\nl2"""\n[x.y]\nz = -4\n[[aot]]\nq = 1\n[[aot]]\nq = 2\n',
    "escapes": 'e = "tab\\tq\\"\\u00e9"\n',
    "comments_ws": '# c\n\n  k = "v"   # tail\n',
}
def tomlres(text):
    try:
        d = tomllib.loads(text)
    except tomllib.TOMLDecodeError as exc:
        return {"error": str(exc)}
    def j(v):
        if isinstance(v, dict): return {k: j(x) for k, x in v.items()}
        if isinstance(v, list): return [j(x) for x in v]
        return v
    return {"value": j(d)}
(out / "toml.json").write_text(json.dumps({k: {"text": t, **tomlres(t)} for k, t in toml_cases.items()}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

# shell arrays / ordered-between on the frozen sync scripts + fixtures
sync = (tree / ".claude/skills/upstream-version-watch/scripts/sync_branches.sh").read_text(encoding="utf-8")
sub = (tree / ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh").read_text(encoding="utf-8")
arrays = []
for label, text in (("sync_branches", sync), ("sync_to_sub", sub)):
    for name in ("ALLOWLIST", "MIRROR_DIRS", "ROOT_RELOCATION_TOMBSTONES", "ROOT_RELOCATION_REPLACEMENTS",
                 "OVERLAY_STALE_PATHS", "OVERLAY_RELOCATION_STALE_PATHS", "OVERLAY_RETIREMENT_STALE_PATHS",
                 "RUNTIME_BLOCK_OWNED_ROOTS", "NOPE"):
        arrays.append({"source": label, "name": name, "value": sorted(vd._shell_array(text, name))})
fx = "A=(\n  'x'  # c\n  \"y\"\n\n  # only comment\n  z\n)\nB=(\n)\nC=( inline )\n"
for name in ("A", "B", "C"):
    arrays.append({"source": "fixture", "text": fx, "name": name, "value": sorted(vd._shell_array(fx, name))})
orders = []
otext = "S a b c E a"
for marker, end, toks in (("S", "E", ["a", "b"]), ("S", "E", ["b", "a"]), ("S", "E", ["z"]), ("Q", "E", ["a"]),
                          ("S", "Q", ["a"]), ("S", "E", ["S", "a"]), ("a", "E", ["b", "c"])):
    orders.append({"text": otext, "marker": marker, "end": end, "tokens": toks,
                   "value": vd._ordered_between_detail(otext, marker, end, tuple(toks))})
(out / "shell.json").write_text(json.dumps({"arrays": arrays, "orders": orders}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

# reference-date observation
clock = []
for explicit in ("2026-11-16", "2024-02-29", "2025-02-29", "2026-13-01", "0000-01-01", "2026-1-01", "x", "２０２６-01-01", "2026-04-31"):
    try:
        clock.append({"explicit": explicit, "value": vd._observe_policy_as_of(explicit)})
    except ValueError as exc:
        clock.append({"explicit": explicit, "error": str(exc)})
for secs, micros in ((1794730702, 800541), (0, 0), (951782400, 1), (4102444799, 999999)):
    now = dt.datetime.fromtimestamp(secs, dt.timezone.utc).replace(microsecond=micros)
    clock.append({"unix": secs, "micros": micros, "value": vd._observe_policy_as_of(None, now)})
(out / "clock.json").write_text(json.dumps(clock, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
print("captured", rev)
PY
# Public-surface tripwire: tracked data must not spell non-public skill script paths literally (decoded by pure_goldens.rs).
sed -i "s|terraforming""_node/scripts/|@TFN_SCRIPTS@/|g" "$CHECKS/data/mutations.json" "$CHECKS/data/asserts.json"
