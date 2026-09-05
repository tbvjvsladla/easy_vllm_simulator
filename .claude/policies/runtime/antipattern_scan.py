#!/usr/bin/env python3
"""antipattern_scan.py — 하네스 안티패턴 **인벤토리 + 제거 tripwire**(2026-09-05 · plan_26090516 ③ 3-13).

출신: `audit_26090515` 가 쓰고 버린 일회용 스캐너(`output/single/audit/antipattern_scan_26090515.py`).
비추적 산출물 디렉터리에 있어서 **다음 사람이 같은 감사를 다시 하려면 스크립트부터 다시 써야 했다.**
추적 평면으로 올리면서 두 가지를 더한다:

  ① **disposition 파생** — 인벤토리 행마다 규칙으로 처분을 적는다(미기재 0). 종전 인벤토리는
     1,028 행 중 처분이 손으로 채워지길 기다렸고, 그래서 대부분 비어 있었다. 규칙이 적으면
     사람은 **규칙에 이의를 제기**하면 되고, 그것이 행마다 판단을 반복하는 것보다 싸다.
  ② **제거 tripwire** — ③ 단계에서 걷어낸 과적합 형태가 **되돌아오면** 커밋 전에 잡는다.
     전수 인벤토리를 게이트로 쓰지 않는 이유: 3,900 히트 중 대부분은 정당이고, 총량 게이트는
     그 자체가 과적합이다(하네스 = 안전불변식만 · 3원칙 ①). 여기서 막는 것은 **닫힌 목록**뿐이며
     목록의 각 항목은 ③ 에서 실제로 제거한 결함과 1:1 로 대응한다.

usage:
  antipattern_scan.py --inventory out.jsonl [--root .]      # 전수 인벤토리(디스포지션 포함)
  antipattern_scan.py --tripwire [--files a.py b.sh]        # 제거 tripwire(기본: 스테이징된 파일)
  antipattern_scan.py --self-test
종료: 0=통과 · 2=인자/실행 오류 · 3=tripwire 위반(제거한 형태가 되돌아왔다)
"""
import argparse
import ast
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict

# 이 파일 자신의 저장소 상대경로 — tripwire 의 자기참조 제외에 쓴다.
SELF_REL = os.path.normpath(os.path.join(".claude", "policies", "runtime",
                                         os.path.basename(os.path.abspath(__file__))))

RC_OK = 0
RC_ARGS = 2
RC_REVIVED = 3

# ── 제거 tripwire: 닫힌 목록. 각 항목 = ③ 에서 제거한 결함 하나 ─────────────────────
#   `path_re` 는 그 결함이 살던 자리이며, 다른 파일의 우연한 문자열을 잡지 않기 위한 좁힘이다.
REMOVED_SHAPES = (
    {"id": "G-A1-sonnet-gate",
     "path_re": r"^\.claude/policies/runtime/(providers/)?[a-z_]+\.py$",
     "pattern": r'request\[["\']model["\']\]\s*!=\s*["\']sonnet["\']|REQUESTED_MODEL_NOT_SONNET',
     "why": "모델 게이트 — 모델은 요청의 선언이고 실행 모델은 기록한다(G-A1)"},
    {"id": "G-A2-grade-table",
     "path_re": r"^\.claude/skills/terraforming_node/scripts/(turn_budget|relay|bootstrap_canary)\.py$",
     "pattern": r"^GRADES\s*=|MAX_ATTEMPTS_BEFORE_HITL|ESCALATION_FACTOR|GRADE_MAX_TURNS",
     "why": "턴 예산 등급표 — 예산은 선언이다(G-A2)"},
    {"id": "G-C3-bench-drift-gate",
     "path_re": r"^\.claude/skills/adversarial-benchmark/scripts/resolve_bench_tool\.py$",
     "pattern": r"RC_DRIFT",
     "why": "벤치 도구 digest 드리프트 게이트 — digest 는 provenance 다(G-C3)"},
    {"id": "G-B1-overhead-default",
     "path_re": r"^\.claude/(skills/.*/scripts/.*|policies/.*)\.(py|sh)$",
     "pattern": r"(overhead[_-]?mib\W{0,12}|OVERHEAD_MIB[\"']?\s*[:=-]{1,2}\s*)12288|:-12288",
     "why": "예산 overhead 기본값 12288 — 선언 필수(G-B1 · 정상 로드 사살 실적)"},
    {"id": "G-B2-ttl-mirror",
     "path_re": r"^\.claude/(skills/upstream-version-watch/scripts/(single_serve_up|multinode_serve_smoke)\.sh"
                r"|skills/vllm-recipe-explorer/scripts/run_trial\.py"
                r"|skills/terraforming_node/scripts/node_blackbox/budget_renew_loop\.sh)$",
     "pattern": r"(TTL_S\s*=\s*7200|:-7200|BUDGET_TTL_MIN_S\s*=\s*7200)",
     "why": "예산 TTL 손기재 — 단일 소유자(blackbox_session budget-defaults)에게 묻는다(G-B2)"},
    {"id": "G-B3-decl-const-mirror",
     "path_re": r"^\.claude/skills/(terraforming_node/scripts/node_blackbox/.*|upstream-version-watch/scripts/multinode_serve_smoke\.sh)$",
     "pattern": r"(BB_DECL_(MARGIN|MIN_CEILING)_MIB=\"?\$\{BB_DECL_[A-Z_]+:-[0-9]"
                r"|_(WD|DECL)_FALLBACK\s*=\s*\{"
                r"|_WD_MARGIN=[0-9]|_WD_MIN_CEIL=[0-9])",
     "why": "선언 상수 거울 — 정본은 blackbox_eta.DEFAULTS / eta_params.env 하나다(G-B3)"},
    {"id": "G-B6-ready-max-default",
     "path_re": r"^\.claude/skills/upstream-version-watch/scripts/multinode_serve_smoke\.sh$",
     "pattern": r"READY_MAX:-[0-9]",
     "why": "READY_MAX 기본값 — 로드 시간은 모델·HW 의 함수라 선언한다(G-B6)"},
    {"id": "G-B8-topology-default",
     "path_re": r"^\.claude/skills/(adversarial-benchmark/scripts/lite_metrics\.py"
                r"|terraforming_node/scripts/library_relay\.py)$",
     "pattern": r'get\(\s*["\']topology["\']\s*,\s*["\']single["\']|get\(\s*["\']role["\']\s*,\s*["\']main["\']',
     "why": "토폴로지·역할 기본값 — 인터뷰/manifest 만이 정한다(G-B8 · 헌법)"},
    {"id": "G-C1-worktree-drift-gate",
     "path_re": r"^\.claude/policies/runtime/policy_registry\.py$",
     "pattern": r"worktree_drift|_git_worktree_blob_sha",
     "why": "정책 증거의 워크트리 대조 — 드리프트는 git status 가 답한다(G-C1)"},
)

VERSION_RE = re.compile(r'(?<![\w.])\d+\.\d+\.\d+(?![\w.])')
IMAGE_RE = re.compile(r'(easy-vllm:|nvcr\.io|ghcr\.io|docker\.io|guidellm)', re.I)
HW_RE = re.compile(r'\b(GB10|sm_?121a?|aarch64|dgx-spark|x86_64|cu13\d|cu12\d)\b', re.I)
MODEL_RE = re.compile(r'\b(sonnet|opus|haiku|claude-[a-z0-9.-]+)\b', re.I)
MOCK_RE = re.compile(r'(\bmock\b|dry.?run|\bfake\b|\bstub\b|TEST_OVERRIDE|_OVERRIDE\b|DELEGATED=1)', re.I)
HASH_RE = re.compile(r'(hashlib|sha256|sha1\b|md5|\bdigest\b|blake2)', re.I)
TURN_RE = re.compile(r'(max_turns|MIN_TURNS|turn_budget|timeout_seconds|grade)', re.I)
TRIVIAL = {0, 1, 2, -1, 10, 100, 1000, 1024, 0.0, 1.0, 0.5, 3, 4, 8, 16, 32, 64, 60}
LOUD = ('print', 'log', 'warn', 'error', 'fail', 'die', 'exit', 'diag', 'raise', 'abort',
        'stderr', 'write', 'emit', 'report')


def harness_files(root="."):
    """스캔 대상은 **git 이 아는 것**에서 파생한다 — 목록 파일을 얼리지 않는다(그 자체가 하드코딩)."""
    out = subprocess.run(["git", "-C", root, "ls-files", "--", ".claude", "CLAUDE.md"],
                         capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit("[antipattern_scan] FAIL: git ls-files 실패 — %s" % out.stderr.strip()[:200])
    return [p for p in out.stdout.splitlines()
            if p.endswith((".py", ".sh")) or p.endswith("/pre-commit")]


def disposition(hit):
    """행별 처분을 **규칙으로** 파생한다(미기재 0 · 3-13).

    규칙은 `workflow.md` §4종 안티패턴 판정표를 그대로 옮긴 것이다:
      · 픽스처·자체검사 평면의 상수 → `fixture`(정당)
      · 진단·로그로 나가는 리터럴   → `diagnostic`(정당)
      · 그 파일에서만 쓰는 국소 상수 → `local-const`(정당)
      · 결정·게이트 경로의 기본값   → `review`(사람이 본다)
    """
    path, src = hit.get("file", ""), hit.get("src", "")
    if "/fixtures/" in path or "test" in os.path.basename(path).lower():
        return "fixture"
    if any(tok in src.lower() for tok in ("self_test", "_self_test", "selftest", "self-test",
                                          "fixture", "픽스처")):
        return "fixture"
    if any(name in src.lower() for name in LOUD):
        return "diagnostic"
    if hit.get("cat") in ("MAGIC_NUM_CONST",) and hit.get("scope") == "module":
        return "local-const"
    return "review"


def scan_py(rel, text):
    hits = []
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return hits
    lines = text.splitlines()

    def src(n):
        i = getattr(n, "lineno", 0)
        return lines[i - 1].strip()[:200] if 0 < i <= len(lines) else ""

    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and isinstance(getattr(node, "value", None), ast.Constant):
            v = node.value.value
            if isinstance(v, (int, float)) and not isinstance(v, bool) and v not in TRIVIAL:
                for t in node.targets:
                    if isinstance(t, ast.Name):
                        hits.append({"file": rel, "line": node.lineno, "cat": "MAGIC_NUM_CONST",
                                     "sub": "module", "name": t.id, "value": v, "src": src(node),
                                     "scope": "module"})
        if isinstance(node, ast.Call):
            fn = getattr(node.func, "attr", None) or getattr(node.func, "id", None)
            if fn in ("get", "getenv") and len(node.args) == 2 and isinstance(node.args[1], ast.Constant):
                hits.append({"file": rel, "line": node.lineno, "cat": "DEFAULT_FALLBACK",
                             "sub": fn, "value": node.args[1].value, "src": src(node)})
    for i, line in enumerate(lines, 1):
        for rx, sub in ((VERSION_RE, "version"), (IMAGE_RE, "image"), (HW_RE, "hw"),
                        (MODEL_RE, "model"), (MOCK_RE, "mock"), (HASH_RE, "hash"),
                        (TURN_RE, "turn")):
            if rx.search(line):
                hits.append({"file": rel, "line": i, "cat": "HARDCODE_STR", "sub": sub,
                             "src": line.strip()[:200]})
    return hits


def scan_sh(rel, text):
    hits = []
    for i, line in enumerate(text.splitlines(), 1):
        for rx, sub in ((VERSION_RE, "version"), (IMAGE_RE, "image"), (HW_RE, "hw"),
                        (MODEL_RE, "model"), (MOCK_RE, "mock"), (HASH_RE, "hash"),
                        (TURN_RE, "turn")):
            if rx.search(line):
                hits.append({"file": rel, "line": i, "cat": "HARDCODE_STR", "sub": sub,
                             "src": line.strip()[:200]})
        m = re.search(r':-\s*([0-9]+)\}', line)
        if m and int(m.group(1)) not in TRIVIAL:
            hits.append({"file": rel, "line": i, "cat": "DEFAULT_FALLBACK", "sub": "shell-default",
                         "value": int(m.group(1)), "src": line.strip()[:200]})
    return hits


def inventory(root=".", files=None):
    files = files or harness_files(root)
    hits = []
    for rel in files:
        full = os.path.join(root, rel)
        if not os.path.isfile(full):
            continue
        try:
            text = open(full, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        hits += scan_py(rel, text) if rel.endswith(".py") else scan_sh(rel, text)
    for h in hits:
        h["disposition"] = disposition(h)
    return hits


def staged_files(root="."):
    out = subprocess.run(["git", "-C", root, "diff", "--cached", "--name-only", "--diff-filter=ACMR"],
                         capture_output=True, text=True)
    if out.returncode != 0:
        return []
    return [p for p in out.stdout.splitlines() if p.endswith((".py", ".sh"))]


EXEMPT_RE = re.compile(r"antipattern-ok:\s*([A-Za-z0-9-]+)\s*[—:-]\s*(\S.*)")
DOC_QUOTES = ('"' * 3, "'" * 3)


def code_lines(text, is_py):
    """주석·독스트링을 **뺀** 코드 줄만 (번호, 내용) 으로 낸다.

    왜 필요한가(2026-09-05 첫 실행): tripwire 를 켜자마자 6건이 걸렸고 그중 3건이 **제거 사실을
    설명하는 산문**이었다(독스트링·주석). 서사를 금지하면 사람은 왜 없앴는지 적지 못하게 되고,
    그러면 다음 사람이 같은 것을 다시 만든다. 남은 3건은 픽스처와 tripwire 목록 자신이었고,
    그것들은 `antipattern-ok:<shape> — <사유>` 표식으로 **명시 면제**한다(면제는 보이는 곳에 적는다).
    """
    in_doc = None
    for i, line in enumerate(text.splitlines(), 1):
        stripped = line.strip()
        if is_py:
            if in_doc is not None:
                if in_doc in line:
                    in_doc = None
                continue
            if not stripped.startswith("#"):
                opened = None
                for q in DOC_QUOTES:
                    cnt = line.count(q)
                    if cnt and cnt % 2 == 1:
                        opened = q
                        break
                if opened is not None:
                    in_doc = opened
                    continue
        if stripped.startswith("#"):
            continue
        yield i, line

def tripwire(root=".", files=None):
    """제거한 형태가 되돌아왔는지만 본다. 반환: 위반 리스트(빈 리스트 = 통과)."""
    files = staged_files(root) if files is None else files
    violations = []
    for rel in files:
        # 스캐너 자신은 대상이 아니다: 여기 적힌 문자열은 **형태의 정의**이지 그 형태의 사례가
        # 아니며, 픽스처도 일부러 그 형태를 만든다. 자기 자신을 잡으면 tripwire 는 켜지는 순간
        # 영구 RED 가 되고, 그러면 사람은 검사를 끄는 법부터 배운다(2026-09-05 첫 실행에서 발생).
        if os.path.normpath(rel) == SELF_REL:
            continue
        full = os.path.join(root, rel)
        if not os.path.isfile(full):
            continue
        try:
            text = open(full, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        for shape in REMOVED_SHAPES:
            if not re.match(shape["path_re"], rel):
                continue
            for i, line in code_lines(text, rel.endswith(".py")):
                if not re.search(shape["pattern"], line):
                    continue
                ex = EXEMPT_RE.search(line)
                if ex and ex.group(1) == shape["id"]:
                    continue                     # 명시 면제 — 표식과 사유가 같은 줄에 있다
                violations.append({"shape": shape["id"], "file": rel, "line": i,
                                   "why": shape["why"], "src": line.strip()[:160]})
    return violations


def _self_test():
    import tempfile
    failures = []

    def check(name, cond, detail=""):
        print("  [%s] %s %s" % ("PASS" if cond else "FAIL", name, detail))
        if not cond:
            failures.append(name)

    with tempfile.TemporaryDirectory() as d:
        rel = ".claude/skills/terraforming_node/scripts/turn_budget.py"
        os.makedirs(os.path.join(d, os.path.dirname(rel)))
        open(os.path.join(d, rel), "w").write("GRADES = {'S': {'max_turns': 10}}\n")
        v = tripwire(d, [rel])
        check("T1 등급표 부활을 잡는다", len(v) == 1 and v[0]["shape"] == "G-A2-grade-table")

        open(os.path.join(d, rel), "w").write("# GRADES 표는 2026-09-05 에 삭제됐다(G-A2)\n")
        check("T2 주석의 서술은 잡지 않는다(제거 사실을 적을 수 있어야 한다)",
              tripwire(d, [rel]) == [])

        rel2 = ".claude/policies/runtime/providers/claude_code.py"
        os.makedirs(os.path.join(d, os.path.dirname(rel2)))
        open(os.path.join(d, rel2), "w").write('if request["model"] != "sonnet":\n    pass\n')
        check("T3 모델 게이트 부활을 잡는다",
              [x["shape"] for x in tripwire(d, [rel2])] == ["G-A1-sonnet-gate"])

        rel3 = ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh"
        os.makedirs(os.path.join(d, os.path.dirname(rel3)))
        open(os.path.join(d, rel3), "w").write('READY_MAX="${READY_MAX:-180}"\n')
        check("T4 READY_MAX 기본값 부활을 잡는다",
              [x["shape"] for x in tripwire(d, [rel3])] == ["G-B6-ready-max-default"])

        # 경로 좁힘: 같은 문자열이 무관한 파일에 있으면 잡지 않는다(오탐 억제).
        rel4 = ".claude/skills/wiki-desk/scripts/doc_naming.py"
        os.makedirs(os.path.join(d, os.path.dirname(rel4)))
        open(os.path.join(d, rel4), "w").write('GRADES = {"a": 1}\n')
        check("T5 무관 경로의 같은 문자열은 잡지 않는다", tripwire(d, [rel4]) == [])

        # 인벤토리: 모든 행에 처분이 붙는다(미기재 0).
        hits = inventory(d, [rel3, rel4])
        check("T6 인벤토리 모든 행에 disposition 이 있다",
              hits and all(h.get("disposition") for h in hits))
        check("T7 처분 어휘가 닫혀 있다",
              set(h["disposition"] for h in hits) <= {"fixture", "diagnostic", "local-const", "review"})

    # 실물: 이 저장소의 현재 상태는 tripwire 를 통과해야 한다(③ 이 실제로 제거했는지의 증거).
    repo = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
    # 자기참조 제외가 살아 있는지 — 없으면 tripwire 는 켜는 순간 영구 RED 다(첫 실행에서 실제로 그랬다).
    check("T7b 스캐너 자신은 대상에서 빠진다(형태의 정의 ≠ 형태의 사례)",
          tripwire(os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                                "..", "..", "..")), [SELF_REL]) == [])
    live = tripwire(repo, harness_files(repo))
    check("T8 저장소 현재 상태가 제거 tripwire 를 통과한다",
          live == [], "(위반 %s)" % [v["shape"] for v in live][:5])

    print("[antipattern_scan --self-test] %s" % ("OK — T1~T8 전부 통과" if not failures
                                                 else "FAIL %s" % failures))
    return 0 if not failures else 1


def main(argv=None):
    ap = argparse.ArgumentParser(description="하네스 안티패턴 인벤토리 + 제거 tripwire")
    ap.add_argument("--root", default=".")
    ap.add_argument("--inventory", metavar="OUT.jsonl")
    ap.add_argument("--tripwire", action="store_true")
    ap.add_argument("--files", nargs="*", default=None)
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args(argv)
    if a.self_test:
        return _self_test()
    if a.tripwire:
        v = tripwire(a.root, a.files)
        for x in v:
            print("[antipattern_scan] REVIVED %s — %s:%s\n    %s\n    사유: %s"
                  % (x["shape"], x["file"], x["line"], x["src"], x["why"]), file=sys.stderr)
        if v:
            print("[antipattern_scan] ③ 에서 제거한 형태가 %d건 되돌아왔다 — 우회하지 말고 고쳐라(D3)."
                  % len(v), file=sys.stderr)
            return RC_REVIVED
        return RC_OK
    if a.inventory:
        hits = inventory(a.root, a.files)
        with open(a.inventory, "w", encoding="utf-8") as f:
            for h in hits:
                f.write(json.dumps(h, ensure_ascii=False) + "\n")
        c = Counter(h["disposition"] for h in hits)
        byfile = Counter(h["file"] for h in hits)
        print("hits=%d files=%d → %s" % (len(hits), len(byfile), a.inventory))
        for k, n in sorted(c.items()):
            print("  %6d  %s" % (n, k))
        return RC_OK
    ap.print_help()
    return RC_ARGS


if __name__ == "__main__":
    sys.exit(main())
