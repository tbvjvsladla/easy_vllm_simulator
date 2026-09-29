"""hintlib.lineage — 계보 파생: **발행 시점 파일**에서 mention 그래프를 계산해 `LINEAGE.json` 을 만든다
(plan_26092119 §4.3 · SPEC X1·X17 · 코드맵 lineage_graph.md §10).

무엇을 푸는가
    서사(`02-narrative.md`)는 이 셀 1회분이 아니라 **계보 전체**다(D1). 옛 `hint_collect.render_item2` 는 devlog·testlog
    포인터 2개만 실었고 발행 기록의 plan 포인터(`evidence.plan`)는 버렸다(F5 · 1홉) — 그 결과 태그2 zip 은 부록 B 여정
    44항목 중 가중 43.0% 만 보존했다. 산문은 LLM 이 쓰지만 **무엇을 읽어야 하는가는 기계가 정한다**(plan §4.2 ①) —
    그 목록이 이 모듈의 산출이다.

왜 wiki-desk registry 를 읽지 않는가 (X1 = 옵션 (b) · 2026-09-21 시뮬레이션 `lineage_graph.md` §8·§9)
    라이브 registry 를 태그2 서사 시드에서 따라가면 부록 B 표적 12문서 중 **2개**만 잡힌다. `docs/report`·`docs/benchmark`
    가 서가 root 가 아니라 노드가 아니고(F16 · D-W3), 스템 해소가 주제 없는 last-wins 라 `seed/` 백업 사본·동시각 형제가
    메인 문서를 가린다(D-W2 · 섀도잉 29건), SUPERSEDED 배너 문법이 관행과 어긋나 16건 중 3건만 잡힌다(D-W4). registry 는
    비추적·머신 로컬 **캐시**라 입고 실패가 침묵한다(D-W7). 그래서 계보는 registry 와 **같은 의미론**의 간선을 발행 시점
    파일에서 **직접** 계산한다 — 입력 = (발행 기록 · 파일 본문 · publish_kst · depth · base 슬러그) → 같은 입력이면
    머신 무관하게 같은 LINEAGE. 시뮬레이션 기준 X 그래프 · 시드 S2 · hybrid 순회 · 깊이 4 · 슬러그 필터 = **12/12**.

승계한 불변식 (원 주석·날짜를 새 자리에 옮긴다)
    - **원문 복제 금지는 서가 쪽 원칙**(`init_wiki_desk.py:4-7` "NEVER copies a source body") — LINEAGE 는 포인터·토큰·
      메타만 싣는다. 원문은 템플릿 발췌 규칙(치환 후 부분문자열 검사 · plan §4.4)으로만 zip 에 들어간다.
    - **mention 의미론**: registry 의 cites/realizes/superseded-by 는 A→B = "A 가 B 를 언급", evidences 는 저장 방향이
      역(testlog→devlog = devlog 가 testlog 를 언급)이었다. 여기서는 처음부터 **언급 방향 하나**로 계산하고, 조상(출처)
      방향 = mention 순방향으로 걷는다. registry 의 음영 규칙(강한 관계가 cites 를 지움)은 쓰지 않는다 — 관계 종류를 잃지
      않게 `via[]` 에 전부 싣는다.
    - **무방향 순회 금지 = 허브 가드**: 무방향으로 풀면 계보 산출이 "그 계획서가 있느냐" 의 함수가 된다(L·S0·both 8/12
      중 6개가 plan_26092119 허브 경유). 후손 방향은 **"채택된 testlog 를 서술한 devlog"** 한 종류만 보강한다(hybrid).
    - **필터는 채택을 가르고 통행은 한 홉 허용한다**(2026-09-22 라이브 대조 · 태그2 발행 기록 11/12 → 12/12): 모델을
      캠페인 약어로만 적은 하네스 plan(`plan_26091407`)이 표적 `perf_26091314` 로 가는 유일한 길목이었다. 채택 문서에서
      한 홉인 필터 탈락 문서는 경유(`transit[]` · 읽기 목록 밖)로만 확장하고, 경유 → 경유 연쇄는 끊는다.
    - **헤더 한정 supersede**("body text merely quoting the convention must not create edges" · init_wiki_desk) — 넓힌
      배너 문법(`SUPERSEDED`·`SUPERSEDED-IN-PART` 어느 꼴이든)도 헤더 12줄 안에서만 읽는다.
    - **헌법 비색인**(anti-confirmation-bias · `write_wiki` `:429-432`) — `CLAUDE.md`·`.claude/**` 는 계보 입력이 아니다.
      `seed/`(사용자 보관소 · 백업 사본이 메인을 섀도잉 · D-W2)·`sync_staging/`(서브 회수 미러 · 메인 권위를 흉내 · D-W1)도
      배제한다. docs/report 의 audit/harness(결론문)는 계보 대상이되 `class` 로 구분한다.
    - **시각은 주입만**(`publish_kst` · hint_collect `--generated-kst` 선례 · doc_naming 순수성). 벽시계 금지.
    - **발행 시각 상한은 필수**(D-W8): 추적 report 를 나중에 고치면 mention 순방향도 "과거" 가 아니다
      (`perf_26091122 → plan_26091407`). 상한 뒤 문서는 채택·확장하지 않는다.
    - **같은 셀의 과거 발행 기록 = 관측으로 가른다**(2026-09-22 감사): 발행 기록에는 셀 필드가 없다. id 접미 규칙만 쓰면
      셀 id 가 다른 셀 id 의 접미일 때(`bf-262k-mmp` ⊂ `nv4_bf_262k_mmp`) 남의 계보를 삼킨다 — 기록의 simlog 사본
      `sweep_index.json` meta.config_name 이 있으면 그것만 믿고, 없을 때만 id 접미로 대조하며 근거를 method 에 적는다.
    - **같은 모델의 앞선 셀 = 시드 S3**(2026-09-29 plan_26092908 R-a): S2 는 같은 셀 키만 따라서, 셀 키가 다른 새 셀은 같은 모델의
      bring-up 계보를 휘발 캠페인 grounding 참조로만 얻었다(재생에서 DS4F LINEAGE 12 → 6). 추적 평면의 발행 기록에서 identity.model
      (base 슬러그) · 관측 별칭(`model_aliases`) · gpu 로 결정론 대조하고 서사 문서 포인터만 싣는다(`seeds_from_model_priors`).
    - **저장소 밖 포인터 원문은 싣지 않는다**: LINEAGE.json 은 배포 페이로드다 — `/home/…` 같은 원문은 `<outside-repo>` 로.
    - **"부재와 실패는 다른 사실"**: 시드 결손은 `seeds_missing[]` 로 기재하고 차단하지 않는다. 단 문서 0건은 서사 원재료 0
      이므로 린터가 `HINT_LINEAGE_EMPTY` 로 막는다(`require_documents`).
    - **GIT_SINGLE_AUTHORITY**(2026-09-03): 추적 `docs/report/*` 는 git blob 이 드는 바이트라 digest 를 다시 적으면
      중복층 → **경로+커밋**. 비추적 `docs/{plan,devlog,testlog,benchmark,simlog}` 는 맹점층 → sha256 정당.
    - **문법은 SSOT 에서**(하드코딩 판정표의 결함 조건 회피): 산문 명명 = wiki-desk `doc_naming`(`DATED_DOC_TYPES`·
      `_DATED_DOC_RE`·`is_dated_doc_basename`·`_REPORT_RE`·`simlog_dirname`) · 벤치 명명 = adversarial-benchmark
      `doc_naming`(`_BENCH_NAME_RE`·`BENCH_KIND_DEFAULT_EXT`). 인용 토큰 문법은 그 접두 집합 + 시간 토큰으로 **파생**한다.
      wiki-desk docstring 의 "report 는 날짜 토큰 없음" 은 2026-08-24 개정 전 문구다(D-W9) — docstring 이 아니라 `_REPORT_RE`
      를 믿는다. sweep_map 만 코드 SSOT 가 없어(docs.md §명명 SSOT 산문 · `render_sweep_map.py` 는 호출자 경로를 받는다)
      국소 상수 `SWEEP_MAP_PREFIX` 를 둔다.

해소 규칙 (D-W2 교정)
    - `exact`     본문 토큰이 **주제 포함 전체 이름**과 일치(긴 이름 우선 · 이름 뒤 경계 확인).
    - `unique`    주제 없는 토큰(`testlog_26090921` · `§` 앞의 맨 스템)이 같은 시각 형제 **1건**에 닿는다.
    - `ambiguous` 주제 없는 토큰이 형제 2건 이상(예 `sweep_map_26091008_*.md` 글롭 = 25건) 또는 주제를 달았는데 어느 이름과도
                  일치하지 않는다(개명 등). 형제 **전부**를 싣고 표시한다 — 조용한 last-wins 금지.
    `resolve_stem` 은 모호하면 None 이다(발췌 출처 해소는 유일해야 한다).
"""
from __future__ import annotations

import bisect
import datetime as _dt
import hashlib
import json
import os
import posixpath
import re
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

from . import core

# ── 계약 상수 ────────────────────────────────────────────────────────────────────────────────
SCHEMA_VERSION = 1
DEFAULT_DEPTH = 4
# X1: 계보 입력 root. docs/report·docs/benchmark 는 서가 root 가 아니었다(F16) — 여기서는 직접 수집한다.
DEFAULT_ROOTS = ("docs/plan", "docs/devlog", "docs/testlog", "docs/report", "docs/benchmark", "docs/simlog")
# X17: 헌법 비색인 + 백업/미러 배제(모듈 docstring). 경로 접두(디렉터리는 `/` 로 끝남) 또는 정확한 파일 이름.
EXCLUDED_ROOTS = ("seed/", "sync_staging/", ".claude/", "CLAUDE.md")
HEADER_LINES = 12            # init_wiki_desk.SUPERSEDE_HEADER_LINES 와 같은 창(헤더 한정 의미론 승계)
SWEEP_MAP_PREFIX = "sweep_map"   # 코드 SSOT 부재 — docs.md §명명 SSOT 산문이 정본(모듈 docstring)
EXCLUDED_BASENAMES = ("example.md",)   # 서가 config `exclude_basenames` 와 같은 뜻 — 스켈레톤은 계보가 아니다
# 디렉터리 후보를 파일로 펼 때의 디렉터리당 상한(국소 상수 · 2026-09-22 S2 round 2 F7a). LINEAGE.json 은 배포 페이로드라 무한히 펴면
#   부기 바이트가 자란다 — 태그2 계보의 simlog 재판정 디렉터리 407파일이 상한 근거. 잘린 사실은 후보 행 `expanded.truncated` 가 말한다.
EXPAND_MAX_FILES = 64
_DIR_CANDIDATE_KINDS = ("simlog", "raw_dir", "engine_failure_logs")
# 측정 도구 원천 스냅숏(2026-09-22 · plan_26092119 S2 round 3 · 공유 계약 `tool_snapshots`). 왜: 2차 사실확인이 오기 3건을 "측정
#   시점 도구 원천이 저장소 git 에 있는데 저작자 입력에 없었다" 로 짚었다(run_bench.sh@8ca23d4 의 --temperature 0 · lite_bench.sh@0df14da
#   의 무변경 · sweep_bench.sh@8ca23d4 의 인증서 vllm_version 파생). evidence.tool_snapshots 가 `git show <rev>:<path>` 바이트를 draft 의
#   `inputs/sources/<이름>@<rev12>` 에 쓰고, 이 모듈은 그 경로를 **draft 상대** 후보로 싣는다(저장소 경로가 아니다 — 발췌기가 draft 에서
#   읽는다). 원천이 `.claude/` 아래여도 싣는 이유: 헌법 비색인(X17)은 **mention 그래프 입력**의 규칙이고, 이것은 "측정 때 돈 코드" 의
#   바이트 증거다 — 그래프에 넣지 않고(간선 ✗ · 본문 인용 ✗) 발췌 출처로만 연다. 모양의 단일 소유자 = 이 모듈(`tool_snapshot_rel`).
MODEL_PRIOR_SOURCE = "model_prior_publication:"   # 시드 S3 출처 접두(같은 모델 · 다른 셀의 앞선 발행 기록)
MODEL_PRIOR_KINDS = ("plan", "devlog", "testlog", "report")   # S3 가 시드로 싣는 서사 종류(측정 표면 bench_report·sweep_map ✗)
TOOL_SOURCE_KIND = "tool-source@rev"
TOOL_SNAPSHOT_DIR = "inputs/sources"
_TOOL_SNAPSHOT_RE = re.compile(r"^" + re.escape(TOOL_SNAPSHOT_DIR) + r"/([A-Za-z0-9][A-Za-z0-9._-]*)@([0-9a-f]{12})\Z")
_TOOL_ORIGIN_RE = re.compile(r"^git:([0-9a-f]{40}):([A-Za-z0-9._/-]+)\Z")

DOCUMENT_KINDS = ("plan", "devlog", "testlog", "request", "checklist",
                  "report", "bench_report", "max_envelope", "sweep_map")
DATA_KINDS = ("simlog", "certificate", "sweep_json", "engine_log", "raw_json", "raw_file", "raw_dir")
# 같은 셀의 과거 발행 기록을 "같은 identity" 로 볼 필드. vllm 은 뺀다 — 같은 셀을 새 엔진에서 다시 잰 것도 같은 여정이다
# (태그는 이름의 vLLM 세그먼트가 가르고, 계보는 가르지 않는다). quant 는 모델 슬러그에 이미 들어 있다.
IDENTITY_MATCH_FIELDS = ("model", "gpu", "topology", "tp")
RESOLUTIONS = ("exact", "unique", "ambiguous")
_RES_RANK = {r: i for i, r in enumerate(RESOLUTIONS)}
# 같은 시각 동률의 보조 정렬(D-W10 · 동시각 형제 6쌍) — 시간이 1차, 종류는 2차, basename 이 3차.
_KIND_RANK = {"plan": 0, "testlog": 1, "devlog": 2, "report": 3, "bench_report": 4, "max_envelope": 5,
              "sweep_map": 6, "request": 7, "checklist": 8}
_TEXT_SUFFIXES = (".md", ".html", ".json", ".jsonl", ".yaml", ".yml", ".txt", ".log")
_STRIP_EXTS = (".md", ".html", ".yaml", ".yml", ".json")
_DOC_EXTS = (".md", ".html")
_NAME_EXTRA = "_-+"

_SWEEP_MAP_RE = re.compile(r"^" + SWEEP_MAP_PREFIX + r"_(\d{8})(?:_(\d{2})_(\d{2}))?_(.+)\.(md|json)\Z")
_SIMLOG_MODERN_RE = re.compile(r"^(\d{8})(?:_(\d{2})_(\d{2}))?_(.+)\Z")
# D-W5: docs.md 의 `_seq_` 금지 이전 레거시 `YYYYMMDDHH_<seq>_<topic>`(2026-09-21 실측 42/67 dir) — 4자리 연도.
_SIMLOG_LEGACY_RE = re.compile(r"^(\d{10})_(\d+)_(.+)\Z")
_SIMLOG_TOKEN_RE = re.compile(r"(?<![A-Za-z0-9_])docs/simlog/(?P<tt>\d{10}|\d{8})(?![0-9])")
_UNDATED_PATH_RE = re.compile(r"(?<![A-Za-z0-9_])docs/(?P<dir>[a-z_]+)/(?P<rest>[^\s`'\"<>()\[\]|,;*]+)")
_OUTPUT_PATH_RE = re.compile(r"(?<![A-Za-z0-9_./-])output/[a-z]+/benchlog/[^\s`'\"<>()\[\]|,;*{}]+")
_MMSS_AT_RE = re.compile(r"_(\d{2})_(\d{2})(?![0-9])")
_SUPERSEDE_MARK_RE = re.compile(r"SUPERSEDED(?:-IN-PART)?")
# 헤더 라벨 줄 `> 선행 \`docs/…\`` · `> 계획 \`docs/…\`` — 라벨 단어 **바로 뒤에** 문서 토큰이 올 때만 라벨로 읽는다.
_HEADER_LABEL_RE = re.compile(r"^\s*>\s*\**([가-힣A-Za-z]+)\**\s*[:：]?\s*`?")
# supersede_banners 의 코퍼스 무관 토큰 — 접두 1~2 단어 + 시간 토큰 + 선택 주제, 또는 simlog 경로.
_GENERIC_DOC_TOKEN_RE = re.compile(
    r"(?<![A-Za-z0-9_])(?:docs/[a-z_]+/)?"
    r"(?P<tok>[a-z][a-z0-9]*(?:_[a-z][a-z0-9]*)?_(?:\d{8}|NA)(?![0-9])(?:_[^\s`'\"<>()\[\]|,;*§]+)?)")
_BARE_STEM_RE = re.compile(r"^(?:[a-z][a-z0-9_]*_(?:\d{8}|NA)|\d{8}|\d{10})(?:_\d{2}_\d{2})?\Z")
_KST_TOKEN_RE = re.compile(r"^\d{8}\Z")
_DOCS_DIR_PREFIX_RE = re.compile(r"docs/[a-z_]+/")
_KST = _dt.timezone(_dt.timedelta(hours=9))


# ── 문법 적재 (SSOT 2종 · 지연 · fail-loud) ─────────────────────────────────────────────────────
@dataclass(frozen=True)
class _Grammar:
    dated_types: tuple
    dated_re: object
    is_dated: object
    report_re: object
    bench_re: object
    bench_kinds: tuple
    simlog_dirname: object
    validate_topic: object


_GRAMMAR_CACHE: dict[str, _Grammar] = {}


def _own_checkout() -> Path:
    """hintlib 이 놓인 체크아웃 루트(`__file__` 기반 — import 부수효과 아님)."""
    return core.SKILL_DIR.parents[2]


def _grammar(repo: Path | None = None) -> _Grammar:
    """명명 문법을 **소유 스킬의 모듈에서** 적재한다(사본 ✗). repo 가 없으면 이 코드가 놓인 체크아웃.

    wiki-desk `doc_naming` 은 import 시점에 adversarial-benchmark `doc_naming` 을 자기 경로 기준으로 fail-loud 적재한다
    (`:54-75` · "조용한 대체 구현 금지") — 그 ImportError 를 번역만 하고 삼키지 않는다."""
    root = (Path(repo) if repo is not None else _own_checkout()).resolve()
    key = str(root)
    if key in _GRAMMAR_CACHE:
        return _GRAMMAR_CACHE[key]
    bench = core.load_owner_module(root, core.REL_DOC_NAMING_BENCH, "_hint_lineage_bench_doc_naming",
                                   add_dir_to_path=False)
    try:
        wiki = core.load_owner_module(root, core.REL_DOC_NAMING_WIKI, "_hint_lineage_wiki_doc_naming",
                                      add_dir_to_path=False)
    except ImportError as e:
        core.fail("HINT_OWNER_MODULE_IMPORT_FAILED", f"{core.REL_DOC_NAMING_WIKI} import 실패: {e}",
                  "wiki-desk doc_naming 은 벤치 명명 SSOT 를 재수출한다 — adversarial-benchmark 스킬이 있는 체크아웃에서 실행한다.")
    need = ((wiki, core.REL_DOC_NAMING_WIKI,
             ("DATED_DOC_TYPES", "_DATED_DOC_RE", "is_dated_doc_basename", "_REPORT_RE", "simlog_dirname",
              "validate_topic")),
            (bench, core.REL_DOC_NAMING_BENCH, ("_BENCH_NAME_RE", "BENCH_KIND_DEFAULT_EXT")))
    for mod, rel_path, names in need:
        absent = [n for n in names if not hasattr(mod, n)]
        if absent:
            core.fail("HINT_LINEAGE_GRAMMAR_DRIFT", f"{rel_path} 에 계보가 쓰는 명명 심볼이 없다: {absent}",
                      "명명 SSOT 가 바뀌었다 — hintlib/lineage.py 의 문법 적재(_grammar)를 새 공개 심볼에 맞춘다.")
    if not {"hour", "topic"} <= set(wiki._DATED_DOC_RE.groupindex) \
            or not {"tok", "mm", "ss", "combo"} <= set(bench._BENCH_NAME_RE.groupindex):
        core.fail("HINT_LINEAGE_GRAMMAR_DRIFT", "명명 SSOT 정규식의 그룹 이름이 바뀌었다(hour/topic · tok/mm/ss/combo)",
                  "hintlib/lineage.py 의 _parse_with 를 새 그룹에 맞춘다.")
    g = _Grammar(dated_types=tuple(wiki.DATED_DOC_TYPES), dated_re=wiki._DATED_DOC_RE,
                 is_dated=wiki.is_dated_doc_basename, report_re=wiki._REPORT_RE, bench_re=bench._BENCH_NAME_RE,
                 bench_kinds=tuple(bench.BENCH_KIND_DEFAULT_EXT), simlog_dirname=wiki.simlog_dirname,
                 validate_topic=wiki.validate_topic)
    _GRAMMAR_CACHE[key] = g
    return g


# ── 작은 도우미 ──────────────────────────────────────────────────────────────────────────────
def _strip_ext(name: str) -> str:
    for ext in _STRIP_EXTS:
        if name.endswith(ext) and len(name) > len(ext):
            return name[: -len(ext)]
    return name


def _ext_of(name: str) -> str:
    for ext in _STRIP_EXTS:
        if name.endswith(ext) and len(name) > len(ext):
            return ext
    return ""


def _char_at(text: str, i: int) -> str:
    return text[i] if 0 <= i < len(text) else ""


def _is_name_char(ch: str) -> bool:
    """문서 이름을 잇는 글자인가. `.` 은 경계로 본다(확장자·문장 끝) — 긴 이름 우선 대조가 버전 점을 먼저 소비한다."""
    return bool(ch) and (ch.isalnum() or ch in _NAME_EXTRA)


def _ext_at(text: str, i: int) -> str:
    if _char_at(text, i) != ".":
        return ""
    j = i + 1
    while j < len(text) and text[j].isalnum() and text[j].isascii():
        j += 1
    return text[i:j]


def _excluded(rel_path: str, exclude=EXCLUDED_ROOTS) -> bool:
    for e in exclude:
        if e.endswith("/"):
            if rel_path == e.rstrip("/") or rel_path.startswith(e):
                return True
        elif rel_path == e or rel_path.startswith(e + "/"):
            return True
    return False


def _norm_key(s: str) -> str:
    """셀 id·발행 id 대조용 정규화(`nv4-bf-262k-mmp` ≡ `nv4_bf_262k_mmp`). 한글은 보존한다."""
    return re.sub(r"[-.\s]+", "_", str(s).strip().lower()).strip("_")


def _norm_val(v) -> str:
    return str(v).strip().lower()


def _within_ceiling(yymmddhh: str | None, publish_kst: str) -> bool:
    """발행 시각 상한(포함). 날짜 토큰이 없는 이름(2026-08-24 이전 무날짜 report · `NA` 벤치)은 비교할 수 없어 통과시킨다 —
    이 정책은 LINEAGE.method.undated 에 기록된다(침묵 폴백 ✗)."""
    return yymmddhh is None or yymmddhh <= publish_kst


def _kst_parts_to_utc(tt: str, mm: str, ss: str) -> str | None:
    try:
        d = _dt.datetime(2000 + int(tt[0:2]), int(tt[2:4]), int(tt[4:6]), int(tt[6:8]), int(mm), int(ss),
                         tzinfo=_KST)
    except ValueError:
        return None
    return d.astimezone(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _simlog_conformant(g: _Grammar, name: str, tt: str, mm: str | None, ss: str | None, topic: str) -> bool:
    """simlog 이름이 SSOT `simlog_dirname` 이 **실제로 만들었을 이름**인가(왕복 대조 — 정규식 사본 대신 생성기로 판정)."""
    try:
        g.validate_topic(topic)
    except ValueError:
        return False
    utc = _kst_parts_to_utc(tt, mm or "00", ss or "00")
    if utc is None:
        return False
    existing = () if mm is None else (f"{tt}_{topic}",)   # 접미형은 같은 시각 점유가 있어야 SSOT 가 만든다
    try:
        return g.simlog_dirname(utc, topic, existing) == name
    except ValueError:
        return False


def _ev(ev, name: str, default=None):
    """CellEvidence(dataclass) · dict · SimpleNamespace 어느 모양이든 같은 필드를 읽는다."""
    if ev is None:
        return default
    if isinstance(ev, dict):
        return ev.get(name, default)
    return getattr(ev, name, default)


def _git_blob_sha(data: bytes) -> str:
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


# ── 문서 이름 파싱 ────────────────────────────────────────────────────────────────────────────
def _parse_with(g: _Grammar, path: str) -> dict:
    p = PurePosixPath(str(path).replace("\\", "/"))
    name, parent = p.name, p.parent.name
    stem = _strip_ext(name)
    out = {"kind": None, "class": None, "prefix": None, "token": None, "yymmddhh": None, "mmss": None,
           "topic": None, "legacy": False, "stem": stem, "ext": _ext_of(name)}
    if parent in g.dated_types:
        m = g.dated_re.match(name)
        if m and g.is_dated(parent, name):
            mid = name[m.end("hour"):m.start("topic")]          # "_" 또는 "_MM_SS_"
            out.update(kind=parent, prefix=parent, token=m.group("hour"), yymmddhh=m.group("hour"),
                       mmss=mid.strip("_") or None, topic=m.group("topic"))
        elif name.endswith(".md"):
            out.update(kind=parent, legacy=True, topic=stem)   # 규약 밖 이름 — 경로 인용으로만 닿는다
        return out
    if parent == "report":
        if not name.endswith(_DOC_EXTS):
            return out
        if g.report_re.match(stem):
            cls, rest = stem.split("_", 1)
            tt, rest = rest[:8], rest[9:]
            mm = re.match(r"(\d{2})_(\d{2})_(.+)\Z", rest)
            out.update(kind="report", **{"class": cls}, prefix=cls, token=tt, yymmddhh=tt,
                       mmss=f"{mm.group(1)}_{mm.group(2)}" if mm else None, topic=mm.group(3) if mm else rest)
        else:
            out.update(kind="report", legacy=True, topic=stem)   # 2026-08-24 개정 전 무날짜 kebab
        return out
    if parent == "benchmark":
        if name.startswith("."):
            return out            # 숨김 스윕 상태 `.sweep_<id>.json` — 문서 아님
        m = g.bench_re.match(name)
        if m:
            kraw = name[: m.start("tok") - 1]
            if kraw in g.bench_kinds:
                tok = m.group("tok")
                out.update(kind="certificate" if kraw == "benchmark" else kraw, prefix=kraw, token=tok,
                           yymmddhh=None if tok == "NA" else tok,
                           mmss=f"{m.group('mm')}_{m.group('ss')}" if m.group("mm") else None, topic=m.group("combo"))
            return out
        m = _SWEEP_MAP_RE.match(name)
        if m:
            out.update(kind="sweep_map" if m.group(5) == "md" else "sweep_json", prefix=SWEEP_MAP_PREFIX,
                       token=m.group(1), yymmddhh=m.group(1),
                       mmss=f"{m.group(2)}_{m.group(3)}" if m.group(2) else None, topic=m.group(4))
        return out
    if parent == "simlog":
        m = _SIMLOG_MODERN_RE.match(name)
        if m:
            tt, mm, ss, topic = m.groups()
            out.update(kind="simlog", prefix="simlog", token=tt, yymmddhh=tt,
                       mmss=f"{mm}_{ss}" if mm else None, topic=topic,
                       legacy=not _simlog_conformant(g, name, tt, mm, ss, topic))
            return out
        m = _SIMLOG_LEGACY_RE.match(name)
        if m:
            out.update(kind="simlog", prefix="simlog-legacy", token=m.group(1), yymmddhh=m.group(1)[2:],
                       topic=m.group(3), legacy=True)
        return out
    return out


def parse_doc_name(path, *, repo: Path | None = None) -> dict:
    """문서 경로 → `{kind, class, prefix, token, yymmddhh, mmss, topic, legacy, stem, ext}`.

    kind ∈ DOCUMENT_KINDS ∪ DATA_KINDS ∪ {None}. 문법은 SSOT 2종에서 적재한다(repo 없으면 이 체크아웃). `yymmddhh` 는
    KST 2자리 연도(레거시 simlog 10자리는 앞 2자리 절단) · `mmss` 는 `MM_SS` 또는 None(기본형이 먼저 태어났다)."""
    return _parse_with(_grammar(repo), str(path))


# ── 코퍼스 (발행 시점 파일) ─────────────────────────────────────────────────────────────────────
@dataclass
class _Node:
    path: str
    kind: str
    klass: str | None
    prefix: str | None
    token: str | None
    yymmddhh: str | None
    mmss: str | None
    stem: str
    ext: str
    is_dir: bool
    legacy: bool

    @property
    def is_document(self) -> bool:
        return self.kind in DOCUMENT_KINDS


def _banner_line_indices(lines: list[str]) -> set[int]:
    """헤더 12줄 안의 SUPERSEDED 배너 줄 번호(0-기반). 표지 줄 + 같은 인용 문단의 연속 줄(`> …`)까지 —
    실제 관행은 배너가 여러 줄에 걸친다(`testlog_26091009:3-4` · `perf_26091314:7→`). 헤더 밖은 읽지 않는다."""
    out: set[int] = set()
    n = min(len(lines), HEADER_LINES)
    i = 0
    while i < n:
        if _SUPERSEDE_MARK_RE.search(lines[i]):
            out.add(i)
            if lines[i].lstrip().startswith(">"):
                j = i + 1
                while j < n and lines[j].lstrip().startswith(">") and lines[j].lstrip()[1:].strip():
                    out.add(j)
                    j += 1
                i = j
                continue
        i += 1
    return out


class _SiblingFilter:
    """모호 해소 형제 가르기(2026-09-22 · plan_26092119 S2 round 2 · F7c).

    왜: 주제 없는 토큰(`sweep_map_26091008_*.md` 글롭 · 맨 스템)이 같은 시각 형제 여럿에 닿으면 종전에는 **전부**를 싣고 `ambiguous`
    로 표시했다. 태그2 재생에서 그 형제 25건 중 21건이 **다른 셀**의 지도(fp8·512k·1m…)였고 저작자는 그것을 "(모호 해소)" 소음으로
    읽기 예산에 넣었다. 형제 존재 여부(적중 ≥ 1)는 가르지 못한다 — 25건 전부가 셀 토큰 하나 이상(nv4·bf·262k·mmp 중)을 이름에 든다
    (슬러그 선택도 실측과 같은 결: OR 필터는 no-op). 그래서 **형제끼리 비교**한다: 이름(주제)·헤더 12줄에 적힌 셀 키 토큰 적중 수가
    최대인 형제, 동률이면 이름의 **이질 토큰**(셀 키에 없는 토큰) 수가 최소인 형제만 남긴다(= 이 셀에 가장 가까운 이름). 나머지는
    `ambiguous_excluded[]` 에 사유와 함께 적는다(조용한 탈락 ✗). 어느 형제도 셀 키를 적지 않으면 **무신호** — 가를 근거가 없으니
    전부 유지한다(종전 규칙). 정확 인용(exact)으로 따로 닿은 문서는 이 필터와 무관하게 채택된다(필터는 모호 간선에만 건다)."""

    def __init__(self, corpus: "_Corpus", key_strings):
        self.corpus = corpus
        keys = sorted({str(k).strip().lower() for k in key_strings if k is not None and len(str(k).strip()) >= 2})
        self.parts = {t for k in keys for t in re.split(r"[-_.\s]+", k) if len(t) >= 2}
        self.keys = sorted(set(keys) | self.parts)
        self.pats = _axis_patterns(self.keys)
        self.excluded: dict[str, dict] = {}
        # 어느 무리에서든 **남겨진** 형제 — 다른 무리에서 걸러졌더라도 "모호 해소로 걸러졌다" 고 적지 않는다(그 뒤의 부재는 필터·상한
        #   같은 다른 사유다 · 2026-09-22 round 2 리뷰).
        self.kept: set[str] = set()

    def _topic(self, path: str) -> str:
        info = _parse_with(self.corpus.g, path)
        return str(info.get("topic") or info.get("stem") or PurePosixPath(path).name).lower()

    def _score(self, path: str, extra) -> tuple[int, int]:
        topic = self._topic(path)
        head = "" if (self.corpus.repo / path).is_dir() else "\n".join(self.corpus.text(path).split("\n")[:HEADER_LINES])
        pats = self.pats + extra
        hits = sum(1 for _t, rx in pats if rx.search(topic) or rx.search(head))
        parts = self.parts | {t for t, _ in extra}
        foreign = sum(1 for t in re.split(r"[-_.\s]+", topic) if len(t) >= 2 and t not in parts)
        return hits, foreign

    def keep(self, src: str, token: str, members) -> set[str]:
        members = sorted(set(members))
        if len(members) < 2 or not self.pats:
            self.kept.update(members)
            return set(members)
        # 주제를 단 모호 토큰(개명 등)은 그 적힌 주제가 가장 강한 신호다 — 그 토큰도 이 무리의 셀 키에 더한다.
        typed = re.split(r"_\d{8}(?:_\d{2}_\d{2})?_?", str(token), maxsplit=1)
        extra = _axis_patterns([t for t in re.split(r"[-_.\s]+", typed[1].lower()) if len(t) >= 2]) \
            if len(typed) == 2 and typed[1] else []
        scores = {m: self._score(m, extra) for m in members}
        best_h = max(h for h, _f in scores.values())
        if best_h == 0:
            self.kept.update(members)
            return set(members)                # 무신호 — 가를 근거가 없다(전부 유지 · 종전 규칙)
        best_f = min(f for h, f in scores.values() if h == best_h)
        kept = {m for m, (h, f) in scores.items() if h == best_h and f == best_f}
        self.kept.update(kept)
        for m in members:
            if m in kept:
                continue
            h, f = scores[m]
            reason = (f"모호 해소(`{token}` · 형제 {len(members)}건) — 셀 키 적중 {h}·이질 토큰 {f} "
                      f"< 최선 형제 {best_h}·{best_f}(셀 키 {self.keys})")
            rec = self.excluded.get(m)
            if rec is None:
                rec = self.excluded[m] = {"from": set(), "token": token, "reason": reason}
            elif (token, reason) < (rec["token"], rec["reason"]):
                # 여러 무리에서 걸러지면 사전순 최소 (토큰, 사유) 를 적는다 — 호출 순서(집합 순회 · 문서 방문 순)와 무관한 결정론
                #   (2026-09-22 round 2 리뷰: 종전 setdefault 는 먼저 온 무리를 적어 LINEAGE.json 바이트가 순서에 묶였다).
                rec["token"], rec["reason"] = token, reason
            rec["from"].add(src)
        return kept


class _Corpus:
    """roots 아래 문서·데이터 노드와 인용 해소기. 파일을 읽을 뿐 쓰지 않는다."""

    def __init__(self, repo: Path, roots=DEFAULT_ROOTS, exclude=EXCLUDED_ROOTS, grammar: _Grammar | None = None):
        self.repo = repo
        self.g = grammar or _grammar(repo)
        self.exclude = tuple(exclude)
        self.sibling_filter: _SiblingFilter | None = None   # derive 가 첫 mentions() 전에 건다(mention_graph 는 셀 무관 · None)
        self.nodes: dict[str, _Node] = {}
        self.by_key: dict[tuple[str, str], list[str]] = {}
        self.undated: dict[str, list[str]] = {}
        self.roots_absent: list[str] = []
        self._text: dict[str, str] = {}
        self._mentions: dict[str, list[dict]] = {}
        self._narrators: dict[str, list[tuple[str, str, str]]] | None = None
        self._slug: dict[tuple[str, str], bool] = {}
        for root in roots:
            root = str(root).strip("/")
            if _excluded(root, self.exclude):
                continue
            d = repo / root
            if not d.is_dir():
                self.roots_absent.append(root)
                continue
            dir_nodes = PurePosixPath(root).name == "simlog"     # simlog 는 run 디렉터리 1개 = 노드 1개
            for child in sorted(d.iterdir(), key=lambda c: c.name):
                if child.name.startswith(".") or child.name in EXCLUDED_BASENAMES:
                    continue
                if dir_nodes != child.is_dir():
                    continue
                self._add(f"{root}/{child.name}")
        prefixes = set(self.g.dated_types) | set(self.g.bench_kinds) | {SWEEP_MAP_PREFIX} | {
            n.klass for n in self.nodes.values() if n.klass}
        alt = "|".join(re.escape(p) for p in sorted(prefixes, key=lambda s: (-len(s), s)))
        # 인용 토큰 문법 = SSOT 접두 집합(산문 5 · 벤치 3 · sweep_map · 실재하는 report 분류) + 시간 토큰 — 손 목록 ✗
        self.stem_re = re.compile(r"(?<![A-Za-z0-9_])(?:docs/(?P<dir>[a-z_]+)/)?(?P<pfx>" + alt
                                  + r")_(?P<tt>\d{8}|NA)(?![0-9])")

    def _add(self, rel_path: str) -> _Node | None:
        if rel_path in self.nodes:
            return self.nodes[rel_path]
        if _excluded(rel_path, self.exclude):
            return None
        info = _parse_with(self.g, rel_path)
        if info["kind"] is None:
            return None
        is_dir = (self.repo / rel_path).is_dir()
        if (info["kind"] == "simlog") != is_dir:
            return None
        node = _Node(path=rel_path, kind=info["kind"], klass=info["class"], prefix=info["prefix"],
                     token=info["token"], yymmddhh=info["yymmddhh"], mmss=info["mmss"], stem=info["stem"],
                     ext=info["ext"], is_dir=is_dir, legacy=info["legacy"])
        self.nodes[rel_path] = node
        if node.prefix and node.token:
            self.by_key.setdefault((node.prefix, node.token), []).append(rel_path)
        else:
            self.undated.setdefault(PurePosixPath(rel_path).parent.name, []).append(rel_path)
        return node

    def ensure(self, rel_path: str) -> _Node | None:
        """root 밖이지만 문서 문법을 가진 경로(예 선언된 `docs/request/…`)를 순회 가능한 노드로 들인다."""
        if not (self.repo / rel_path).exists():
            return None
        return self._add(rel_path)

    # 본문
    def text(self, rel_path: str) -> str:
        if rel_path not in self._text:
            p = self.repo / rel_path
            if p.is_dir():
                self._text[rel_path] = ""
            else:
                try:
                    self._text[rel_path] = p.read_text(encoding="utf-8", errors="replace")
                except OSError as e:
                    core.fail("HINT_LINEAGE_DOC_UNREADABLE", f"계보 입력 문서를 읽을 수 없다: {rel_path}: {e}")
        return self._text[rel_path]

    def slug_hit(self, rel_path: str, pat: re.Pattern) -> bool:
        key = (rel_path, pat.pattern)
        if key not in self._slug:
            p = self.repo / rel_path
            hit = False
            if p.is_dir():
                for f in sorted(p.rglob("*")):
                    if f.is_file() and f.suffix in _TEXT_SUFFIXES:
                        try:
                            if pat.search(f.read_text(encoding="utf-8", errors="replace")):
                                hit = True
                                break
                        except OSError as e:
                            core.fail("HINT_LINEAGE_DOC_UNREADABLE", f"계보 후보를 읽을 수 없다: {f}: {e}")
            else:
                hit = bool(pat.search(self.text(rel_path)))
            self._slug[key] = hit
        return self._slug[key]

    # 인용 해소
    def _pick(self, text: str, start: int, after_tt: int, cands: list[str]):
        for c in sorted(cands, key=lambda c: (-len(self.nodes[c].stem), c)):
            st = self.nodes[c].stem
            if text.startswith(st, start) and not _is_name_char(_char_at(text, start + len(st))):
                twins = [x for x in cands if self.nodes[x].stem == st]
                if len(twins) > 1:          # sweep_map md/json 쌍둥이 — 명시 확장자가 있으면 그것
                    ext = _ext_at(text, start + len(st))
                    twins = [x for x in twins if self.nodes[x].ext == ext] or twins
                return start, sorted(twins), "exact", st
        pos = after_tt
        mmss = None
        m = _MMSS_AT_RE.match(text, pos)
        if m:
            mmss, pos = f"{m.group(1)}_{m.group(2)}", m.end()
        topic_carrying = _char_at(text, pos) == "_" and _is_name_char(_char_at(text, pos + 1))
        sib = sorted(c for c in cands if mmss is None or self.nodes[c].mmss == mmss)
        if not sib:
            return None
        if topic_carrying:           # 주제를 달았는데 어느 이름과도 불일치(개명 등) — 형제 전부를 모호로
            end = pos + 1
            while end < len(text) and (_is_name_char(text[end])
                                       or (text[end] == "." and _is_name_char(_char_at(text, end + 1)))):
                end += 1
            return start, sib, "ambiguous", text[start:end]
        return start, sib, ("unique" if len(sib) == 1 else "ambiguous"), text[start:pos]

    def resolve_text(self, text: str) -> list[tuple[int, list[str], str, str]]:
        """본문의 모든 문서 인용 → [(위치, 대상 경로들, 해소, 토큰)]. 위치 순."""
        out = []
        for m in self.stem_re.finditer(text):
            cands = self.by_key.get((m.group("pfx"), m.group("tt")), [])
            if m.group("dir"):
                cands = [c for c in cands if c.startswith(f"docs/{m.group('dir')}/")]
            if cands:
                r = self._pick(text, m.start("pfx"), m.end("tt"), cands)
                if r:
                    out.append(r)
        for m in _SIMLOG_TOKEN_RE.finditer(text):
            tt = m.group("tt")
            cands = self.by_key.get(("simlog-legacy" if len(tt) == 10 else "simlog", tt), [])
            if cands:
                r = self._pick(text, m.start("tt"), m.end("tt"), cands)
                if r:
                    out.append(r)
        for m in _UNDATED_PATH_RE.finditer(text):
            paths = self.undated.get(m.group("dir"), [])
            start = m.start("rest")
            for c in sorted(paths, key=lambda c: (-len(PurePosixPath(c).name), c)):
                hit = None
                for cand_name in (PurePosixPath(c).name, self.nodes[c].stem):
                    if text.startswith(cand_name, start) \
                            and not _is_name_char(_char_at(text, start + len(cand_name))):
                        hit = cand_name
                        break
                if hit:
                    out.append((start, [c], "exact", hit))
                    break
        out.sort(key=lambda r: (r[0], r[3]))
        return out

    def mentions(self, rel_path: str) -> list[dict]:
        """문서 하나가 언급하는 코퍼스 노드 — `[{to, edge, token, resolution}]`(중복 제거·정렬).
        edge ∈ mention | header | header:<라벨> | supersede. 데이터 노드(simlog·인증서…)는 인용 원천이 아니다
        (init_wiki_desk "Do not interpret raw run logs")."""
        if rel_path in self._mentions:
            return self._mentions[rel_path]
        node = self.nodes.get(rel_path)
        if node is None or not node.is_document:
            self._mentions[rel_path] = []
            return []
        text = self.text(rel_path)
        lines = text.split("\n")
        starts = [0]
        for mm in re.finditer("\n", text):
            starts.append(mm.end())
        banner = _banner_line_indices(lines)
        seen: set[tuple[str, str, str, str]] = set()
        for start, targets, res, token in self.resolve_text(text):
            li = bisect.bisect_right(starts, start) - 1
            if li in banner:
                edge = "supersede"
            elif li < HEADER_LINES:
                edge = "header"
                lm = _HEADER_LABEL_RE.match(lines[li])
                if lm:
                    # 라벨 단어 **바로 뒤**의 첫 문서 토큰만 라벨 간선이다(`> 선행 \`docs/report/…\``). 사이에 허용되는
                    # 것은 백틱·공백과 `docs/<dir>/` 접두 하나뿐 — 같은 줄 뒤쪽 토큰까지 라벨을 붙이면 "선행" 이 번진다.
                    tok_at = start - starts[li] - lm.end()
                    gap = lines[li][lm.end():lm.end() + tok_at].replace("`", "").strip() if tok_at >= 0 else None
                    if gap is not None and (gap == "" or _DOCS_DIR_PREFIX_RE.fullmatch(gap)):
                        edge = f"header:{lm.group(1)}"
            else:
                edge = "mention"
            for t in targets:
                if t != rel_path:
                    seen.add((t, edge, token, res))
        if self.sibling_filter is not None:
            groups: dict[str, set] = {}
            for t, _e, k, r in seen:
                if r == "ambiguous":
                    groups.setdefault(k, set()).add(t)
            drop = {(m, k) for k, members in sorted(groups.items())
                    for m in members - self.sibling_filter.keep(rel_path, k, members)}
            seen = {row for row in seen if not (row[3] == "ambiguous" and (row[0], row[2]) in drop)}
        out = [{"to": t, "edge": e, "token": k, "resolution": r} for t, e, k, r in sorted(seen)]
        self._mentions[rel_path] = out
        return out

    def output_mentions(self, rel_path: str) -> list[str]:
        """문서가 언급한 원시 증거 경로(`output/<t>/benchlog/…`) 중 **실재하는** 것. 글롭·중괄호는 앞 디렉터리까지."""
        node = self.nodes.get(rel_path)
        if node is None or not node.is_document:
            return []
        found = set()
        for m in _OUTPUT_PATH_RE.finditer(self.text(rel_path)):
            s = m.group(0).rstrip(".,:;…·").rstrip("/")
            if ".." in s.split("/") or _excluded(s, self.exclude):
                continue
            if (self.repo / s).exists():
                found.add(s)
        return sorted(found)

    def narrators(self) -> dict[str, list[tuple[str, str, str]]]:
        """testlog → 그것을 언급한 devlog 목록(registry `evidences` 의 서술자 방향 · hybrid 보강 입력)."""
        if self._narrators is None:
            idx: dict[str, list[tuple[str, str, str]]] = {}
            for p in sorted(self.nodes):
                if self.nodes[p].kind != "devlog":
                    continue
                for e in self.mentions(p):
                    if self.nodes[e["to"]].kind == "testlog":
                        idx.setdefault(e["to"], []).append((p, e["token"], e["resolution"]))
            self._narrators = {k: sorted(set(v)) for k, v in idx.items()}
        return self._narrators


# ── 공개: 그래프 · 배너 ─────────────────────────────────────────────────────────────────────────
def mention_graph(repo: Path, *, roots=DEFAULT_ROOTS, exclude=EXCLUDED_ROOTS) -> dict:
    """roots 아래 모든 문서의 mention 간선 `{src: [{to, edge, token, resolution}]}`(src·간선 정렬). 순회 없이 그래프만."""
    corpus = _Corpus(Path(repo), roots, exclude)
    return {p: corpus.mentions(p) for p in sorted(corpus.nodes) if corpus.nodes[p].is_document
            and corpus.mentions(p)}


def supersede_banners(path) -> list[str]:
    """헤더 12줄 안의 SUPERSEDED 배너(어느 꼴이든 · 인용 문단 연속 줄 포함)에 적힌 **문서 토큰**(원문 그대로 · 등장 순 ·
    확장자 제거 · 자기 자신 제외). 누가 누구를 뒤집었는가의 해석(날짜 방향)은 `derive` 가 한다 — 배너 줄에는 뒤집은 쪽이
    뒤집힌 문서를 적는 관행(`devlog_26091109:5` "뒤집은 선행: … (SUPERSEDED-IN-PART)")도 섞여 있다."""
    p = Path(path)
    try:
        lines = p.read_text(encoding="utf-8", errors="replace").split("\n")[:HEADER_LINES]
    except OSError as e:
        core.fail("HINT_LINEAGE_DOC_UNREADABLE", f"배너를 읽을 수 없다: {path}: {e}")
    self_stem = _strip_ext(p.name)
    out: list[str] = []
    for i in sorted(_banner_line_indices(lines)):
        for m in _GENERIC_DOC_TOKEN_RE.finditer(lines[i]):
            tok = _strip_ext(m.group("tok").rstrip(".,:;…·"))
            if tok and tok != self_stem and tok not in out:
                out.append(tok)
        for m in _SIMLOG_TOKEN_RE.finditer(lines[i]):
            tail = re.match(r"[^\s`'\"<>()\[\]|,;*§/]+", lines[i][m.start("tt"):])
            tok = tail.group(0).rstrip(".,:;…·") if tail else m.group("tt")
            if tok not in out:
                out.append(tok)
    return out


# ── 발행 기록 → 시드 ──────────────────────────────────────────────────────────────────────────
def load_publication_records(repo: Path) -> list[dict]:
    """`docs/_evidence/*.json`(`*.work-manifest.json` 제외) 원문 dict + `_id`(파일 stem). evidence 모듈이 넘기지 않을 때의
    대체 적재기 — evidence.publication_records 와 **같은 규칙**이다. 읽지 못한 기록은 버리지 않고
    `{"_id", "_unreadable": 사유}` 로 남긴다(부재와 실패는 다른 사실)."""
    d = Path(repo) / core.REL_EVIDENCE_DIR
    out: list[dict] = []
    if not d.is_dir():
        return out
    for f in sorted(d.glob("*.json"), key=lambda x: x.name):
        if f.name.endswith(".work-manifest.json"):
            continue
        stem = f.name[: -len(".json")]
        try:
            data = json.loads(f.read_text(encoding="utf-8"))
        except (OSError, ValueError) as e:
            out.append({"_id": stem, "_unreadable": f"{type(e).__name__}: {e}"})
            continue
        if not isinstance(data, dict):
            out.append({"_id": stem, "_unreadable": f"최상위가 객체가 아니다({type(data).__name__})"})
            continue
        rec = dict(data)
        rec["_id"] = stem
        out.append(rec)
    return out


def _record_pointers(rec: dict) -> list[tuple[str, str, str]]:
    """발행 기록의 증거 포인터 → [(포인터 이름, 원 경로, 기준 디렉터리)]. evidence_publisher 기록의 세 칸
    (narrative_status·scaffolded·raw_log_paths)은 저장소 상대, work-manifest 모양의 `evidence.<k>.path` 는 manifest
    디렉터리 기준(옛 hint_collect `ev_path` 규칙)."""
    out: list[tuple[str, str, str]] = []
    ns = rec.get("narrative_status")
    if isinstance(ns, dict):
        for k in sorted(ns):
            v = ns[k]
            if isinstance(v, dict) and isinstance(v.get("source"), str):
                out.append((f"narrative_status.{k}", v["source"], ""))
    for group in ("scaffolded", "raw_log_paths"):
        g = rec.get(group)
        if isinstance(g, dict):
            for k in sorted(g):
                if isinstance(g[k], str):
                    out.append((f"{group}.{k}", g[k], ""))
    ev = rec.get("evidence")
    if isinstance(ev, dict):
        for k in sorted(ev):
            v = ev[k]
            if isinstance(v, dict) and isinstance(v.get("path"), str):
                out.append((f"evidence.{k}", v["path"], core.REL_EVIDENCE_DIR))
    return out


def _norm_pointer(repo: Path, raw: str, base: str) -> tuple[str | None, str | None]:
    """(저장소 상대 경로, 실패 사유). 절대경로는 저장소 안이면 상대화, 밖이면 outside-repo."""
    s = raw.strip()
    if not s:
        return None, "empty"
    if os.path.isabs(s):
        try:
            return core.rel(repo, s), None
        except core.HintError:
            return None, "outside-repo"
    s = posixpath.normpath(posixpath.join(base, s) if base else s)
    if s == ".." or s.startswith("../"):
        return None, "outside-repo"
    return s, None


def _identity_equal(a, b) -> bool:
    if not isinstance(a, dict) or not isinstance(b, dict):
        return False
    for f in IDENTITY_MATCH_FIELDS:
        if a.get(f) in (None, "") or b.get(f) in (None, ""):
            return False
        if _norm_val(a[f]) != _norm_val(b[f]):
            return False
    return True


def _record_kst(rec: dict) -> str | None:
    u = rec.get("generated_utc")
    if isinstance(u, str):
        try:
            return core.kst_token(u)
        except core.HintError:
            return None
    return None


def seeds_from_publications(repo: Path, records, *, this_topic: str | None, identity: dict | None,
                            cell_key: str | None, publish_kst: str | None = None) -> dict:
    """시드 S2(`lineage_graph.md` §8.2): 이 발행 기록 + **같은 identity · 같은 셀 키**의 과거 발행 기록의 포인터.

    records   = `docs/_evidence/*.json` 원문 dict 목록(각 `_id` = 파일 stem · evidence 모듈이 넘긴다 · 없으면
                `load_publication_records`). 셀 대조 = 기록의 simlog 사본 `sweep_index.json` meta.config_name(관측) 이 있으면
                그것, 없을 때만 정규화 발행 id 가 셀 키와 같거나 `_<셀 키>` 로 끝남(`qwen38fn_full_nv4_bf_262k_mmp` ↔ 셀
                `nv4-bf-262k-mmp`). identity 대조 = IDENTITY_MATCH_FIELDS(대소문자 무시 — 같은 셀의 옛 기록이
                `Qwen3.8-Flash-Next-NVFP4` 로 적혀 있다).
    반환      {"seeds": [{path, source, pointer}], "missing": [{path, source, reason}], "this": id|None,
               "prior": [ids], "prior_after_ceiling": [ids], "cell_key": 정규화 키|None,
               "prior_match": {id: 대조 근거}, "prior_cell_mismatch": [id 는 셀처럼 보이나 측정 셀이 다른 기록]}
    """
    repo = Path(repo)
    seeds: list[dict] = []
    missing: list[dict] = []
    by_id: dict[str, dict] = {}
    for r in records or ():
        if isinstance(r, dict):
            rid = r.get("_id") or r.get("publication_id")
            if isinstance(rid, str) and rid:
                by_id.setdefault(rid, r)
    this = by_id.get(this_topic) if this_topic else None
    this_ok = this is not None and "_unreadable" not in this
    if this_topic and this is None:
        missing.append({"path": f"{core.REL_EVIDENCE_DIR}/{this_topic}.json", "source": f"publication:{this_topic}",
                        "reason": "record-absent"})
    elif this_topic and not this_ok:
        missing.append({"path": f"{core.REL_EVIDENCE_DIR}/{this_topic}.json", "source": f"publication:{this_topic}",
                        "reason": "record-unreadable"})

    def take(rec: dict, source: str) -> None:
        for pointer, raw, base in _record_pointers(rec):
            path, why = _norm_pointer(repo, raw, base)
            if path is None:
                missing.append({"path": raw, "source": source, "reason": why})
            else:
                seeds.append({"path": path, "source": source, "pointer": pointer})

    if this_ok:
        take(this, f"publication:{this_topic}")
    ident = identity if isinstance(identity, dict) and identity else (this.get("identity") if this_ok else None)
    ck = _norm_key(cell_key) if cell_key else None
    prior: list[str] = []
    late: list[str] = []
    how: dict[str, str] = {}
    cell_mismatch: list[str] = []
    if ident and ck:
        for rid in sorted(by_id):
            rec = by_id[rid]
            if rid == this_topic or "_unreadable" in rec:
                continue
            if not _identity_equal(rec.get("identity"), ident):
                continue
            # 셀 대조 = **관측 먼저**(2026-09-22 감사): 발행 기록에는 셀 필드가 없고 id 는 사람이 지은 이름이라, 접미 규칙만으로는
            # `bf-262k-mmp` 가 `nv4_bf_262k_mmp` 의 기록을 삼킨다. 기록의 simlog 사본 `sweep_index.json` 의 `meta.config_name`
            # (측정이 실제로 돈 셀 · evidence._cell_of_publication 과 같은 원천)이 있으면 그것만 믿고, 없을 때만 id 접미 규칙을 쓰고
            # 그 사실을 method 에 적는다.
            via, mismatch = record_cell_match(repo, rid, rec, ck)
            if via is None:
                if mismatch:
                    cell_mismatch.append(rid)     # id 는 이 셀처럼 보이지만 측정은 다른 셀이었다 — 삼키지 않는다
                continue
            k = _record_kst(rec)
            if publish_kst and k is not None and k > publish_kst:
                late.append(rid)          # 발행 시각 뒤의 기록 — 그때는 없던 정보(D-W8 와 같은 결)
                continue
            prior.append(rid)
            how[rid] = via
            take(rec, f"prior_publication:{rid}")
    return {"seeds": seeds, "missing": missing, "this": this_topic if this_ok else None, "prior": prior,
            "prior_after_ceiling": late, "cell_key": ck, "prior_match": how, "prior_cell_mismatch": cell_mismatch}


_CAMPAIGN_ID_TOKEN_RE = re.compile(r"^camp\d*\Z")    # 캠페인 id 접두(`camp-YYMMDDHH-…` · `camp26090721_…`) — 모델 별칭이 아니다


def _id_tokens(rid: str) -> list[str]:
    return [t for t in re.split(r"[^0-9a-z]+", str(rid).lower()) if t]


def _alias_token_ok(tok: str) -> bool:
    return (len(tok) >= 4 and any(c.isalpha() for c in tok) and any(c.isdigit() for c in tok)
            and not _CAMPAIGN_ID_TOKEN_RE.match(tok))


def model_aliases(records, pat: re.Pattern) -> dict:
    """같은 모델의 **관측된 별칭**(`ds4f0731` · `qwen38fn`)을 발행 기록에서 파생한다 — 손으로 적은 약어표가 아니다.

    규칙(결정론 · 입력 = 발행 기록 id·identity.model 뿐): identity.model 이 base 슬러그에 걸리는 기록(= 같은 모델)의 **id 첫 토큰**
    중 ① 영문·숫자를 함께 든 4자 이상 ② 캠페인 id 접두(`camp…`)가 아님 ③ 같은 모델 기록 **2건 이상**의 첫 토큰 ④ 다른 모델 기록 id
    의 어느 토큰으로도 나오지 않음 — 을 모두 만족하는 토큰. 별칭은 약어라 문서 본문 필터에는 쓰지 않는다(`slug_pattern` docstring ·
    재현율 근거) — 기록 대조(identity.model 이 빈 기록)와 method 기재에만 쓴다.
    반환 {"aliases": [...], "evidence": {alias: [기록 id…]}}"""
    same: dict[str, list[str]] = {}
    other_tokens: set[str] = set()
    for r in records or ():
        if not isinstance(r, dict) or "_unreadable" in r:
            continue
        rid = r.get("_id") or r.get("publication_id")
        if not isinstance(rid, str) or not rid:
            continue
        ident = r.get("identity") if isinstance(r.get("identity"), dict) else {}
        model = ident.get("model")
        toks = _id_tokens(rid)
        if isinstance(model, str) and model.strip() and pat.search(model):
            if toks and _alias_token_ok(toks[0]):
                same.setdefault(toks[0], []).append(rid)
        elif isinstance(model, str) and model.strip():
            other_tokens.update(toks)
    aliases = {t: sorted(ids) for t, ids in same.items() if len(ids) >= 2 and t not in other_tokens}
    return {"aliases": sorted(aliases), "evidence": {t: aliases[t] for t in sorted(aliases)}}


def seeds_from_model_priors(repo: Path, records, *, this_topic: str | None, identity: dict | None, pat: re.Pattern,
                            publish_kst: str | None, exclude=()) -> dict:
    """시드 S3 — **같은 모델의 앞선 셀** 발행 기록의 문서 포인터(2026-09-29 plan_26092908 §4.6 R-a).

    왜: S2(`seeds_from_publications`)는 같은 identity · **같은 셀 키**의 기록만 따른다. 새 셀(DS4F `ds4f0731-1m-spec7-roce`)은 같은
    모델의 앞선 bring-up(`ds4f0731_vllm029rc6_multi_bump` · `ds4f0731_029rc6_hint_l4combo`)과 셀 키가 달라 한 건도 못 잡고, 이번 캠페인
    문서는 앞선 계보를 본문에 인용하지 않았다 — v6 발행 때 그 계보를 날라 준 것은 **휘발 캠페인 인스턴스의 grounding 참조**였고, 재생
    (purge 뒤)·캠페인 밖 발행에서는 그 통로가 없다(LINEAGE 12 → 6). 계보의 입력이 휘발 파일에 달려 있으면 같은 셀이 발행 시점에 따라
    다른 계보를 낸다 — 추적 평면(발행 기록)에서 결정론으로 파생한다.

    대조(같은 모델) = 기록 identity.model 이 base 슬러그 패턴에 걸림(체크포인트 슬러그 `…-nvfp4`·대소문자 변형 포함) · 또는 identity.model
    이 빈 기록의 id 첫 토큰이 관측 별칭(`model_aliases`). gpu 가 양쪽에 있으면 같아야 한다(HW 가 다른 계보는 이 태그의 계보가 아니다).
    발행 시각 상한 뒤 기록 ✗(D-W8) · S2 가 이미 잡은 같은 셀 기록 ✗(exclude). **문서 포인터만** 싣는다 — 다른 셀의 simlog·원시 jsonl 은
    이 셀의 측정 증거가 아니다(데이터 후보에 섞이면 발췌 출처가 남의 측정을 가리킨다) — 실제 거름은 derive 의 MODEL_PRIOR_KINDS. 기록의 identity 가 모델을 선언했으므로 슬러그
    본문 필터는 면제다(별칭만 적은 bring-up 문서 `plan_26090820_ds4f0731_…` 가 필터에 걸려 사라지지 않게).
    반환 {"seeds": [{path, source, pointer}], "missing": [...], "records": [ids], "match": {id: 근거}, "after_ceiling": [ids],
          "gpu_mismatch": [ids], "aliases": model_aliases(...)}"""
    repo = Path(repo)
    al = model_aliases(records, pat)
    alias_set = set(al["aliases"])
    ident = identity if isinstance(identity, dict) else {}
    gpu = ident.get("gpu")
    excl = set(exclude or ())
    seeds: list[dict] = []
    missing: list[dict] = []
    took: list[str] = []
    how: dict[str, str] = {}
    late: list[str] = []
    gpu_mis: list[str] = []
    by_id: dict[str, dict] = {}
    for r in records or ():
        if isinstance(r, dict):
            rid = r.get("_id") or r.get("publication_id")
            if isinstance(rid, str) and rid:
                by_id.setdefault(rid, r)
    for rid in sorted(by_id):
        rec = by_id[rid]
        if rid == this_topic or rid in excl or "_unreadable" in rec:
            continue
        ri = rec.get("identity") if isinstance(rec.get("identity"), dict) else {}
        model = ri.get("model")
        if isinstance(model, str) and model.strip():
            if not pat.search(model):
                continue
            via = "identity.model~base_slug"
        else:
            toks = _id_tokens(rid)
            if not toks or toks[0] not in alias_set:
                continue
            via = f"id-alias:{toks[0]}(identity.model 부재)"
        if gpu not in (None, "") and ri.get("gpu") not in (None, "") and _norm_val(ri["gpu"]) != _norm_val(gpu):
            gpu_mis.append(rid)
            continue
        k = _record_kst(rec)
        if publish_kst and k is not None and k > publish_kst:
            late.append(rid)
            continue
        took.append(rid)
        how[rid] = via
        for pointer, raw, base in _record_pointers(rec):
            path, why = _norm_pointer(repo, raw, base)
            if path is None:
                missing.append({"path": raw, "source": f"{MODEL_PRIOR_SOURCE}{rid}", "reason": why})
            else:
                seeds.append({"path": path, "source": f"{MODEL_PRIOR_SOURCE}{rid}", "pointer": pointer})
    return {"seeds": seeds, "missing": missing, "records": took, "match": how, "after_ceiling": late,
            "gpu_mismatch": gpu_mis, "aliases": al}


def record_cell_match(repo: Path, rid: str, rec: dict, cell_key: str) -> tuple[str | None, bool]:
    """발행 기록이 이 셀의 측정인가 → (대조 근거 | None, id 는 이 셀처럼 보이나 관측된 측정 셀이 다른가). 관측(simlog 사본
    `sweep_index.meta.config_name`) 먼저, 없을 때만 발행 id 접미. 계보 시드(seeds_from_publications)와 evidence 의 과거 측정 창
    (event_timeline · 2026-09-22 S2 round 3)이 **같은 규칙 한 벌**을 쓴다 — 두 자리에 적으면 갈라진다(매직넘버·중복 개념 판정표)."""
    ck = _norm_key(cell_key)
    nid = _norm_key(rid)
    looks = nid == ck or nid.endswith("_" + ck)
    observed = _record_cell(repo, rec)
    if observed is not None:
        if _norm_key(observed) != ck:
            return None, looks
        return "simlog:sweep_index.meta.config_name", False
    return ("publication-id-suffix(simlog 미관측)" if looks else None), False


def identity_equal(a, b) -> bool:
    """IDENTITY_MATCH_FIELDS 대조(대소문자 무시 · 빈 값은 불일치) — evidence 과거 측정 창이 같은 규칙을 쓴다(공개 별칭)."""
    return _identity_equal(a, b)


def _record_cell(repo: Path, rec: dict) -> str | None:
    """발행 기록이 가리키는 simlog 사본의 측정 셀(`sweep_index.json` meta.config_name → config). 읽지 못하면 None(모름 ≠ 다름)."""
    for group in ("raw_log_paths", "scaffolded"):
        g = rec.get(group)
        raw = g.get("simlog") if isinstance(g, dict) else None
        if not isinstance(raw, str) or not raw.strip():
            continue
        path, _ = _norm_pointer(repo, raw, "")
        if path is None or _excluded(path):
            continue
        f = repo / path / "sweep_index.json"
        try:
            doc = json.loads(f.read_text(encoding="utf-8")) if f.is_file() else None
        except (OSError, ValueError):
            doc = None
        if isinstance(doc, dict):
            meta = doc.get("meta") if isinstance(doc.get("meta"), dict) else {}
            name = meta.get("config_name") or doc.get("config")
            if isinstance(name, str) and name.strip():
                return name.strip()
    return None


# ── 필터 · 관련도 ─────────────────────────────────────────────────────────────────────────────
def slug_pattern(base_slug: str) -> re.Pattern:
    """base 슬러그 → 구분자 무관 정규식(`qwen3.8-flash-next` ≡ `Qwen3.8 Flash Next` ≡ `qwen3_8_flash_next`).

    **채택 필터는 모델 슬러그 하나**다(2026-09-21 선택도 실측 `lineage_graph.md` §8.4): 정규화 슬러그는 표적 12/12 ·
    코퍼스 26% 에 걸린다. 축 토큰(nvfp4·262k·mmap·ple …)은 코퍼스 10~54% 에 걸려 OR 필터로는 no-op 이라 관련도 순위에만
    쓴다. 양자화 접미를 뗀 **base** 여야 한다(`-nvfp4` 를 붙이면 devlog_26091114 가 떨어진다) — 떼는 규칙의 단일 소유자는
    `naming.base_slug` 이고 이 모듈은 결과만 받는다."""
    toks = [t for t in re.split(r"[^0-9a-z]+", str(base_slug).lower()) if t]
    if not toks:
        core.fail("HINT_LINEAGE_FILTER_ABSENT", f"계보 필터 base 슬러그가 비었다: {base_slug!r}",
                  "PAYLOAD.identity.base_slug(naming.base_slug 파생)를 넘긴다.")
    body = r"[-_. ]?".join(re.escape(t) for t in toks)
    return re.compile(r"(?<![0-9a-z])" + body + r"(?![0-9a-z])", re.IGNORECASE)


def _axis_patterns(tokens) -> list[tuple[str, re.Pattern]]:
    out = []
    for t in sorted({str(t).strip().lower() for t in tokens if t is not None and len(str(t).strip()) >= 2}):
        body = r"[-_]".join(re.escape(p) for p in re.split(r"[-_]", t) if p)
        if body:
            out.append((t, re.compile(r"(?<![0-9a-z])" + body + r"(?![0-9a-z])", re.IGNORECASE)))
    return out


# ── 선언(--lineage-add) ──────────────────────────────────────────────────────────────────────
def _parse_declared(repo: Path, declared) -> list[tuple[str, str]]:
    """사람이 보충한 계보 `PATH=REASON` → [(상대경로, 사유)]. 사람 입력이 불변식을 어기면 **소리 내어** 막는다
    (출처 없는 추가 ✗ · 배제 root ✗ · 부재 ✗)."""
    out: list[tuple[str, str]] = []
    for item in declared or ():
        if isinstance(item, str):
            path, _, reason = item.partition("=")
        elif isinstance(item, dict):
            path, reason = item.get("path"), item.get("reason")
        elif isinstance(item, (tuple, list)) and len(item) == 2:
            path, reason = item
        else:
            core.fail("HINT_LINEAGE_DECLARED_SHAPE", f"--lineage-add 모양이 아니다: {item!r}", "PATH=REASON 로 넘긴다.")
        path, reason = str(path or "").strip(), str(reason or "").strip()
        if not path or not reason:
            core.fail("HINT_LINEAGE_DECLARED_REASON_ABSENT", f"보충 계보에 경로·사유가 모두 있어야 한다: {item!r}",
                      "--lineage-add PATH=REASON — 사유 없는 보충은 출처 없는 추가다(plan §4.3).")
        rel = core.rel(repo, path)
        if _excluded(rel):
            core.fail("HINT_LINEAGE_DECLARED_EXCLUDED", f"배제 root 의 문서는 계보에 넣을 수 없다: {rel}",
                      f"배제 root = {', '.join(EXCLUDED_ROOTS)} (헌법 비색인 · 백업/미러 섀도잉). docs/ 의 정본 경로를 넣는다.")
        if not (repo / rel).exists():
            core.fail("HINT_LINEAGE_DECLARED_ABSENT", f"보충 계보 경로가 없다: {rel}", "저장소 상대 경로를 확인한다.")
        out.append((rel, reason))
    return sorted(set(out))


def tool_snapshot_rel(name: str, rev: str) -> str:
    """측정 도구 스냅숏의 draft 상대 경로 `inputs/sources/<이름>@<rev12>`(모양의 단일 소유자 · evidence.tool_snapshots 가 부른다)."""
    rel_path = f"{TOOL_SNAPSHOT_DIR}/{name}@{str(rev)[:12]}"
    if not _TOOL_SNAPSHOT_RE.match(rel_path) or not re.fullmatch(r"[0-9a-f]{40}", str(rev)):
        core.fail("HINT_LINEAGE_TOOL_SOURCE_SHAPE", f"도구 스냅숏 이름·커밋 모양이 아니다: {name!r}@{str(rev)[:12]!r}",
                  "이름은 파일 basename, 커밋은 40자 SHA 여야 한다.")
    return rel_path


def _tool_candidate_problem(repo: Path, c: dict) -> str | None:
    """`kind: tool-source@rev` 후보의 결함 사유(없으면 None). 경로(draft 상대 `inputs/sources/<이름>@<rev12>`) ↔ origin
    (`git:<40자>:<저장소 경로>`)이 같은 것을 말해야 하고(이름 = basename · rev12 = 접두) git 이 그 바이트를 들어야 한다
    (`cat-file -e <rev>:<path>` — git 이 아니면 확인 불가로 기재). 저장소 경로는 정규화된 상대 경로만(탈출 ✗)."""
    pm = _TOOL_SNAPSHOT_RE.match(str(c.get("path") or ""))
    om = _TOOL_ORIGIN_RE.match(str(c.get("origin") or ""))
    if pm is None or om is None:
        return "tool-source-shape(path 는 inputs/sources/<이름>@<rev12> · origin 은 git:<40자 SHA>:<저장소 상대 경로>)"
    rev, rpath = om.group(1), om.group(2)
    if posixpath.normpath(rpath) != rpath or rpath.startswith(("../", "/")) or rpath == "..":
        return "tool-source-origin-path(정규화된 저장소 상대 경로가 아니다)"
    if PurePosixPath(rpath).name != pm.group(1) or rev[:12] != pm.group(2):
        return "tool-source-mismatch(경로의 이름·rev12 가 origin 과 다르다)"
    if not (repo / ".git").exists():
        return "tool-source-unverifiable(git 저장소가 아니다 — origin 바이트를 확인할 수 없다)"
    if core.git(repo, "cat-file", "-e", f"{rev}:{rpath}", check=False).returncode != 0:
        return "git-object-absent(origin 커밋에 그 경로가 없다)"
    return None


def _data_kind_of_path(rel_path: str, is_dir: bool) -> str:
    if is_dir:
        return "raw_dir"
    if rel_path.endswith(".log"):
        return "engine_log"
    if rel_path.endswith((".json", ".jsonl")):
        return "raw_json"
    return "raw_file"


def _expand_dir_candidates(repo: Path, cand: dict, add_cand, cell) -> dict[str, dict]:
    """디렉터리 후보 → 그 안의 파일 후보(2026-09-22 · plan_26092119 S2 round 2 · F7a). 반환 {디렉터리: {listed, total, truncated}}.

    왜: 1차 재생에서 계보 안의 유일한 스택 트레이스(`serve_fail_q38fn-nvfp4-mmap/master_failure.log`)를 발췌하지 못했다 — 후보는
    디렉터리였고 `hint.py excerpt` 는 디렉터리를 읽지 못하며(HINT_EXCERPT_SOURCE_UNREADABLE) 그 안의 파일은 후보가 아니었다
    (OUTSIDE_LINEAGE). 파일을 후보로 편다 — **디렉터리 바로 아래 파일 + 경로에 이 셀 키를 든 파일**만, 상한 EXPAND_MAX_FILES
    까지(결정론 순서: 셀 키 파일 먼저 → 얕은 깊이 → 사전순). 태그2 계보의 simlog 재판정 디렉터리는 407파일이고 대부분 다른 셀의
    것이라 전부 펴면 배포 LINEAGE.json 부기 바이트만 자란다(1차 시도 실측 +20KB). 펴지 않은 파일은 사라지지 않는다 — 디렉터리 후보가
    남아 있어 `resolve_stem` 이 그 안의 **전체 경로**를 받는다(`expanded.unlisted` 에 수를 적는다 · 조용한 절단 ✗). 숨김 경로·심링크는
    펴지 않는다."""
    out: dict[str, dict] = {}
    key = _norm_key(cell) if cell else None
    for p in sorted(cand):
        d = repo / p
        if not d.is_dir() or d.is_symlink():
            continue
        files: list[tuple[str, int, bool]] = []
        for f in d.rglob("*"):
            if f.is_symlink() or not f.is_file():
                continue
            parts = f.relative_to(d).parts
            if any(x.startswith(".") for x in parts):
                continue
            r = f.relative_to(repo).as_posix()
            files.append((r, len(parts), bool(key) and key in _norm_key(f.relative_to(d).as_posix())))
        files.sort(key=lambda it: (not it[2], it[1], it[0]))
        eligible = [r for r, depth, mine in files if (mine or depth == 1) and not _excluded(r)]
        take = eligible[:EXPAND_MAX_FILES]
        for r in take:
            add_cand(r, _data_kind_of_path(r, False), f"expanded:{p}")
        out[p] = {"listed": len(take), "total": len(files), "unlisted": len(files) - len(take),
                  "truncated": len(eligible) > EXPAND_MAX_FILES}
    return out


# ── 파생 ─────────────────────────────────────────────────────────────────────────────────────
def derive(repo: Path, ev, *, base_slug: str, publish_kst: str, depth: int = DEFAULT_DEPTH, declared=(),
           records=None, this_topic: str | None = None, identity: dict | None = None, cell_key: str | None = None,
           axis_tokens=(), base_slug_source: str = "PAYLOAD.identity.base_slug") -> dict:
    """LINEAGE.json 객체(schema_version 1 · SPEC §5.5). 순수 함수 — 파일을 읽을 뿐 쓰지 않는다.

    ev 에서 읽는 것(CellEvidence · dict · 이름공간 무관): `publication.topic`(이 발행 기록) · `cell`(셀 키) ·
    `pointers`(캠페인 evidence_pointers · 이 셀) · `lockset`·`cell_config`·`declaration`·`triplet`(본문 인용) ·
    `certificate_path`·`bench_report_path` · `sweep.dir`(엔진 로그 후보) · `lineage_seeds`(선택 키:
    `publication_records`·`this_topic`·`identity`·`cell_key`·`axis_tokens`·`evidence_candidates` + 그 밖의 키는 본문 인용).
    인자로 준 records/this_topic/identity/cell_key 가 ev 보다 우선한다.

    순회 = 시드(depth 0) → mention 순방향(조상) + "채택된 testlog 를 서술한 devlog"(hybrid) · 깊이 ≤ depth · 상한
    publish_kst(포함) · base 슬러그 본문 필터(시드 중 발행 포인터·선언은 필터 면제). 필터 탈락 문서는 **채택 문서에서 한
    홉일 때만** 경유 노드(`transit[]`)로 확장하고 읽기 목록에는 넣지 않는다(경유 → 경유 ✗).

    LINEAGE.json 은 SPEC §5.5 스키마의 상위집합이다 — `documents[]` 에 `class·bytes·sha256|commit` 을, 최상위에 `transit[]`
    를 더한다(`documents[].via[].from` 이 경유 노드를 가리킬 수 있어서다).
    """
    repo = Path(repo)
    if not isinstance(publish_kst, str) or not _KST_TOKEN_RE.match(publish_kst):
        core.fail("HINT_TIME_NOT_INJECTED", f"publish_kst 는 KST `YYMMDDHH` 로 주입해야 한다: {publish_kst!r}",
                  "core.kst_token(--generated-utc) 를 넘긴다(벽시계 금지).")
    if not isinstance(depth, int) or isinstance(depth, bool) or depth < 0:
        core.fail("HINT_LINEAGE_DEPTH_INVALID", f"depth 는 0 이상 정수: {depth!r}")
    pat = slug_pattern(base_slug)
    corpus = _Corpus(repo)
    decl = _parse_declared(repo, declared)
    for rel, _ in decl:
        node = corpus.ensure(rel)
        if node is not None and not _within_ceiling(node.yymmddhh, publish_kst):
            core.fail("HINT_LINEAGE_DECLARED_AFTER_CEILING",
                      f"보충 계보가 발행 시각 상한({publish_kst}) 뒤의 문서다: {rel}",
                      "발행 시점에 없던 문서는 계보가 아니다(D-W8) — 발행 시각을 확인한다.")

    ls = _ev(ev, "lineage_seeds") or {}
    if not isinstance(ls, dict):
        ls = {}
    pub = _ev(ev, "publication") or {}
    topic = this_topic or (pub.get("topic") if isinstance(pub, dict) else None) or ls.get("this_topic")
    recs = records if records is not None else ls.get("publication_records")
    if recs is None:
        recs = load_publication_records(repo)
    ident = identity if identity is not None else ls.get("identity")
    cell = _ev(ev, "cell")
    ck = cell_key or ls.get("cell_key") or cell
    # 모호 해소 형제 필터의 셀 키 = 셀 id(+ 구성 토큰) + 셀 선언 축 값(declared_axes). mentions() 첫 호출 전에 건다(캐시).
    cfg = _ev(ev, "cell_config") or {}
    decl_axes = cfg.get("declared_axes") if isinstance(cfg, dict) and isinstance(cfg.get("declared_axes"), dict) else {}
    sib_keys = [ck] + [v for v in decl_axes.values() if isinstance(v, (str, int)) and not isinstance(v, bool)
                       and "<<FILL>>" not in str(v)]
    corpus.sibling_filter = _SiblingFilter(corpus, sib_keys) if ck else None
    sp = seeds_from_publications(repo, recs, this_topic=topic, identity=ident, cell_key=ck,
                                 publish_kst=publish_kst)

    # 시드 S3 — 같은 모델의 앞선 셀(셀 키가 달라 S2 가 못 잡는 bring-up 계보 · R-a). identity 는 S2 와 같은 규칙으로 고른다.
    this_rec = next((r for r in (recs or ()) if isinstance(r, dict) and (r.get("_id") or r.get("publication_id")) == topic
                     and "_unreadable" not in r), None)
    mp_ident = ident if isinstance(ident, dict) and ident else ((this_rec or {}).get("identity") or {})
    mp = seeds_from_model_priors(repo, recs, this_topic=topic, identity=mp_ident, pat=pat, publish_kst=publish_kst,
                                 exclude=set(sp["prior"]))

    # 시드 후보 모으기: (경로, 출처, 포인터 이름, 필터 면제?)
    raw_seeds: list[tuple[str, str, str, bool]] = [(s["path"], s["source"], s["pointer"], True) for s in sp["seeds"]]
    raw_seeds += [(s["path"], s["source"], s["pointer"], True) for s in mp["seeds"]]
    seeds_missing: list[dict] = list(sp["missing"]) + list(mp["missing"])
    model_prior_skipped: set[str] = set()
    for k in ("bench_report_path", "certificate_path"):
        v = _ev(ev, k)
        if isinstance(v, str) and v:
            path, why = _norm_pointer(repo, v, "")
            if path:
                raw_seeds.append((path, f"evidence:{k}", k, True))
            else:
                seeds_missing.append({"path": v, "source": f"evidence:{k}", "reason": why})
    for i, ptr in enumerate(_ev(ev, "pointers") or ()):
        if not isinstance(ptr, dict) or not isinstance(ptr.get("path"), str):
            continue
        if ptr.get("cell_id") not in (None, "") and cell and ptr.get("cell_id") != cell:
            continue
        path, why = _norm_pointer(repo, ptr["path"], "")
        src = f"campaign_evidence_pointer:{ptr.get('kind') or 'unknown'}"
        if path:
            raw_seeds.append((path, src, f"pointers[{ptr.get('kind')}]", True))
        else:
            seeds_missing.append({"path": ptr["path"], "source": src, "reason": why})
    for rel, reason in decl:
        raw_seeds.append((rel, "declared", reason, True))
    # 셀 산출물이 본문으로 인용한 문서(lockset·config·선언·트리플렛·lineage_seeds 기타) — 필터 대상
    cite_sources: list[tuple[str, str]] = []
    for k in ("lockset", "cell_config", "declaration"):
        v = _ev(ev, k)
        if v:
            cite_sources.append((f"{k}_citation", json.dumps(v, ensure_ascii=False, sort_keys=True)))
    trip = _ev(ev, "triplet") or {}
    if isinstance(trip, dict):
        for k in sorted(trip):
            if not isinstance(trip[k], str):
                continue
            tpath, why = _norm_pointer(repo, trip[k], "")
            if tpath is None:          # 저장소 밖 트리플렛은 읽지 않는다(계보 입력은 저장소 안 파일뿐)
                seeds_missing.append({"path": trip[k], "source": f"triplet_citation:{k}", "reason": why})
                continue
            if (repo / tpath).is_file():
                cite_sources.append((f"triplet_citation:{k}",
                                     (repo / tpath).read_text(encoding="utf-8", errors="replace")))
    # attestation = evidence 가 묶은 데이터 파일 경로(evidence_candidates 에도 실린다) — 인용 원천이 아니다.
    reserved = {"publication_records", "this_topic", "identity", "cell_key", "axis_tokens", "evidence_candidates",
                "attestation"}
    for k in sorted(ls):
        if k not in reserved and ls[k]:
            cite_sources.append((f"lineage_seeds:{k}", json.dumps(ls[k], ensure_ascii=False, sort_keys=True)))
            # 값이 저장소 안 **비문서 파일**을 가리키면 그 본문도 인용 원천이다 — 캠페인 `journey.jsonl`(여정: 이탈·반증 사유)이
            # 판정 문서를 인용한다. 문서 자체는 위 경로 해소가 이미 잡으므로 여기서는 데이터 파일만 읽는다(1 MiB 상한).
            for v in (ls[k] if isinstance(ls[k], list) else [ls[k]]):
                if not isinstance(v, str) or not v.strip():
                    continue
                vp, _ = _norm_pointer(repo, v, "")
                if vp is None or _excluded(vp) or not (repo / vp).is_file():
                    continue
                if corpus.nodes.get(vp) is not None or (repo / vp).stat().st_size > (1 << 20):
                    continue
                cite_sources.append((f"lineage_seeds:{k}:file",
                                     (repo / vp).read_text(encoding="utf-8", errors="replace")))
    for src, text in cite_sources:
        for _, targets, res, token in corpus.resolve_text(text):
            if res == "ambiguous" and corpus.sibling_filter is not None:
                targets = sorted(corpus.sibling_filter.keep(src, token, targets))
            for t in targets:
                raw_seeds.append((t, src, token, False))

    # 시드 분류 → 문서 시드(depth 0) · 데이터 후보 · 결손
    adopted: dict[str, int] = {}
    seed_via: dict[str, set] = {}
    seed_rows: set[tuple[str, str]] = set()
    cand: dict[str, dict] = {}
    rejected = {"filter": set(), "ceiling": set()}

    def add_cand(path: str, kind: str, source: str, origin: str | None = None) -> None:
        c = cand.setdefault(path, {"kind": kind, "sources": set()})
        c["sources"].add(source)
        if origin is not None:
            if c.get("origin") not in (None, origin):
                core.fail("HINT_LINEAGE_TOOL_ORIGIN_CONFLICT", f"같은 후보 경로에 origin 이 둘이다: {path}",
                          "evidence.tool_snapshots 의 이름·커밋이 겹친다 — 같은 basename 의 두 도구를 한 draft 에 싣지 않는다.")
            c["origin"] = origin

    for path, source, pointer, exempt in sorted(set(raw_seeds)):
        if _excluded(path):
            seeds_missing.append({"path": path, "source": source, "reason": "excluded-root"})
            continue
        if not (repo / path).exists():
            seeds_missing.append({"path": path, "source": source, "reason": "file-absent"})
            continue
        node = corpus.ensure(path)
        if source.startswith(MODEL_PRIOR_SOURCE) and (node is None or node.kind not in MODEL_PRIOR_KINDS):
            # 다른 셀의 측정 표면(bench_report·인증서·simlog·원시 jsonl)은 이 셀의 증거가 아니다 — 서사 문서만 시드로 싣는다.
            #   그 셀의 측정 문서가 계보에 필요하면 서사 문서의 본문 인용(BFS)이 데려온다(DS4F sweep_map_26090912_* 가 그 길).
            model_prior_skipped.add(path)
            continue
        if node is not None and not _within_ceiling(node.yymmddhh, publish_kst):
            seeds_missing.append({"path": path, "source": source, "reason": "after-publish-ceiling"})
            continue
        if node is not None and node.is_document:
            if not exempt and not corpus.slug_hit(path, pat):
                rejected["filter"].add(path)
                continue
            adopted[path] = 0
            seed_rows.add((path, source))
            seed_via.setdefault(path, set()).add((source, "seed", pointer, "exact"))
        else:
            kind = node.kind if node is not None else _data_kind_of_path(path, (repo / path).is_dir())
            if exempt or corpus.slug_hit(path, pat):
                add_cand(path, kind, source)
            else:
                rejected["filter"].add(path)

    # BFS — 조상(mention 순방향) + testlog 서술 devlog + **필터 경유 1홉**
    #
    # 경유(transit) 규칙(2026-09-22 라이브 대조 · 태그2 발행 기록): 슬러그 필터는 **채택**을 가르는 것이지 **통행**을
    # 가르는 것이 아니다. 부록 B 표적 `perf_26091314`(타 HW 선례 · 중요도 상)는 `perf_26091122 → plan_26091407 →
    # audit_26091323 → perf_26091314` 로만 닿는데, 가운데 `plan_26091407`(하네스 교정 plan)은 모델을 캠페인 약어
    # (`qwen38fn`)로만 적어 base 슬러그가 0회다 — 필터 탈락 노드를 확장하지 않으면(prune) 표적이 조용히 사라진다(11/12).
    # 약어를 필터에 더하는 것은 답이 아니다: 약어는 파생 불가다(lineage_graph.md §8.4 · 재현율 8/12). 그래서 필터 탈락
    # 문서도 **채택된 문서에서 한 홉**이면 경유 노드로 확장하되 읽기 목록(documents)에는 넣지 않고 `transit[]` 에 적는다.
    # 경유 노드에서 다시 경유 노드로는 가지 않는다(연속 2홉 필터 탈락 = 모델과 무관한 영역으로 번지는 신호).
    narr = corpus.narrators()
    transit: dict[str, int] = {}
    frontier = sorted(adopted)                      # 채택 노드(확장 가능)
    frontier_transit: list[str] = []                # 경유 노드(확장하되 그 이웃은 필터를 통과해야 채택)
    for d in range(1, depth + 1):
        reach: dict[str, bool] = {}                 # v → 채택 부모에서 닿았나(참이면 경유 자격)
        for u in frontier:
            nbrs = [e["to"] for e in corpus.mentions(u)]
            if corpus.nodes[u].kind == "testlog":
                nbrs += [dv for dv, _, _ in narr.get(u, ())]
            for v in nbrs:
                reach[v] = True
        for u in frontier_transit:
            for e in corpus.mentions(u):             # 서술자 보강은 채택된 testlog 에만(SPEC X17)
                reach.setdefault(e["to"], False)
        nxt: list[str] = []
        nxt_transit: list[str] = []
        for v in sorted(reach):
            if v in adopted or v in transit or v in rejected["ceiling"]:
                continue
            node = corpus.nodes[v]
            if not node.is_document:
                continue            # 데이터 노드는 아래에서 채택 문서 전체 기준으로 모은다
            if not _within_ceiling(node.yymmddhh, publish_kst):
                rejected["ceiling"].add(v)
                continue
            if corpus.slug_hit(v, pat):
                rejected["filter"].discard(v)
                adopted[v] = d
                nxt.append(v)
            elif reach[v] and d < depth:
                rejected["filter"].discard(v)
                transit[v] = d
                nxt_transit.append(v)
            else:
                rejected["filter"].add(v)
        frontier = sorted(nxt)
        frontier_transit = sorted(nxt_transit)

    # 데이터 후보: 채택 문서가 언급한 simlog·인증서·sweep json + 원시 증거 경로 + 스윕 디렉터리 + 선언
    for u in sorted(adopted):
        for e in corpus.mentions(u):
            node = corpus.nodes[e["to"]]
            if node.is_document or e["to"] in rejected["filter"] or e["to"] in rejected["ceiling"]:
                continue
            if not _within_ceiling(node.yymmddhh, publish_kst):
                rejected["ceiling"].add(e["to"])
                continue
            if e["to"] in cand or corpus.slug_hit(e["to"], pat):
                add_cand(e["to"], node.kind, f"mention:{u}")
            else:
                rejected["filter"].add(e["to"])
        for op in corpus.output_mentions(u):
            add_cand(op, _data_kind_of_path(op, (repo / op).is_dir()), f"mention:{u}")
    sweep = _ev(ev, "sweep") or {}
    sdir = sweep.get("dir") if isinstance(sweep, dict) else None
    if isinstance(sdir, str) and sdir:
        srel = core.rel(repo, sdir)
        sp_dir = repo / srel
        if sp_dir.is_dir():
            for f in sorted(sp_dir.rglob("*")):
                # 숨김 경로(`.tokstage/tokenizer.json` 등 토크나이저 스테이징 사본)는 측정 원시가 아니다.
                if not f.is_file() or any(p.startswith(".") for p in f.relative_to(sp_dir).parts):
                    continue
                r = f.relative_to(repo).as_posix()
                if f.suffix == ".log":
                    add_cand(r, "engine_log", "sweep_dir")
                elif f.suffix == ".json":
                    # 레벨 원시(level_NN/{bench_<셀>,measured,post_health_<셀>}.json · 반복 축 run_KK/) 도 싣는다(2026-09-22 · S2 round 2
                    #   F7b): 인증서 FACT 는 그 파일들을 자격 출처로 인용하는데 종전 후보는 스윕 최상위 JSON 뿐이라 저작자가 발췌할 수
                    #   없었다(1차 저작자 보고).
                    add_cand(r, "sweep_json" if f.name == "sweep_index.json" and f.parent == sp_dir else "raw_json",
                             "sweep_dir")
        else:
            seeds_missing.append({"path": srel, "source": "evidence:sweep.dir", "reason": "file-absent"})
    for c in ls.get("evidence_candidates") or ():
        if isinstance(c, dict) and c.get("kind") == TOOL_SOURCE_KIND:
            # 측정 도구 스냅숏(draft 상대 · 2026-09-22 S2 round 3) — 저장소 실재가 아니라 origin 의 git 바이트로 검증한다.
            prob = _tool_candidate_problem(repo, c)
            if prob is None:
                add_cand(c["path"], TOOL_SOURCE_KIND, "lineage_seeds:evidence_candidates", origin=c["origin"])
            else:
                seeds_missing.append({"path": str(c.get("path")), "source": "lineage_seeds:evidence_candidates",
                                      "reason": prob})
            continue
        if isinstance(c, dict) and isinstance(c.get("path"), str):
            path, why = _norm_pointer(repo, c["path"], "")
            if path and not _excluded(path) and (repo / path).exists():
                add_cand(path, str(c.get("kind") or _data_kind_of_path(path, (repo / path).is_dir())),
                         "lineage_seeds:evidence_candidates")
            else:
                seeds_missing.append({"path": c["path"], "source": "lineage_seeds:evidence_candidates",
                                      "reason": why or ("excluded-root" if path and _excluded(path) else "file-absent")})
    expanded = _expand_dir_candidates(repo, cand, add_cand, cell)

    # via · 해소 · 뒤집힘 — 경유 노드도 간선의 출발점으로 싣는다(via.from 이 transit[] 를 가리킬 수 있다)
    carriers = set(adopted) | set(transit)
    via: dict[str, set] = {p: set(seed_via.get(p, ())) for p in carriers}
    for u in sorted(carriers):
        for e in corpus.mentions(u):
            if e["to"] in carriers:
                via[e["to"]].add((u, e["edge"], e["token"], e["resolution"]))
        if u in adopted and corpus.nodes[u].kind == "testlog":
            for dv, tok, res in narr.get(u, ()):
                if dv in carriers:
                    via[dv].add((u, "narrates", tok, res))
    # 뒤집힘은 **양쪽 배너**에서 읽는다. 관행이 둘이다: 뒤집힌 문서가 "SUPERSEDED-IN-PART — <뒤집은 문서>" 를 달거나
    # (testlog_26091009:3), 뒤집은 문서가 "뒤집은 선행: <뒤집힌 문서> (SUPERSEDED-IN-PART)" 를 단다(devlog_26091109:5).
    # 한쪽만 보면 계보 2.4 "오진과 정정" 의 기계 입력이 절반만 남는다. 방향은 날짜로 가른다 — 뒤에 태어난 쪽이 뒤집은 쪽.
    overturned_by: dict[str, set] = {}
    for q in sorted(corpus.nodes):
        qn = corpus.nodes[q]
        if not qn.is_document or not _within_ceiling(qn.yymmddhh, publish_kst):
            continue
        for e in corpus.mentions(q):
            if e["edge"] != "supersede" or e["resolution"] == "ambiguous" or e["to"] not in adopted:
                continue
            t = corpus.nodes[e["to"]]
            if qn.yymmddhh and t.yymmddhh and qn.yymmddhh > t.yymmddhh:
                overturned_by.setdefault(e["to"], set()).add(q)
    axis = list(axis_tokens or ()) + list(ls.get("axis_tokens") or ())
    if cell:
        # relevance 는 원 셀 id 와 구성 토큰을 함께 싣는다. 원 셀 id 가 없으면 `c1-a`가 `c1`·`a`로만
        # 갈라져 수신자가 "이 문서가 정확히 어느 셀과 맞닿았나"를 잃는다(자체검사 음성대조).
        axis += [cell] + [t for t in re.split(r"[-_.]", str(cell)) if t]
    # 셀 id 안의 `-`/`_`는 이름 구분자이며 본문에서는 서로 교환되어 나타난다 — 토큰 경계는 엄격히 유지하되 두 구분자를
    # 동치로 읽는다(`_axis_patterns` 한 벌 · 옛 판은 같은 규칙을 여기에 한 번 더 적어 두 벌이었다).
    axis_pats = _axis_patterns(axis)
    tracked = _tracked_blobs(repo, sorted(adopted))

    documents = []
    for p in adopted:
        node = corpus.nodes[p]
        data = (repo / p).read_bytes()
        commit = None
        if p in tracked and tracked[p] == _git_blob_sha(data):
            commit = _last_commit(repo, p)
        sup = set(overturned_by.get(p, ()))
        for e in corpus.mentions(p):
            if e["edge"] != "supersede" or e["resolution"] == "ambiguous":
                continue
            t = corpus.nodes[e["to"]]
            if not t.is_document or not _within_ceiling(t.yymmddhh, publish_kst):
                continue
            if node.yymmddhh and t.yymmddhh and t.yymmddhh < node.yymmddhh:
                continue           # 배너 줄에 적힌 **이전** 문서 = 이 문서가 뒤집은 쪽(관행) — 뒤집은 문서는 뒤에 태어났다
            sup.add(e["to"])
        body = corpus.text(p)
        vres = [v[3] for v in via[p]]
        documents.append({
            "path": p, "kind": node.kind, "class": node.klass, "date_key": node.yymmddhh, "depth": adopted[p],
            "bytes": len(data), "sha256": None if commit else hashlib.sha256(data).hexdigest(), "commit": commit,
            "via": [{"from": f, "edge": e, "token": t, "resolution": r} for f, e, t, r in sorted(via[p])],
            "relevance": {"axis_tokens": [t for t, rx in axis_pats if rx.search(body) or rx.search(p)]},
            "superseded_by": sorted(sup),
            "resolution": min(vres, key=lambda r: _RES_RANK[r]) if vres else "exact",
        })
    documents.sort(key=lambda x: (x["date_key"] or "99999999", corpus.nodes[x["path"]].mmss or "",
                                  _KIND_RANK.get(x["kind"], 99), PurePosixPath(x["path"]).name, x["path"]))

    transit_out = []
    for p in sorted(transit, key=lambda x: (transit[x], x)):
        tn = corpus.nodes[p]
        transit_out.append({"path": p, "kind": tn.kind, "class": tn.klass, "date_key": tn.yymmddhh,
                            "depth": transit[p], "reason": "filter-miss(base 슬러그 부재) — 경유만 · 읽기 목록 밖",
                            "via": [{"from": f, "edge": e, "token": t, "resolution": r}
                                    for f, e, t, r in sorted(via.get(p, ()))]})

    seeds_out = sorted({(p, s) for p, s in seed_rows})
    # 저장소 밖 포인터의 원문(운영자 절대경로 `/home/…`·`/mnt/…`)은 싣지 않는다 — LINEAGE.json 은 배포 페이로드이고 그 원문은
    # 배포 PII 스캔(abs-op-path)에 걸려 발행 전체를 막는다. 결손 사실과 출처(포인터 이름)만 남긴다(2026-09-22 감사).
    dedup_missing = sorted({("<outside-repo>" if m["reason"] == "outside-repo" else str(m.get("path")), m["source"],
                             m["reason"]) for m in seeds_missing})
    # 모호 해소에서 걸러진 형제 — 다른 길(정확 인용·시드·후보)로 이미 실린 것은 빼고 적는다(부재가 아니라 가려낸 것이라는 기록).
    sf = corpus.sibling_filter
    amb_out = []
    for p in sorted((sf.excluded if sf is not None else {})):
        if p in adopted or p in transit or p in cand or p in sf.kept:
            continue
        rec = sf.excluded[p]
        node = corpus.nodes.get(p)
        amb_out.append({"path": p, "kind": node.kind if node else None, "token": rec["token"],
                        "from": sorted(rec["from"]), "reason": rec["reason"]})

    def cand_row(p: str) -> dict:
        row = {"path": p, "kind": cand[p]["kind"], "sources": sorted(cand[p]["sources"])}
        if cand[p].get("origin"):
            row["origin"] = cand[p]["origin"]
        if p in expanded:
            row["expanded"] = expanded[p]
        return row

    return {
        "schema_version": SCHEMA_VERSION,
        "method": {
            "graph": "mention(files@publish)",
            "traversal": "ancestors+testlog-narrators+filter-transit(1hop)",
            "depth": depth,
            "filter": {"base_slug": base_slug, "source": base_slug_source, "pattern": pat.pattern},
            "publish_kst": publish_kst,
            "roots": list(DEFAULT_ROOTS),
            "excluded_roots": list(EXCLUDED_ROOTS),
            "resolution_order": list(RESOLUTIONS),
            "undated": "pass-ceiling",
            "grammar": {"prose": core.REL_DOC_NAMING_WIKI, "bench": core.REL_DOC_NAMING_BENCH,
                        "sweep_map": "docs.md §명명 SSOT(국소 상수 SWEEP_MAP_PREFIX)"},
            "seed_records": {"this": sp["this"], "prior": sp["prior"], "prior_after_ceiling": sp["prior_after_ceiling"],
                             "prior_match": sp["prior_match"], "prior_cell_mismatch": sp["prior_cell_mismatch"],
                             "cell_key": sp["cell_key"], "identity_fields": list(IDENTITY_MATCH_FIELDS)},
            "model_prior_records": {"rule": ("같은 모델의 앞선 셀 발행 기록(S3) — identity.model 이 base 슬러그에 걸림 또는 identity.model 이 "
                                             "빈 기록의 id 첫 토큰이 관측 별칭 · gpu 같음(양쪽에 있을 때) · 발행 시각 상한 이내 · S2 기록 제외 · "
                                             "서사 문서 포인터만(plan·devlog·testlog·report · 필터 면제) · 다른 셀의 측정 표면·데이터 포인터는 싣지 않는다"),
                                    "records": mp["records"], "match": mp["match"], "after_ceiling": mp["after_ceiling"],
                                    "gpu_mismatch": mp["gpu_mismatch"], "aliases": mp["aliases"]["aliases"],
                                    "alias_evidence": mp["aliases"]["evidence"],
                                    "skipped_non_narrative": sorted(model_prior_skipped)},
            "supersede": ("헤더 12줄 배너(SUPERSEDED·SUPERSEDED-IN-PART 어느 꼴이든) 양쪽 관행 · 방향 = 날짜 휴리스틱(뒤에 태어난 "
                          "쪽이 뒤집은 쪽 · 같은 시각은 판정 안 함) · 모호 해소 간선은 쓰지 않는다"),
            "stats": {"rejected_filter": len(rejected["filter"]), "rejected_ceiling": len(rejected["ceiling"]),
                      "transit": len(transit), "roots_absent": sorted(corpus.roots_absent),
                      "ambiguous_excluded": len(amb_out)},
            "ambiguous_siblings": {"rule": ("모호 간선의 형제 무리마다 셀 키 토큰 적중(이름·헤더 12줄) 최대 → 이름의 이질 토큰 최소만 유지 · "
                                            "무신호(적중 0)면 전부 유지 · 정확 인용은 무관 · 다른 자리에서 남겨진 형제는 제외 목록에 적지 않는다 · "
                                            "여러 무리에서 걸러지면 사전순 최소 (토큰, 사유)"),
                                   "key_tokens": list(sf.keys) if sf is not None else []},
            "tool_sources": {"kind": TOOL_SOURCE_KIND, "path": f"draft 상대 {TOOL_SNAPSHOT_DIR}/<이름>@<rev12>",
                             "origin": "git:<rev>:<저장소 경로>(git show 바이트 · 측정 시각 직전 판본)",
                             "graph": "mention 그래프 입력 아님(발췌 출처 전용)"},
            "dir_expansion": {"max_files_per_dir": EXPAND_MAX_FILES,
                              "scope": "디렉터리 바로 아래 파일 + 디렉터리 안 경로에 셀 키를 든 파일(나머지는 디렉터리 후보 경유 전체 경로로만)",
                              "order": "셀 키 파일 먼저 → 얕은 깊이 → 경로 사전순 · 숨김 경로·심링크 제외"},
        },
        "seeds": [{"path": p, "source": s} for p, s in seeds_out],
        "documents": documents,
        "transit": transit_out,
        "evidence_candidates": [cand_row(p) for p in sorted(cand)],
        "ambiguous_excluded": amb_out,
        "declared": [{"path": p, "source": "declared", "reason": r} for p, r in decl],
        "seeds_missing": [{"path": p, "source": s, "reason": r} for p, s, r in dedup_missing],
    }


def _tracked_blobs(repo: Path, paths: list[str]) -> dict[str, str]:
    """HEAD 가 드는 blob SHA(경로 → sha). git 이 아니거나 커밋이 없으면 빈 dict — 그 노드의 git 은 이 바이트를 들지
    않는다(GIT_SINGLE_AUTHORITY Q1 = 아니오 → digest 가 유일한 증거)."""
    if not paths or not (repo / ".git").exists():
        return {}
    specs = sorted({posixpath.dirname(p) or "." for p in paths})
    out = core.git(repo, "-c", "core.quotePath=false", "ls-tree", "-r", "-z", "HEAD", "--", *specs, check=False)
    if out.returncode != 0:
        return {}
    blobs: dict[str, str] = {}
    for rec in out.stdout.split("\0"):
        if "\t" not in rec:
            continue
        meta, path = rec.split("\t", 1)
        parts = meta.split()
        if len(parts) == 3 and parts[1] == "blob":
            blobs[path] = parts[2]
    return blobs


def _last_commit(repo: Path, path: str) -> str | None:
    # log.showSignature 가 켜진 운영자 git config 에서는 서명 검증 출력이 %H 앞에 섞인다 — 끈다.
    out = core.git(repo, "-c", "core.quotePath=false", "-c", "log.showSignature=false", "log", "-1", "--format=%H",
                   "--", path, check=False)
    sha = out.stdout.strip() if out.returncode == 0 else ""
    return sha or None


# ── 소비자 도우미 ─────────────────────────────────────────────────────────────────────────────
def reading_list(lineage: dict) -> list[dict]:
    """PROMPT "읽을 것" 렌더용 목록 — 모호 해소로만 닿은 문서는 뒤 계층(`tier: ambiguous`)으로 민다.
    정렬 = 계층 → 깊이 → 관련도(셀 축 토큰 수) 내림 → 날짜 → 경로. 예산 안에서 자르는 것은 소비자 몫(d≤2 전문 ·
    d3-4 헤더 권고 · `lineage_graph.md` §8.5)."""
    items = []
    for d in lineage.get("documents") or ():
        toks = list((d.get("relevance") or {}).get("axis_tokens") or ())
        items.append({"path": d["path"], "kind": d.get("kind"), "class": d.get("class"),
                      "date_key": d.get("date_key"), "depth": d.get("depth", 0), "bytes": d.get("bytes", 0),
                      "relevance": len(toks), "axis_tokens": toks, "resolution": d.get("resolution"),
                      "superseded_by": list(d.get("superseded_by") or ()),
                      "tier": "ambiguous" if d.get("resolution") == "ambiguous" else "primary"})
    items.sort(key=lambda x: (x["tier"] != "primary", x["depth"], -x["relevance"], x["date_key"] or "99999999",
                              x["path"]))
    return items


def resolve_stem(lineage: dict, stem: str) -> str | None:
    """발췌 출처 표기(`testlog_26090921` · 전체 stem · 저장소 상대 경로) → LINEAGE 안의 경로. **유일할 때만** 경로,
    모호·부재면 None(발췌는 한 문서의 부분 문자열이어야 검사가 성립한다). 데이터 후보 디렉터리(simlog) 안의 파일 경로도
    그 후보에 속하면 받는다(엔진 로그·스윕 원시 발췌)."""
    if not isinstance(stem, str) or not stem.strip():
        return None
    s = stem.strip().strip("`")
    docs = [d["path"] for d in lineage.get("documents") or ()]
    cands = lineage.get("evidence_candidates") or ()
    paths = docs + [c["path"] for c in cands]
    if "/" in s:
        sn = posixpath.normpath(s)
        if sn in paths:
            return sn
        for c in cands:
            if (c.get("kind") in _DIR_CANDIDATE_KINDS or c.get("expanded")) and sn.startswith(c["path"] + "/"):
                return sn
        s = PurePosixPath(sn).name
    base = _strip_ext(s)
    exact = sorted({p for p in paths if _strip_ext(PurePosixPath(p).name) == base})
    if len(exact) == 1:
        return exact[0]
    if len(exact) > 1:
        named = [p for p in exact if PurePosixPath(p).name == s]
        return named[0] if len(named) == 1 else None
    if _BARE_STEM_RE.match(base):
        sib = sorted({p for p in paths if _strip_ext(PurePosixPath(p).name).startswith(base + "_")})
        if len(sib) == 1:
            return sib[0]
    return None


def require_documents(lineage: dict) -> None:
    """문서 0건 = 서사 원재료 0 → 차단(린터가 부른다 · 계약 "필수는 여정 정보 하나")."""
    if not (lineage or {}).get("documents"):
        core.fail("HINT_LINEAGE_EMPTY", "LINEAGE 에 계보 문서가 0건이다 — 서사를 쓸 원재료가 없다.",
                  "발행 기록의 plan·devlog·testlog 바인딩을 확인하거나 `--lineage-add PATH=REASON` 으로 보충한다.")


# ── 수신자 요약 · 봉인 출처 스냅샷 (2026-09-29 · plan_26092908 §4.6·§4.9 · V13·V5) ─────────────────────────
# 왜 둘로 가르는가: v6 페이로드의 LINEAGE.json(= derive 전체)은 zip 최대 파일(82~160KB)인데 블라인드 수신자 3/3 이 무용이라 판정했다
#   (V13) — 해시 · via 간선 · 모호 해소 기록 · 후보 원문 경로는 **발행자**가 린트 · 발췌 대조에 쓰는 것이지 수신자가 읽을 것이 아니다.
#   그래서 전체는 draft `inputs/LINEAGE.full.json`(발행자 평면 · 배포 ✗)에 두고, 페이로드 LINEAGE.json 은 수신자 요약(문서 stem · 역할 ·
#   날짜 · 발췌 수 · ≤ SUMMARY_MAX_BYTES)과 봉인 출처 스냅샷만 싣는다. 린트 · excerpt 는 전체를 읽는다(hint.py).
# 봉인 출처 스냅샷(`sealed_sources: [{path, sha256}]` · tag.SEALED_SOURCES_KEY 와 같은 키): 발췌 · 서명 · 벤치 절이 대조한 출처 파일의
#   봉인 시점 sha256. 비추적 docs 는 git 이 바이트를 들지 않으므로 GIT_SINGLE_AUTHORITY 2문항상 **맹점층**(기록 정당) — verify 는
#   이 스냅샷과 현재 출처가 다르면 출처 의존 린트 코드를 FAIL 대신 INFO 로 강등한다(V5 · DS4F 봉인 13초 뒤 devlog 정정).
LINEAGE_FULL_NAME = "LINEAGE.full.json"      # draft `inputs/` 안(발행자 평면)
SUMMARY_SCHEMA_VERSION = 2
SUMMARY_KIND = "receiver-summary"
# 수신자 요약 상한(국소 상수 · plan §7 AC9 "zip 안 LINEAGE 요약 ≤ 20KB"). 넘으면 요약이 아니라 전사다 — 발행을 막는다.
SUMMARY_MAX_BYTES = 20 * 1024
SEALED_SOURCES_KEY = "sealed_sources"


def receiver_summary(full: dict, *, excerpt_counts: dict | None = None, sealed_sources: list | None = None) -> dict:
    """전체 LINEAGE(derive 산출) → 페이로드용 수신자 요약(순수 · 부작용 0).

    documents[] = {stem, path, role(= kind), date(= date_key · KST YYMMDDHH), depth, excerpts(이 페이로드가 그 문서에서 인용한 발췌 수)}
    evidence_candidates = {count, cited[{path, excerpts}]} — 후보 원문 경로 전체는 싣지 않는다(인용된 것만).
    sealed_sources 가 주어지면(봉인 시점 · continue) 그대로 싣는다 — 없으면 키 자체가 없다(publish 스캐폴드 · 스냅샷 전).
    결과가 SUMMARY_MAX_BYTES 를 넘으면 HINT_LINEAGE_SUMMARY_TOO_LARGE(요약이 전사로 자라지 않게 · fail-closed)."""
    if not isinstance(full, dict):
        core.fail("HINT_LINEAGE_UNREADABLE", f"LINEAGE 전체가 사전이 아니다: {type(full).__name__}")
    counts = {str(k): int(v) for k, v in (excerpt_counts or {}).items() if isinstance(v, int) and v > 0}
    docs = []
    for d in full.get("documents") or ():
        if not isinstance(d, dict) or not d.get("path"):
            continue
        p = str(d["path"])
        docs.append({"stem": PurePosixPath(p).stem, "path": p, "role": d.get("kind"), "date": d.get("date_key"),
                     "depth": d.get("depth"), "excerpts": counts.get(p, 0)})
    doc_paths = {x["path"] for x in docs}
    cands = [c for c in (full.get("evidence_candidates") or ()) if isinstance(c, dict) and c.get("path")]
    cited = sorted((p, n) for p, n in counts.items() if p not in doc_paths)
    method = full.get("method") if isinstance(full.get("method"), dict) else {}
    out = {"schema_version": SUMMARY_SCHEMA_VERSION, "kind": SUMMARY_KIND,
           "note": ("수신자 요약 — 서사가 읽은 계보 문서의 stem · 역할 · 날짜 · 이 페이로드의 발췌 수. 해시 · 간선 · 후보 전체 · 모호 해소 "
                    "기록은 발행자 평면(draft inputs/LINEAGE.full.json)에 있다(배포 ✗ · plan_26092908 §4.6)."),
           "publish_kst": method.get("publish_kst"), "depth": method.get("depth"),
           "documents": docs,
           "evidence_candidates": {"count": len(cands), "cited": [{"path": p, "excerpts": n} for p, n in cited]}}
    if sealed_sources is not None:
        out[SEALED_SOURCES_KEY] = [{"path": str(x["path"]), "sha256": str(x["sha256"])} for x in sealed_sources]
    size = len(core.dumps(out).encode("utf-8"))
    if size > SUMMARY_MAX_BYTES:
        core.fail("HINT_LINEAGE_SUMMARY_TOO_LARGE",
                  f"LINEAGE 수신자 요약이 {size:,} B > 상한 {SUMMARY_MAX_BYTES:,} B 다 — 요약이 아니라 전사다.",
                  "계보 문서 수(깊이 · --lineage-add)를 확인한다 — 요약 칸을 늘리지 않는다(plan_26092908 §7 AC9).")
    return out


def sealed_sources(repo: Path, paths) -> list[dict]:
    """봉인 시점 출처 스냅샷 `[{path, sha256}]`(저장소 상대 · 정렬 · 중복 제거). 읽기만 한다. 경로 모양 결함(절대 · `..`) · 읽기 불가 =
    HINT_SEALED_SOURCE_UNREADABLE(스냅샷 없이 봉인하면 verify 가 시간에 따라 FAIL 로 바뀐다 — 조용히 빼지 않는다)."""
    repo = Path(repo)
    out: list[dict] = []
    for p in sorted({str(x) for x in (paths or ()) if x}):
        if p.startswith("/") or "\\" in p or any(seg in ("", ".", "..") for seg in p.split("/")):
            core.fail("HINT_SEALED_SOURCE_UNREADABLE", f"봉인 출처 경로가 저장소 상대 posix 가 아니다: {p!r}")
        f = repo / p
        try:
            data = f.read_bytes() if f.is_file() else None
        except OSError:
            data = None
        if data is None:
            core.fail("HINT_SEALED_SOURCE_UNREADABLE", f"봉인 출처를 읽을 수 없다: {p}",
                      "린트가 통과한 출처가 봉인 직전에 사라졌다 — 출처를 복원하고 continue 를 다시 실행한다.")
        out.append({"path": p, "sha256": hashlib.sha256(data).hexdigest()})
    return out


# ── 자체검사 (격리 임시 저장소 · 라이브 문서·태그·캠페인 비의존) ────────────────────────────────────────
def _selftest_tool_sources(ck, repo: Path, ev: dict, kw: dict) -> None:
    """2026-09-22(S2 round 3) — `tool-source@rev` 후보: draft 상대 경로 · origin git 바이트 검증 · 발췌 해소. ★ = 음성대조."""
    sha = core.git(repo, "rev-parse", "HEAD").stdout.strip()
    good = {"path": tool_snapshot_rel("run_bench.sh", sha), "kind": TOOL_SOURCE_KIND, "origin": f"git:{sha}:tools/run_bench.sh"}
    other = "f" * 40
    bads = (
        ("★rev12 불일치", dict(good, path=f"{TOOL_SNAPSHOT_DIR}/run_bench.sh@{other[:12]}"), "tool-source-mismatch"),
        ("★이름 불일치", dict(good, path=f"{TOOL_SNAPSHOT_DIR}/lite_bench.sh@{sha[:12]}"), "tool-source-mismatch"),
        ("★origin 경로 탈출", dict(good, origin=f"git:{sha}:tools/../run_bench.sh"), "tool-source-origin-path"),
        ("★origin 부재", {k: v for k, v in good.items() if k != "origin"}, "tool-source-shape"),
        ("★저장소 경로 모양(draft 밖)", dict(good, path="tools/run_bench.sh"), "tool-source-shape"),
        ("★git 이 들지 않는 경로", dict(good, origin=f"git:{sha}:tools/none.sh", path=f"{TOOL_SNAPSHOT_DIR}/none.sh@{sha[:12]}"),
         "git-object-absent"),
    )
    for name, c, want in bads:
        got = _tool_candidate_problem(repo, c) or ""
        ck(f"{name} — 결손 사유 {want}", got.split("(")[0] == want)
    ck("정상 후보는 결함 없음", _tool_candidate_problem(repo, good) is None)
    ck("★git 저장소가 아니면 검증 불가(후보 ✗)", str(_tool_candidate_problem(repo / "docs", good)).startswith("tool-source-unverifiable"))
    seeds = dict(ev.get("lineage_seeds") or {})
    seeds["evidence_candidates"] = list(seeds.get("evidence_candidates") or []) + [good] + [c for _, c, _ in bads]
    lin = derive(repo, dict(ev, lineage_seeds=seeds), **kw)
    cands = {c["path"]: c for c in lin["evidence_candidates"]}
    row = cands.get(good["path"], {})
    ck("도구 스냅숏 후보 = draft 상대 경로 · kind · origin(저장소 실재 검사 ✗ · git 바이트 검사 ○)",
       row.get("kind") == TOOL_SOURCE_KIND and row.get("origin") == good["origin"] and not (repo / good["path"]).exists())
    ck("★결함 후보는 후보가 아니고 결손으로 적힌다(같은 경로의 결함 후보가 정상 origin 을 덮지 않는다)",
       not any(p in cands for p in (f"{TOOL_SNAPSHOT_DIR}/run_bench.sh@{other[:12]}", f"{TOOL_SNAPSHOT_DIR}/lite_bench.sh@{sha[:12]}",
                                    f"{TOOL_SNAPSHOT_DIR}/none.sh@{sha[:12]}"))
       and cands.get("tools/run_bench.sh", {}).get("kind") != TOOL_SOURCE_KIND
       and sum(1 for m in lin["seeds_missing"] if m["source"] == "lineage_seeds:evidence_candidates"
               and m["reason"].startswith(("tool-source", "git-object-absent"))) == len(bads))
    ck("발췌 출처 해소 — `<이름>@<rev12>` · 전체 draft 상대 경로", resolve_stem(lin, f"run_bench.sh@{sha[:12]}") == good["path"]
       and resolve_stem(lin, good["path"]) == good["path"])
    ck("★rev 가 다른 표기는 해소되지 않는다", resolve_stem(lin, f"run_bench.sh@{other[:12]}") is None)
    try:
        tool_snapshot_rel("../x", sha)
        ck("★도구 스냅숏 이름 모양 거부", False)
    except core.HintError as e:
        ck("★도구 스냅숏 이름 모양 거부", e.code == "HINT_LINEAGE_TOOL_SOURCE_SHAPE")
    dup = dict(good, origin=f"git:{sha}:tools/sub/run_bench.sh")
    try:
        derive(repo, dict(ev, lineage_seeds=dict(seeds, evidence_candidates=[good, dup])), **kw)
        ck("★같은 경로에 서로 다른 유효 origin 둘 = 차단", False)
    except core.HintError as e:
        ck("★같은 경로에 서로 다른 유효 origin 둘 = 차단", e.code == "HINT_LINEAGE_TOOL_ORIGIN_CONFLICT")


def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 합성 docs 트리 + 임시 git 저장소 — 문법 SSOT 2종만 이 체크아웃에서 복사한다."""
    bad: list[str] = []

    def ck(name: str, cond: bool) -> None:
        if not cond:
            bad.append(f"lineage: {name}")

    own = _own_checkout()
    with tempfile.TemporaryDirectory(prefix="hint_lineage_selftest_") as td:
        repo = Path(td).resolve()
        for rel_path in (core.REL_DOC_NAMING_WIKI, core.REL_DOC_NAMING_BENCH):
            dst = repo / rel_path
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes((own / rel_path).read_bytes())

        def w(rel_path: str, text: str) -> None:
            p = repo / rel_path
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")

        S = "Zeta2.5-Flash"          # 모델 표기(필터는 base 슬러그 zeta2.5-flash)
        w("docs/plan/plan_26010001_zeta_zero.md", f"# {S} 영점\n`docs/plan/plan_26010000_zeta_origin.md` 를 잇는다.\n")
        w("docs/plan/plan_26010000_zeta_origin.md", f"# {S} 기원 — 깊이 5 라 잘려야 한다\n")
        w("docs/plan/plan_26010100_zeta_first.md", f"# {S} 첫 계획\n선행 `plan_26010001_zeta_zero.md`.\n")
        w("docs/plan/plan_26010115_alpha.md", f"# {S} alpha\n")
        w("docs/plan/plan_26010115_beta.md", f"# {S} beta\n")
        # ★무신호 형제의 이질 토큰 수가 달라도 전부 유지(2026-09-22 round 2 리뷰 — alpha/beta 는 이질 토큰이 같아 규칙을 꺼도 초록이었다)
        w("docs/plan/plan_26010115_gamma_delta.md", f"# {S} gamma delta\n")
        w("docs/testlog/testlog_26010200_zeta_smoke.md",
          "# testlog\n\n> **SUPERSEDED-IN-PART** — `docs/testlog/testlog_26010600_zeta_verdict.md` §2 ·\n"
          "> `docs/report/perf_26010400_zeta_종합.md` (2026-01-06)\n\n"
          f"{S}-NVFP4 스모크. 계획 `plan_26010100_zeta_first.md` · 형제 `plan_26010115` · 무관 `testlog_26010250_other`.\n"
          "고유문장-본문복제감지-7f3a\n")
        w("docs/testlog/testlog_26010250_other.md", "# 다른 모델 zeta2.50-flash 와 zeta2.5-flashy 만 적힌 문서\n")
        w("docs/devlog/devlog_26010300_zeta_narr.md",
          f"# devlog\n> 뒤집은 선행: `testlog_26010200` (SUPERSEDED-IN-PART)\n\n{S} 서사 — testlog_26010200 을 서술.\n")
        w("docs/report/perf_26010400_zeta_종합.md",
          f"# perf\n{S} 종합. `docs/testlog/testlog_26010200_zeta_smoke.md` §1 · "
          "`docs/benchmark/sweep_map_26010400_*.md` · `docs/benchmark/sweep_map_26010400_c1-a-levers.md` · "
          "후속 교정 `docs/plan/plan_26010455_harness.md`\n")
        # 필터 경유: 모델을 약어(ZF)로만 적은 하네스 plan → 그 너머의 슬러그 문서 / 경유 → 경유 연쇄는 끊긴다
        w("docs/plan/plan_26010455_harness.md",
          "# 하네스 교정 — 모델 약어 ZF 만 적는다\n`docs/report/perf_26010460_zeta_far.md` · `plan_26010465_misc`\n")
        w("docs/report/perf_26010460_zeta_far.md", f"# {S} 타 HW 선례\n")
        w("docs/plan/plan_26010465_misc.md", "# 무관 plan(약어도 없음)\n`docs/report/perf_26010470_zeta_chain.md`\n")
        w("docs/report/perf_26010470_zeta_chain.md", f"# {S} 연쇄 끝 — 닿으면 안 된다\n")
        w("docs/report/perf_26010450_zeta_dirty.md", f"# perf dirty\n{S}\n")
        w("docs/benchmark/sweep_map_26010400_c1-a-levers.md", f"# sweep {S}\n")
        w("docs/benchmark/sweep_map_26010400_c1-a-levers.json", json.dumps({"model": "zeta2.5-flash-nvfp4"}))
        w("docs/benchmark/sweep_map_26010400_c2-b-levers.md", f"# sweep {S} c2\n")
        # 모호 해소 형제 필터(2026-09-22 · S2 round 2 F7c): c3-c 는 글롭 형제이면서 다른 문서가 **정확 인용**한다 — 필터와 무관하게 채택.
        w("docs/benchmark/sweep_map_26010400_c3-c-levers.md", f"# sweep {S} c3\n")
        # ★동점 가르기(이질 토큰 최소): c1-a-levers-extra 는 셀 키 적중이 c1-a-levers 와 같고(2) 이질 토큰이 하나 더 많다(levers·extra).
        #   태그2 라이브에서 표적 지도(nv4-262k-mmp-levers 3·1)를 이웃(nv4-bf-512k-mmp-levers 3·2)과 가른 것이 이 규칙이다(2026-09-22 리뷰).
        w("docs/benchmark/sweep_map_26010400_c1-a-levers-extra.md", f"# sweep {S} extra\n")
        # ★인용 원천(lineage_seeds 본문)의 모호 토큰에도 같은 필터가 걸린다 — 문서가 아니라 시드 텍스트만 이 무리를 부른다.
        w("docs/benchmark/sweep_map_26010410_c1-a-note.md", f"# note {S} c1-a\n")
        w("docs/benchmark/sweep_map_26010410_c9-z-note.md", f"# note {S} c9\n")
        # ★한 무리에서 걸러지고 다른 자리(자기 시각 형제를 부르는 c1-a-x 본문 — 남은 형제 1건)에서는 남겨진 뒤 슬러그 필터로 빠진 형제는
        #   "모호 해소로 걸러졌다" 고 적지 않는다(부재 사유는 필터다).
        w("docs/benchmark/sweep_map_26010420_c1-a-x.md", f"# {S} c1-a x\n자매 `sweep_map_26010420`\n")
        w("docs/benchmark/sweep_map_26010420_c2-b-y.md", "# 다른 모델 c2 y\n")
        w("docs/plan/plan_26010500_zeta_second.md",
          f"# plan\n> 선행 `docs/report/perf_26010400_zeta_종합.md`\n> 참고 `docs/report/perf_26010450_zeta_dirty.md`\n\n"
          f"{S} 두 번째 계획. `docs/report/perf_26010900_zeta_later.md` 는 사후 편집 인용(D-W8).\n")
        w("docs/testlog/testlog_26010600_zeta_verdict.md",
          f"# testlog\n> 계획 `docs/plan/plan_26010500_zeta_second.md`\n\n{S} 판정. "
          "`docs/simlog/26010600_run` · `docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml` · "
          "`output/multi/benchlog/serve_fail_x/master_failure.log`\n")
        w("docs/devlog/devlog_26010600_zeta_main.md",
          f"# devlog\n{S} 본 서사. 형제 비교 `docs/benchmark/sweep_map_26010400_c3-c-levers.md` 정확 인용.\n")
        w("docs/benchmark/bench_report_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.md", f"# bench {S}\n")
        w("docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml",
          "model: zeta2.5-flash-nvfp4\nmeasured_utc: '2026-01-05T15:00:00Z'\n")
        w("docs/simlog/26010600_run/sweep_index.json", json.dumps({"meta": {"model": "zeta2.5-flash-nvfp4"}}))
        w("output/multi/benchlog/serve_fail_x/master_failure.log", "ValueError: boom\n")
        w("output/multi/benchlog/sweep_c1-a/engine_1.log", "engine\n")
        w("output/multi/benchlog/sweep_c1-a/sweep_index.json", "{}")
        # 레벨 원시(F7b) · ★숨김 토크나이저 스테이징은 후보가 아니다
        w("output/multi/benchlog/sweep_c1-a/level_01/bench_c1-a.json", '{"completed": 4}')
        w("output/multi/benchlog/sweep_c1-a/level_01/.tokstage/tokenizer.json", "{}")
        # 디렉터리 후보 펼치기(F7a): 바로 아래 파일 + 셀 키 경로 파일만 · ★깊은 타 셀 파일·숨김 경로는 펴지 않는다 · ★상한
        w("output/multi/benchlog/serve_fail_y/master_failure.log", "AttributeError: fixture\n")
        w("output/multi/benchlog/serve_fail_y/slave_failure.log", "slave\n")
        w("output/multi/benchlog/serve_fail_y/deep/c1-a/w.log", "cell-deep\n")
        w("output/multi/benchlog/serve_fail_y/deep/other/z.log", "other-deep\n")
        w("output/multi/benchlog/serve_fail_y/.hidden/x.log", "hidden\n")
        for i in range(EXPAND_MAX_FILES + 2):
            w(f"output/multi/benchlog/serve_fail_big/f{i:03d}.log", "x\n")
        # ★허브: 시드를 언급하는 **후손** plan · plan 을 언급하는 devlog — 둘 다 조상이 아니다
        w("docs/plan/plan_26010700_hub.md", f"# hub {S}\n`testlog_26010600_zeta_verdict` · `plan_26010500_zeta_second`\n")
        w("docs/devlog/devlog_26010710_hub_narr.md", f"# {S}\n`docs/plan/plan_26010500_zeta_second.md` 를 서술\n")
        # ★상한 뒤: 시드 testlog 를 서술하는 devlog · 사후 편집으로 인용된 report
        w("docs/devlog/devlog_26010900_zeta_late.md", f"# {S}\n`testlog_26010600` 서술(발행 뒤)\n")
        w("docs/report/perf_26010900_zeta_later.md", f"# {S} later\n")
        # ★seed/·sync_staging/ 사본 — 같은 이름이어도 노드가 아니다
        w("seed/이전 plan 백업/plan_26010100_zeta_first.md", f"# 백업 {S}\n")
        w("sync_staging/sub_docs/report/perf_26010400_zeta_종합.md", f"# 미러 {S}\n")
        w("docs/plan/example.md", "# 스켈레톤\n")
        rec_this = {"publication_id": "campx_bench_c1_a", "generated_utc": "2026-01-05T16:00:00Z",
                    "identity": {"model": "zeta2.5-flash-nvfp4", "gpu": "NVIDIA GB10", "topology": "multi",
                                 "tp": 2, "vllm": "0.1.0"},
                    "narrative_status": {"plan": {"source": "docs/plan/plan_26010500_zeta_second.md"},
                                         "devlog": {"source": "docs/devlog/devlog_26010600_zeta_main.md"},
                                         "testlog": {"source": "docs/testlog/testlog_26010600_zeta_verdict.md"}},
                    "scaffolded": {"bench_report": "docs/benchmark/bench_report_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.md",
                                   "certificate": "docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml",
                                   "simlog": "docs/simlog/26010600_run", "report": None,
                                   "verification": "sync_staging/sub_docs/report/perf_26010400_zeta_종합.md"}}
        rec_prior = {"publication_id": "old_full_c1_a", "generated_utc": "2026-01-01T02:00:00Z",
                     "identity": {"model": "Zeta2.5-Flash-NVFP4", "gpu": "NVIDIA GB10", "topology": "multi",
                                  "tp": 2, "vllm": "0.0.9"},
                     "scaffolded": {"plan": "docs/plan/plan_26010100_zeta_first.md",
                                    "report": "seed/이전 plan 백업/plan_26010100_zeta_first.md",
                                    "devlog": "/" + "home/op-fixture/elsewhere/devlog_26010099_x.md"}}
        rec_other_model = {"identity": {"model": "omega-1b", "gpu": "NVIDIA GB10", "topology": "multi", "tp": 2},
                           "scaffolded": {"plan": "docs/plan/plan_26010115_alpha.md"}}
        # 다른 셀 기록의 plan 은 형제 모호 대조(plan_26010115_*)와 겹치지 않는 문서를 가리킨다 — S3 가 그 기록을 시드로 삼으면
        #   정확 시드가 되어 모호 해소 대조가 공허해진다.
        w("docs/plan/plan_26010016_c2b.md", f"# {S} 다른 셀(c2-b) 계획\n")
        rec_other_cell = {"identity": rec_this["identity"], "scaffolded": {"plan": "docs/plan/plan_26010016_c2b.md"}}
        rec_late = {"generated_utc": "2026-01-09T03:00:00Z", "identity": rec_this["identity"],
                    "scaffolded": {"plan": "docs/report/perf_26010900_zeta_later.md"}}
        # ★셀 키 접미 충돌(2026-09-22 감사): id 는 `_c1_a` 로 끝나지만 측정 셀은 `b-c1-a` — simlog 관측이 이긴다.
        #   거꾸로 id 는 셀과 무관해도 simlog 측정 셀이 `c1-a` 면 같은 셀의 과거 기록이다.
        w("docs/simlog/26010103_bc1a_run/sweep_index.json", json.dumps({"meta": {"config_name": "b-c1-a"}}))
        w("docs/simlog/26010102_zz_run/sweep_index.json", json.dumps({"meta": {"config_name": "c1-a"}}))
        w("docs/plan/plan_26010103_bc1a.md", f"# {S} 다른 셀(b-c1-a) 계획 — 시드가 되면 안 된다\n")
        w("docs/plan/plan_26010102_zz_sim.md", f"# {S} 같은 셀(c1-a)을 잰 기록의 계획\n")
        rec_suffix_clash = {"generated_utc": "2026-01-01T03:00:00Z", "identity": rec_this["identity"],
                            "raw_log_paths": {"simlog": "docs/simlog/26010103_bc1a_run"},
                            "scaffolded": {"plan": "docs/plan/plan_26010103_bc1a.md"}}
        rec_sim_match = {"generated_utc": "2026-01-01T02:30:00Z", "identity": rec_this["identity"],
                         "raw_log_paths": {"simlog": "docs/simlog/26010102_zz_run"},
                         "scaffolded": {"plan": "docs/plan/plan_26010102_zz_sim.md"}}
        # 시드 S3(같은 모델 · 앞선 다른 셀 · 2026-09-29 R-a): identity.model 대조 · 관측 별칭 · gpu · 서사 종류만.
        w("docs/plan/plan_26010020_zbr_bringup.md", "# 브링업 계획 — 별칭 zbr7 만 적었다(base 슬러그 0회)\n")
        w("docs/testlog/testlog_26010021_zbr_two.md", f"# {S} 브링업 판정\n")
        w("docs/devlog/devlog_26010022_zbr_three.md", f"# {S} 브링업 서사(identity.model 없는 기록 · 별칭 대조)\n")
        w("docs/plan/plan_26010023_qq9x_three.md", f"# {S} 별칭 아님(다른 모델 id 에도 나오는 토큰) — 시드 ✗\n")
        w("docs/plan/plan_26010024_zeta_h100.md", f"# {S} 다른 HW — 시드 ✗\n")
        w("docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md", f"# {S} 다른 셀의 측정 표면 — 시드 ✗\n")
        zid = {"model": "zeta2.5-flash", "gpu": "NVIDIA GB10", "topology": "single", "tp": 1}
        s3_recs = (
            ("zbr7_one", {"generated_utc": "2026-01-01T00:00:00Z", "identity": zid,
                          "scaffolded": {"plan": "docs/plan/plan_26010020_zbr_bringup.md",
                                         "bench_report": "docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md",
                                         "simlog": "docs/simlog/26010020_zbr_run"}}),
            ("zbr7_two", {"generated_utc": "2026-01-01T00:00:00Z", "identity": zid,
                          "scaffolded": {"testlog": "docs/testlog/testlog_26010021_zbr_two.md"}}),
            ("zbr7_three", {"generated_utc": "2026-01-01T00:00:00Z", "identity": {"gpu": "NVIDIA GB10"},
                            "scaffolded": {"devlog": "docs/devlog/devlog_26010022_zbr_three.md"}}),
            ("qq9x_one", {"generated_utc": "2026-01-01T00:00:00Z", "identity": zid, "scaffolded": {}}),
            ("qq9x_two", {"generated_utc": "2026-01-01T00:00:00Z", "identity": zid, "scaffolded": {}}),
            ("omega_qq9x", {"generated_utc": "2026-01-01T00:00:00Z", "identity": {"model": "omega-1b"}, "scaffolded": {}}),
            ("qq9x_three", {"generated_utc": "2026-01-01T00:00:00Z", "identity": {},
                            "scaffolded": {"plan": "docs/plan/plan_26010023_qq9x_three.md"}}),
            ("zeta_h100_run", {"generated_utc": "2026-01-01T00:00:00Z", "identity": {**zid, "gpu": "NVIDIA H100"},
                               "scaffolded": {"plan": "docs/plan/plan_26010024_zeta_h100.md"}}),
        )
        w("docs/simlog/26010020_zbr_run/sweep_index.json", json.dumps({"meta": {"config_name": "zbr-cell"}}))
        for rid, rec in (("campx_bench_c1_a", rec_this), ("old_full_c1_a", rec_prior),
                         ("other_full_c1_a", rec_other_model), ("campx_bench_c2_b", rec_other_cell),
                         ("redo_full_c1_a", rec_late), ("old_full_b_c1_a", rec_suffix_clash),
                         ("zz_unrelated_name", rec_sim_match)) + s3_recs:
            w(f"docs/_evidence/{rid}.json", json.dumps(rec, ensure_ascii=False))
        w("docs/_evidence/campx_bench_c1_a.work-manifest.json", "{}")
        w("docs/_evidence/broken.json", "{not json")

        # 임시 git — 추적 report 는 경로+커밋, 추적 후 수정(dirty)·비추적은 sha256 (GIT_SINGLE_AUTHORITY)
        genv = {"GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_AUTHOR_NAME": core.SYNTHETIC_NAME, "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL,
                "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL,
                "GIT_AUTHOR_DATE": core.git_date("2026-01-04T00:00:00Z"),
                "GIT_COMMITTER_DATE": core.git_date("2026-01-04T00:00:00Z")}
        git_ok = True
        # 측정 도구 원천(S2 round 3 · tool-source@rev 후보의 origin 바이트) — 커밋에 함께 싣는다
        w("tools/run_bench.sh", "#!/bin/bash\n# fixture bench tool\nvllm bench serve --temperature 0\n")
        w("tools/sub/run_bench.sh", "#!/bin/bash\n# 같은 basename 의 다른 도구(origin 충돌 대조)\n")
        try:
            core.git(repo, "init", "-q", env_extra=genv)
            core.git(repo, "add", "docs/report/perf_26010400_zeta_종합.md", "docs/report/perf_26010450_zeta_dirty.md",
                     "tools/run_bench.sh", "tools/sub/run_bench.sh", env_extra=genv)
            core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m",
                     "fixture", env_extra=genv)
        except core.HintError as e:
            git_ok = False
            bad.append(f"lineage: 임시 git 픽스처 실패 {e.code}")
        w("docs/report/perf_26010450_zeta_dirty.md", f"# perf dirty (추적 후 수정)\n{S}\n")

        # ── 문법 파싱(SSOT 적재) ──
        pn = lambda p: parse_doc_name(p, repo=repo)   # noqa: E731
        a = pn("docs/testlog/testlog_26090317_08_56_x.md")
        ck("산문 이름 _MM_SS", a["kind"] == "testlog" and a["yymmddhh"] == "26090317" and a["mmss"] == "08_56"
           and a["topic"] == "x")
        ck("★7자리 시간 토큰은 규약 밖(legacy)", pn("docs/plan/plan_2601010_x.md")["legacy"] is True)
        r = pn("docs/report/harness_26082218_08_32_노드.md")
        ck("report 분류·접미", r["kind"] == "report" and r["class"] == "harness" and r["mmss"] == "08_32")
        ck("무날짜 report = legacy", pn("docs/report/rtxpro6000-benchmark-explorer.html")["legacy"] is True)
        ck("bench_report", pn("docs/benchmark/bench_report_26091304_m_GB10_0.29.0.md")["kind"] == "bench_report")
        ck("인증서 = 데이터", pn("docs/benchmark/benchmark_NA_m_GB10_0.29.0.yaml")["kind"] == "certificate")
        ck("sweep json 쌍둥이", pn("docs/benchmark/sweep_map_26091008_x.json")["kind"] == "sweep_json")
        s1 = pn("docs/simlog/26091304_48_40_camp_x")
        ck("simlog 접미형 왕복 대조", s1["kind"] == "simlog" and s1["mmss"] == "48_40" and not s1["legacy"])
        s2 = pn("docs/simlog/2026090115_1_gptoss120b_single")
        ck("레거시 simlog 10자리 → YYMMDDHH", s2["yymmddhh"] == "26090115" and s2["legacy"])
        ck("★숨김 스윕 상태는 문서 아님", pn("docs/benchmark/.sweep_x.json")["kind"] is None)

        # ── 배너 ──
        bn = supersede_banners(repo / "docs/testlog/testlog_26010200_zeta_smoke.md")
        ck("배너 여러 줄(연속 인용 문단)", bn == ["testlog_26010600_zeta_verdict", "perf_26010400_zeta_종합"])

        # ── 파생 ──
        ev = {"cell": "c1-a", "publication": {"topic": "campx_bench_c1_a"}, "pointers": [],
              "lineage_seeds": {"journey_note": "형제 지도 sweep_map_26010410 비교 · sweep_map_26010420", "evidence_candidates": [
                  {"path": "output/multi/benchlog/serve_fail_y", "kind": "engine_failure_logs"},
                  {"path": "output/multi/benchlog/serve_fail_big", "kind": "engine_failure_logs"}]},
              "sweep": {"dir": "output/multi/benchlog/sweep_c1-a"}}
        kw = dict(base_slug="zeta2.5-flash", publish_kst="26010800")
        lin = derive(repo, ev, **kw)
        docs = {d["path"]: d for d in lin["documents"]}
        P = lambda n: next((p for p in docs if PurePosixPath(p).name.startswith(n)), None)   # noqa: E731
        ck("시드 문서 채택(depth 0)", all(docs.get(p, {}).get("depth") == 0 for p in (
            "docs/plan/plan_26010500_zeta_second.md", "docs/testlog/testlog_26010600_zeta_verdict.md",
            "docs/devlog/devlog_26010600_zeta_main.md",
            "docs/benchmark/bench_report_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.md",
            "docs/plan/plan_26010100_zeta_first.md")))
        sr_ = lin["method"]["seed_records"]
        ck("과거 발행 기록(대소문자 다른 identity · id 접미 · simlog 관측) 시드",
           sr_["prior"] == ["old_full_c1_a", "zz_unrelated_name"]
           and sr_["prior_match"] == {"old_full_c1_a": "publication-id-suffix(simlog 미관측)",
                                      "zz_unrelated_name": "simlog:sweep_index.meta.config_name"})
        src_of = lambda p: {x["source"] for x in lin["seeds"] if x["path"] == p}   # noqa: E731
        ck("★셀 키 접미 충돌(측정 셀이 다른 기록)은 같은 셀 시드(S2) 아님 · 같은 모델 앞선 셀(S3)로만",
           sr_["prior_cell_mismatch"] == ["old_full_b_c1_a"]
           and src_of("docs/plan/plan_26010103_bc1a.md") == {MODEL_PRIOR_SOURCE + "old_full_b_c1_a"}
           and docs.get("docs/plan/plan_26010102_zz_sim.md", {}).get("depth") == 0)
        ck("★다른 모델 기록은 어떤 시드도 아님 · 다른 셀 기록은 S2 시드 아님",
           not any(s["source"].endswith("other_full_c1_a") for s in lin["seeds"])
           and not any(s["source"] == "prior_publication:campx_bench_c2_b" for s in lin["seeds"]))
        mpr = lin["method"]["model_prior_records"]
        ck("S3: 같은 모델 앞선 다른 셀 기록 = 시드(identity.model · 대소문자·양자화 접미 무관)",
           {"campx_bench_c2_b", "zbr7_one", "zbr7_two"} <= set(mpr["records"])
           and mpr["match"].get("zbr7_one") == "identity.model~base_slug"
           and src_of("docs/plan/plan_26010016_c2b.md") == {MODEL_PRIOR_SOURCE + "campx_bench_c2_b"})
        ck("S3: 별칭만 적은 브링업 문서도 채택(identity 가 모델을 선언 · 필터 면제)",
           docs.get("docs/plan/plan_26010020_zbr_bringup.md", {}).get("depth") == 0)
        ck("S3: 관측 별칭 파생(같은 모델 id 첫 토큰 2건+ · 다른 모델 id 에 없음)",
           mpr["aliases"] == ["zbr7"] and mpr["alias_evidence"] == {"zbr7": ["zbr7_one", "zbr7_two"]})
        ck("S3: identity.model 없는 기록은 별칭으로 대조", str(mpr["match"].get("zbr7_three", "")).startswith("id-alias:zbr7")
           and docs.get("docs/devlog/devlog_26010022_zbr_three.md", {}).get("depth") == 0)
        ck("★S3: 다른 모델 id 에도 나오는 토큰은 별칭 아님 → 모델 없는 기록 불채택",
           "qq9x_three" not in mpr["records"] and "docs/plan/plan_26010023_qq9x_three.md" not in docs)
        ck("★S3: 다른 모델·다른 HW·발행 뒤 기록 제외", "other_full_c1_a" not in mpr["records"]
           and mpr["gpu_mismatch"] == ["zeta_h100_run"] and "docs/plan/plan_26010024_zeta_h100.md" not in docs
           and "redo_full_c1_a" in mpr["after_ceiling"] and "redo_full_c1_a" not in mpr["records"])
        ck("★S3: S2 가 잡은 같은 셀 기록은 S3 에 중복하지 않는다", not set(sr_["prior"]) & set(mpr["records"]))
        ck("★S3: 다른 셀의 측정 표면·데이터 포인터는 싣지 않는다",
           "docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md" not in docs
           and not any(c["path"].startswith("docs/simlog/26010020_zbr_run") for c in lin["evidence_candidates"])
           and "docs/benchmark/bench_report_26010020_zeta2.5-flash_GB10_0.0.8.md" in mpr["skipped_non_narrative"])
        ck("★발행 뒤 과거 기록은 제외", lin["method"]["seed_records"]["prior_after_ceiling"] == ["redo_full_c1_a"])
        ck("조상 체인(시드 plan_first → plan_zero depth 1 → plan_origin depth 2)",
           docs.get("docs/plan/plan_26010001_zeta_zero.md", {}).get("depth") == 1
           and docs.get("docs/plan/plan_26010000_zeta_origin.md", {}).get("depth") == 2)
        # ★깊이 상한: 옛 검사는 "없거나 depth ≤ 4" 라 픽스처가 깊이 2 에서 끝나 **항상 참**이었다(공허한 검사 ·
        #   2026-09-22 감사). 상한을 1 로 낮춰 depth 2 문서가 실제로 잘리는지 본다.
        lin_d1 = derive(repo, ev, base_slug="zeta2.5-flash", publish_kst="26010800", depth=1)
        d1 = {d["path"] for d in lin_d1["documents"]}
        ck("★깊이 상한(depth=1) 밖 제외", "docs/plan/plan_26010001_zeta_zero.md" in d1
           and "docs/plan/plan_26010000_zeta_origin.md" not in d1
           and all(x["depth"] <= 1 for x in lin_d1["documents"]))
        # 필터 경유 1홉(2026-09-22 라이브: plan_26091407 이 슬러그 없이 perf_26091314 로 가는 유일한 길목이었다)
        ck("필터 탈락 문서 경유 → 너머의 슬러그 문서 채택", docs.get("docs/report/perf_26010460_zeta_far.md", {})
           .get("depth") == 3 and any(t["path"] == "docs/plan/plan_26010455_harness.md" for t in lin["transit"]))
        ck("★경유 노드는 읽기 목록 밖", "docs/plan/plan_26010455_harness.md" not in docs)
        ck("★경유 → 경유 연쇄 금지(연속 2홉 필터 탈락)", "docs/report/perf_26010470_zeta_chain.md" not in docs
           and not any(t["path"] == "docs/plan/plan_26010465_misc.md" for t in lin["transit"]))
        ck("경유 간선도 via 에 실린다", any(v["from"] == "docs/plan/plan_26010455_harness.md" for v in
                                            docs.get("docs/report/perf_26010460_zeta_far.md", {}).get("via", [])))
        ck("★허브(후손 plan) 제외", "docs/plan/plan_26010700_hub.md" not in docs)
        ck("★plan 만 서술한 devlog 는 서술자 아님", "docs/devlog/devlog_26010710_hub_narr.md" not in docs)
        ck("testlog 서술 devlog 보강(narrates)", any(v["edge"] == "narrates" for v in
                                                   docs.get("docs/devlog/devlog_26010300_zeta_narr.md", {}).get("via", [])))
        ck("★상한 뒤 문서 제외(사후 편집 인용·발행 뒤 서술)", "docs/report/perf_26010900_zeta_later.md" not in docs
           and "docs/devlog/devlog_26010900_zeta_late.md" not in docs)
        ck("★필터(모델 슬러그 불일치·경계) 제외", "docs/testlog/testlog_26010250_other.md" not in docs)
        amb = [docs.get(p, {}).get("resolution") for p in ("docs/plan/plan_26010115_alpha.md",
                                                             "docs/plan/plan_26010115_beta.md")]
        ck("★주제 없는 토큰의 동시각 형제 = 모호 전부", amb == ["ambiguous", "ambiguous"])
        ck("유일 형제 = unique", any(v["resolution"] == "unique" and v["edge"] == "narrates" for v in
                                   docs.get("docs/devlog/devlog_26010300_zeta_narr.md", {}).get("via", [])))
        # 2026-09-22(S2 round 2 · F7c): 글롭 형제는 셀 키 적중으로 가른다 — 종전 기대("c2-b 도 모호로 채택")는 옛 규칙이다.
        amb_ex = {a["path"]: a for a in lin["ambiguous_excluded"]}
        ck("글롭 형제: 정확 인용 exact 유지", docs.get("docs/benchmark/sweep_map_26010400_c1-a-levers.md", {})
           .get("resolution") == "exact")
        ck("★글롭 형제 중 셀 키 불일치(c2-b)는 채택하지 않고 ambiguous_excluded 에 사유·출처와 함께 적는다",
           "docs/benchmark/sweep_map_26010400_c2-b-levers.md" not in docs
           and amb_ex.get("docs/benchmark/sweep_map_26010400_c2-b-levers.md", {}).get("from")
           == ["docs/report/perf_26010400_zeta_종합.md"]
           and "셀 키 적중 0" in amb_ex["docs/benchmark/sweep_map_26010400_c2-b-levers.md"]["reason"]
           and lin["method"]["stats"]["ambiguous_excluded"] == len(lin["ambiguous_excluded"]))
        ck("★글롭 형제여도 다른 문서가 정확 인용하면 채택(필터는 모호 간선에만) · 제외 목록에 없다",
           "docs/benchmark/sweep_map_26010400_c3-c-levers.md" in docs
           and "docs/benchmark/sweep_map_26010400_c3-c-levers.md" not in amb_ex)
        ck("최선 형제 동률(md/json 쌍둥이)은 둘 다 유지", "docs/benchmark/sweep_map_26010400_c1-a-levers.json" not in amb_ex)
        ck("★셀 키 적중이 같으면 이질 토큰이 적은 형제만(c1-a-levers-extra 제외 · 사유 기재)",
           "docs/benchmark/sweep_map_26010400_c1-a-levers-extra.md" not in docs
           and "이질 토큰 2 < 최선 형제 2·1" in amb_ex.get("docs/benchmark/sweep_map_26010400_c1-a-levers-extra.md", {}).get("reason", ""))
        ck("★인용 원천(lineage_seeds 본문)의 모호 토큰도 형제 필터를 탄다(c9-z 제외 · 출처 = 그 시드 키)",
           "docs/benchmark/sweep_map_26010410_c1-a-note.md" in docs and "docs/benchmark/sweep_map_26010410_c9-z-note.md" not in docs
           and amb_ex.get("docs/benchmark/sweep_map_26010410_c9-z-note.md", {}).get("from") == ["lineage_seeds:journey_note"])
        # 깊이 1 이면 c2-b-y 는 경유(transit)도 못 되고 슬러그 필터로 빠진다 — 경유 기록이 가리지 않는 자리에서 규칙을 잰다.
        lin_d1 = derive(repo, ev, **dict(kw, depth=1))
        ck("★다른 자리에서 남겨진 형제는 ambiguous_excluded 에 적지 않는다(그 뒤 부재 = 슬러그 필터 · 경유 아님)",
           "docs/benchmark/sweep_map_26010420_c1-a-x.md" in docs and "docs/benchmark/sweep_map_26010420_c2-b-y.md" not in docs
           and "docs/benchmark/sweep_map_26010420_c2-b-y.md" not in amb_ex
           and not any(t["path"] == "docs/benchmark/sweep_map_26010420_c2-b-y.md" for t in lin_d1["transit"])
           and "docs/benchmark/sweep_map_26010420_c2-b-y.md" not in {x["path"] for x in lin_d1["ambiguous_excluded"]})
        # ★여러 무리에서 걸러진 형제의 기록은 호출 순서와 무관하다(사전순 최소 (토큰, 사유) · 2026-09-22 round 2 리뷰)
        mem2 = ["docs/benchmark/sweep_map_26010400_c1-a-levers.md", "docs/benchmark/sweep_map_26010400_c2-b-levers.md"]
        recs2 = []
        for order in (("sweep_map_26010400_z", "sweep_map_26010400"), ("sweep_map_26010400", "sweep_map_26010400_z")):
            sf2 = _SiblingFilter(_Corpus(repo), ["c1-a"])
            for i, tok in enumerate(order):
                sf2.keep(f"src{i}", tok, mem2)
            recs2.append({k: (v["token"], v["reason"], sorted(v["from"])) for k, v in sf2.excluded.items()})
        ck("★형제 제외 기록 = 호출 순서 무관(결정론)", recs2[0] == recs2[1] and len(recs2[0]) == 1
           and recs2[0]["docs/benchmark/sweep_map_26010400_c2-b-levers.md"][0] == "sweep_map_26010400")
        ck("무신호 형제(셀 키 적중 0 — alpha/beta)는 제외 목록에 없다(전부 유지)",
           not any(p.startswith("docs/plan/plan_26010115_") for p in amb_ex))
        # 셀 선언 축(declared_axes)도 셀 키다: 셀 id 가 c1 만 공유하면 c1-a 가 최선이지만, 선언 축 c2-b 가 있으면 c2-b 가 최선이다.
        ev_q = dict(ev, cell="c1-q")
        lin_q = derive(repo, ev_q, **kw)
        ev_ax = dict(ev_q, cell_config={"declared_axes": {"lever": "c2-b", "blank": "<<FILL>>"}})
        lin_ax = derive(repo, ev_ax, **kw)
        ck("★셀 선언 축 없음 — c2-b 는 셀 키 적중 0 이라 제외", "docs/benchmark/sweep_map_26010400_c2-b-levers.md"
           not in {d["path"] for d in lin_q["documents"]})
        ck("셀 선언 축(declared_axes) 값이 셀 키에 든다 — c2-b 채택", "docs/benchmark/sweep_map_26010400_c2-b-levers.md"
           in {d["path"] for d in lin_ax["documents"]} and "c2-b" in lin_ax["method"]["ambiguous_siblings"]["key_tokens"]
           and not any("fill" in k for k in lin_ax["method"]["ambiguous_siblings"]["key_tokens"]))
        ck("★seed/·sync_staging/ 사본은 노드 아님", not any(p.startswith(("seed/", "sync_staging/")) for p in docs)
           and any(m["reason"] == "excluded-root" for m in lin["seeds_missing"]))
        ck("★스켈레톤 example.md 제외", "docs/plan/example.md" not in docs)
        ck("헤더 라벨 간선", any(v["edge"] == "header:선행" for v in
                                docs.get("docs/report/perf_26010400_zeta_종합.md", {}).get("via", [])))
        smoke = docs.get("docs/testlog/testlog_26010200_zeta_smoke.md", {})
        # 양쪽 배너: 자기 배너의 뒤 문서 2건 + 뒤집은 쪽(devlog_26010300)이 "뒤집은 선행: testlog_26010200" 으로 단 배너
        ck("뒤집힘(양쪽 배너 · 날짜 방향)", smoke.get("superseded_by") == [
            "docs/devlog/devlog_26010300_zeta_narr.md", "docs/report/perf_26010400_zeta_종합.md",
            "docs/testlog/testlog_26010600_zeta_verdict.md"])
        ck("★배너의 이전 문서는 뒤집은 쪽이 아님", docs.get("docs/devlog/devlog_26010300_zeta_narr.md", {})
           .get("superseded_by") == [])
        cands = {c["path"]: c for c in lin["evidence_candidates"]}
        ck("데이터 후보(simlog·인증서·엔진 로그·원시 경로)", {"docs/simlog/26010600_run",
                                                  "docs/benchmark/benchmark_26010600_zeta2.5-flash-nvfp4_GB10_0.1.0.yaml",
                                                  "output/multi/benchlog/sweep_c1-a/engine_1.log",
                                                  "output/multi/benchlog/serve_fail_x/master_failure.log"} <= set(cands))
        ck("★데이터 후보는 문서 목록에 없다", not any(p in docs for p in cands))
        # F7b 레벨 원시 · F7a 디렉터리 펼치기(2026-09-22 · S2 round 2)
        ck("스윕 레벨 원시 JSON 도 후보", "output/multi/benchlog/sweep_c1-a/level_01/bench_c1-a.json" in cands)
        ck("★숨김 경로(.tokstage)는 후보가 아니다", not any("/." in p for p in cands))
        yd = "output/multi/benchlog/serve_fail_y"
        ck("디렉터리 후보 → 바로 아래 파일 + 셀 키 경로 파일", {f"{yd}/master_failure.log", f"{yd}/slave_failure.log",
                                                     f"{yd}/deep/c1-a/w.log"} <= set(cands)
           and cands[f"{yd}/master_failure.log"]["sources"] == [f"expanded:{yd}"])
        ck("★깊은 타 셀 파일은 펴지 않되 개수를 적고(unlisted) 전체 경로로는 닿는다",
           f"{yd}/deep/other/z.log" not in cands
           and cands[yd].get("expanded") == {"listed": 3, "total": 4, "unlisted": 1, "truncated": False}
           and resolve_stem(lin, f"{yd}/deep/other/z.log") == f"{yd}/deep/other/z.log")
        big = cands.get("output/multi/benchlog/serve_fail_big", {}).get("expanded") or {}
        ck("★펼치기 상한(EXPAND_MAX_FILES) · 잘림 기재", big.get("listed") == EXPAND_MAX_FILES and big.get("truncated") is True
           and sum(1 for p in cands if p.startswith("output/multi/benchlog/serve_fail_big/")) == EXPAND_MAX_FILES)
        ck("펼친 파일은 발췌 출처로 해소된다(전체 경로)", resolve_stem(lin, f"{yd}/master_failure.log") == f"{yd}/master_failure.log")
        if git_ok:
            rep = docs.get("docs/report/perf_26010400_zeta_종합.md", {})
            dirty = docs.get("docs/report/perf_26010450_zeta_dirty.md", {})
            ck("추적 report = 경로+커밋(digest 없음)", bool(rep.get("commit")) and rep.get("sha256") is None)
            ck("★추적 후 수정 = sha256", dirty.get("commit") is None and bool(dirty.get("sha256")))
            _selftest_tool_sources(ck, repo, ev, kw)
        ck("비추적 = sha256", bool(docs.get("docs/plan/plan_26010500_zeta_second.md", {}).get("sha256")))
        ck("relevance 셀 축 토큰", "c1-a" in docs.get("docs/benchmark/sweep_map_26010400_c1-a-levers.md", {})
           .get("relevance", {}).get("axis_tokens", []))
        dump = core.dumps(lin)
        ck("★저장소 밖 포인터 원문(운영자 경로) 비유출 · 결손은 기재", "op-fixture" not in dump and any(
            m["path"] == "<outside-repo>" and m["reason"] == "outside-repo" for m in lin["seeds_missing"]))
        ck("★원문 복제 금지(본문 문장 미포함)", "고유문장-본문복제감지-7f3a" not in dump)
        ck("결정론(두 번 파생 동일)", dump == core.dumps(derive(repo, ev, **kw)))
        ck("절대경로 미포함", str(repo) not in dump)

        # 캠페인 여정 파일 본문의 인용 → 시드(필터 대상)
        w("campaigns/campx/journey.jsonl", json.dumps({"reason": "반증 — `testlog_26010200_zeta_smoke` 참조"}) + "\n")
        lin_j = derive(repo, dict(ev, lineage_seeds={"journey": "campaigns/campx/journey.jsonl"}), **kw)
        ck("여정 파일 본문 인용 = 시드", any(s["path"] == "docs/testlog/testlog_26010200_zeta_smoke.md"
                                              and s["source"] == "lineage_seeds:journey:file" for s in lin_j["seeds"]))

        # ── 선언 ──
        lin2 = derive(repo, ev, declared=["docs/testlog/testlog_26010250_other.md=사람 보충(간선 끊김)"], **kw)
        d2 = {d["path"]: d for d in lin2["documents"]}
        ck("선언 = 필터 면제 시드", d2.get("docs/testlog/testlog_26010250_other.md", {}).get("depth") == 0
           and lin2["declared"] == [{"path": "docs/testlog/testlog_26010250_other.md", "source": "declared",
                                     "reason": "사람 보충(간선 끊김)"}])
        for label, decl_item, code in (
                ("★사유 없는 선언", "docs/testlog/testlog_26010250_other.md=", "HINT_LINEAGE_DECLARED_REASON_ABSENT"),
                ("★seed/ 선언", "seed/이전 plan 백업/plan_26010100_zeta_first.md=x", "HINT_LINEAGE_DECLARED_EXCLUDED"),
                ("★부재 선언", "docs/plan/plan_26010199_none.md=x", "HINT_LINEAGE_DECLARED_ABSENT"),
                ("★상한 뒤 선언", "docs/report/perf_26010900_zeta_later.md=x", "HINT_LINEAGE_DECLARED_AFTER_CEILING")):
            try:
                derive(repo, ev, declared=[decl_item], **kw)
                ck(label, False)
            except core.HintError as e:
                ck(f"{label}({e.code})", e.code == code)
        for label, kwargs, code in (("★비주입 시각", dict(base_slug="zeta2.5-flash", publish_kst="2026-01-08"),
                                     "HINT_TIME_NOT_INJECTED"),
                                    ("★빈 슬러그", dict(base_slug="--", publish_kst="26010800"),
                                     "HINT_LINEAGE_FILTER_ABSENT")):
            try:
                derive(repo, ev, **kwargs)
                ck(label, False)
            except core.HintError as e:
                ck(f"{label}({e.code})", e.code == code)

        # ── 소비자 도우미 ──
        rl = reading_list(lin)
        ck("읽기 목록 = 문서 수", len(rl) == len(lin["documents"]))
        ck("읽기 목록: 모호 계층이 뒤", [x["tier"] for x in rl] == sorted((x["tier"] for x in rl),
                                                                    key=lambda t: t != "primary"))
        ck("읽기 목록: primary 는 깊이 오름차순", all(a["depth"] <= b["depth"] for a, b in zip(
            [x for x in rl if x["tier"] == "primary"], [x for x in rl if x["tier"] == "primary"][1:])))
        ck("resolve_stem 전체 stem", resolve_stem(lin, "testlog_26010200_zeta_smoke")
           == "docs/testlog/testlog_26010200_zeta_smoke.md")
        ck("resolve_stem 맨 stem(유일)", resolve_stem(lin, "testlog_26010600") ==
           "docs/testlog/testlog_26010600_zeta_verdict.md")
        ck("★resolve_stem 모호 = None", resolve_stem(lin, "plan_26010115") is None)
        ck("★resolve_stem LINEAGE 밖 = None", resolve_stem(lin, "plan_26010700_hub") is None)
        ck("resolve_stem md/json 쌍둥이 확장자", resolve_stem(lin, "sweep_map_26010400_c1-a-levers.md")
           == "docs/benchmark/sweep_map_26010400_c1-a-levers.md")
        ck("resolve_stem 후보 디렉터리 안 파일", resolve_stem(lin, "docs/simlog/26010600_run/sweep_index.json")
           == "docs/simlog/26010600_run/sweep_index.json")
        try:
            require_documents({"documents": []})
            ck("★문서 0건 차단", False)
        except core.HintError as e:
            ck("★문서 0건 차단(code)", e.code == "HINT_LINEAGE_EMPTY")
        mg = mention_graph(repo)
        ck("mention_graph 는 seed/ 원천 없음", not any(k.startswith(("seed/", "sync_staging/")) for k in mg))
        recs = load_publication_records(repo)
        ck("발행 기록 적재: work-manifest 제외 · 깨진 기록 표시", [r["_id"] for r in recs] == sorted(
            ["broken", "campx_bench_c1_a", "campx_bench_c2_b", "old_full_b_c1_a", "old_full_c1_a", "other_full_c1_a",
             "redo_full_c1_a", "zz_unrelated_name"] + [rid for rid, _ in s3_recs])
           and "_unreadable" in recs[0])
        sp = seeds_from_publications(repo, recs, this_topic="nope", identity=None, cell_key="c1-a")
        ck("★이 발행 기록 부재 = 결손 기재(차단 ✗)", sp["missing"][0]["reason"] == "record-absent" and sp["seeds"] == [])

        # ── 수신자 요약 · 봉인 출처 스냅샷(2026-09-29 · plan_26092908 §4.6·§4.9) ──
        d0 = lin["documents"][0]["path"]
        summ = receiver_summary(lin, excerpt_counts={d0: 2, "docs/simlog/26010600_run/sweep_index.json": 1})
        ck("요약: 문서 수 = 전체 · stem · 역할 · 날짜 · 발췌 수만(해시 · via ✗)", len(summ["documents"]) == len(lin["documents"])
           and summ["documents"][0]["excerpts"] == 2 and summ["documents"][0]["stem"] == PurePosixPath(d0).stem
           and not any(k in summ["documents"][0] for k in ("sha256", "via", "commit"))
           and SEALED_SOURCES_KEY not in summ and summ["kind"] == SUMMARY_KIND)
        ck("요약: 후보는 수 + 인용된 것만", summ["evidence_candidates"]["count"] == len(lin["evidence_candidates"])
           and summ["evidence_candidates"]["cited"] == [{"path": "docs/simlog/26010600_run/sweep_index.json", "excerpts": 1}])
        ck("요약 ≤ 상한 · 전체보다 작다", len(core.dumps(summ).encode()) <= SUMMARY_MAX_BYTES
           and len(core.dumps(summ)) < len(core.dumps(lin)))
        big = {**lin, "documents": [{**lin["documents"][0], "path": f"docs/plan/plan_26010100_{'x' * 200}_{i}.md"}
                                    for i in range(200)]}
        try:
            receiver_summary(big)
            ck("★요약 상한 초과 = HINT_LINEAGE_SUMMARY_TOO_LARGE", False)
        except core.HintError as e:
            ck("★요약 상한 초과 = HINT_LINEAGE_SUMMARY_TOO_LARGE(code)", e.code == "HINT_LINEAGE_SUMMARY_TOO_LARGE")
        ss = sealed_sources(repo, [d0, d0])
        ck("봉인 스냅샷: 중복 제거 · sha256 = 현재 바이트", ss == [{"path": d0, "sha256": hashlib.sha256(
            (repo / d0).read_bytes()).hexdigest()}])
        ck("요약에 스냅샷 싣기", receiver_summary(lin, sealed_sources=ss)[SEALED_SOURCES_KEY] == ss)
        for label, bad_paths in (("★읽을 수 없는 출처", ["docs/plan/nope.md"]), ("★저장소 밖 경로", ["../x.md"]),
                                 ("★절대 경로", ["/etc/hosts"])):
            try:
                sealed_sources(repo, bad_paths)
                ck(f"{label} = HINT_SEALED_SOURCE_UNREADABLE", False)
            except core.HintError as e:
                ck(f"{label} = HINT_SEALED_SOURCE_UNREADABLE(code)", e.code == "HINT_SEALED_SOURCE_UNREADABLE")
    return bad
