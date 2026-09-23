"""hintlib.core — 공통 기반: 저장소 경로 · 실패 표현 · git · 주입 시각 · 결정론 직렬화 · 교차 스킬 모듈 적재.

불변식(깨면 되돌아오는 사고들)
    - **import 시점 부수효과 0**: 경로·git·파일을 모듈 적재 때 건드리지 않는다(패키지 docstring 참조).
    - **실패는 예외로 표현한다**(`HintError(code, message, remedy)`). 옛 스크립트 4벌이 각자 `die()` 를
      두었고 종료코드가 1·2 로 갈렸다(코드맵 D12). 종료코드는 CLI 한 곳(`hint.py`)이 정한다.
    - **시각은 주입만**: 벽시계를 읽지 않는다(docs.md `--now` 주입 선례 · `--generated-utc`). 옛 `reverify`·
      `finalize` 색인 경로의 `date.today()` 두 곳이 hint 엔진의 유일한 벽시계였고 그 경로는 폐기됐다.
    - **교차 스킬 모듈은 경로로 적재하고, 없으면 죽는다**(대체 구현 ✗ · 2026-07-31 completion_gate
      ModuleNotFoundError 선례). 조용한 None 반환은 검사를 끄는 것과 같다 — campaign_template_validator 가
      `hint_tag.py` 부재 시 arch 검사를 침묵 생략하던 결함이 그 형태였다(코드맵 H2/H3).
"""
from __future__ import annotations

import datetime as _dt
import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from types import ModuleType
from typing import NoReturn

# ── 파일 위치 (git 없이 __file__ 에서만 계산 — import 부수효과 아님) ─────────────────────────
#   scripts/hintlib/core.py → parents: [0]=hintlib [1]=scripts [2]=hint-publisher [3]=skills [4]=.claude [5]=<repo>
HINTLIB_DIR = Path(__file__).resolve().parent
SCRIPTS_DIR = HINTLIB_DIR.parent
SKILL_DIR = SCRIPTS_DIR.parent
TEMPLATES_DIR = SKILL_DIR / "templates"
_DIST_ROOT_GUESS = HINTLIB_DIR.parents[4]

# 저장소 상대 경로(정본) — 절대경로는 호출 시점에 repo 와 합친다.
REL_SKILL = ".claude/skills/hint-publisher"
REL_SCRIPTS = f"{REL_SKILL}/scripts"
REL_HINT_CLI = f"{REL_SCRIPTS}/hint.py"
REL_VOCAB = "hints/vocab.json"
REL_INDEX = "hints/index.json"
REL_HINTS_MD = "HINTS.md"
REL_CONTRACT = "hints/HINT_ISSUANCE_CONTRACT.md"
REL_CENTRAL_FLAG = "hints/.central_authority"
REL_DRAFTS = "hints/.drafts"
REL_PII_TERMS = ".claude/pii_terms.txt"
REL_EVIDENCE_DIR = "docs/_evidence"
REL_COMPLETION_GATE = ".claude/policies/runtime/completion_gate.py"
REL_EVIDENCE_PUBLISHER = ".claude/policies/runtime/evidence_publisher.py"
REL_CAMPAIGN_INIT = ".claude/skills/terraforming_node/scripts/campaign_init.py"
REL_CAMPAIGN_VALIDATOR = ".claude/skills/terraforming_node/scripts/campaign_template_validator.py"
REL_SLAVE_FORWARD = ".claude/skills/upstream-version-watch/scripts/slave_forward.py"
REL_RENDER_DOCKERFILE = ".claude/skills/upstream-version-watch/scripts/render_dockerfile.py"
REL_RENDER_BENCH_SECTION = f"{REL_SCRIPTS}/render_bench_section.py"
REL_DOC_NAMING_WIKI = ".claude/skills/wiki-desk/scripts/doc_naming.py"
REL_DOC_NAMING_BENCH = ".claude/skills/adversarial-benchmark/scripts/doc_naming.py"

HINT_BRANCH_REF = "refs/heads/hint"
HINT_TAG_PREFIX = "hint/"

# 배포 오브젝트(태그 tagger · hint 페이로드 커밋 author/committer)에 박히는 **프로젝트 합성 신원**.
# 발행자 개인 신원이 아니다. `.invalid` 는 RFC2606 예약 TLD 라 도달하지 않는다.
# 2026-09-21 코드맵 K1/H10: tagger 만 합성이었고 페이로드 커밋 57/57 은 운영자 git config 신원이었다 —
# zip 을 받는 모든 수신자에게 그 신원이 배포됐다. 이제 한 상수가 셋(tagger·author·committer) 모두를 덮는다.
SYNTHETIC_NAME = "easy-vllm-simulator"
# 값은 옛 `hint_tag.DEFAULT_TAGGER_EMAIL` 과 바이트 동일(발행된 태그들이 이미 이 신원이다) — 조각으로 조립하는 이유는 추적
# 배포 파일에 메일 모양 리터럴을 두지 않기 위해서다(배포 4종 자기스캔 · 2026-08-06 docs.md 사고와 같은 규율 · 2026-09-22 리뷰).
SYNTHETIC_EMAIL = "@".join(("hints", "easy-vllm.invalid"))


# ── 실패 표현 ────────────────────────────────────────────────────────────────────────────────
class HintError(Exception):
    """안정 사유코드를 가진 실패. **판정은 code 로만 한다**(메시지 substring 판정 금지 — 감사 ② 실측:
    태그 저자가 통제하는 문자열로 차단이 경고로 강등됐다).

    remedy: 고치는 사람이 무엇을 해야 하는지(파일·행·명령). fail-loud 안내가 곧 운영 부담 완화다(plan R9).
    exit_code: CLI 가 이 예외로 끝날 때의 종료코드(기본 1).
    """

    def __init__(self, code: str, message: str, remedy: str | None = None, *, exit_code: int = 1):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message
        self.remedy = remedy
        self.exit_code = exit_code

    def render(self) -> str:
        out = f"[hint] FAIL {self.code}: {self.message}"
        if self.remedy:
            out += f"\n  → {self.remedy}"
        return out


def fail(code: str, message: str, remedy: str | None = None, *, exit_code: int = 1) -> NoReturn:
    raise HintError(code, message, remedy, exit_code=exit_code)


# ── 저장소 ──────────────────────────────────────────────────────────────────────────────────
def resolve_repo(explicit: str | os.PathLike | None = None, *, require_git: bool = True) -> Path:
    """저장소 루트. **호출 시점에만** 계산한다.

    explicit 가 있으면 그것(존재 확인). 없으면 CWD 의 `git rev-parse --show-toplevel`.
    require_git=False 는 읽기 전용 수신자 명령(`match`)만 쓴다 — git 이 없는 배포 아카이브에서도
    `hints/index.json` 만으로 동작해야 한다(verify_distribution `gitless_hint_match`).
    """
    if explicit is not None:
        p = Path(explicit).resolve()
        if not p.is_dir():
            fail("HINT_REPO_ABSENT", f"저장소 경로가 디렉터리가 아니다: {p}")
        if require_git and not (p / ".git").exists():
            fail("HINT_REPO_NOT_GIT", f"git 저장소가 아니다: {p}",
                 "match 이외 명령은 git 체크아웃에서만 실행한다.")
        return p
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    except OSError:
        out = None
    if out is not None and out.returncode == 0:
        return Path(out.stdout.strip()).resolve()
    if not require_git and (_DIST_ROOT_GUESS / REL_INDEX).is_file():
        return _DIST_ROOT_GUESS
    fail("HINT_REPO_NOT_GIT", "git 체크아웃(또는 hints/index.json 을 가진 배포 루트)을 찾지 못했다.",
         "match 이외 명령은 git 저장소 안에서 실행하거나 --repo 로 지정한다.")


def git(repo: Path, *args: str, check: bool = True, input_text: str | None = None,
        env_extra: dict | None = None) -> subprocess.CompletedProcess:
    """텍스트 모드 git. check=True 에서 비-0 이면 HintError(HINT_GIT_FAILED)."""
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    if env_extra:
        env.update(env_extra)
    try:
        out = subprocess.run(["git", *args], capture_output=True, text=True, cwd=str(repo), env=env,
                             input=input_text)
    except OSError as e:
        fail("HINT_GIT_UNAVAILABLE", f"git 실행 불가: {e}")
    if check and out.returncode != 0:
        fail("HINT_GIT_FAILED", f"git {' '.join(args)} (rc={out.returncode}): {out.stderr.strip()}")
    return out


def git_out(repo: Path, *args: str) -> str:
    return git(repo, *args).stdout.strip()


def git_bytes(repo: Path, *args: str, check: bool = True) -> bytes | None:
    """바이너리 출력 git(blob 읽기). check=False 에서 실패하면 None."""
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    out = subprocess.run(["git", *args], capture_output=True, cwd=str(repo), env=env)
    if out.returncode != 0:
        if check:
            fail("HINT_GIT_FAILED", f"git {' '.join(args)} (rc={out.returncode}): "
                                    f"{out.stderr.decode('utf-8', 'replace').strip()}")
        return None
    return out.stdout


def rel(repo: Path, path: str | os.PathLike) -> str:
    """저장소 상대 posix 경로. 저장소 밖이면 HintError(HINT_PATH_OUTSIDE_REPO) —
    옛 hint_collect 의 `--slot-root` 저장소 밖 입력이 미포착 ValueError 로 죽던 결함(코드맵)의 교정."""
    p = Path(path)
    if not p.is_absolute():
        p = repo / p
    try:
        return p.resolve().relative_to(repo.resolve()).as_posix()
    except ValueError:
        fail("HINT_PATH_OUTSIDE_REPO", f"저장소 밖 경로: {path}")


# ── 시각 (주입만) ─────────────────────────────────────────────────────────────────────────────
_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
_KST = _dt.timezone(_dt.timedelta(hours=9))


def require_utc(value: str | None, what: str = "--generated-utc") -> str:
    if not value or not _UTC_RE.match(value):
        fail("HINT_TIME_NOT_INJECTED", f"{what} 는 UTC `YYYY-MM-DDTHH:MM:SSZ` 로 주입해야 한다: {value!r}",
             "벽시계를 읽지 않는다 — 호출자가 시각을 넘긴다.")
    return value


def parse_utc(value: str) -> _dt.datetime:
    return _dt.datetime.strptime(require_utc(value), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=_dt.timezone.utc)


def kst_iso(utc: str) -> str:
    """UTC → KST `YYYY-MM-DDTHH:MM:SS`(문서 규약의 KST 절대시각)."""
    return parse_utc(utc).astimezone(_KST).strftime("%Y-%m-%dT%H:%M:%S")


def kst_token(utc: str) -> str:
    """UTC → docs 명명 규약의 `YYMMDDHH`(KST)."""
    return parse_utc(utc).astimezone(_KST).strftime("%y%m%d%H")


def git_date(utc: str) -> str:
    """git 의 GIT_*_DATE 형식(`@<epoch> +0000`) — 커밋/태그 SHA 가 주입 시각의 함수가 되게 한다
    (코드맵 K2: 벽시계 커밋 시각이 SHA 를 비결정으로 만들었다)."""
    return f"@{int(parse_utc(utc).timestamp())} +0000"


# ── 결정론 직렬화 · 해시 ──────────────────────────────────────────────────────────────────────
def dumps(obj) -> str:
    """배포·비교 대상 JSON 의 단일 직렬화(키 정렬 · UTF-8 원문 · 들여쓰기 2 · 끝 개행)."""
    return json.dumps(obj, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def write_json(path: Path, obj) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(dumps(obj), encoding="utf-8")


def read_json(path: Path, *, code: str = "HINT_JSON_UNREADABLE"):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        fail(code, f"JSON 을 읽을 수 없다({path}): {e}")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(Path(path).read_bytes())


# ── 교차 스킬 모듈 적재 (경로 · fail-loud) ─────────────────────────────────────────────────────
_MODULE_CACHE: dict[tuple[str, str], ModuleType] = {}


def load_owner_module(repo: Path, rel_path: str, name: str, *, add_dir_to_path: bool = True) -> ModuleType:
    """다른 스킬·정책이 소유한 모듈을 **그 저장소의 경로에서** 적재한다.

    - 부재 = HintError(HINT_OWNER_MODULE_MISSING). 대체 구현으로 계속하지 않는다(모듈 docstring).
    - add_dir_to_path: 대상이 형제 모듈을 import 하는 경우(evidence_publisher → completion_gate,
      wiki-desk doc_naming → adversarial-benchmark doc_naming 등) 그 디렉터리를 sys.path 에 둔다.
    - 캐시 키 = (저장소, 경로) — 격리 저장소 자체검사가 실제 저장소 모듈을 잘못 재사용하지 않게.
    """
    path = (repo / rel_path).resolve()
    key = (str(path), name)
    if key in _MODULE_CACHE:
        return _MODULE_CACHE[key]
    if not path.is_file():
        fail("HINT_OWNER_MODULE_MISSING", f"소유 모듈 부재: {rel_path}",
             "대체 구현으로 계속하지 않는다 — 그 스킬이 설치된 체크아웃에서 실행한다.")
    if add_dir_to_path and str(path.parent) not in sys.path:
        sys.path.insert(0, str(path.parent))
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        fail("HINT_OWNER_MODULE_MISSING", f"모듈 spec 생성 실패: {rel_path}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules.setdefault(name, mod)
    try:
        spec.loader.exec_module(mod)
    except SystemExit as e:  # 소유 모듈이 import 중 die() 하는 경우 — 원인을 삼키지 않고 번역한다
        fail("HINT_OWNER_MODULE_IMPORT_FAILED", f"{rel_path} import 중 SystemExit({e.code})")
    _MODULE_CACHE[key] = mod
    return mod


def run_python(repo: Path, rel_script: str, *args: str, input_text: str | None = None,
               env_extra: dict | None = None, check: bool = False) -> subprocess.CompletedProcess:
    """소유 스크립트를 **서브프로세스로** 실행(포맷 소유 1 · 호출부 N — campaign_init·evidence_publisher·
    completion_gate 는 CLI 가 계약이다). check=True 면 비-0 에서 HintError(HINT_OWNER_CLI_FAILED)."""
    script = repo / rel_script
    if not script.is_file():
        fail("HINT_OWNER_MODULE_MISSING", f"소유 스크립트 부재: {rel_script}")
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    if env_extra:
        env.update(env_extra)
    out = subprocess.run([sys.executable, str(script), *args], capture_output=True, text=True,
                         cwd=str(repo), env=env, input=input_text)
    if check and out.returncode != 0:
        fail("HINT_OWNER_CLI_FAILED",
             f"{rel_script} {' '.join(args[:2])}… rc={out.returncode}\n{out.stdout[-2000:]}{out.stderr[-2000:]}")
    return out


# ── 레벨 원시의 도구 귀속 ────────────────────────────────────────────────────────────────────
# 측정 도구별 레벨 원시 이름(run_bench.sh:394 — vllm=bench_<cfg>.json · guidellm=guidellm_<cfg>.json).
LEVEL_RAW_BY_TOOL = {"vllm": "bench_{cell}.json", "guidellm": "guidellm_{cell}.json"}


def level_raw_is_measured_tool(path: Path, cell: str) -> bool:
    """레벨 디렉터리의 도구 원시가 **그 디렉터리의 측정 도구**(형제 `bench_tool_<cell>.json` 의 `tool`)의 것인가.

    왜(2026-09-23 D1): 도구를 바꿔 재스윕한 자리(vllm → GuideLLM)에 옛 `bench_<cell>.json` 이 남아, 발행기가 그 `date` 를
    이번 측정의 첫 측정 시각으로 · 그 필드를 이번 레벨 명령으로 읽었다(재현 표 bench 245 시간). 도구 기록이 다른 도구를
    말하면 그 원시는 이 측정의 것이 아니다. 도구 기록이 없거나 읽지 못하면 판정하지 않는다(True — 종전 동작 · 옛 배치).
    """
    path = Path(path)
    for tool, pat in LEVEL_RAW_BY_TOOL.items():
        if path.name == pat.format(cell=cell):
            break
    else:
        return True
    try:
        rec = json.loads((path.parent / f"bench_tool_{cell}.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return True
    said = rec.get("tool") if isinstance(rec, dict) else None
    return not isinstance(said, str) or said == tool


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 라이브 저장소 상태에 의존하지 않는다."""
    bad: list[str] = []

    def ck(name: str, cond: bool) -> None:
        if not cond:
            bad.append(f"core: {name}")

    ck("주입 시각 KST 변환", kst_iso("2026-09-21T10:21:01Z") == "2026-09-21T19:21:01")
    ck("KST 토큰", kst_token("2026-09-21T10:21:01Z") == "26092119")
    ck("git 날짜 결정론", git_date("1970-01-01T00:00:10Z") == "@10 +0000")
    try:
        require_utc("2026-09-21 10:21:01")
        ck("★음성대조 비-UTC 시각 차단", False)
    except HintError as e:
        ck("★음성대조 비-UTC 시각 차단(code)", e.code == "HINT_TIME_NOT_INJECTED")
    ck("결정론 직렬화 키 정렬", dumps({"b": 1, "a": "가"}) == '{\n  "a": "가",\n  "b": 1\n}\n')
    e = HintError("X_CODE", "msg", "do this")
    ck("HintError 렌더", e.render().startswith("[hint] FAIL X_CODE: msg") and "→ do this" in e.render())
    ck("합성 신원은 예약 TLD", SYNTHETIC_EMAIL.endswith(".invalid"))
    ck("합성 신원 = 발행된 태그의 신원(바이트 불변)", SYNTHETIC_EMAIL == "hints" + "@" + "easy-vllm.invalid")
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        d = Path(td)
        (d / "bench_c.json").write_text("{}", encoding="utf-8")
        (d / "guidellm_c.json").write_text("{}", encoding="utf-8")
        ck("도구 기록 없음 → 판정 안 함(옛 배치)", level_raw_is_measured_tool(d / "bench_c.json", "c"))
        (d / "bench_tool_c.json").write_text('{"tool": "guidellm"}', encoding="utf-8")
        ck("★음성대조 GuideLLM 레벨의 옛 vllm 원시 → 이 측정 아님", not level_raw_is_measured_tool(d / "bench_c.json", "c"))
        ck("GuideLLM 레벨의 GuideLLM 원시 → 이 측정", level_raw_is_measured_tool(d / "guidellm_c.json", "c"))
        ck("도구 원시가 아닌 파일 → 판정 안 함", level_raw_is_measured_tool(d / "measured.json", "c"))
    return bad
