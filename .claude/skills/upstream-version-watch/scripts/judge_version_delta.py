#!/usr/bin/env python3
"""버전 델타 영향판정 Judge — "동일빌드 vs 다른빌드"를 축별로 결정론화한다.

정본 계획 = `docs/plan/plan_26082112_버전델타_영향판정_Judge_강화안.md`.
현재 구현 범위 = **P1(D0+D1) + P2(D2)**. P3(D3+D4 · MCPS 폐포·심볼교차)는 미구현이며
`axis_C_model_path.verdict = "NOT_IMPLEMENTED"` 로 **명시**된다(조용한 생략 금지 — 계획 L5).

이 스크립트는 **증거 생산자**이지 합격 판정자가 아니다(계획 머리말). 최종 중재자는 여전히 스모크다.

── 축 (계획 §4.1) ──────────────────────────────────────────────────────────────
  A. 빌드입력 동일성 : 델타가 *우리* 빌드 입력(ABI·deps·csrc·빌드시스템)을 건드리는가
  B. 이식 스코프 교차 : 델타가 **우리 번들이 덮어쓰는 파일**과 겹치는가(+침묵-되돌림 프로브)
  C. 모델 코드경로   : 델타가 대상 모델이 실제로 도는 경로의 심볼에 닿는가  ← P3, 미구현

축별 verdict ∈ {NO_IMPACT, IMPACT, UNDETERMINED}. 전역 = **최악값 흡수**(worst-wins).
`UNDETERMINED` 는 게이팅상 `IMPACT` 와 같이 취급하되 **기록은 분리한다** —
미판정이 무영향으로 세탁되는 것이 이 설계가 막으려는 첫 번째 것이다(계획 L5).

── 결정론 규율 ────────────────────────────────────────────────────────────────
  • stdlib 만 사용(계획 E7). 외부 패키지 의존 없음.
  • 모든 값에 출처 필드(`provenance`·`delta_source`·`*_source`)를 붙인다(workflow.md §결정론 규율).
  • 벽시계 금지 — 시각은 `--generated-kst` 주입만 사용한다(docs.md §기계판독 데이터 평면 선례).
  • 추정 금지 — 델타 소스 둘 다 실패하면 비-0 으로 죽는다(계획 D0 fail-closed · R4).

사용:
  # 라이브(로컬 clone 1차 권위, 없으면 compare API 폴백)
  python3 judge_version_delta.py --from-ref v0.27.0 --to-ref v0.27.1 \
      --generated-kst 26082212 \
      [--repo-cache /path/to/vllm] [--provenance output/multi/build_patches_src/PROVENANCE.json] \
      [--resolved output/multi/resolved.json [--write-resolved]] [--emit-fixture <path>]

  # 골든 재생(네트워크·git 불요)
  python3 judge_version_delta.py --check-fixture <fixture.json>
  python3 judge_version_delta.py --fixture <fixture.json>          # 판정만 출력

  python3 judge_version_delta.py --self-test
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

SCHEMA_VERSION = 1
DEFAULT_REPO = "vllm-project/vllm"
HTTP_TIMEOUT = 30

# ── D1 · 닫힌 평면 열거 ──────────────────────────────────────────────────────
# 순서 있는 규칙. **첫 매치 승리**. 열거 밖 = UNKNOWN_PLANE = fail-closed (계획 E6/R3:
# "낡으면 막히지, 새지 않는다"). glob 규약: `*`·`?` 는 `/` 를 넘지 않고 `**` 는 넘는다.
#
# 근거(measured 2026-08-22):
#   ① vLLM v0.27.1 루트 트리 전수 — GET /repos/vllm-project/vllm/git/trees/v0.27.1
#   ② 우리 소스빌드가 상류에서 **실제로 소비하는 것** —
#      templates/Dockerfile.source-build.template L73-75(git clone) · L77(use_existing_torch.py)
#      · L82(pyproject[build-system]) · L167(pip install -e .).
#      → 우리는 상류 `docker/**` 도 `.buildkite/**` 도 소비하지 않는다. 그래서 그 둘은 axis_A 밖이다.
#
# ⚠ 오분류 비용은 비대칭이다: 누락(빌드입력을 딴 평면으로) → **거짓 NO_IMPACT**(위험),
#   과포함(무관한 것을 BUILD_INPUT 으로) → 불필요한 IMPACT(안전). 그래서 **보수적으로 넓게** 잡는다.
PLANE_RULES = [
    # ── BUILD_INPUT: 우리 이미지의 컴파일 산출물/ABI/의존 해소에 들어가는 것 ──
    ("pyproject.toml", "BUILD_INPUT"),
    ("setup.py", "BUILD_INPUT"),
    ("use_existing_torch.py", "BUILD_INPUT"),
    ("CMakeLists.txt", "BUILD_INPUT"),
    ("MANIFEST.in", "BUILD_INPUT"),
    ("build_rust.sh", "BUILD_INPUT"),
    ("build_vllm_ppc64le.sh", "BUILD_INPUT"),
    ("rust-toolchain.toml", "BUILD_INPUT"),
    ("cmake/**", "BUILD_INPUT"),
    ("csrc/**", "BUILD_INPUT"),
    ("requirements/**", "BUILD_INPUT"),
    ("rust/**", "BUILD_INPUT"),
    # vllm/ 안의 네이티브 소스도 컴파일 대상 → 보수적으로 빌드입력(RUNTIME_PY 규칙보다 앞).
    ("vllm/**.cu", "BUILD_INPUT"),
    ("vllm/**.cuh", "BUILD_INPUT"),
    ("vllm/**.cpp", "BUILD_INPUT"),
    ("vllm/**.cc", "BUILD_INPUT"),
    ("vllm/**.h", "BUILD_INPUT"),
    ("vllm/**.hpp", "BUILD_INPUT"),
    ("vllm/**.rs", "BUILD_INPUT"),
    # ── RUNTIME_PY: serve 시점에 도는 vllm 패키지 내용(py·데이터) ──
    ("vllm/**", "RUNTIME_PY"),
    # ── UPSTREAM_OWN_DOCKER: 상류 자체 도커 자산. 우리는 자체 템플릿으로 렌더한다 ──
    ("docker/**", "UPSTREAM_OWN_DOCKER"),
    (".dockerignore", "UPSTREAM_OWN_DOCKER"),
    # ── CI: 상류 CI/lint 설정 ──
    (".buildkite/**", "CI"),
    (".github/**", "CI"),
    (".pre-commit-config.yaml", "CI"),
    (".clang-format", "CI"),
    (".markdownlint.yaml", "CI"),
    (".shellcheckrc", "CI"),
    (".coveragerc", "CI"),
    ("codecov.yml", "CI"),
    (".gitignore", "CI"),
    (".git-blame-ignore-revs", "CI"),
    (".readthedocs.yaml", "CI"),
    # ── TESTS ──
    ("tests/**", "TESTS"),
    # ── DOCS ──
    ("docs/**", "DOCS"),
    ("mkdocs.yaml", "DOCS"),
    ("LICENSE", "DOCS"),
    ("DCO", "DOCS"),
    ("*.md", "DOCS"),
    # ── TOOLING: 상류 개발자 도구. 우리 이미지에 들어가지 않는다 ──
    ("benchmarks/**", "TOOLING"),
    ("examples/**", "TOOLING"),
    ("scripts/**", "TOOLING"),
    ("tools/**", "TOOLING"),
    # ── REPO_META: 상류 리포의 에이전트/편집기 메타 ──
    (".claude/**", "REPO_META"),
    (".gemini/**", "REPO_META"),
]

BUILD_INPUT_GLOBS = [g for g, p in PLANE_RULES if p == "BUILD_INPUT"]
UNKNOWN_PLANE = "UNKNOWN_PLANE"

VERDICT_RANK = {"NO_IMPACT": 0, "IMPACT": 1, "UNDETERMINED": 2}
# 종료코드 계약: 0=NO_IMPACT · 1=IMPACT · 4=UNDETERMINED. 게이팅은 호출부가 한다.
#   라이브·재생 **양 분기가 같은 계약**을 진다(한쪽만 지키면 그 분기가 새는 문이 된다).
VERDICT_EXIT = {"NO_IMPACT": 0, "IMPACT": 1, "UNDETERMINED": 4, "NOT_IMPLEMENTED": 4}


def glob_to_re(pat):
    """결정론 glob→regex. `**`=경로구분자 통과, `*`/`?`=구분자 미통과."""
    out, i = ["^"], 0
    while i < len(pat):
        c = pat[i]
        if c == "*":
            if pat.startswith("**", i):
                out.append(".*")
                i += 2
            else:
                out.append("[^/]*")
                i += 1
            continue
        if c == "?":
            out.append("[^/]")
        else:
            out.append(re.escape(c))
        i += 1
    out.append("$")
    return re.compile("".join(out))


_PLANE_RE = [(g, p, glob_to_re(g)) for g, p in PLANE_RULES]


def classify_plane(path):
    """→ (plane, matched_glob). 열거 밖이면 (UNKNOWN_PLANE, None)."""
    for glob, plane, rx in _PLANE_RE:
        if rx.match(path):
            return plane, glob
    return UNKNOWN_PLANE, None


# ── 공용 ────────────────────────────────────────────────────────────────────
def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def _portable_path(path):
    """산출물에 적을 경로 → 리포 안이면 repo-relative, 밖이면 None.

    attestation·fixture 는 **배포되는 추적 산출물**이라 운영자 절대경로(환경 지문)를 담지 않는다
    (docs.md §PII 스캔 적용 범위 — 배포 산출물은 `abs-op-path` 포함 4종 전부). 리포 밖 경로의
    의미는 `--provenance-origin` 라벨이 진다."""
    if not path:
        return None
    repo = os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__)))))
    ap = os.path.abspath(path)
    rel = os.path.relpath(ap, repo)
    return rel if not rel.startswith(os.pardir) else None


def die(msg, code=2):
    print(json.dumps({"error": msg}, ensure_ascii=False, indent=2), file=sys.stderr)
    sys.exit(code)


def _norm_status(raw):
    """git name-status 문자 / compare API 문자열 → 공통 어휘."""
    if not raw:
        return "unknown"
    r = str(raw)
    table = {"M": "modified", "A": "added", "D": "removed", "T": "typechanged",
             "C": "copied", "R": "renamed"}
    if r[0] in table and (len(r) == 1 or r[1:].isdigit()):
        return table[r[0]]
    return {"modified": "modified", "added": "added", "removed": "removed",
            "renamed": "renamed", "copied": "copied", "changed": "modified",
            "unchanged": "unchanged"}.get(r, r)


# ── D0 · 델타 수집 ──────────────────────────────────────────────────────────
def _git(cache, args, check=True):
    cmd = ["git", "-c", "core.quotePath=false", "-C", cache] + args
    p = subprocess.run(cmd, capture_output=True, text=False)
    if check and p.returncode != 0:
        raise RuntimeError(f"git 실패(rc={p.returncode}): {' '.join(args)}\n"
                           f"{p.stderr.decode('utf-8', 'replace').strip()}")
    return p


def _count_hunks_from_patch(text):
    if text is None:
        return None
    return sum(1 for ln in text.splitlines() if ln.startswith("@@ "))


def collect_delta_git(cache, from_ref, to_ref):
    """1차 권위 — 로컬 clone. `git diff --name-status/--numstat <from>..<to>` (계획 D0)."""
    from_sha = _git(cache, ["rev-parse", f"{from_ref}^{{commit}}"]).stdout.decode().strip()
    to_sha = _git(cache, ["rev-parse", f"{to_ref}^{{commit}}"]).stdout.decode().strip()
    rng = f"{from_ref}..{to_ref}"

    # name-status (-z: 경로 인용 회피)
    raw = _git(cache, ["diff", "--name-status", "-z", rng]).stdout.decode("utf-8", "replace")
    toks = [t for t in raw.split("\0") if t != ""]
    status_by_path, prev_by_path, i = {}, {}, 0
    while i < len(toks):
        st = toks[i]
        if st[0] in ("R", "C"):
            old, new = toks[i + 1], toks[i + 2]
            status_by_path[new] = st
            prev_by_path[new] = old
            i += 3
        else:
            status_by_path[toks[i + 1]] = st
            i += 2

    # numstat
    raw = _git(cache, ["diff", "--numstat", "-z", rng]).stdout.decode("utf-8", "replace")
    toks = [t for t in raw.split("\0") if t != ""]
    nums, i = {}, 0
    while i < len(toks):
        parts = toks[i].split("\t")
        add, dele = parts[0], parts[1]
        if len(parts) >= 3 and parts[2]:
            path = parts[2]
            i += 1
        else:                                    # rename/copy: 다음 두 토큰이 old/new
            path = toks[i + 2]
            i += 3
        nums[path] = (None if add == "-" else int(add), None if dele == "-" else int(dele))

    # hunks: 전체 diff 1회 파싱
    diff_txt = _git(cache, ["diff", "-U0", "--no-color", rng]).stdout.decode("utf-8", "replace")
    hunks, cur = {}, None
    for ln in diff_txt.splitlines():
        if ln.startswith("diff --git "):
            m = re.match(r"^diff --git a/(.*) b/(.*)$", ln)
            cur = m.group(2) if m else None
            if cur is not None:
                hunks.setdefault(cur, 0)
        elif ln.startswith("@@ ") and cur is not None:
            hunks[cur] = hunks.get(cur, 0) + 1

    files = []
    for path in sorted(set(status_by_path) | set(nums)):
        add, dele = nums.get(path, (None, None))
        rec = {"path": path, "status": _norm_status(status_by_path.get(path)),
               "status_raw": status_by_path.get(path),
               "additions": add, "deletions": dele,
               "hunks": hunks.get(path)}
        if path in prev_by_path:
            rec["previous_path"] = prev_by_path[path]
        files.append(rec)
    return {"files": files, "from_sha": from_sha, "to_sha": to_sha,
            "delta_source": "git-local", "delta_source_detail": os.path.abspath(cache)}


def _http_get(url, accept="application/vnd.github+json"):
    req = urllib.request.Request(url, headers={
        "Accept": accept,
        "User-Agent": "judge_version_delta.py (easy_vllm_simulator upstream-version-watch)",
    })
    tok = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if tok:
        req.add_header("Authorization", f"Bearer {tok}")
    with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as r:
        return r.read()


def collect_delta_api(repo, from_ref, to_ref):
    """폴백 — GitHub compare API (계획 E10). `delta_source:"github-compare-api"` 라벨을 단다."""
    url = f"https://api.github.com/repos/{repo}/compare/{from_ref}...{to_ref}"
    data = json.loads(_http_get(url).decode("utf-8"))
    files = []
    for f in data.get("files", []):
        rec = {"path": f["filename"], "status": _norm_status(f.get("status")),
               "status_raw": f.get("status"),
               "additions": f.get("additions"), "deletions": f.get("deletions"),
               "hunks": _count_hunks_from_patch(f.get("patch"))}
        if f.get("patch") is None:
            rec["hunks_source"] = "unavailable(patch 미제공 — binary/대형)"
        if f.get("previous_filename"):
            rec["previous_path"] = f["previous_filename"]
        files.append(rec)
    files.sort(key=lambda r: r["path"])
    commits = data.get("commits") or []
    from_sha = (data.get("base_commit") or {}).get("sha")
    to_sha = commits[-1]["sha"] if commits else from_sha
    return {"files": files, "from_sha": from_sha, "to_sha": to_sha,
            "delta_source": "github-compare-api", "delta_source_detail": url,
            "ahead_by": data.get("ahead_by"), "total_commits": data.get("total_commits")}


def collect_delta(repo, from_ref, to_ref, repo_cache):
    """로컬 clone 1차 · API 폴백. **둘 다 실패하면 죽는다 — 추정 금지**(계획 D0)."""
    errors = []
    if repo_cache:
        try:
            return collect_delta_git(repo_cache, from_ref, to_ref)
        except Exception as e:                      # fail-loud 폴백(로그 남김) — 침묵 폴백 아님
            errors.append(f"git-local: {e}")
            print(f"[judge] WARN git-local 델타 수집 실패 → API 폴백: {e}", file=sys.stderr)
    try:
        return collect_delta_api(repo, from_ref, to_ref)
    except Exception as e:
        errors.append(f"github-compare-api: {e}")
    die("델타 수집 실패 — 로컬 clone·compare API 둘 다 불가. **추정하지 않는다**(계획 D0/R4). "
        + " | ".join(errors), code=3)


# ── 상류 blob 조회 (D2 프로브 입력) ─────────────────────────────────────────
class BlobReader:
    """상류 파일 @ref → sha256. 결과는 ('ok', sha) / ('absent', None) / ('error', msg).

    `absent`(상류에 그 ref 에서 파일이 없음)와 `error`(조회 실패)를 **구분한다** —
    구분하지 않으면 fail-closed 가 무엇 때문인지 알 수 없다."""

    def __init__(self, repo, repo_cache=None, recorded=None, to_sha_hint=None):
        self.repo, self.cache, self.recorded = repo, repo_cache, recorded or {}
        self.seen = {}
        self.to_sha_hint = to_sha_hint        # to_ref 의 커밋 sha(파생 베이스 선언 대조용)

    def sha(self, ref, path):
        key = (ref, path)
        if key in self.seen:
            return self.seen[key]
        res = self._lookup(ref, path)
        self.seen[key] = res
        return res

    def _lookup(self, ref, path):
        if self.recorded:                                   # fixture 재생: 기록된 사실만
            table = self.recorded.get(ref)
            if table is None or path not in table:
                return ("error", f"fixture 에 {ref}:{path} 기록 없음")
            v = table[path]
            return ("absent", None) if v is None else ("ok", v)
        if self.cache:
            try:
                p = _git(self.cache, ["cat-file", "-e", f"{ref}:{path}"], check=False)
                if p.returncode != 0:
                    return ("absent", None)
                blob = _git(self.cache, ["show", f"{ref}:{path}"]).stdout
                return ("ok", sha256_bytes(blob))
            except Exception as e:
                return ("error", f"git-local: {e}")
        url = f"https://raw.githubusercontent.com/{self.repo}/{ref}/{path}"
        try:
            return ("ok", sha256_bytes(_http_get(url, accept="*/*")))
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return ("absent", None)
            return ("error", f"raw HTTP {e.code}")
        except Exception as e:
            return ("error", f"raw: {e}")

    def source_label(self):
        if self.recorded:
            return "fixture-recorded"
        return "git-local" if self.cache else "raw.githubusercontent.com"


# ── D1 · 평면 분류 + axis_A ─────────────────────────────────────────────────
def classify_files(delta_files):
    out = []
    for rec in delta_files:
        plane, glob = classify_plane(rec["path"])
        r = dict(rec)
        r["plane"] = plane
        r["plane_source"] = "closed-enum"
        r["plane_rule"] = glob
        out.append(r)
    return out


def axis_a(files):
    unknown_paths = [f["path"] for f in files if f["plane"] == UNKNOWN_PLANE]
    delta = [f["path"] for f in files if f["plane"] == "BUILD_INPUT"]
    if unknown_paths:
        # 열거 밖 경로는 **빌드 입력일 수도** 있다 → 무영향을 주장할 수 없다(E6).
        verdict = "UNDETERMINED"
    elif delta:
        verdict = "IMPACT"
    else:
        verdict = "NO_IMPACT"
    return {
        "verdict": verdict,
        "delta": delta,
        "checked": list(BUILD_INPUT_GLOBS),
        "checked_source": "closed-enum(PLANE_RULES BUILD_INPUT)",
        "unknown_plane_paths": unknown_paths,
        "note": ("열거 밖 경로가 있어 빌드입력 무영향을 주장할 수 없다(계획 E6)."
                 if unknown_paths else
                 "상류 delta 중 우리 빌드입력 평면에 드는 파일 없음."),
    }


# ── D2 · 이식 스코프 교차 + 침묵-되돌림 프로브 ──────────────────────────────
def axis_b(files, provenance, bundle_files_root, reader, from_ref, to_ref,
           scope_origin=None, scope_path=None):
    if provenance is None:
        return {"verdict": "NO_IMPACT", "applicable": False,
                "overlap": [], "probes": [], "silent_revert_risk": [],
                "note": "이식 번들(PROVENANCE) 미제공 — stock 트랙. 교차할 스코프가 구조적으로 없다."}, []

    scope = provenance.get("files") or {}
    if not isinstance(scope, dict) or not scope:
        return {"verdict": "UNDETERMINED", "applicable": True,
                "overlap": [], "probes": [], "silent_revert_risk": [],
                "note": "PROVENANCE.files 가 비었거나 dict 가 아님 — 스코프 미해소."}, \
               ["PROVENANCE.files 미해소"]

    # 번들이 **어느 상류 좌표에서 파생됐다고 선언하는가**. 되돌림 판정의 보조 증거다.
    #   ⚠ 이것은 **선언이지 증명이 아니다** — 선언이 to_ref 라도 내용이 낡았을 수 있으므로
    #   verdict 를 낮추는 데 쓰지 않는다(fail-closed 유지). 사람 판정용 증거로만 붙는다.
    psrc = provenance.get("source") or {}
    base_tag, base_sha = psrc.get("upstream_base_tag"), psrc.get("upstream_base_sha")

    delta_paths = [f["path"] for f in files]
    overlap = sorted(set(delta_paths) & set(scope))
    risks, unknown = [], []
    for path in overlap:
        entry = {"path": path}
        # ① 번들 사본의 sha256 — 실파일 우선, 없으면 PROVENANCE 선언값
        bundle_sha, bsrc = None, None
        if bundle_files_root:
            fp = os.path.join(bundle_files_root, path)
            if os.path.isfile(fp) and not os.path.islink(fp):
                with open(fp, "rb") as fh:
                    bundle_sha = sha256_bytes(fh.read())
                bsrc = "bundle-file"
        if bundle_sha is None:
            bundle_sha, bsrc = scope.get(path), "provenance-declared"
        entry["bundle_sha"] = bundle_sha
        entry["bundle_sha_source"] = bsrc
        # ② 상류 @to_ref / @from_ref
        st_to, sha_to = reader.sha(to_ref, path)
        st_from, sha_from = reader.sha(from_ref, path)
        entry["upstream_to_sha"] = sha_to
        entry["upstream_from_sha"] = sha_from
        entry["upstream_sha_source"] = reader.source_label()
        # ③ 판정
        if bundle_sha is None:
            entry["verdict"] = "UNDETERMINED"
            entry["reason"] = "번들 사본 sha 미해소(PROVENANCE 선언·실파일 모두 부재)"
        elif st_to == "error" or st_from == "error":
            entry["verdict"] = "UNDETERMINED"
            entry["reason"] = f"상류 조회 실패(to={st_to}:{sha_to} from={st_from}:{sha_from})"
        elif st_to == "absent":
            entry["verdict"] = "UNDETERMINED"
            entry["reason"] = ("상류 @to_ref 에 파일 부재 — 번들이 삭제된 파일을 되살린다. "
                               "자동 판정 대상 아님(사람 판정 필요)")
        elif bundle_sha == sha_to:
            entry["verdict"] = "NO_REVERT"
            entry["reason"] = "번들 사본이 상류 @to_ref 와 byte-identical"
        elif st_from == "absent":
            entry["verdict"] = "WOULD_REVERT"
            entry["reason"] = "상류 @to_ref 신규 파일을 번들이 다른 내용으로 덮는다"
        elif sha_from == sha_to:
            entry["verdict"] = "DIVERGENT_NO_DELTA"
            entry["reason"] = ("상류 내용이 from↔to 동일(rename/mode 변경 등) — "
                               "번들이 상류와 다르지만 되돌릴 델타가 없다")
        else:
            entry["verdict"] = "WOULD_REVERT"
            entry["reason"] = ("번들 사본이 상류 @to_ref 와 다르고 상류가 from→to 사이에 변했다 "
                               "→ 이식본을 그대로 얹으면 그 변경이 조용히 되돌아간다")
        entry["bundle_equals_upstream_from"] = (bundle_sha is not None and bundle_sha == sha_from)
        entry["bundle_equals_upstream_to"] = (bundle_sha is not None and bundle_sha == sha_to)
        # 파생 베이스 선언(증거 — 판정 아님)
        entry["bundle_base_tag"] = base_tag
        entry["bundle_base_sha"] = base_sha
        entry["bundle_base_source"] = "PROVENANCE.source (선언 · 미검증)"
        entry["bundle_base_is_to_ref"] = (
            None if not (base_tag or base_sha)
            else (base_tag == to_ref or (base_sha is not None and base_sha == reader.to_sha_hint)))
        if entry["verdict"] == "WOULD_REVERT" and entry["bundle_base_is_to_ref"]:
            entry["note"] = (
                "번들이 to_ref 에서 파생됐다고 **선언**한다 → 되돌림이 아니라 **파생 변종**일 수 있다. "
                "그러나 선언은 증명이 아니므로 verdict 는 fail-closed 로 유지한다. "
                "확증은 내용 술어(예 plan_26082111 S2-c)와 스모크가 한다.")
        if entry["verdict"] == "UNDETERMINED":
            unknown.append(f"axis_B:{path}: {entry['reason']}")
        risks.append(entry)

    # §5.3 의 정지 조건은 `silent_revert_risk == []` 이다 → 이 리스트에는 **실제 위험분만** 담는다.
    #   전체 프로브는 `probes` 에 남긴다(NO_REVERT 도 증거다 — 침묵 삭제 금지).
    at_risk = [r for r in risks if r["verdict"] in ("WOULD_REVERT", "UNDETERMINED")]
    if any(r["verdict"] == "UNDETERMINED" for r in risks):
        verdict = "UNDETERMINED"
    elif overlap:
        verdict = "IMPACT"
    else:
        verdict = "NO_IMPACT"
    return {
        "verdict": verdict,
        "applicable": True,
        "scope_files": len(scope),
        "scope_source": "PROVENANCE.files",
        "scope_path": scope_path,
        "scope_origin": scope_origin or "in-tree path",
        "scope_base_tag": base_tag,
        "scope_base_sha": base_sha,
        "overlap": overlap,
        "probes": risks,
        "silent_revert_risk": at_risk,
        "note": ("이식 스코프와 델타가 겹친다 → `regen_build_patches_src.py derive` 재파생 필수. "
                 "재파생 후 silent_revert_risk == [] 가 될 때까지 진행 금지(계획 §5.3)."
                 if overlap else "이식 스코프와 델타 교차 없음."),
    }, unknown


# ── 손저작 블록 감사 (계획 §1.1 흔적 A) ─────────────────────────────────────
_DELTA_RE = re.compile(r"^\s*\+(\d+)\s*/\s*[-−](\d+)\s*$")


def audit_manual_block(resolved, files, from_ref, to_ref):
    """resolved.json 의 손저작 `upstream_delta_<a>_to_<b>` 를 실측과 대조한다."""
    if not resolved:
        return None
    keys = [k for k in resolved if k.startswith("upstream_delta_") and k != "upstream_delta"]
    if not keys:
        return None
    key = keys[0]
    block = resolved[key] or {}
    measured = {f["path"]: f for f in files}
    disc = []
    for d in block.get("detail") or []:
        path = d.get("file")
        m = measured.get(path)
        if m is None:
            disc.append({"path": path, "field": "presence",
                         "manual": "present", "measured": "absent",
                         "note": "손저작에 있으나 실측 델타에 없음"})
            continue
        mm = _DELTA_RE.match(str(d.get("delta", "")))
        if not mm:
            disc.append({"path": path, "field": "delta", "manual": d.get("delta"),
                         "measured": f"+{m['additions']}/-{m['deletions']}",
                         "note": "손저작 delta 문자열 파싱 불가"})
            continue
        ma, md = int(mm.group(1)), int(mm.group(2))
        if ma != m["additions"] or md != m["deletions"]:
            disc.append({"path": path, "field": "delta",
                         "manual": f"+{ma}/-{md}",
                         "measured": f"+{m['additions']}/-{m['deletions']}",
                         "note": "전사 오류 — 사실 행은 스크립트 JSON 을 그대로 테이블화해야 한다"
                                 "(resolve-and-render.md §2 사실/판단 분리)"})
    for path in measured:
        if path not in {d.get("file") for d in (block.get("detail") or [])}:
            disc.append({"path": path, "field": "presence", "manual": "absent",
                         "measured": "present", "note": "실측 델타에 있으나 손저작에 없음"})
    nfiles = block.get("files")
    if nfiles is not None and int(nfiles) != len(files):
        disc.append({"path": "(총계)", "field": "files", "manual": nfiles,
                     "measured": len(files), "note": "파일 수 불일치"})
    return {"manual_key": key, "manual_source": block.get("_source"),
            "discrepancies": disc, "match": not disc,
            "note": ("손저작 블록이 실측과 일치" if not disc else
                     f"손저작 블록에서 {len(disc)}건 불일치 검출 — 스크립트 산출로 대체해야 한다")}


# ── D5 · verdict 합성 ───────────────────────────────────────────────────────
AXIS_C_STUB = {
    "verdict": "NOT_IMPLEMENTED",
    "implemented": False,
    "excluded_from_global": True,
    "note": ("P3(D3 MCPS 폐포 + D4 심볼 교차) 미구현 — 별도 승인 대기"
             "(plan_26082112 §6 P3, advisory-only 착수 예정). "
             "**C축 미구현은 '기능 무영향'을 뜻하지 않는다**; 그 판정은 이 attestation 밖에 있다."),
}


def synthesize(meta, files, ax_a, ax_b, unknown, manual_audit):
    axes = {"axis_A_build_input": ax_a, "axis_B_port_scope": ax_b}
    if unknown:
        verdict = "UNDETERMINED"
    else:
        verdict = max((v["verdict"] for v in axes.values()), key=lambda x: VERDICT_RANK[x])
    out = dict(meta)
    out["files"] = files
    out["axis_A_build_input"] = ax_a
    out["axis_B_port_scope"] = ax_b
    out["axis_C_model_path"] = dict(AXIS_C_STUB)
    out["axes_in_global"] = ["axis_A_build_input", "axis_B_port_scope"]
    out["verdict"] = verdict
    out["unknown"] = unknown
    out["limitations"] = [
        "C축(모델 코드경로 도달성) 미구현 — 전역 verdict 에 기능 무영향 근거는 들어있지 않다.",
        "정적 판정은 증명이 아니다(E3 Hyrum · E9 정적 불건전성) — 스모크가 최종 중재자다.",
        "NO_IMPACT 는 '모든 구현된 축이 완전 커버리지로 계산됨' 을 뜻하며 스모크 면제가 아니다.",
    ]
    if manual_audit is not None:
        out["manual_block_audit"] = manual_audit
    return out


# ── 파이프라인 ──────────────────────────────────────────────────────────────
def run_pipeline(*, repo, from_ref, to_ref, generated_kst, repo_cache=None,
                 provenance=None, bundle_files_root=None, resolved=None,
                 replay=None, scope_origin=None, scope_path=None):
    """replay 가 주어지면 fixture 기록값으로 D0/blob 조회를 대체한다(네트워크·git 불요)."""
    if replay is not None:
        delta = {"files": [dict(f) for f in replay["delta_files"]],
                 "from_sha": replay.get("from_sha"), "to_sha": replay.get("to_sha"),
                 "delta_source": replay.get("delta_source", "unknown"),
                 "delta_source_detail": replay.get("delta_source_detail"),
                 "ahead_by": replay.get("ahead_by"),
                 "total_commits": replay.get("total_commits")}
        reader = BlobReader(repo, recorded=replay.get("upstream_blob_sha"),
                            to_sha_hint=replay.get("to_sha"))
    else:
        delta = collect_delta(repo, from_ref, to_ref, repo_cache)
        reader = BlobReader(repo, repo_cache=repo_cache, to_sha_hint=delta.get("to_sha"))

    files = classify_files(delta["files"])
    ax_a = axis_a(files)
    ax_b, unk_b = axis_b(files, provenance, bundle_files_root, reader, from_ref, to_ref,
                         scope_origin=scope_origin, scope_path=scope_path)
    unknown = [f"UNKNOWN_PLANE: {p}" for p in ax_a["unknown_plane_paths"]] + unk_b

    meta = {
        "schema_version": SCHEMA_VERSION,
        "from_ref": from_ref, "to_ref": to_ref,
        "from_sha": delta.get("from_sha"), "to_sha": delta.get("to_sha"),
        "provenance": "measured",
        "delta_source": delta["delta_source"],
        "delta_source_detail": delta.get("delta_source_detail"),
        "repo": repo,
        "generated_kst": generated_kst,
        "judge": "judge_version_delta.py (P1:D0+D1 · P2:D2)",
    }
    if delta.get("ahead_by") is not None:
        meta["ahead_by"] = delta["ahead_by"]
    if delta.get("total_commits") is not None:
        meta["total_commits"] = delta["total_commits"]

    manual_audit = audit_manual_block(resolved, files, from_ref, to_ref)
    out = synthesize(meta, files, ax_a, ax_b, unknown, manual_audit)
    return out, reader


# ── fixture ─────────────────────────────────────────────────────────────────
# 라이브 실행 ↔ fixture 재생 사이에서 **정당하게** 달라지는 필드(=출처 라벨).
# 이 목록 밖의 차이는 emit 단계에서 fail-loud 로 죽는다 — fixture 가 자기충족적이 되는 것을 막는다.
REPLAY_LABEL_SUFFIXES = (".upstream_sha_source",)


def strip_volatile(obj):
    """fixture 대조에서 제외할 재생-메타를 벗긴다."""
    o = json.loads(json.dumps(obj, ensure_ascii=False, sort_keys=True))
    o.pop("replay_of", None)
    return o


def deep_diff(a, b, path="$"):
    out = []
    if type(a) is not type(b) and not (isinstance(a, (int, float)) and isinstance(b, (int, float))):
        return [f"{path}: 타입 불일치 {type(a).__name__} vs {type(b).__name__}"]
    if isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a:
                out.append(f"{path}.{k}: expected 에만 있음 → {json.dumps(b[k], ensure_ascii=False)[:120]}")
            elif k not in b:
                out.append(f"{path}.{k}: actual 에만 있음 → {json.dumps(a[k], ensure_ascii=False)[:120]}")
            else:
                out += deep_diff(a[k], b[k], f"{path}.{k}")
    elif isinstance(a, list):
        if len(a) != len(b):
            out.append(f"{path}: 길이 {len(a)} vs {len(b)}")
        for i in range(min(len(a), len(b))):
            out += deep_diff(a[i], b[i], f"{path}[{i}]")
    elif a != b:
        out.append(f"{path}: {json.dumps(a, ensure_ascii=False)} vs {json.dumps(b, ensure_ascii=False)}")
    return out


def replay_from_fixture(fx):
    inp = fx["inputs"]
    out, _ = run_pipeline(
        repo=inp.get("repo", DEFAULT_REPO),
        from_ref=inp["from_ref"], to_ref=inp["to_ref"],
        generated_kst=inp["generated_kst"],
        provenance=inp.get("provenance"),
        bundle_files_root=None,
        resolved=inp.get("resolved_manual_block"),
        replay=inp,
        scope_origin=inp.get("scope_origin"), scope_path=inp.get("scope_path"),
    )
    out["replay_of"] = {"fixture": fx.get("case"), "recorded_kst": fx.get("recorded_kst"),
                        "note": "이 실행은 fixture 기록 사실을 재생한 것이다(재측정 아님)."}
    return out


# ── main ────────────────────────────────────────────────────────────────────
def main():
    ap = argparse.ArgumentParser(
        description="judge_version_delta.py — 버전 델타 영향판정 Judge (P1:D0+D1 · P2:D2)")
    ap.add_argument("--repo", default=DEFAULT_REPO, help="상류 리포 owner/name")
    ap.add_argument("--from-ref", help="기준 ref (예 v0.27.0)")
    ap.add_argument("--to-ref", help="대상 ref (예 v0.27.1)")
    ap.add_argument("--generated-kst", help="발행 시각 YYMMDDHH (주입 필수 — 벽시계 금지)")
    ap.add_argument("--repo-cache", help="로컬 vLLM clone (1차 권위). 없으면 compare API 폴백")
    ap.add_argument("--provenance", help="이식 번들 PROVENANCE.json (이식 트랙일 때만)")
    ap.add_argument("--bundle-root", help="build_patches_src 루트(기본=--provenance 의 디렉터리)")
    ap.add_argument("--provenance-origin",
                    help="PROVENANCE 의 출처 라벨(예 'git:<sha>:<path>') — in-tree 경로가 아닐 때 기재")
    ap.add_argument("--no-bundle-files", action="store_true",
                    help="번들 실파일을 읽지 않고 PROVENANCE 선언 sha 만 쓴다")
    ap.add_argument("--resolved", help="resolved.json — 손저작 블록 감사/대체 대상")
    ap.add_argument("--write-resolved", action="store_true",
                    help="resolved.json 에 upstream_delta 를 써넣는다(손저작 블록은 _superseded_ 로 보존)")
    ap.add_argument("--emit-fixture", help="이번 실행을 골든 fixture 로 기록")
    ap.add_argument("--fixture", help="fixture 재생(판정만 출력)")
    ap.add_argument("--check-fixture", help="fixture 재생 후 expected 와 대조 (0=PASS)")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()

    if a.self_test:
        return self_test()

    if a.fixture or a.check_fixture:
        path = a.fixture or a.check_fixture
        with open(path, encoding="utf-8") as f:
            fx = json.load(f)
        out = replay_from_fixture(fx)
        if a.check_fixture:
            diffs = deep_diff(strip_volatile(out), strip_volatile(fx["expected"]))
            report = {"fixture": path, "case": fx.get("case"),
                      "result": "PASS" if not diffs else "FAIL",
                      "diff_count": len(diffs), "diffs": diffs[:60]}
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 0 if not diffs else 1
        print(json.dumps(out, ensure_ascii=False, indent=2))
        # 재생도 라이브와 **같은 종료코드 계약**을 진다 — 한쪽 분기만 계약을 지키면
        # 호출부가 `judge --fixture … && proceed` 로 UNDETERMINED 를 통과시킨다.
        return VERDICT_EXIT[out["verdict"]]

    for req in ("from_ref", "to_ref", "generated_kst"):
        if not getattr(a, req):
            die(f"--{req.replace('_', '-')} 필요 (벽시계·추정 금지)")

    provenance, bundle_files_root = None, None
    if a.provenance:
        with open(a.provenance, encoding="utf-8") as f:
            provenance = json.load(f)
        if not a.no_bundle_files:
            root = a.bundle_root or os.path.dirname(os.path.abspath(a.provenance))
            cand = os.path.join(root, "files")
            bundle_files_root = cand if os.path.isdir(cand) else None

    resolved, resolved_obj = None, None
    if a.resolved:
        with open(a.resolved, encoding="utf-8") as f:
            resolved_obj = json.load(f)
        resolved = resolved_obj

    out, reader = run_pipeline(
        repo=a.repo, from_ref=a.from_ref, to_ref=a.to_ref,
        generated_kst=a.generated_kst, repo_cache=a.repo_cache,
        provenance=provenance, bundle_files_root=bundle_files_root, resolved=resolved,
        scope_origin=a.provenance_origin,
        scope_path=_portable_path(a.provenance))

    if a.emit_fixture:
        recorded = {}
        for (ref, path), (st, sha) in reader.seen.items():
            recorded.setdefault(ref, {})[path] = None if st == "absent" else sha
        inputs = {
            "repo": a.repo, "from_ref": a.from_ref, "to_ref": a.to_ref,
            "from_sha": out.get("from_sha"), "to_sha": out.get("to_sha"),
            "generated_kst": a.generated_kst,
            "delta_source": out["delta_source"],
            "delta_source_detail": out.get("delta_source_detail"),
            "ahead_by": out.get("ahead_by"),
            "total_commits": out.get("total_commits"),
            "delta_files": [{k: v for k, v in f.items()
                             if k not in ("plane", "plane_source", "plane_rule")}
                            for f in out["files"]],
            "upstream_blob_sha": recorded,
        }
        if provenance is not None:
            inputs["provenance"] = provenance
            inputs["scope_origin"] = a.provenance_origin
            inputs["scope_path"] = _portable_path(a.provenance)
        if resolved_obj is not None:
            keys = [k for k in resolved_obj if k.startswith("upstream_delta_") and k != "upstream_delta"]
            if keys:
                inputs["resolved_manual_block"] = {k: resolved_obj[k] for k in keys}
        # expected 는 **재생 산출물**로 고정한다(=`--check-fixture` 가 제외 필드 없이 전량 대조).
        # 동시에 라이브 산출물과 대조해 **출처 라벨 밖의 차이가 0** 임을 증명한다 — 이 게이트가 없으면
        # fixture 가 자기 자신을 검증하는 자기충족 루프가 된다.
        replayed = replay_from_fixture({"case": None, "recorded_kst": a.generated_kst,
                                        "inputs": inputs})
        residual = [d for d in deep_diff(strip_volatile(replayed), strip_volatile(out))
                    if not any(d.split(":")[0].endswith(sfx) for sfx in REPLAY_LABEL_SUFFIXES)]
        if residual:
            die("fixture 재생이 라이브 산출물과 출처 라벨 밖에서 갈린다 — fixture 발행 중단:\n  "
                + "\n  ".join(residual[:20]), code=5)
        expected = strip_volatile(replayed)
        fx = {"fixture_version": 1,
              "case": f"vLLM {a.from_ref} → {a.to_ref} · judge_version_delta 골든",
              "recorded_kst": a.generated_kst,
              "note": ("inputs 는 기록된 **실측** 사실(출처는 inputs.delta_source · "
                       "upstream_blob_sha 는 조회 시점 값)이며, expected 는 그 사실로부터 "
                       "파이프라인이 산출한 판정이다. 재생은 재측정이 아니다."),
              "inputs": inputs, "expected": expected}
        with open(a.emit_fixture, "w", encoding="utf-8") as f:
            json.dump(fx, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"[judge] fixture 발행: {a.emit_fixture}", file=sys.stderr)

    if a.write_resolved:
        if not a.resolved:
            die("--write-resolved 는 --resolved 가 필요하다")
        for k in [k for k in resolved_obj if k.startswith("upstream_delta_") and k != "upstream_delta"]:
            # 침묵 삭제 금지 — 손저작 블록은 감사 흔적으로 보존한다.
            resolved_obj[f"_superseded_manual_{k}"] = resolved_obj.pop(k)
        resolved_obj["upstream_delta"] = out
        with open(a.resolved, "w", encoding="utf-8") as f:
            json.dump(resolved_obj, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"[judge] resolved.json 갱신: {a.resolved}#upstream_delta", file=sys.stderr)

    print(json.dumps(out, ensure_ascii=False, indent=2))
    return VERDICT_EXIT[out["verdict"]]


# ── self-test ───────────────────────────────────────────────────────────────
def self_test():
    fails = []

    def check(name, cond, detail=""):
        print(f"  {'ok  ' if cond else 'FAIL'} {name}{(' — ' + detail) if detail and not cond else ''}")
        if not cond:
            fails.append(name)

    print("[glob/plane]")
    check("vllm py → RUNTIME_PY",
          classify_plane("vllm/model_executor/models/qwen3_dspark.py")[0] == "RUNTIME_PY")
    check("vllm cu → BUILD_INPUT", classify_plane("vllm/a/b.cu")[0] == "BUILD_INPUT")
    check("docker → UPSTREAM_OWN_DOCKER",
          classify_plane("docker/Dockerfile")[0] == "UPSTREAM_OWN_DOCKER")
    check("buildkite → CI", classify_plane(".buildkite/release-pipeline.yaml")[0] == "CI")
    check("csrc → BUILD_INPUT", classify_plane("csrc/moe/x.cu")[0] == "BUILD_INPUT")
    check("root md → DOCS", classify_plane("README.md")[0] == "DOCS")
    check("*.md 는 / 를 넘지 않음", classify_plane("newplane/a.md")[0] == UNKNOWN_PLANE)
    check("열거 밖 → UNKNOWN_PLANE", classify_plane("newplane/thing.toml")[0] == UNKNOWN_PLANE)

    print("[axis_a]")
    f_ok = classify_files([{"path": "docker/Dockerfile"}, {"path": ".buildkite/x.yaml"}])
    check("빌드입력 없음 → NO_IMPACT", axis_a(f_ok)["verdict"] == "NO_IMPACT")
    f_bi = classify_files([{"path": "csrc/x.cu"}])
    check("csrc 변경 → IMPACT", axis_a(f_bi)["verdict"] == "IMPACT")
    f_un = classify_files([{"path": "newplane/thing.toml"}])
    check("UNKNOWN_PLANE → UNDETERMINED", axis_a(f_un)["verdict"] == "UNDETERMINED")

    print("[axis_b 프로브]")
    P = "vllm/x.py"
    prov = {"files": {P: "aaa"}}

    def mk(rec):
        return BlobReader("r", recorded=rec)

    b, _ = axis_b(classify_files([{"path": P}]), prov, None,
                  mk({"F": {P: "aaa"}, "T": {P: "bbb"}}), "F", "T")
    check("번들=from ≠ to → WOULD_REVERT",
          b["silent_revert_risk"][0]["verdict"] == "WOULD_REVERT" and b["verdict"] == "IMPACT")
    b, _ = axis_b(classify_files([{"path": P}]), {"files": {P: "bbb"}}, None,
                  mk({"F": {P: "aaa"}, "T": {P: "bbb"}}), "F", "T")
    check("번들=to → NO_REVERT", b["probes"][0]["verdict"] == "NO_REVERT")
    check("NO_REVERT 는 silent_revert_risk 에 담기지 않음(§5.3 정지조건 도달 가능)",
          b["silent_revert_risk"] == [] and len(b["probes"]) == 1)
    b, _ = axis_b(classify_files([{"path": P}]),
                  {"files": {P: "ccc"}, "source": {"upstream_base_tag": "T"}}, None,
                  mk({"F": {P: "aaa"}, "T": {P: "bbb"}}), "F", "T")
    e = b["probes"][0]
    check("파생베이스=to_ref 여도 verdict 는 fail-closed 유지",
          e["verdict"] == "WOULD_REVERT" and e["bundle_base_is_to_ref"] is True and "note" in e)
    b, u = axis_b(classify_files([{"path": P}]), prov, None,
                  mk({"F": {P: "aaa"}}), "F", "T")
    check("상류 조회 실패 → UNDETERMINED",
          b["verdict"] == "UNDETERMINED" and len(u) == 1)
    b, _ = axis_b(classify_files([{"path": "other.md"}]), prov, None, mk({}), "F", "T")
    check("교차 없음 → NO_IMPACT", b["verdict"] == "NO_IMPACT" and b["overlap"] == [])
    b, _ = axis_b(classify_files([{"path": P}]), None, None, mk({}), "F", "T")
    check("PROVENANCE 없음 → applicable=False·NO_IMPACT",
          b["verdict"] == "NO_IMPACT" and b["applicable"] is False)

    print("[worst-wins]")
    o = synthesize({}, [], {"verdict": "NO_IMPACT"}, {"verdict": "IMPACT"}, [], None)
    check("A=NO,B=IMPACT → IMPACT", o["verdict"] == "IMPACT")
    o = synthesize({}, [], {"verdict": "NO_IMPACT"}, {"verdict": "NO_IMPACT"}, ["x"], None)
    check("unknown 있으면 UNDETERMINED", o["verdict"] == "UNDETERMINED")
    o = synthesize({}, [], {"verdict": "NO_IMPACT"}, {"verdict": "NO_IMPACT"}, [], None)
    check("전부 NO → NO_IMPACT", o["verdict"] == "NO_IMPACT")
    check("C축은 전역에서 제외", o["axes_in_global"] == ["axis_A_build_input", "axis_B_port_scope"]
          and o["axis_C_model_path"]["verdict"] == "NOT_IMPLEMENTED")

    print("[손저작 감사]")
    res = {"upstream_delta_a_to_b": {"files": 1, "detail": [
        {"file": "docker/Dockerfile", "delta": "+6/-2"}]}}
    au = audit_manual_block(res, [{"path": "docker/Dockerfile", "additions": 5, "deletions": 1}],
                            "a", "b")
    check("+6/-2 vs +5/-1 불일치 검출",
          au and not au["match"] and au["discrepancies"][0]["measured"] == "+5/-1")

    print("[D0 git-local (합성 리포)]")
    import tempfile
    import shutil
    tmp = tempfile.mkdtemp(prefix="judge-selftest-")
    try:
        env_args = ["-c", "user.email=t@t", "-c", "user.name=t"]
        subprocess.run(["git", "init", "-q", tmp], check=True)
        os.makedirs(os.path.join(tmp, "vllm"), exist_ok=True)
        with open(os.path.join(tmp, "vllm", "m.py"), "w") as fh:
            fh.write("a=1\n")
        subprocess.run(["git", "-C", tmp, "add", "-A"], check=True)
        subprocess.run(["git", "-C", tmp] + env_args + ["commit", "-qm", "c1"], check=True)
        subprocess.run(["git", "-C", tmp, "tag", "T1"], check=True)
        with open(os.path.join(tmp, "vllm", "m.py"), "a") as fh:
            fh.write("b=2\nc=3\n")
        with open(os.path.join(tmp, "docker"), "w") as fh:
            fh.write("x\n")
        os.remove(os.path.join(tmp, "docker"))
        os.makedirs(os.path.join(tmp, "docker"), exist_ok=True)
        with open(os.path.join(tmp, "docker", "Dockerfile"), "w") as fh:
            fh.write("FROM x\n")
        subprocess.run(["git", "-C", tmp, "add", "-A"], check=True)
        subprocess.run(["git", "-C", tmp] + env_args + ["commit", "-qm", "c2"], check=True)
        subprocess.run(["git", "-C", tmp, "tag", "T2"], check=True)
        d = collect_delta_git(tmp, "T1", "T2")
        got = {f["path"]: (f["status"], f["additions"], f["deletions"], f["hunks"])
               for f in d["files"]}
        check("git-local 델타 수집", got == {
            "vllm/m.py": ("modified", 2, 0, 1),
            "docker/Dockerfile": ("added", 1, 0, 1)}, json.dumps(got, ensure_ascii=False))
        check("git-local delta_source 라벨", d["delta_source"] == "git-local")
        r = BlobReader("x", repo_cache=tmp)
        st, sha = r.sha("T1", "vllm/m.py")
        check("blob@T1", st == "ok" and sha == sha256_bytes(b"a=1\n"))
        check("blob 부재 판별", r.sha("T1", "docker/Dockerfile") == ("absent", None))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print(f"\nself-test: {'PASS' if not fails else 'FAIL ' + ', '.join(fails)}")
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main() or 0)
