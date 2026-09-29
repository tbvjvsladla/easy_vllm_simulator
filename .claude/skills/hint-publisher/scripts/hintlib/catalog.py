"""hintlib.catalog — 원격 발행 태그 → `hints/index.json` + `HINTS.md` 파생 · 수신자 `match`
(plan_26092119 §4.9·§4.10 · SPEC §5.9 · 옛 `hint_catalog.py` + 옛 `hint_tag.cmd_match` 의 후신).

진실원천 = `git ls-remote` (plan_26090107 D1.1)
    옛 `index`/`reindex` 는 **로컬 상태를 신뢰해** 행을 썼다. 감사 `audit_26090106` 이 잡은 두 결함이 그래서 가능했다:
      ④ `cmd_index` 가 증거 검증 0회로 `status: "active"` 를 박는다 — 격리된 태그를 다시 넣으면 격리가 **세탁**된다.
      ⑤ 손저작 brief 가 오염되면 카탈로그가 깨진다(실측: 59항목 중 11 brief 손상 · `<!--` 4개가 렌더에서 18행을 삼킴).
    이제 원격에 있는 것만 카탈로그에 들어간다 — "발행됐다"의 증거가 카탈로그 **바깥**에 생기므로 ④ 가 구조적으로
    소멸하고, 사람이 brief 를 타이핑할 자리가 없으므로 ⑤ 가 재발 불가다. 로컬에만 있는 미발행 태그는 들어갈 수 없다.

fail-closed
    - 원격 조회 실패 = **캐시로 대체하지 않고 중단**. 낡은 카탈로그는 "기록이 원래 없었던 것"과 구분되지 않는다
      (docs.md 부재≠결측). 2026-09-01 변이시험: "실패 시 조용히 빈 목록" 변이를 E2E 가 초록으로 통과시켰다 → 음성대조 고정.
    - 원격에 있는데 **로컬에 태그 오브젝트가 없으면** brief 를 지어낼 수 없다(합성 금지). 기본 계약은 중단 +
      전용 종료코드 `EXIT_LOCAL_TAG_OBJECT_MISSING`(4) + **조회한 그 원격** 을 가리키는 fetch 안내(주 워크트리 기준)다.
      자동 fetch 는 하지 않는다(원격 태그를 로컬로 들이는 것은 사람이 정한다 · 2026-09-14 ⑧-pre D2 S7).
    - `record_missing=True`(X15 · 2026-09-21): 부재 태그를 `object: absent-local` 행(brief `—` · 결손 `미수령`)으로
      **정직하게 싣고** rc 0 + NOTE. 타 PC 발행 1건(코드맵 K4 · 원격 47 / 로컬 46)이 카탈로그 갱신 **전체**를 막던 것을
      D10("과거·타 PC 결함은 신규를 막지 않는다")과 합성 금지를 둘 다 지키며 푼다. `continue` 와 S5 재생성이 이 모드를 쓴다.
    - 같은 이유로 record_missing 모드는 **annotated 가 아닌 원격 태그**(lightweight 등)도 `object: not-annotated` 행
      (brief `—` · 합성 ✗)으로 싣고 NOTE 한다(2026-09-22 감사). 기본 모드는 옛 계약 그대로 중단한다(★lightweight 거부 이관).
      왜: 원격 태그는 리콜할 수 없다(P1) — 다른 발행처가 올린 lightweight 1건이 `continue` 의 마지막 단계(push **뒤**
      카탈로그 파생)를 영구히 막으면, 이미 push 된 신규 발행의 캠페인 publish 기록이 영원히 닫히지 않는다(D10 위반).

원격 SHA 를 버리지 않는다 (코드맵 K5 · 2026-09-21)
    옛 파생기는 ls-remote 의 태그 **이름**만 남기고 로컬의 같은 이름 태그를 읽었다 — 로컬 태그가 원격과 다른 오브젝트면
    (재태깅·타 PC 동명 발행) 원격 행에 로컬 brief 가 붙는다. 이제 **원격이 광고한 태그 오브젝트 SHA 를 직접 읽는다.**
    그 오브젝트가 로컬 오브젝트 DB 에 없으면 = 로컬 부재. 같은 이름의 로컬 ref 가 다른 오브젝트면 고지만 한다.

과거 태그는 판정하지 않는다 (P1 리콜 금지 · D10)
    옛 문법(`legacy-5seg-node`·`legacy-5seg`) 행은 **그대로 나열**한다. 문법 열은 판정이 아니라 읽는 법 안내다.
    원격에 올라간 태그는 교정·리콜하지 않으며 개정판은 새 이름의 신규 발행으로만 나온다.

HINTS.md 는 **전량 생성물**이다 (plan §4.10 · 코드맵 §11)
    옛 1-96행 손산문은 사라진 태그 인용·"20+종"·없는 rtx5090 행·존재하지 않는 3세그먼트 태그 사용법을 들고 낡았다.
    이제 머리말 산문까지 이 모듈이 index 에서 만든다. 마커 쌍(`HINTS_MARKER`)은 기계 표 구역 경계로 계속 찍고,
    덮어쓰기 전에 기존 파일이 마커를 **정확히 2개** 가졌는지 본다(감사 ⑬ 침묵 파괴 방지 — 카탈로그가 아닌 파일을
    조용히 지우지 않는다).

파일 경로 단독 적재 가능 (코드맵 H5)
    `closing_sequence.sh` 는 이 **파일**을 `spec_from_file_location` 으로 적재해 `EXIT_LOCAL_TAG_OBJECT_MISSING` 을 읽고
    오류를 `2>/dev/null || true` 로 삼킨다. 모듈 최상위에 형제/패키지 상대 import 가 있으면 적재가 조용히 실패하고
    원인 분류가 사라진다(selftest_branch_sync S7 이 잡는다). 그래서 최상위는 stdlib 만 import 하고, `hintlib.core`·
    `hintlib.naming` 은 **함수 안에서** 지연 적재한다(`_hl`).
"""
from __future__ import annotations

import importlib
import json
import re
import sys
from pathlib import Path

# ── 공개 상수 ─────────────────────────────────────────────────────────────────────────────────
# 원격에만 있는 태그의 **로컬 태그 오브젝트 부재** 전용 종료코드(2026-09-14 · ⑧-pre D2 S7). 다른 실패(원격 조회 실패·
#   규약 밖 이름·lightweight 태그 = 기본 1)와 갈라야 호출부(종료 시퀀스 ⑤)가 **원인을 말하고** 사람에게 fetch 명령을
#   건넬 수 있다 — 종전에는 모든 실패가 같은 코드라 "원인은 위 출력" 한 줄로 뭉개졌다. 호출부는 이 상수를 **이 파일**에서
#   읽는다(손으로 다시 적지 않는다 · H5).
EXIT_LOCAL_TAG_OBJECT_MISSING = 4
# 중앙 권위 선언(plan_26082009 D8 권한 비대칭 · 역할 선언이지 자격증명이 아니다). **gitignored** 라 배포본에 실리지 않고,
#   각 저장소가 자기 원격에 대해 스스로 선언한다. 발행(봉인)은 분산, 색인·배포는 중앙집중이다.
#   값은 core.REL_CENTRAL_FLAG 와 같아야 한다(정적 파일 두 곳 → 자체검사 교차검증이 차선 · workflow.md 판정표).
CENTRAL_FLAG = "hints/.central_authority"
HINTS_MARKER = "<!-- hint-index:rows -->"
# 열은 한 곳에서만 정의한다 — 2026-09-07 실측: 렌더러가 두 벌(hint_tag.HINTS_COLUMNS · hint_catalog)이었고 헤더 5열에
#   10셀 행이 써져 HINTS.md 가 깨졌다. `문법` 열은 plan §4.7(구·신 문법 공존)의 신설.
#   `판정` 열(2026-09-29 · plan_26092908 §4.4 V1·V2): REFUTE 와 "인증서 없는 PASS" 가 같은 결손 코드로만 보였다.
CATALOG_COLUMNS = ("태그", "문법", "vLLM", "모델", "arch", "판정", "bench_mode", "결손", "brief")
INDEX_SCHEMA = 4            # 3 → 4: verdict·verdict_source 행 키(v7 · plan_26092908 §4.4) · base_model 은 v6·v7
#                             (2 → 3: grammar · base_model(v6) · object=absent-local|not-annotated 행(X15))
OBJECT_ABSENT_LOCAL = "absent-local"
OBJECT_NOT_ANNOTATED = "not-annotated"   # record_missing 모드의 annotated 아닌 원격 태그(본문 없음 · 2026-09-22)
BRIEF_UNRECEIVED = "—"      # 본문을 읽을 수 없는 행(로컬 오브젝트 부재 · annotation 없음)의 brief(합성 ✗ · X15)
# 태그 오브젝트는 있으나 결손을 **선언하지 않은** 행의 결손 칸(선언 0 `—` 와 가른다) — PAYLOAD.json 을 읽지 못했거나
#   PAYLOAD 에 `missing[]` 목록이 없다(2026-09-22 실측: 최초 형식 페이로드 2건 `gb10-sim-h100`·`gb10x2-sim-h100` 은 missing
#   키 자체가 없는데 옛 파생기는 이를 `—`(결손 0 선언)으로 적었다). 어느 쪽인지는 bench_mode_source 가 말한다.
MISSING_UNDECLARED = "미선언"

# ── bench_mode 파생 컬럼 (2026-09-14 · plan_26091407 §4.5 · 사용자 결정 Q10) ─────────────────────────────
# 등급은 태그 이름에 새기지 않는다(이름 불변). 카탈로그는 페이로드가 스스로 적은 `measurement_config` 에서 **결정론으로**
# 한 칸을 파생한다. 과거 태그에는 그 키가 없다 — 없는 것을 full 로도 lite 로도 접지 않고 `미기재` 로 **정직하게**
# 표시한다. 키는 있는데 값이 없으면(판정 기록을 못 읽은 측정) `미확정`. 로컬 오브젝트가 없어 읽지 못한 것은 `미수령`
# (부재와 결측의 구분 — "없음"과 "못 읽음"을 같은 칸으로 접지 않는다 · 2026-09-21 X15).
BENCH_MODE_ABSENT = "미기재"
BENCH_MODE_UNDETERMINED = "미확정"
BENCH_MODE_UNRECEIVED = "미수령"

# ── 판정 파생 컬럼 (2026-09-29 · plan_26092908 §4.4) ────────────────────────────────────────────────
# 출처 = 페이로드 `PAYLOAD.measurement.verdict`(evidence.measurement 가 판정 원천에서 채운다 — 인증서 유무 무관). 닫힌 어휘 밖
#   값은 접지 않고 원문에 `(어휘 밖)` 을 붙여 보인다. 칸이 없으면 `—`(v6 REFUTE 태그 D1·N1 은 measurement 에 verdict 가 없다 —
#   재판정 ✗ · P1), 오브젝트를 못 받았으면 `미수령`.
VERDICTS = ("PASS", "REFUTE", "OBSERVATION-ONLY")
VERDICT_ABSENT = "—"

# 열 설명(HINTS.md 머리말 생성용) — (뜻, 출처). 자체검사가 CATALOG_COLUMNS 와 키 집합 일치를 확인한다.
COLUMN_DOCS = {
    "태그": ("원격에 발행된 태그 이름 그대로(recipe 세그먼트 포함)", "원격 `git ls-remote`"),
    "문법": ("이름 문법 세대 — 판정이 아니라 **읽는 법** 안내", "`hintlib.naming.parse_tag`·`grammar_of`"),
    "vLLM": ("이름의 vLLM 세그먼트(v6·v7 = 빌드 입력: 릴리스 태그 또는 `<직전 릴리스>-g<sha12>`)", "태그 이름"),
    "모델": ("모델 슬러그(체크포인트 basename 소문자)", "태그 이름"),
    "arch": ("하드웨어·노드 형상", "태그 이름"),
    "판정": (f"`PASS` · `REFUTE` · `OBSERVATION-ONLY`(lite 전용 관측) · `{VERDICT_ABSENT}` = 페이로드에 판정 칸 없음(v6 이전·"
             f"판정 기록 전 태그 — 재판정 ✗) · `{BENCH_MODE_UNRECEIVED}` = 로컬 오브젝트 부재",
             "페이로드 `PAYLOAD.json` 의 `measurement.verdict`"),
    "bench_mode": (f"`full` · `lite(선언)` · `lite(강등·<사유>)` · `lite` · `{BENCH_MODE_UNDETERMINED}` · "
                   f"`{BENCH_MODE_ABSENT}`(측정 구성 기재 전 페이로드) · `{BENCH_MODE_UNRECEIVED}`(로컬 오브젝트 부재)",
                   "페이로드 `PAYLOAD.json` 의 `measurement_config`"),
    "결손": (f"페이로드가 **스스로 선언한** 결손 사유코드 · `—` = 선언 0 · `{BENCH_MODE_UNRECEIVED}` = 로컬 오브젝트 "
             f"부재로 읽지 못함 · `{MISSING_UNDECLARED}` = 태그는 있으나 결손을 선언하지 않음(`PAYLOAD.json` 을 읽지 못했거나 "
             "`missing[]` 목록이 없는 옛 형식 — 선언 0 과 다르다)",
             "페이로드 `PAYLOAD.json` 의 `missing[]`"),
    "brief": (f"v6·v7 = annotation 첫 문단 · 옛 문법 = annotation 의 첫 서술 줄 · `{BRIEF_UNRECEIVED}` = 본문 없음"
              f"(로컬 오브젝트 미수령 또는 annotation 없는 태그 · 합성 ✗)", "태그 오브젝트(annotation)"),
}

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent   # __file__ 기반 계산 = import 부수효과 아님
_KST_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$")
_REMOTE_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_HTML_COMMENT = re.compile(r"<!--.*?-->", re.S)
_OUT = "[hint catalog]"


# ── 지연 적재 (H5 — 모듈 최상위에 패키지 상대 import 금지) ────────────────────────────────────────
def _hl(name: str):
    """hintlib 형제 모듈을 **호출 시점에** 적재한다.

    패키지로 적재됐으면(`hintlib.catalog`) 같은 패키지에서, 파일 경로로 단독 적재됐으면 scripts/ 를 sys.path 에
    두고 `hintlib.<name>` 으로 적재한다. 어느 쪽이든 모듈 최상위는 stdlib 만 본다(closing_sequence 단독 적재 계약).
    """
    pkg = __package__ or "hintlib"
    if pkg not in sys.modules:          # 파일 경로 단독 적재 — 형제 패키지를 이 파일의 scripts/ 에서 찾는다
        scripts = str(_SCRIPTS_DIR)
        if scripts not in sys.path:
            sys.path.insert(0, scripts)
    return importlib.import_module(f"{pkg}.{name}")


def _core():
    return _hl("core")


def _naming():
    return _hl("naming")


def _fail(code: str, message: str, remedy: str | None = None, *, exit_code: int = 1):
    _core().fail(code, message, remedy, exit_code=exit_code)


# ── 순수 헬퍼 ────────────────────────────────────────────────────────────────────────────────
def extract_brief(body: str) -> str:
    """옛 문법 태그 본문에서 brief 한 줄(옛 규칙 그대로).

    ⑤ 의 기전을 여기서 닫는다 — 이전 `_brief_of` 는 *"첫 비어있지 않은 줄"* 이라는 **위치 규약**만 봤고, 템플릿
    맨 위에 경고 HTML 주석이 추가되자 그 경고문이 brief 가 됐다(실측 4건). 위치가 아니라 **모양**으로 거른다:
    HTML 주석·인용부호·헤딩·표·목록·코드펜스는 brief 가 아니다. `cat-file -p` 헤더가 섞여 들어오던 경로(실측 7건
    오염)도 모양으로 거른다.
    """
    text = _HTML_COMMENT.sub("", body)
    for raw in text.split("\n"):
        line = raw.strip()
        if not line:
            continue
        if line.startswith(("<!--", ">", "#", "|", "-", "*", "```")):
            continue
        if line.startswith(("object ", "type ", "tag ")):
            continue
        return line
    return ""


def annotation_brief(body: str) -> str:
    """v6 태그 annotation 의 **첫 문단**(SPEC §2.2 — brief 는 00-hint §0.1 요약 첫 문단 ≤3줄의 기계 추출).

    annotation = brief 문단 · 빈 줄 · zip 포인터 문단 · 빈 줄 · 증거 footer(HTML 주석). 주석을 먼저 걷고 첫 비어있지
    않은 문단의 줄들을 공백 하나로 잇는다. 위치 덫(⑤)은 v6 에서 구조로 사라졌다 — brief 가 봉인 시 기계 추출되어
    annotation 맨 앞에 선다.
    """
    text = _HTML_COMMENT.sub("", body).strip()
    for para in re.split(r"\n[ \t]*\n", text):
        lines = [ln.strip() for ln in para.splitlines() if ln.strip()]
        if lines:
            return " ".join(lines)
    return ""


def md_cell(s) -> str:
    """마크다운 표 셀로 안전하게(⑤ 사슬 4단계: 이스케이프가 없어 카탈로그가 깨졌다). 파이프 이스케이프 ·
    줄바꿈 평탄화 · **HTML 주석 무해화**(미닫힘 `<!--` 가 뒤 행 전부를 삼키던 경로)."""
    s = "" if s is None else str(s)
    s = s.replace("|", "\\|").replace("\n", " ").replace("\r", " ")
    s = _HTML_COMMENT.sub("", s).replace("<!--", "&lt;!--").replace("-->", "--&gt;")
    return " ".join(s.split())


def bench_mode_cell(doc: dict | None) -> tuple[str, str]:
    """PAYLOAD.json → (카탈로그 칸, 출처). 렌더러가 하나이므로 소비자도 하나다(옛 hint_tag `_catalog_module()` 적재 폐지)."""
    if not isinstance(doc, dict):
        # 부재와 결측의 구분(docs.md) — 페이로드 자체를 읽지 못한 것과 '측정 구성 기재 전 페이로드'를 같은 출처로 접지 않는다.
        return BENCH_MODE_ABSENT, "absent(PAYLOAD.json 을 읽지 못함 — 페이로드가 없는 태그)"
    mc = doc.get("measurement_config")
    if not isinstance(mc, dict):
        return BENCH_MODE_ABSENT, "absent(PAYLOAD.measurement_config 없음 — 측정 구성 기재 신설 전 페이로드)"
    mode, kind, reason = mc.get("bench_mode"), mc.get("bench_mode_kind"), mc.get("downgrade_reason")
    if mode == "full":
        cell = "full"
    elif mode == "lite" and reason:
        cell = f"lite(강등·{reason})"
    elif mode == "lite" and kind == "declared-lite":
        cell = "lite(선언)"
    elif mode == "lite":
        cell = "lite"
    else:
        cell = BENCH_MODE_UNDETERMINED
    return cell, f"payload(PAYLOAD.measurement_config · {mc.get('source') or '출처 미표시'})"


def verdict_cell(doc: dict | None) -> tuple[str, str]:
    """PAYLOAD.json → (판정 칸, 출처). 판정은 페이로드가 스스로 적은 `measurement.verdict` 하나에서만 읽는다(재판정 ✗ ·
    카탈로그는 비권위 캐시 · plan_26092908 §4.4). v6 페이로드도 칸이 있으면 그대로 보인다(DS4F = PASS)."""
    if not isinstance(doc, dict):
        return VERDICT_ABSENT, "absent(PAYLOAD.json 을 읽지 못함)"
    m = doc.get("measurement")
    v = m.get("verdict") if isinstance(m, dict) else None
    if not isinstance(v, str) or not v.strip():
        return VERDICT_ABSENT, "absent(PAYLOAD.measurement.verdict 없음 — 판정 기록 전 페이로드)"
    v = v.strip()
    per_key = m.get("sources") if isinstance(m.get("sources"), dict) else {}
    origin = per_key.get("verdict") or m.get("source") or "출처 미표시"
    src = f"payload(PAYLOAD.measurement.verdict · {origin})"
    return (v if v in VERDICTS else f"{v}(어휘 밖)"), src


def query_slug(s: str | None) -> str:
    """수신자 검색 키 = `naming.norm_slug`(정규화기 단일 소유 · 2026-08-20 `gemma-4-e2b-it`↔`gemma-4-E2B-it` 철자
    갈림으로 match 가 정확일치 태그를 '다른 모델'로 판정한 사건) 를 **basename** 에 적용한 것 — `Org/Name` 으로 물어도
    된다. 값이 없으면 빈 문자열(관계 없음 · `None` 을 "none" 슬러그로 접지 않는다)."""
    base = (s or "").strip().rstrip("/").rsplit("/", 1)[-1] if isinstance(s, str) else ""
    return _naming().norm_slug(base) if base else ""


def _remote_label(remote: str) -> str:
    """추적·배포 파일(index.json · HINTS.md)에 적을 원격 표기. 원격 **이름**(`github-ssh`·`origin`)만 그대로 적고
    경로·URL 은 적지 않는다 — 운영자 절대경로·호스트가 배포 평면으로 새면 PII(abs-op-path) 다(docs.md §PII:
    추적 템플릿은 4종 전부). 격리 자체검사는 원격을 경로로 넘기므로 이 경로가 실제로 쓰인다."""
    return remote if _REMOTE_NAME_RE.match(remote or "") else "<경로·URL 원격 — 배포 파일에 적지 않는다>"


# ── git 관측 ─────────────────────────────────────────────────────────────────────────────────
def remote_hint_tags(repo: Path, remote: str, *, allow_empty: bool = False) -> dict[str, dict]:
    """**카탈로그의 진실원천.** `{태그: {"object": <원격 광고 오브젝트 SHA>, "peeled": <커밋 SHA|None>}}`.

    조회 실패(rc≠0) = 중단 — '0건'이 아니라 '모른다'이고, 모르는 것을 0 으로 적으면 침묵 폴백이다.
    `allow_empty` 는 **0건을 정상으로 선언**하는 스위치(기본 False = fail-closed): 태그 전량 폐기 직후처럼 '0건이 사실'인
    상태가 실재하며, 그때 게이트가 열리지 않으면 카탈로그가 낡은 행을 영원히 든다. 조회 **실패**는 이 스위치로도 열리지 않는다.
    원격 인자가 `-` 로 시작하면 거부한다 — `git ls-remote` 는 그것을 옵션(`--upload-pack=<명령>` 등)으로 읽는다.
    """
    core = _core()
    if not isinstance(remote, str) or not remote.strip() or remote.startswith("-"):
        _fail("HINT_CATALOG_REMOTE_INVALID", f"원격 인자가 원격 이름·경로·URL 이 아니다: {remote!r}",
              "`git remote -v` 의 이름(예 origin · github-ssh)을 넘긴다(`-` 로 시작하는 값은 git 옵션으로 해석된다).")
    p = core.git(repo, "ls-remote", "--tags", remote, "refs/tags/hint/*", check=False)
    if p.returncode != 0:
        _fail("HINT_CATALOG_REMOTE_QUERY_FAILED",
              f"원격 태그 조회 실패({remote}) — 캐시로 대체하지 않는다(D1.1 fail-closed): {p.stderr.strip()}",
              "원격 이름·네트워크·자격을 확인하고 다시 실행한다. 낡은 카탈로그는 '기록이 원래 없었던 것'과 구분되지 않는다.")
    tags: dict[str, dict] = {}
    for line in p.stdout.splitlines():
        if "\t" not in line:
            continue
        sha, ref = (x.strip() for x in line.split("\t", 1))
        if not ref.startswith("refs/tags/"):
            continue
        name = ref[len("refs/tags/"):]
        if name.endswith("^{}"):
            tags.setdefault(name[:-3], {})["peeled"] = sha
        else:
            tags.setdefault(name, {})["object"] = sha
    out = {}
    for name in sorted(tags):
        rec = tags[name]
        if "object" not in rec:           # 피일 줄만 있고 오브젝트 줄이 없는 광고는 ls-remote 계약 밖
            _fail("HINT_CATALOG_REMOTE_QUERY_FAILED", f"원격 광고가 불완전하다(오브젝트 줄 없음): {name}")
        out[name] = {"object": rec["object"], "peeled": rec.get("peeled")}
    if not out and not allow_empty:
        _fail("HINT_CATALOG_REMOTE_EMPTY",
              f"원격 {remote} 에 hint 태그가 0건이다 — 빈 카탈로그를 발행하지 않는다.",
              "정말 0건이 맞다면(태그 전량 폐기 직후) allow_empty 로 명시한다.")
    return out


def payload_doc(repo: Path, obj: str) -> dict | None:
    """태그 오브젝트가 가리키는 페이로드 커밋의 `PAYLOAD.json`(배관 읽기 · 체크아웃 ✗). 없거나 못 읽으면 None.

    카탈로그는 **비권위 캐시**다 — 부재(낡은 태그·페이로드 없음)를 여기서 fail-closed 하면 발행 사실이 아니라
    카탈로그 갱신이 막힌다. 판정의 권위는 봉인 전 로컬 검증(`hint.py verify` · 이 태그 1개 · D10)이고 여기는 표시다.
    옛 파생기는 태그당 cat-file 을 2회 불렀다(payload_doc·payload_missing) — 한 번 읽어 나눠 쓴다.
    """
    raw = _core().git_bytes(repo, "cat-file", "-p", f"{obj}^{{}}:PAYLOAD.json", check=False)
    if raw is None:
        return None
    try:
        doc = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        return None
    return doc if isinstance(doc, dict) else None


def main_worktree(repo: Path) -> Path:
    """`repo` 가 속한 저장소의 **주 워크트리** 경로 — 안내 명령에 찍을 자리.

    왜(2026-09-14 ⑧-pre D2 리뷰): 종료 시퀀스 ⑤ 는 파생기를 **임시 워크트리**(`<repo>.wt-<반대>`)에서 돌리고 곧바로
    지운다. 안내가 `git -C <임시 워크트리>` 를 찍으면 사람이 읽는 순간 그 경로는 없다. 태그 ref 는 워크트리가 아니라
    저장소 공용이므로 주 워크트리에서 fetch 해도 같다. 찾지 못하면 `repo` 를 그대로 쓴다(안내 문자열일 뿐 판정이 아니다).
    """
    listing = _core().git(repo, "worktree", "list", "--porcelain", check=False)
    first = listing.stdout.splitlines()[0] if listing.returncode == 0 and listing.stdout else ""
    return Path(first[len("worktree "):]) if first.startswith("worktree ") else repo


def fetch_hint(repo: Path, remote: str | None) -> str:
    """로컬 오브젝트 부재 해소 명령. **조회한 그 원격**을 가리킨다 — 기본 원격을 보는 전량 태그 fetch 는 `--remote` 가
    다른 이름이면 엉뚱한 곳을 긁고 같은 실패를 되풀이한다(2026-09-14 실측: 원격 전용 태그 10건)."""
    if not remote:
        return "git fetch <원격> 'refs/tags/hint/*:refs/tags/hint/*'"
    return f"git -C {main_worktree(repo)} fetch {remote} 'refs/tags/hint/*:refs/tags/hint/*'"


# ── 파생 ─────────────────────────────────────────────────────────────────────────────────────
def parse_name(name: str) -> dict:
    """원격 태그 이름 → `{grammar, vllm, model, arch, recipe}`(읽기 전용 · **예외 없음**). 이름 해체는 naming 이 단일
    소유한다(2026-09-04 CP7: 5세그먼트 전환 때 옛 파생기가 개정 표면 계획에서 빠져 있었다 — 파서가 흩어져 있으면
    같은 누락이 재발한다).

    세대 전부를 **나열용으로 수용**한다(plan §4.7 "옛 4·5세그먼트 태그를 읽기 전용으로 수용" · P1 · D10). 옛 파생기는
    규약 밖 이름 1건으로 카탈로그 전체를 중단했다 — 그러나 원격 태그는 리콜할 수 없으므로 그 중단은 **영구**가 된다.
    5세그먼트 = naming.parse_tag · 4세그먼트 = `legacy-4seg`(recipe 없음) · 그 밖 = `unknown`(세그먼트를 추측해 채우지
    않는다 · 합성 ✗).
    """
    core, naming = _core(), _naming()
    try:
        tn = naming.parse_tag(name)
        return {"grammar": tn.grammar, "vllm": tn.vllm, "model": tn.model, "arch": tn.arch, "recipe": tn.recipe}
    except core.HintError:
        grammar = naming.grammar_of(name)
    parts = name.split("/")
    if grammar == naming.GRAMMAR_LEGACY_4SEG:
        return {"grammar": grammar, "vllm": parts[1], "model": parts[2], "arch": parts[3], "recipe": None}
    return {"grammar": grammar, "vllm": None, "model": None, "arch": None, "recipe": None}


def _payload_grammars() -> tuple[str, ...]:
    """annotation 첫 문단 brief · base_model 을 읽는 세대(= 페이로드 커밋 + 정본 annotation 형식) — v6·v7."""
    n = _naming()
    return (n.GRAMMAR_V7, n.GRAMMAR_V6)


def _base_row(name: str, parsed: dict, payload_grammars: tuple[str, ...]) -> dict:
    row = {"tag": name, **parsed,
           "published": True,           # 원격에 있다 = 발행됐다. 이것이 유일한 근거다.
           "source": "remote-derived"}  # 출처 표시(헌법 §결정론 규율)
    if parsed["grammar"] in payload_grammars:
        row["base_model"] = None
    return row


def derive_entries(repo: Path, remote_tags: dict[str, dict], *, remote: str | None = None,
                   record_missing: bool = False) -> tuple[list[dict], dict]:
    """원격 광고 → index 항목(태그 이름순). 반환 `(entries, notes)` ·
    notes = {"absent_local":[…], "not_annotated":[…], "local_ref_differs":[…]}.

    - 이름 해체(세대 전부 읽기 전용 수용 · 중단 ✗) → 로컬 오브젝트 존재 확인(원격 SHA 기준 · K5) → 부재가 있고
      record_missing 이 아니면 **아무것도 쓰기 전에** rc 4 로 중단.
    - annotated 가 아닌 태그(lightweight 등) = 기본 모드는 중단(본문이 없으면 brief 를 지어낼 수 없다) · record_missing
      모드는 `object: not-annotated` 행(brief `—`)으로 싣는다(D10 — 모듈 docstring).
    - `anchor` = 태그가 가리키는 **페이로드 커밋**(v6 footer 의 `anchor:` 와 같은 뜻 · 코드맵 K7: PROVENANCE 의
      source_anchor 와 다른 층이다). `object` = 태그 오브젝트 SHA(원격 광고값) 또는 `absent-local` · `not-annotated`
      (그 행의 anchor = 원격이 광고한 오브젝트 그 자체).
    """
    core = _core()
    pay_g = _payload_grammars()
    parsed = {name: parse_name(name) for name in remote_tags}
    absent: list[str] = []
    for name, rec in remote_tags.items():
        if core.git(repo, "cat-file", "-e", rec["object"], check=False).returncode != 0:
            absent.append(name)
    if absent and not record_missing:
        _fail("HINT_CATALOG_LOCAL_OBJECT_MISSING",
              "원격에 있으나 **로컬에 태그 오브젝트가 없다** — brief 를 지어낼 수 없다(합성 금지):\n  "
              + "\n  ".join(absent[:10]) + f"\n  (총 {len(absent)}건)",
              f"`{fetch_hint(repo, remote)}` 후 다시 실행하라(자동 fetch 하지 않는다). 부재를 행으로 기재하고 "
              "진행하려면 record-missing 모드(object: absent-local · brief '—')로 파생한다.",
              exit_code=EXIT_LOCAL_TAG_OBJECT_MISSING)
    absent_set = set(absent)
    entries: list[dict] = []
    differs: list[str] = []
    not_annotated: list[str] = []
    for name, rec in remote_tags.items():
        pn = parsed[name]
        row = _base_row(name, pn, pay_g)
        if name in absent_set:
            # anchor = 원격이 광고한 피일 커밋. 피일 줄이 없으면 원격 ref 가 태그 오브젝트가 아닌 것(lightweight)을 직접
            #   가리킨다는 광고이므로 그 오브젝트가 곧 대상이다(ls-remote 는 annotated 태그에만 `^{}` 줄을 낸다).
            row.update({"brief": BRIEF_UNRECEIVED, "anchor": rec.get("peeled") or rec["object"],
                        "object": OBJECT_ABSENT_LOCAL,
                        # 모름(None)은 결손 0([])과 다르다 — 부재와 결측을 같은 값으로 접지 않는다.
                        "missing": None, "bench_mode": BENCH_MODE_UNRECEIVED,
                        "bench_mode_source": "absent-local(로컬 태그 오브젝트 부재 — 조회한 원격에서 fetch 전 · 합성 ✗)",
                        "verdict": BENCH_MODE_UNRECEIVED,
                        "verdict_source": "absent-local(로컬 태그 오브젝트 부재 · 합성 ✗)"})
            entries.append(row)
            continue
        obj = rec["object"]
        otype = core.git_out(repo, "cat-file", "-t", obj)
        if otype != "tag" and not record_missing:
            _fail("HINT_CATALOG_NOT_ANNOTATED",
                  f"{name} 가 annotated 태그가 아니다({otype}) — 본문이 없으면 brief 를 지어낼 수 없다.",
                  "원격 태그는 교정하지 않는다(P1). 발행 경로 밖에서 만든 태그다 — 그 원격의 발행자에게 알린다. "
                  "행으로 기재하고 진행하려면 record-missing 모드(object: not-annotated · brief '—')로 파생한다.")
        if otype == "tag":
            # 바이트로 읽고 UTF-8 로 푼다 — 본문(한국어)의 해석이 실행 로캘에 따라 달라지지 않게(결정론).
            raw = core.git_bytes(repo, "cat-file", "tag", obj).decode("utf-8", "replace")
            body = raw.split("\n\n", 1)[1] if "\n\n" in raw else ""
            anchor = rec.get("peeled") or core.git_out(repo, "rev-parse", f"{obj}^{{}}")
            # v6·v7 brief = annotation 첫 문단 · 옛 문법 = 모양 규칙(⑤) — 문법으로 분기(코드맵 O-cat2 · v7 도 같은 annotation 형식).
            brief = annotation_brief(body) if pn["grammar"] in pay_g else extract_brief(body)
            obj_cell = obj
        else:                                   # record_missing 모드의 not-annotated 행(본문 없음 · 합성 ✗)
            not_annotated.append(name)
            anchor, brief, obj_cell = obj, BRIEF_UNRECEIVED, OBJECT_NOT_ANNOTATED
        doc = payload_doc(repo, obj)
        bm, bm_src = bench_mode_cell(doc)
        vd, vd_src = verdict_cell(doc)
        row.update({
            "brief": brief,
            "anchor": anchor,
            "object": obj_cell,
            # 결손은 **파생 컬럼**이다(2026-09-07 · 인터뷰 Q5). 이름에 등급을 새기지 않는다 — 이름은 불변인데 결손은
            # 재발행으로 바뀔 수 있고, 이름에 새기면 그 순간 이름이 거짓이 된다. 출처는 페이로드 자기 선언 하나다.
            # PAYLOAD.json 을 읽지 못했거나 `missing[]` **목록**이 없으면 None(선언 0 `[]` 과 다르다 · 부재와 결측의 구분 ·
            # 2026-09-22) — 판정이 아니라 표시다(카탈로그는 비권위 캐시 · 여기서 중단하지 않는다).
            "missing": (sorted(m for m in doc["missing"] if isinstance(m, str))
                        if isinstance(doc, dict) and isinstance(doc.get("missing"), list) else None),
            "bench_mode": bm,
            "bench_mode_source": bm_src,
            "verdict": vd,
            "verdict_source": vd_src,
        })
        if pn["grammar"] in pay_g:
            # base_model = PAYLOAD.identity.base_model(v6 만 · X14). families.json(수동 판정 19/19 · 5세그먼트에서 0)을
            # 대체한다 — match 가 git 없이 관계를 찾으려면 index 에 실려야 한다(코드맵 hint_tag_b §1.9).
            ident = doc.get("identity") if isinstance(doc, dict) else None
            bmodel = ident.get("base_model") if isinstance(ident, dict) else None
            row["base_model"] = bmodel if isinstance(bmodel, str) and bmodel.strip() else None
        local = core.git(repo, "rev-parse", "--verify", "--quiet", f"refs/tags/{name}", check=False).stdout.strip()
        if local and local != obj:
            differs.append(name)
        entries.append(row)
    return entries, {"absent_local": absent, "not_annotated": not_annotated, "local_ref_differs": differs}


def build_index(entries: list[dict], *, remote: str, generated_kst: str) -> dict:
    return {
        "_note": ("카탈로그는 **원격 발행 태그에서 파생**한다(plan_26090107 D1.1 · plan_26092119). 손저작 금지 — 이 파일을 "
                  "직접 편집하면 다음 derive 에서 사라진다. 진실원천은 원격의 `refs/tags/hint/*` 광고이고, brief·결손·"
                  "bench_mode 는 원격이 광고한 태그 오브젝트(로컬 오브젝트 DB)에서 파싱한다(합성 금지). "
                  f"`object: {OBJECT_ABSENT_LOCAL}` 행은 로컬에 그 오브젝트가 없어 읽지 못한 원격 태그 · "
                  f"`object: {OBJECT_NOT_ANNOTATED}` 행은 annotation 없는 원격 태그다(둘 다 brief 합성 ✗)."),
        "schema": INDEX_SCHEMA,
        "source": "remote-derived",
        "remote": _remote_label(remote),
        "generated_kst": generated_kst,
        "count": len(entries),
        "hints": entries,
    }


def render_rows(entries: list[dict]) -> str:
    """카탈로그 표(헤더 + 구분 + 행). `결손` 열(2026-09-07): "무엇을 모른 채 발행됐는가" 가 카탈로그에서 보여야 비교할 수
    있다. `bench_mode` 열(2026-09-14): 등급은 이름이 아니라 이 파생 컬럼이 말한다. `문법` 열(2026-09-21 · plan §4.7)."""
    rows = ["| " + " | ".join(CATALOG_COLUMNS) + " |", "|" + "|".join("---" for _ in CATALOG_COLUMNS) + "|"]
    for e in entries:
        miss = e.get("missing")
        if miss is None:
            # 모름은 두 갈래다 — 오브젝트를 못 받은 것(미수령)과 받았지만 결손 선언이 없는 것. 선언 0(`—`)으로 접지 않는다.
            miss_cell = BENCH_MODE_UNRECEIVED if e.get("object") == OBJECT_ABSENT_LOCAL else MISSING_UNDECLARED
        elif not miss:
            miss_cell = "—"
        else:
            miss_cell = md_cell(" · ".join(miss))
        rows.append("| " + " | ".join((
            f"`{md_cell(e.get('tag'))}`", md_cell(e.get("grammar") or "?"), md_cell(e.get("vllm")),
            md_cell(e.get("model")), md_cell(e.get("arch")), md_cell(e.get("verdict") or VERDICT_ABSENT),
            md_cell(e.get("bench_mode") or BENCH_MODE_ABSENT),
            miss_cell, md_cell(e.get("brief")))) + " |")
    return "\n".join(rows)


def _grammar_order() -> tuple[str, ...]:
    n = _naming()
    return (n.GRAMMAR_V7, n.GRAMMAR_V6, n.GRAMMAR_LEGACY_ARCH_NODE, n.GRAMMAR_LEGACY_ARCH)


def render_hints_md(index: dict) -> str:
    """HINTS.md **전량**(머리말 산문 포함)을 index 에서 생성한다(plan §4.10 · 손산문 ✗).

    산문의 사실 부분(개수·세대별 건수·예시 태그·arch 분포·열 설명)은 전부 index 와 이 모듈의 상수에서 온다 — 낡을 수 있는
    손 서술을 두지 않는다(옛 머리말: 사라진 태그 인용 · "20+종" · 카탈로그에 0건인 발행처 행 · 존재하지 않는 3세그먼트 태그
    사용법 · 존재하지 않는 열 설명). 운영자 경로·호스트는 적지 않는다(추적 배포 파일 · 4종 PII).
    """
    core, naming = _core(), _naming()
    entries = list(index.get("hints") or [])
    g_v7, g_v6, g_node, g_flat = _grammar_order()
    by_g: dict[str, list[dict]] = {}
    for e in entries:
        by_g.setdefault(e.get("grammar") or "?", []).append(e)
    absent = [e for e in entries if e.get("object") == OBJECT_ABSENT_LOCAL]
    unannotated = [e for e in entries if e.get("object") == OBJECT_NOT_ANNOTATED]
    cli = core.REL_HINT_CLI

    def example(g: str) -> str:
        rows = by_g.get(g) or []
        return f"`{md_cell(rows[0]['tag'])}`" if rows else "— (이 카탈로그에 없음)"

    counts = " · ".join(f"`{g}` {len(by_g.get(g, []))}" for g in (g_v7, g_v6, g_node, g_flat))
    others = sorted(g for g in by_g if g not in (g_v7, g_v6, g_node, g_flat))
    if others:
        counts += " · " + " · ".join(f"`{g}` {len(by_g[g])}" for g in others)
    arch_count: dict[tuple[str, str], int] = {}
    for e in entries:
        k = (e.get("arch") or "?", e.get("grammar") or "?")
        arch_count[k] = arch_count.get(k, 0) + 1
    arch_rows = "\n".join(f"| `{md_cell(a)}` | `{md_cell(g)}` | {n} |"
                          for (a, g), n in sorted(arch_count.items(), key=lambda kv: (-kv[1], kv[0])))
    extra_doc = {naming.GRAMMAR_LEGACY_4SEG: ("`hint/<vllm>/<model>/<arch>` 네 세그먼트(recipe 없음)", "—"),
                 naming.GRAMMAR_UNKNOWN: ("어느 세대 모양에도 맞지 않는 이름(다른 발행처 등 · 세그먼트를 추측해 채우지 "
                                          "않는다)", "—")}
    extra_grammar_rows = [f"| `{g}` | {extra_doc[g][0]} | {extra_doc[g][1]} | {example(g)} |"
                          for g in (naming.GRAMMAR_LEGACY_4SEG, naming.GRAMMAR_UNKNOWN) if by_g.get(g)]
    col_rows = "\n".join(f"| {c} | {COLUMN_DOCS[c][0]} | {COLUMN_DOCS[c][1]} |" for c in CATALOG_COLUMNS)
    absent_line = ((f" · 로컬 오브젝트 부재(미수령) {len(absent)}건" if absent else "")
                   + (f" · annotation 없는 태그 {len(unannotated)}건" if unannotated else ""))

    parts = [
        "<!-- 이 파일 **전체**가 생성물이다 — `hint.py catalog derive` 가 원격 발행 태그에서 통째로 다시 만든다"
        "(plan_26092119 §4.10). 손으로 고치지 마라: 다음 derive 에서 사라진다. 진실원천 = 원격의 refs/tags/hint/* 광고 ·"
        " 색인 = hints/index.json(같은 derive 가 쓴다). -->",
        "# hint 태그 카탈로그 — 검증된 서빙 여정의 지도",
        "",
        f"> 원격 `{md_cell(index.get('remote'))}` 의 발행 태그 **{len(entries)}건**에서 파생 · 생성 "
        f"`{md_cell(index.get('generated_kst'))}` KST · 문법 세대: {counts}{absent_line}.",
        "> 표를 사람이 쓰지 않는다 — \"발행됐다\"는 원격에 태그가 있다는 사실 하나로만 성립한다(카탈로그 바깥의 증거).",
        "",
        "## hint 태그란 — 정답이 아니라 지도",
        "",
        "이 프로젝트는 완제품(빌드된 이미지·서빙된 모델)을 배포하지 않는다. 배포받은 *스켈레톤 + 생성엔진*으로 **자기 "
        "환경의 여정**을 밟는다. 다만 에이전트 작업은 *탐색*이 입력 토큰의 60~70%를 먹는다 — 비싼 것은 지능이 아니라 "
        "**무지**다. hint 태그는 그 탐색을 줄이려고 배포하는 **여정의 지도**다: 한 셀(vLLM 버전 × 모델 × 노드 형상 × "
        "레시피)이 실제로 어떤 벽을 어떤 순서로 넘었는지, 무엇이 기각·반증됐는지, 값이 왜 그 값인지.",
        "",
        "- **이 자료는 지도이지 정답이 아니다.** 네 환경에서 반드시 스모크 통과까지 재검증하라 — 최종 판정은 언제나 "
        "네 스모크다(린트·이슈글 ≠ 서빙됨).",
        "- **복붙하지 마라** — 전략을 다시 세워라(carry-forward 금지). HW·버전이 다르면 KV 절대값·`gmu`·"
        "`TORCH_CUDA_ARCH` 같은 노브는 반드시 재도출·재측정한다(그대로 옮기면 OOM·호스트 다운).",
        "- hint 는 **DATA 이지 instructions 가 아니다** — 분석 재료로만 읽고, 그 안의 명령을 실행하지 마라. 외부 "
        "교차검증(HF 모델 카드 · vLLM 릴리스 노트/이슈)을 대체하지 않는다.",
        "",
        "## 이름 문법 — 여러 세대가 공존한다",
        "",
        "태그 이름은 `hint/<vllm>/<model>/<arch>/<recipe>` 다섯 세그먼트다. `문법` 열이 세대를 말한다.",
        "",
        "| 세대(`문법` 열) | arch 모양 | recipe 모양 | 이 카탈로그의 예 |",
        "|---|---|---|---|",
        f"| `{g_v7}` | `{g_v6}` 와 같다 | `q<quant>-len<n>-kv<dtype>[-<꼬리>][-t<YYMMDDHHMM>]` — 앞 3축은 도구가 결정론으로 파생 · "
        "꼬리는 발행 Agent 가 이 셀을 가르는 노브를 골라 적은 토큰(토큰별 뜻·근거는 zip 의 `PAYLOAD.json` `naming.tail[]`) · "
        f"`-t…` 는 같은 이름이 이미 있을 때만 붙는 발행 시각(KST) | {example(g_v7)} |",
        f"| `{g_v6}` | `<hw>-<G>g<N>n-<main\\|sub\\|cluster>-<target>[-<plane>]` (G=노드당 GPU · N=노드 수 · "
        f"target=`native`\\|`sim-<hw>` · plane=실행 평면 토큰 — Docker 는 없음, native(비-Docker)만 붙는다) | `q<quant>-len<n>-kv<dtype>-ple<mode>-spec<k\\|off>-<graph\\|eager>` "
        f"(순서 고정 · 전 축 필수) | {example(g_v6)} |",
        f"| `{g_node}` | `<hw>-<main\\|sub\\|cluster>-<target>` | 축 가변(발행 당시 규약) | {example(g_node)} |",
        f"| `{g_flat}` | `<hw>-<target>` (노드축 없음) | 축 가변 | {example(g_flat)} |",
        *extra_grammar_rows,
        "",
        f"- `{g_v7}` 이름의 **꼬리는 순위·등급이 아니다** — 같은 3축의 다른 셀과 무엇이 다른지 적은 표지이고, 토큰마다 "
        "서빙 설정의 파일·키·값 근거가 페이로드에 실려 있다(근거 없는 꼬리는 발행되지 않는다). 판정은 `판정` 열이 말한다.",
        f"- `{g_v6}` 이름은 **도구가 셀 증거에서 전량 파생**한다(발행자 입력 ✗). vLLM 세그먼트는 **빌드 입력**이다 — "
        "릴리스 태그로 빌드했으면 그 버전, 커밋에 핀했으면 `<직전 릴리스>-g<sha12>`. 엔진 자기보고 버전은 `00-hint.md` "
        "사실 블록에 따로 적힌다. 셀 1개 = 태그 1개.",
        "- 옛 세대의 hw 토큰 `x2` 는 두 뜻으로 쓰였다 — 한 노드의 GPU 2장(예: `rtxpro6000x2`)과 노드 2대(예: `gb10x2`). "
        f"`{g_v6}` 는 `<G>g<N>n` 으로 둘을 가른다.",
        "- **옛 세대 행은 그대로 나열한다.** 원격에 올라간 태그는 교정·리콜하지 않는다 — 개정판은 새 이름의 **신규 "
        "발행**으로만 나온다. `문법` 열은 판정이 아니라 읽는 법 안내다.",
        "",
        "## 태그 받아 읽기",
        "",
        "```bash",
        "# 1) 무엇이 있나 — 원격 조회만(아무것도 받지 않는다)",
        "git ls-remote <원격> 'refs/tags/hint/*'",
        "# 2) 고른 태그 하나만 받는다",
        "git fetch <원격> 'refs/tags/<태그>:refs/tags/<태그>'",
        "# 3) 태그 = zip(재현 키트) — seed/ 아래에 푼다",
        "mkdir -p seed/hints/<이름>",
        "git archive --format=zip -o seed/hints/<이름>.zip <태그>",
        "unzip -q seed/hints/<이름>.zip -d seed/hints/<이름>",
        "```",
        "",
        "GitHub 원격이라면 그 태그의 \"Source code (zip)\" 로 git 없이 같은 트리를 받는다(최상위에 `<저장소>-<태그>` 폴더가 "
        "한 겹 더 붙는다 — 그 안에서 `00-hint.md` 부터 읽는다).",
        "",
        f"- **`{g_v7}`·`{g_v6}` 태그** — zip 안에 전부 있다. 읽는 순서: `00-hint.md`(지도 · **가장 먼저**) → `01-artifacts.md`"
        "(적용 판정·값의 지위·재현 절차) → `02-narrative.md`(계보 서사 · 벽 순서만 필요하면 `hint-event` 코드블록 중 "
        "`kind: wall` 만 grep) → `03-benchmark.md`(측정 · like-with-like 한정자) → `PAYLOAD.json`·`LINEAGE.json`·"
        "`PROVENANCE.json`(기계 사실) → `artifacts/`(**실제로 쓰인 것만**). annotation 에는 brief·포인터·증거 footer 뿐이다.",
        "- **옛 세대 태그** — 지도(본문)는 annotation 안에 있다: `git tag -l --format='%(contents)' <태그>` "
        "(`git show <태그>` 는 커밋 diff 까지 딸려 온다). 옛 태그 중에는 페이로드 커밋이 아닌 커밋을 가리키는 것이 있을 "
        "수 있으니 archive 전에 `git ls-tree --name-only <태그>` 로 최상위를 확인한다.",
        "- **왜 `seed/` 냐면** — (a) 추적 트리(스켈레톤 + 생성엔진)가 그대로 남는다(HEAD 에 남의 정답 파일이 박히지 "
        "않는다 — 여정의 순수성), (b) `seed/` 는 이 프로젝트에서 에이전트가 부트스트랩·참조 자료로 읽는 비추적 자리라 "
        "자연스럽게 그라운딩된다. 코드에이전트에게는 이렇게 건넨다: *\"`seed/hints/<이름>/00-hint.md` 부터 읽고 "
        "`vllm-recipe-explorer` 인터뷰의 warm-start 근거로 넣어, 평소대로 plan → 레시피 수렴 → 스모크 게이트를 밟아 "
        "전략을 **다시 세워라**.\"*",
        "",
        "## 가까운 태그 찾기 — `match`",
        "",
        "```bash",
        f"python3 {cli} match --vllm <V> --model <M> [--arch <A>] [--include-other] [--json]",
        "```",
        "",
        "- git 없이 `hints/index.json` 만 읽는다(배포 아카이브에서도 동작).",
        "- 모델 비교는 **정규화 슬러그 동치**(대소문자·구두점 무시 — `gemma-4-E2B-it` = `gemma-4-e2b-it` · `Org/Name` "
        f"으로 물어도 된다) + **base_model 관계**(`{g_v7}`·`{g_v6}` 태그만 · 페이로드 `PAYLOAD.identity.base_model` 에서 파생)다 — "
        "같은 기반 모델의 양자화 변종·원본을 함께 찾는다.",
        "- 관계없는 모델은 기본으로 숨긴다(`--include-other` 로 본다). vLLM·arch 가 다르면 무엇을 다시 확인해야 하는지 "
        "행마다 안내한다.",
        "- **관계 판정은 도구가 하지 않는다.** 어떤 태그가 패턴이고 어떤 것이 안티패턴인지는 한 태그만 봐서는 알 수 "
        "없다 — 발행 시점엔 그게 최선이었지만, 버전이 오르고 더 나은 전략이 나오면 옛 태그는 회고적으로 안티패턴이 된다. "
        "발행자는 미래를 모르니 그 관계를 적어줄 수 없다 — **시간축은 수집한 당신만 볼 수 있다.** 같은 모델의 태그를 "
        "전부 받아 버전 순으로 비교하게 하라.",
        "",
        "## 열 설명",
        "",
        "| 열 | 뜻 | 출처 |",
        "|---|---|---|",
        col_rows,
        "",
        "## arch 분포 (파생)",
        "",
        "| arch | 문법 | 태그 수 |",
        "|---|---|---|",
        arch_rows if arch_rows else "| — | — | 0 |",
        "",
        "## 카탈로그",
        "",
        HINTS_MARKER,
        render_rows(entries),
        HINTS_MARKER,
        "",
    ]
    return "\n".join(parts)


def check_existing_hints_md(text: str) -> None:
    """덮어쓰기 전 가드(감사 ⑬ 침묵 파괴 방지 · 옛 `splice_hints_md` 의 마커 검사 계승). 기존 HINTS.md 가 마커를
    **정확히 2개** 가지지 않으면 카탈로그 생성물이 아니거나 손상된 것이다 — 그 파일을 조용히 지우지 않고 중단한다."""
    n = text.count(HINTS_MARKER)
    if n != 2:
        _fail("HINT_CATALOG_MARKER_BROKEN",
              f"HINTS.md 의 마커 `{HINTS_MARKER}` 가 정확히 2개가 아니다(발견 {n}개) — 기존 내용을 지우지 않고 중단한다.",
              "HINTS.md 가 카탈로그 생성물인지 확인한다. 생성물이면 마커 쌍을 되살리거나 파일을 지우고 다시 파생한다.")


def _write(path: Path, text: str) -> None:
    # 임시 파일을 두지 않는다 — 저장소 루트에 등재 밖 잔재가 남으면 그 자체가 거처 오류다(policy:ROOT_SURFACE_REGISTRY).
    # 대신 모든 판정(원격·이름·오브젝트·마커)이 끝난 뒤에만 쓴다.
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def derive(repo, remote: str, generated_kst: str, *, dry_run: bool = False, record_missing: bool = False,
           allow_empty: bool = False) -> int:
    """원격 발행 태그 → `hints/index.json`(schema 3) + `HINTS.md`(전량). 반환 0. 실패는 HintError.

    순서가 곧 계약이다: 시각 형식 → **중앙 권위 플래그**(없으면 네트워크도 건드리지 않는다) → 원격 조회 → 이름 파싱 →
    로컬 오브젝트 확인(부재 = rc 4 · record_missing 이면 행 기재) → 생성 → 기존 HINTS.md 마커 가드 → 쓰기.
    쓰기는 모든 판정이 끝난 뒤에만 일어난다 — 실패 경로에서 index.json·HINTS.md 는 한 바이트도 바뀌지 않는다.
    """
    core = _core()
    repo = Path(repo).resolve()
    if not generated_kst or not _KST_RE.match(generated_kst):
        _fail("HINT_TIME_NOT_INJECTED",
              f"generated_kst 는 KST `YYYY-MM-DDTHH:MM:SS` 로 주입해야 한다: {generated_kst!r}",
              "벽시계를 읽지 않는다 — 호출자가 시각을 넘긴다(예: core.kst_iso(<UTC>)).")
    if not (repo / CENTRAL_FLAG).is_file():
        # 안내가 막다른 길로 읽히면 사람도 에이전트도 우회를 택한다(workflow.md D5 · plan_26082017 W7). 옛 문구는
        # "중앙 전용이다"에서 끝나 **여는 법**을 말하지 않았고, 배포받은 프로젝트가 맨 `git tag -a` + `git push` 로
        # 돌아갔다(2026-08-20 실증). 처방을 함께 적는다.
        _fail("HINT_CATALOG_CENTRAL_FLAG_ABSENT",
              f"`{CENTRAL_FLAG}` 부재 — 카탈로그는 자기 원격의 색인·배포 권위를 가진 체크아웃만 갱신한다(D8 권한 비대칭).",
              "이 체크아웃이 자기 원격의 hint 카탈로그를 소유한다면 권위를 선언한다: "
              f"printf '%s\\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > {CENTRAL_FLAG} "
              "(비추적 — 배포본에 실리지 않으므로 각 저장소가 스스로 선언한다 · 선언은 권한이자 책임이다). "
              "상류에 기여하는 입장이면 봉인까지만 하고 태그를 상류에 전달한다. 어느 쪽도 아니면 우회하지 말고 먼저 정한다.")
    remote_tags = remote_hint_tags(repo, remote, allow_empty=allow_empty)
    entries, notes = derive_entries(repo, remote_tags, remote=remote, record_missing=record_missing)
    index = build_index(entries, remote=remote, generated_kst=generated_kst)
    index_text = core.dumps(index)
    md_text = render_hints_md(index)
    idx_p, md_p = repo / core.REL_INDEX, repo / core.REL_HINTS_MD

    if dry_run:
        # dry-run 도 실제 실행이 멈출 자리에서 멈춘다(쓰기만 뺀다) — "DRY-RUN 초록 → 실제 실행 중단" 은 예측이 거짓인 것이다.
        if md_p.exists():
            check_existing_hints_md(md_p.read_text(encoding="utf-8"))
        rows = render_rows(entries)
        print(f"{_OUT} DRY-RUN — 원격 {len(remote_tags)}건 · 파생 {len(entries)}항목 · 쓰기 0")
        print(rows[:400] + ("…" if len(rows) > 400 else ""))
    else:
        if md_p.exists():
            check_existing_hints_md(md_p.read_text(encoding="utf-8"))
        _write(idx_p, index_text)
        _write(md_p, md_text)
        print(f"{_OUT} 파생 완료 — 원격 {len(remote_tags)}건 → {core.REL_INDEX} {len(entries)}항목 · "
              f"{core.REL_HINTS_MD} {len(entries)}행")
    if notes["absent_local"]:
        print(f"{_OUT} NOTE: 로컬 태그 오브젝트 부재 {len(notes['absent_local'])}건을 `object: {OBJECT_ABSENT_LOCAL}` "
              f"행으로 실었다(brief '{BRIEF_UNRECEIVED}' · 결손 '{BENCH_MODE_UNRECEIVED}' · 합성 ✗). 채우려면 "
              f"`{fetch_hint(repo, remote)}` 후 다시 파생한다.", file=sys.stderr)
        for t in notes["absent_local"][:10]:
            print(f"  {t}", file=sys.stderr)
    if notes["not_annotated"]:
        print(f"{_OUT} NOTE: annotated 가 아닌 원격 태그 {len(notes['not_annotated'])}건을 `object: {OBJECT_NOT_ANNOTATED}` "
              f"행으로 실었다(brief '{BRIEF_UNRECEIVED}' · 합성 ✗ · 원격 태그는 교정하지 않는다 — P1). 발행 경로 밖에서 "
              "만든 태그다 — 그 원격의 발행자에게 알린다.", file=sys.stderr)
        for t in notes["not_annotated"][:10]:
            print(f"  {t}", file=sys.stderr)
    if notes["local_ref_differs"]:
        print(f"{_OUT} 고지: 같은 이름의 로컬 태그가 원격과 **다른 오브젝트**를 가리키는 태그 "
              f"{len(notes['local_ref_differs'])}건 — 카탈로그는 원격이 광고한 오브젝트에서 파생했다(로컬 ref 불변):",
              file=sys.stderr)
        for t in notes["local_ref_differs"][:10]:
            print(f"  {t}", file=sys.stderr)
    local_only = sorted(set(core.git(repo, "tag", "-l", "hint/*").stdout.split()) - set(remote_tags))
    if local_only:
        print(f"{_OUT} 고지: 로컬에만 있는 미발행 태그 {len(local_only)}건 — 카탈로그에 **들어가지 않았다**(발행 사실이 "
              "없다):", file=sys.stderr)
        for t in local_only[:10]:
            print(f"  {t}", file=sys.stderr)
    return 0


# ── 수신자 쪽 (git 없이 동작) ─────────────────────────────────────────────────────────────────
def load_index(repo) -> dict:
    """`hints/index.json` 을 읽는다 — **git 을 부르지 않는다**(배포 아카이브의 gitless `match` · verify_distribution
    `gitless_hint_match`). 부재·손상 = 중단(빈 결과로 접지 않는다: "태그 없음"과 "색인 없음"은 다른 사실이다)."""
    core = _core()
    p = Path(repo) / core.REL_INDEX
    if not p.is_file():
        _fail("HINT_CATALOG_INDEX_ABSENT", f"{core.REL_INDEX} 가 없다.",
              "중앙 저장소라면 `hint.py catalog derive` 로 만든다. 배포본이라면 hints/index.json 이 함께 온다 — 체크아웃을 확인한다.")
    try:
        doc = json.loads(p.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        _fail("HINT_CATALOG_INDEX_MALFORMED", f"{core.REL_INDEX} 를 읽을 수 없다: {e}")
    if not isinstance(doc, dict) or not isinstance(doc.get("hints"), list):
        _fail("HINT_CATALOG_INDEX_MALFORMED", f"{core.REL_INDEX} 에 `hints` 목록이 없다.")
    return doc


REL_SAME = "same-slug"               # 정규화 슬러그 동치
REL_DERIVED = "derived-from-query"   # 그 태그의 base_model 이 질의 모델이다(질의 모델의 양자화 변종 등)
REL_BASE = "base-of-query"           # 그 태그의 모델이 질의 모델의 base_model 이다
REL_SIBLING = "shared-base"          # 질의 모델과 같은 base_model 을 가진다
_REL_RANK = {REL_SAME: 3, REL_DERIVED: 2, REL_BASE: 2, REL_SIBLING: 1}
_ARCH_RANK = {"exact": 3, "prefix": 2, "hw": 1, "none": 0, "unspecified": 0}


def _hw_token(arch: str) -> str:
    head = (arch or "").split("-", 1)[0].lower()
    return re.sub(r"x\d+$", "", head)      # 옛 문법 `gb10x2`·`rtxpro6000x2` 의 배수 접미 제거


def _arch_match(entry_arch: str, query: str | None) -> str:
    if not query:
        return "unspecified"
    ea, q = (entry_arch or "").lower(), query.strip().lower()
    if ea == q:
        return "exact"
    if ea.startswith(q + "-"):
        return "prefix"
    return "hw" if _hw_token(ea) and _hw_token(ea) == _hw_token(q) else "none"


def _vnorm(v: str | None) -> str:
    s = (v or "").strip()
    return s[1:] if s[:1] in ("v", "V") and s[1:2].isdigit() else s


def match(index: dict, *, vllm: str, model: str, arch: str | None = None,
          include_other: bool = False) -> list[dict]:
    """근-미스 발견(읽기 전용 · gitless). families 폐기(O3 · X14) — 모델 관계 = **정규화 슬러그 동치 + base_model**.

    base_model 은 index 안에서만 찾는다(네트워크·HF 카드 ✗): 질의와 같은 슬러그 행이 base_model 을 가지면 그 기반을
    공유하는 행(shared-base)과 기반 자신(base-of-query)을, 행의 base_model 이 질의 모델이면 derived-from-query 로 잇는다.
    관계없는 모델은 기본 숨김(2026-08-20 실측: 출력 9,563 B 중 46/49 가 잡음이었다). 정렬 = 관계 → vLLM 일치 → arch
    근접 → 버전(오름차순 · 시간축) → 태그. 관계 판정(패턴/안티패턴)은 하지 않는다 — 시간축은 수신자만 본다(사용자 D3).
    """
    naming = _naming()
    if not (vllm or "").strip() or not query_slug(model):
        _fail("HINT_CATALOG_MATCH_QUERY_EMPTY", "match 에는 --vllm 과 --model 이 필요하다.")
    entries = index.get("hints") if isinstance(index, dict) else None
    if not isinstance(entries, list):
        _fail("HINT_CATALOG_INDEX_MALFORMED", "index 에 `hints` 목록이 없다.")
    q = query_slug(model)
    q_bases = sorted({e["base_model"] for e in entries
                      if isinstance(e, dict) and isinstance(e.get("base_model"), str)
                      and query_slug(e.get("model")) == q})
    q_base_norms = {query_slug(b) for b in q_bases}
    out = []
    for e in entries:
        if not isinstance(e, dict) or not isinstance(e.get("tag"), str):
            continue
        em = query_slug(e.get("model"))
        eb = e["base_model"] if isinstance(e.get("base_model"), str) else None
        if em and em == q:
            rel, basis = REL_SAME, f"slug≡{e.get('model')}"
        elif eb and query_slug(eb) == q:
            rel, basis = REL_DERIVED, f"base_model={eb}"
        elif em and em in q_base_norms:
            rel, basis = REL_BASE, f"질의 모델의 base_model={' · '.join(q_bases)}"
        elif eb and query_slug(eb) in q_base_norms:
            rel, basis = REL_SIBLING, f"base_model={eb}"
        else:
            rel, basis = None, None
        if rel is None and not include_other:
            continue
        am = _arch_match(e.get("arch"), arch)
        out.append({
            "tag": e["tag"], "grammar": e.get("grammar"), "vllm": e.get("vllm"), "model": e.get("model"),
            "arch": e.get("arch"), "recipe": e.get("recipe"), "brief": e.get("brief"),
            "base_model": e.get("base_model"), "object": e.get("object"), "missing": e.get("missing"),
            "bench_mode": e.get("bench_mode"), "verdict": e.get("verdict"),
            "relation": rel or "other-model", "relation_basis": basis,
            "vllm_match": _vnorm(e.get("vllm")) == _vnorm(vllm), "arch_match": am,
        })
    out.sort(key=lambda r: (-_REL_RANK.get(r["relation"], 0), not r["vllm_match"], -_ARCH_RANK[r["arch_match"]],
                            naming.version_key(r["vllm"] or ""), r["tag"]))
    return out


def format_match(index: dict, results: list[dict], *, model: str, include_other: bool = False) -> str:
    """`match` 결과의 사람용 출력(축별 안내 · 숨김 수). `hint.py match` 가 --json 이 아닐 때 쓴다."""
    total = len(index.get("hints") or []) if isinstance(index, dict) else 0
    lines = []
    for r in results:
        guide = []
        if r["relation"] == REL_SAME and r["vllm_match"] and r["arch_match"] in ("exact", "unspecified"):
            guide.append("정확 일치 — 그래도 네 스모크로 재검증" if r["arch_match"] == "exact"
                         else "모델·vLLM 일치(arch 미지정) — arch 를 대조하고 네 스모크로 재검증")
        else:
            if r["relation"] == "other-model":
                guide.append("다른 모델→서빙전략 독립(참고만)")
            elif r["relation"] != REL_SAME:
                guide.append(f"관계 모델({r['relation']}: {r['relation_basis']})→서빙전략은 다시 세운다")
            if not r["vllm_match"]:
                guide.append("다른 vLLM→릴리스 노트 재확인(carry-forward ✗)")
            if r["arch_match"] not in ("exact", "unspecified"):
                guide.append("다른 arch→빌드트랙·벽지도 이식 가능 · KV/gmu/TORCH_CUDA_ARCH 재도출")
        if r.get("object") == OBJECT_ABSENT_LOCAL:
            guide.append("카탈로그 저장소에 오브젝트 미수령(brief 없음) — 원격에서 받아 읽는다")
        elif r.get("object") == OBJECT_NOT_ANNOTATED:
            guide.append("annotation 없는 태그(brief 없음 · 발행 경로 밖) — 트리를 받아 직접 확인한다")
        lines.append(f"[{r['relation']}] {r['tag']}  ({r.get('grammar') or '?'})  ·  {' · '.join(guide)}")
    shown_other = sum(1 for r in results if r["relation"] == "other-model")
    hidden = 0 if include_other else total - len(results)
    related = len(results) - shown_other
    if not related:
        lines.append(f"{_OUT} 같은 모델(정규화 슬러그 · base_model) 태그 없음: {model!r}"
                     + (f" · 다른 모델 {hidden}건은 --include-other 로." if hidden else ""))
    elif hidden:
        lines.append(f"{_OUT} (관계없는 모델 {hidden}건 숨김 — --include-other)")
    return "\n".join(lines)


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 격리 임시 저장소 + bare 원격만 쓴다(라이브 태그·브랜치·캠페인 비의존).

    옛 `hint_catalog._run_self_test` 의 음성대조를 전부 이관하고(⑤ brief·셀 · ⑬ 마커 · S7 전용 코드·원격·주 워크트리 ·
    lightweight · 조회 실패 · 0건 · allow_empty), 신설을 더한다: 결정론(두 번 파생 = 바이트 동일) · 문법 열 · base_model
    (v6 만) · ★원격 전용 태그 rc 4 + 쓰기 0 · record-missing 행 · ★중앙 플래그 부재 · ★K5 로컬 ref ≠ 원격 · ★원격 경로
    비기재 · H5 단독 적재 · match 의 base_model 관계. (옛 "★4세그먼트 거부"는 naming 이 5세그먼트 아닌 이름을 거부하는
    한 그대로 "규약 밖 = 전용 코드 아님" 음성대조로 산다.)
    """
    import contextlib
    import io
    import subprocess
    import tempfile

    core, naming = _core(), _naming()
    bad: list[str] = []

    def ck(name: str, cond) -> None:
        if not cond:
            bad.append(f"catalog: {name}")

    def err_of(fn):
        buf_o, buf_e = io.StringIO(), io.StringIO()
        try:
            with contextlib.redirect_stdout(buf_o), contextlib.redirect_stderr(buf_e):
                fn()
        except core.HintError as e:
            return e, buf_o.getvalue() + buf_e.getvalue()
        return None, buf_o.getvalue() + buf_e.getvalue()

    # ── 순수 헬퍼 (⑤ · ⑬) ───────────────────────────────────────────────────────────────
    warn = ("<!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 지우지 마라 — seal 의 린터 L1 이 fail-closed 로 -->\n\n"
            "진짜 brief 한 줄\n\n## 1. 벽 지도\n")
    ck("★⑤회귀 HTML 주석을 brief 로 캐지 않는다", extract_brief(warn) == "진짜 brief 한 줄")
    ck("★⑤회귀 cat-file 헤더를 brief 로 캐지 않는다",
       extract_brief("object abc\ntype commit\ntag hint/a/b/c/d\n\n실제 brief\n") == "실제 brief")
    ck("인용부호 줄은 brief 아님", extract_brief("> ⚠ 유효맥락 …\n\nbrief 다\n") == "brief 다")
    ck("헤딩은 brief 아님", extract_brief("# 제목\n\nbrief\n") == "brief")
    ck("본문이 비면 빈 문자열", extract_brief("") == "")
    ck("v6 brief = 첫 문단(여러 줄은 한 줄로)",
       annotation_brief("첫 줄 요약\n둘째 줄\n\n전체는 zip 의 00-hint.md\n\n<!-- hint-evidence-binding:v1\nx\n-->\n")
       == "첫 줄 요약 둘째 줄")
    ck("v6 brief 는 앞선 HTML 주석을 건너뛴다", annotation_brief("<!-- c -->\n\n요약\n") == "요약")
    ck("★파이프 이스케이프", md_cell("a | b") == "a \\| b")
    ck("★미닫힘 주석 무해화", "<!--" not in md_cell("깨진 <!-- 주석"))
    ck("줄바꿈 평탄화", "\n" not in md_cell("a\nb"))
    e, _ = err_of(lambda: check_existing_hints_md(f"머리\n{HINTS_MARKER}\n표\n{HINTS_MARKER}\n"))
    ck("마커 2개 = 통과", e is None)
    e, _ = err_of(lambda: check_existing_hints_md(f"머리\n{HINTS_MARKER}\n꼬리\n"))
    ck("★음성대조 마커 1개면 거부", e is not None and e.code == "HINT_CATALOG_MARKER_BROKEN")
    e, _ = err_of(lambda: check_existing_hints_md("머리만\n"))
    ck("★음성대조 마커 0개면 거부", e is not None and e.code == "HINT_CATALOG_MARKER_BROKEN")
    for doc, want in (({"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "declared-lite",
                                               "downgrade_reason": None, "source": "bench_report(x.md)"}}, "lite(선언)"),
                      ({"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "downgraded-lite",
                                               "downgrade_reason": "blackbox_kill"}}, "lite(강등·blackbox_kill)"),
                      ({"measurement_config": {"bench_mode": "full"}}, "full"),
                      ({"measurement_config": {"bench_mode": "lite"}}, "lite"),
                      ({"measurement_config": {"bench_mode": None}}, BENCH_MODE_UNDETERMINED),
                      ({"missing": []}, BENCH_MODE_ABSENT), (None, BENCH_MODE_ABSENT)):
        ck(f"bench_mode 파생 칸: {want}", bench_mode_cell(doc)[0] == want)
    # 판정 열(2026-09-29 · plan_26092908 §4.4) — 페이로드 자기 기재에서만 · 재판정 ✗
    for doc, want in (({"measurement": {"verdict": "REFUTE", "sources": {"verdict": "sweep verdict.json"}}}, "REFUTE"),
                      ({"format": "hint-payload/v6", "measurement": {"verdict": "PASS", "source": "certificate"}}, "PASS"),
                      ({"measurement": {"verdict": "OBSERVATION-ONLY"}}, "OBSERVATION-ONLY"),
                      ({"measurement": {"source": "absent(인증서·스윕 모두 없다)"}}, VERDICT_ABSENT),
                      ({"measurement": {"verdict": ""}}, VERDICT_ABSENT), ({}, VERDICT_ABSENT), (None, VERDICT_ABSENT)):
        ck(f"판정 파생 칸: {want}", verdict_cell(doc)[0] == want)
    ck("판정 출처: 키별 출처(sources.verdict)가 있으면 그것", "sweep verdict.json" in
       verdict_cell({"measurement": {"verdict": "REFUTE", "sources": {"verdict": "sweep verdict.json"}}})[1])
    ck("★판정 어휘 밖 값은 접지 않고 표시한다", verdict_cell({"measurement": {"verdict": "MAYBE"}})[0] == "MAYBE(어휘 밖)")
    ck("판정 열이 있다(plan_26092908 §4.4) · arch 뒤", CATALOG_COLUMNS[CATALOG_COLUMNS.index("arch") + 1] == "판정")
    ck("열 설명이 열 전부를 덮는다(생성 머리말의 열 표 = CATALOG_COLUMNS)", set(COLUMN_DOCS) == set(CATALOG_COLUMNS))
    ck("문법 열이 있다(plan §4.7 구·신 공존)", "문법" in CATALOG_COLUMNS)
    ck("중앙 권위 경로 교차검증(core.REL_CENTRAL_FLAG)", CENTRAL_FLAG == core.REL_CENTRAL_FLAG)
    ck("정규화 슬러그 동치(2026-08-20 gemma 철자 갈림)", query_slug("gemma-4-E2B-it") == query_slug("gemma-4-e2b-it"))
    ck("검색 키는 Org/Name 의 basename 을 본다", query_slug("Qwen/Qwen3.8-Flash-Next") == query_slug("qwen3.8-flash-next"))
    ck("★값 없는 모델은 빈 키(None 을 'none' 슬러그로 접지 않는다)", query_slug(None) == "" and query_slug("") == "")
    vs = ["0.29.0-g0123456789ab", "0.29.0", "0.29.0rc6", "0.1.1.dev53+g30118ba27", "0.29.0rc5", "0.19.0"]
    ck("match 가 쓰는 버전 키의 시간축(dev<rc<릴리스<커밋 핀 · naming.version_key)",
       sorted(vs, key=naming.version_key) == ["0.1.1.dev53+g30118ba27", "0.19.0", "0.29.0rc5", "0.29.0rc6", "0.29.0",
                                              "0.29.0-g0123456789ab"])
    ck("이름 해체: 4세그먼트 = legacy-4seg 읽기 전용(recipe 없음) · 그 밖 = unknown(추측 채움 ✗)",
       parse_name("hint/0.23.0/deepseek-v4-flash/gb10") == {"grammar": naming.GRAMMAR_LEGACY_4SEG, "vllm": "0.23.0",
                                                           "model": "deepseek-v4-flash", "arch": "gb10", "recipe": None}
       and parse_name("hint/0.19.1/gpt-oss-120b") == {"grammar": naming.GRAMMAR_UNKNOWN, "vllm": None, "model": None,
                                                      "arch": None, "recipe": None})
    ck("★원격 경로·URL 은 배포 파일에 적지 않는다", _remote_label("/srv/x/remote.git").startswith("<")
       and _remote_label("ssh://example.invalid/o/r.git").startswith("<") and _remote_label("github-ssh") == "github-ssh")

    # ── H5: 파일 경로 단독 적재(격리 인터프리터 · scripts/ 가 sys.path 에 없다) ─────────────────────
    # 감사 훅으로 적재 중 프로세스 실행·비 .py 파일 열기를 관측한다(import 부수효과 0 · 패키지 불변식) — PATH 도 끊는다.
    probe = ("import importlib.util, json, sys\n"
             "ev = []\n"
             "W = ('subprocess.Popen', 'os.system', 'os.exec', 'os.posix_spawn', 'os.spawn', 'os.fork')\n"
             "def h(e, a):\n"
             "    if e in W: ev.append(e)\n"
             "    elif e == 'open':\n"
             "        p = a[0].decode('utf-8', 'replace') if isinstance(a[0], bytes) else a[0]\n"
             "        if isinstance(p, str) and not p.endswith(('.py', '.pyc')): ev.append(p)\n"
             "sys.addaudithook(h)\n"
             "s = importlib.util.spec_from_file_location('_standalone_catalog_probe', sys.argv[1])\n"
             "m = importlib.util.module_from_spec(s); s.loader.exec_module(m)\n"
             "print(m.EXIT_LOCAL_TAG_OBJECT_MISSING, 'hintlib' in sys.modules, json.dumps(ev))\n")
    # -I 는 PYTHON* 환경변수를 무시한다 → -B 로 바이트코드 잔재를 막는다(추적 트리에 __pycache__ 를 남기지 않는다).
    r = subprocess.run([sys.executable, "-I", "-B", "-c", probe, str(Path(__file__).resolve())],
                       capture_output=True, text=True, env={"PATH": "/nonexistent"})
    ck("★H5 파일 경로 단독 적재로 EXIT 상수를 읽는다(형제 import 없음)",
       r.returncode == 0 and r.stdout.split()[:2] == [str(EXIT_LOCAL_TAG_OBJECT_MISSING), "False"])
    ck(f"★단독 적재는 프로세스를 띄우거나 파일을 열지 않는다(import 부수효과 0 · {r.stdout.strip()[-200:]})",
       r.returncode == 0 and r.stdout.split()[2:] == ["[]"])
    import ast
    top = ast.parse(Path(__file__).read_text(encoding="utf-8")).body
    rel_imports = [n for n in top if isinstance(n, ast.ImportFrom)
                   and (n.level > 0 or (n.module or "").split(".")[0] == "hintlib")]
    ck("★H5 모듈 최상위에 패키지 상대 import 가 없다", not rel_imports)

    # ── 격리 저장소 · bare 원격 ────────────────────────────────────────────────────────────────
    ident = {"GIT_AUTHOR_NAME": core.SYNTHETIC_NAME, "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL,
             "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL,
             "GIT_AUTHOR_DATE": core.git_date("2026-09-21T00:00:00Z"),
             "GIT_COMMITTER_DATE": core.git_date("2026-09-21T00:00:00Z")}
    cfg = ("-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", "-c", "core.hooksPath=/dev/null",
           "-c", f"user.name={core.SYNTHETIC_NAME}", "-c", f"user.email={core.SYNTHETIC_EMAIL}")

    def g(repo: Path, *args: str, input_text: str | None = None) -> str:
        return core.git(repo, *cfg, *args, input_text=input_text, env_extra=ident).stdout.strip()

    def payload_commit(repo: Path, payload: dict | None, marker: str) -> str:
        files = {"marker.txt": marker + "\n"}
        if payload is not None:
            files["PAYLOAD.json"] = core.dumps(payload)
        lines = []
        for fname in sorted(files):
            blob = g(repo, "hash-object", "-w", "--stdin", input_text=files[fname])
            lines.append(f"100644 blob {blob}\t{fname}")
        tree = g(repo, "mktree", input_text="\n".join(lines) + "\n")
        return g(repo, "commit-tree", tree, "-m", f"hint: {marker}")

    def atag(repo: Path, name: str, target: str, message: str) -> None:
        g(repo, "tag", "-a", "-F", "-", "--cleanup=verbatim", name, target, input_text=message)

    kst = "2026-09-21T19:00:00"
    v6_tag = ("hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/"
              "qnvfp4-len262144-kvauto-plemmap-spec3-eager")
    legacy_tag = "hint/0.19.0/gpt-oss-120b/gb10x2-cluster-native/qmxfp4-len131072-kvfp8"
    remote_only = "hint/0.27.1/qwen3-4b/h10080gb-main-native/len32768-kvauto-pleresident"   # 코드맵 K4 의 실물 모양
    # v7 태그(plan_26092908 §4.1): 결정론 3축 + 자율 꼬리 · 판정 REFUTE 가 페이로드 measurement 에 실린다(§4.4)
    v7_tag = "hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-t2609290823"
    v7_annotation = ("DS4F 1M 컨텍스트 spec7 셀 — 루브릭 바닥 미달(REFUTE).\n\n"
                     "전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다.\n\n"
                     "<!-- hint-evidence-binding:v2\nversion: 2\ntag: " + v7_tag + "\n-->\n")
    v6_annotation = ("NVFP4 체크포인트를 GB10 2노드(TP=2 · Ray)에서 262144 컨텍스트로 서빙한 여정.\n"
                     "PLE mmap 이 생사를 가른 축이었다.\n\n"
                     "전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다.\n\n"
                     "<!-- hint-evidence-binding:v1\nversion: 1\ntag: " + v6_tag + "\n-->\n")
    with tempfile.TemporaryDirectory() as td:
        root = Path(td).resolve()
        repo = root / "main"
        repo.mkdir()
        g(repo, "init", "-q", "-b", "main")
        g(repo, "commit", "--allow-empty", "-q", "-m", "base")
        (repo / "hints").mkdir()
        (repo / CENTRAL_FLAG).write_text("selftest — 이 체크아웃이 자기 원격의 hint 색인 권위다.\n", encoding="utf-8")
        (repo / core.REL_HINTS_MD).write_text(f"# 옛 손산문\n\n{HINTS_MARKER}\n| 옛 |\n{HINTS_MARKER}\n", encoding="utf-8")
        v6_commit = payload_commit(repo, {
            "schema_version": 2, "format": "hint-payload/v6", "tag": v6_tag,
            "identity": {"model": "qwen3.8-flash-next-nvfp4", "base_model": "Qwen/Qwen3.8-Flash-Next",
                         "hf_repo": "Org/Qwen3.8-Flash-Next-NVFP4"},
            "missing": ["BENCH_MODE_LITE"],
            "measurement_config": {"bench_mode": "lite", "bench_mode_kind": "declared-lite",
                                   "downgrade_reason": None, "source": "bench_report(r.md)"}}, "v6")
        atag(repo, v6_tag, v6_commit, v6_annotation)
        v7_commit = payload_commit(repo, {
            "schema_version": 2, "format": "hint-payload/v7", "tag": v7_tag,
            "identity": {"model": "deepseek-v4-flash-0731", "base_model": "deepseek-ai/DeepSeek-V4-Flash"},
            "missing": [], "measurement": {"verdict": "REFUTE", "sources": {"verdict": "sweep verdict.json"}},
            "measurement_config": {"bench_mode": "full", "source": "bench_mode.json"}}, "v7")
        atag(repo, v7_tag, v7_commit, v7_annotation)
        legacy_commit = payload_commit(repo, {"missing": ["HINT_MISSING_SLAVE_ATTESTATION"],
                                              "identity": {"base_model": "openai/should-not-be-read"}}, "legacy")
        atag(repo, legacy_tag, legacy_commit,
             "<!-- ⚠ 헤딩을 지우지 마라 -->\nGB10 x2 클러스터 · 네이티브 예산\n\n## 1. 벽 지도\n본문\n")
        bare = root / "remote.git"
        g(root, "init", "--bare", "-q", str(bare))
        g(repo, "remote", "add", "fx", str(bare))
        g(repo, "push", "-q", "fx", f"refs/tags/{v6_tag}:refs/tags/{v6_tag}",
          f"refs/tags/{legacy_tag}:refs/tags/{legacy_tag}", f"refs/tags/{v7_tag}:refs/tags/{v7_tag}")
        idx_p, md_p = repo / core.REL_INDEX, repo / core.REL_HINTS_MD

        e, out = err_of(lambda: derive(repo, "fx", kst))
        ck("파생 성공(rc 0 · 예외 없음)", e is None)
        if e is None:
            idx_b1, md_b1 = idx_p.read_bytes(), md_p.read_bytes()
            e2, _ = err_of(lambda: derive(repo, "fx", kst))
            ck("★결정론: 같은 입력으로 두 번 파생 = index.json·HINTS.md 바이트 동일",
               e2 is None and idx_p.read_bytes() == idx_b1 and md_p.read_bytes() == md_b1)
            idx = json.loads(idx_b1)
            md = md_b1.decode("utf-8")
            by = {h["tag"]: h for h in idx["hints"]}
            h6, hl, h7 = by.get(v6_tag, {}), by.get(legacy_tag, {}), by.get(v7_tag, {})
            ck("index schema 4 · 출처 remote-derived · 개수", idx.get("schema") == INDEX_SCHEMA == 4
               and idx.get("source") == "remote-derived" and idx.get("count") == 3 and idx.get("remote") == "fx")
            keys = {"tag", "grammar", "vllm", "model", "arch", "recipe", "brief", "anchor", "object", "missing", "bench_mode",
                    "bench_mode_source", "verdict", "verdict_source", "base_model", "published", "source"}
            ck("index 항목 키 = SPEC 목록(v6·v7 · verdict 추가 외 호환)", set(h6) == keys and set(h7) == keys)
            ck("문법 열: v7 태그 = v7 · base_model · brief = 첫 문단", h7.get("grammar") == naming.GRAMMAR_V7
               and h7.get("base_model") == "deepseek-ai/DeepSeek-V4-Flash"
               and h7.get("brief") == "DS4F 1M 컨텍스트 spec7 셀 — 루브릭 바닥 미달(REFUTE).")
            ck("★판정 열: v7 REFUTE 가 기계 표면에 보인다(출처 = 키별 출처)", h7.get("verdict") == "REFUTE"
               and "sweep verdict.json" in h7.get("verdict_source", ""))
            ck("판정 열: 판정 칸 없는 v6 페이로드 = '—'(재판정 ✗ · P1)", h6.get("verdict") == VERDICT_ABSENT
               and hl.get("verdict") == VERDICT_ABSENT)
            ck("문법 열: v6 태그 = v6", h6.get("grammar") == naming.GRAMMAR_V6)
            ck("문법 열: 옛 태그 = 옛 세대(읽기 전용 수용)",
               hl.get("grammar") in (naming.GRAMMAR_LEGACY_ARCH_NODE, naming.GRAMMAR_LEGACY_ARCH))
            ck("v6 base_model = PAYLOAD.identity.base_model", h6.get("base_model") == "Qwen/Qwen3.8-Flash-Next")
            ck("★옛 태그에는 base_model 을 싣지 않는다(v6 만 · 옛 PAYLOAD 를 v6 로 읽지 않는다)", "base_model" not in hl)
            ck("v6 brief = annotation 첫 문단(두 줄 → 한 줄 · 포인터·footer 제외)",
               h6.get("brief") == "NVFP4 체크포인트를 GB10 2노드(TP=2 · Ray)에서 262144 컨텍스트로 서빙한 여정. "
                                  "PLE mmap 이 생사를 가른 축이었다.")
            ck("옛 태그 brief = 옛 규칙(주석 건너뜀)", hl.get("brief") == "GB10 x2 클러스터 · 네이티브 예산")
            ck("anchor = 페이로드 커밋 · object = 태그 오브젝트",
               h6.get("anchor") == v6_commit and h6.get("object") == g(repo, "rev-parse", f"refs/tags/{v6_tag}"))
            ck("★페이로드 lite 선언 → bench_mode lite(선언) · 결손 BENCH_MODE_LITE",
               h6.get("bench_mode") == "lite(선언)" and h6.get("missing") == ["BENCH_MODE_LITE"]
               and h6.get("bench_mode_source", "").startswith("payload("))
            ck("★과거 태그(measurement_config 없음) bench_mode = 미기재(full·lite 로 접지 않는다)",
               hl.get("bench_mode") == BENCH_MODE_ABSENT and hl.get("bench_mode_source", "").startswith("absent("))
            ck("발행 사실 = 원격 존재", h6.get("published") is True and h6.get("source") == "remote-derived")
            rows = md.split(HINTS_MARKER)[1].strip().splitlines() if md.count(HINTS_MARKER) == 2 else []
            ck("HINTS.md 마커 쌍 · 표 = 헤더+구분+3행", md.count(HINTS_MARKER) == 2 and len(rows) == 5)
            ck("HINTS.md 표에 판정 열 · v7 행의 REFUTE", any(v7_tag in rw and "| REFUTE |" in rw for rw in rows)
               and "| 판정 |" in rows[0])
            ck("카탈로그 행은 헤더와 같은 열 수(문법 열 포함)",
               rows and all(rw.replace("\\|", "").count("|") == len(CATALOG_COLUMNS) + 1 for rw in rows))
            ck("HINTS.md 는 손산문을 남기지 않는다(전량 생성)", "옛 손산문" not in md and "| 옛 |" not in md)
            ck("머리말: 지도이지 정답이 아니다 · 두 문법 세대 · 옛 행 리콜 ✗ · zip·00-hint · 옛 태그 annotation · match",
               all(s in md for s in ("지도이지 정답이 아니다", f"`{naming.GRAMMAR_V6}`", f"`{naming.GRAMMAR_V7}`",
                                     "-t<YYMMDDHHMM>", "naming.tail[]",
                                     f"`{naming.GRAMMAR_LEGACY_ARCH_NODE}`", "교정·리콜하지 않는다", "git archive",
                                     "`00-hint.md`", "%(contents)", " match --vllm", "base_model")))
            ck("머리말 예시·분포는 index 에서 파생(실재 태그만 인용)", f"`{v6_tag}`" in md and "`gb10x2-cluster-native`" in md)
            ck("★격리 경로가 배포 파일에 새지 않는다", str(root) not in md and str(root) not in idx_b1.decode("utf-8"))

            # ★K5: 같은 이름의 로컬 태그가 원격과 다른 오브젝트 → 원격 오브젝트에서 파생(로컬 brief 를 붙이지 않는다)
            other = root / "other-pc"
            g(root, "clone", "-q", str(bare), str(other))
            k5_tag = "hint/0.29.0/qwen3-4b/gb10-1g1n-main-native/qbf16-len32768-kvauto-plenone-specoff-graph"
            atag(other, k5_tag, payload_commit(other, {"format": "hint-payload/v6", "identity": {"base_model": None}},
                                              "k5-remote"), "원격이 광고한 brief\n\n포인터\n")
            g(other, "push", "-q", "origin", f"refs/tags/{k5_tag}:refs/tags/{k5_tag}")
            g(repo, "fetch", "-q", "--no-tags", "fx", f"refs/tags/{k5_tag}:refs/hint-remote-selftest/{k5_tag}")  # 오브젝트만
            atag(repo, k5_tag, payload_commit(repo, None, "k5-local"), "로컬의 다른 brief\n")
            e, out = err_of(lambda: derive(repo, "fx", kst))
            k5 = {h["tag"]: h for h in json.loads(idx_p.read_bytes())["hints"]}.get(k5_tag, {}) if e is None else {}
            ck("★K5 로컬 ref ≠ 원격 → 원격 광고 오브젝트에서 파생 + 고지",
               e is None and k5.get("brief") == "원격이 광고한 brief" and "다른 오브젝트" in out
               and k5.get("object") == g(repo, "rev-parse", f"refs/hint-remote-selftest/{k5_tag}"))

            # ★원격 전용 태그(로컬 오브젝트 부재) → rc 4 · 쓰기 0 · 안내 = 조회한 그 원격 + 주 워크트리
            atag(other, remote_only, payload_commit(other, None, "remote-only"), "원격 전용 brief\n")
            g(other, "push", "-q", "origin", f"refs/tags/{remote_only}:refs/tags/{remote_only}")
            idx_b, md_b = idx_p.read_bytes(), md_p.read_bytes()
            e, out = err_of(lambda: derive(repo, "fx", kst))
            ck("★원격 전용 태그 w/o 로컬 오브젝트 → 전용 종료코드 4",
               e is not None and e.code == "HINT_CATALOG_LOCAL_OBJECT_MISSING"
               and e.exit_code == EXIT_LOCAL_TAG_OBJECT_MISSING)
            ck("★rc 4 경로는 아무것도 쓰지 않는다(index.json·HINTS.md 바이트 불변)",
               idx_p.read_bytes() == idx_b and md_p.read_bytes() == md_b)
            text = e.render() if e is not None else ""
            ck("★S7 해소 명령은 조회한 그 원격 · 주 워크트리를 가리킨다(기본 원격 전량 태그 fetch 가 아니다)",
               f"git -C {repo} fetch fx 'refs/tags/hint/*:refs/tags/hint/*'" in text and "--tags" not in text)
            ck("★자동 fetch 하지 않는다(원격 전용 태그는 여전히 로컬에 없다)",
               core.git(repo, "rev-parse", "--verify", "--quiet", f"refs/tags/{remote_only}",
                        check=False).returncode != 0)
            wt = root / "main.wt-other"
            g(repo, "worktree", "add", "-q", "--detach", str(wt))
            (wt / "hints").mkdir(exist_ok=True)
            (wt / CENTRAL_FLAG).write_text("selftest\n", encoding="utf-8")   # 종료 시퀀스가 하듯 임시로 둔다
            e, _ = err_of(lambda: derive(wt, "fx", kst))
            text = e.render() if e is not None else ""
            ck("★S7 임시 워크트리에서 파생해도 fetch 안내는 주 워크트리(곧 지워질 경로를 찍지 않는다)",
               e is not None and e.exit_code == EXIT_LOCAL_TAG_OBJECT_MISSING
               and f"git -C {repo} fetch fx" in text and str(wt) not in text)
            g(repo, "worktree", "remove", "--force", str(wt))
            e, out = err_of(lambda: derive(repo, "fx", kst, dry_run=True))
            ck("dry-run 도 부재를 삼키지 않는다(rc 4)", e is not None and e.exit_code == EXIT_LOCAL_TAG_OBJECT_MISSING)

            # --record-missing(X15) → rc 0 · object absent-local 행 · brief '—' · 결손 미수령 · NOTE
            e, out = err_of(lambda: derive(repo, "fx", kst, record_missing=True))
            ro = {h["tag"]: h for h in json.loads(idx_p.read_bytes())["hints"]}.get(remote_only, {}) if e is None else {}
            ck("record-missing → rc 0 · absent-local 행(brief '—' · missing None · 미수령)",
               e is None and ro.get("object") == OBJECT_ABSENT_LOCAL and ro.get("brief") == BRIEF_UNRECEIVED
               and ro.get("missing") is None and ro.get("bench_mode") == BENCH_MODE_UNRECEIVED
               and ro.get("published") is True)
            ck("record-missing NOTE 가 부재와 해소 명령을 말한다", "NOTE" in out and "fetch fx" in out)
            md_rm = md_p.read_text(encoding="utf-8")
            ck("record-missing 행이 HINTS.md 에 미수령으로 실린다(합성 ✗)",
               f"`{remote_only}`" in md_rm and "원격 전용 brief" not in md_rm and "미수령" in md_rm)
            idx_b = idx_p.read_bytes()
            md_p.write_text(md_rm.replace("미수령", "미수령 "), encoding="utf-8")   # 쓰기가 일어나면 드러나게 흔든다
            e, _ = err_of(lambda: derive(repo, "fx", kst, dry_run=True, record_missing=True))
            ck("dry-run 은 쓰지 않는다", e is None and idx_p.read_bytes() == idx_b
               and md_p.read_text(encoding="utf-8") == md_rm.replace("미수령", "미수령 "))
            md_p.write_text(md_rm, encoding="utf-8")

            # match — base_model 관계 · 정규화 슬러그 · 기본 숨김
            index = load_index(repo)
            res = match(index, vllm="0.29.0rc6", model="Qwen3.8-Flash-Next")
            ck("★match 가 base_model 관계 태그를 찾는다(질의 = 기반 모델)",
               [r["tag"] for r in res] == [v6_tag] and res[0]["relation"] == REL_DERIVED)
            res = match(index, vllm="0.29.0rc6", model="Qwen3.8-Flash-Next-NVFP4", arch="gb10-1g2n-cluster-native")
            ck("match 정규화 슬러그 동치(대소문자) · 정확 일치", res and res[0]["tag"] == v6_tag
               and res[0]["relation"] == REL_SAME and res[0]["vllm_match"] and res[0]["arch_match"] == "exact")
            ck("★match 는 관계없는 모델을 기본으로 숨긴다", all(r["relation"] != "other-model" for r in res)
               and legacy_tag not in [r["tag"] for r in res])
            res_all = match(index, vllm="0.29.0rc6", model="qwen3.8-flash-next-nvfp4", include_other=True)
            ck("--include-other 면 다른 모델도 보이고 관계 행이 먼저 온다",
               legacy_tag in [r["tag"] for r in res_all] and res_all[0]["tag"] == v6_tag)
            sib_index = {"hints": [dict(by[v6_tag]), {"tag": "hint/x/qwen3.8-flash-next-fp8/a/r", "model":
                                                      "qwen3.8-flash-next-fp8", "vllm": "0.29.0", "arch": "a",
                                                      "base_model": "Qwen/Qwen3.8-Flash-Next"},
                                   {"tag": "hint/y/qwen3.8-flash-next/a/r", "model": "qwen3.8-flash-next",
                                    "vllm": "0.29.0", "arch": "a"}]}
            rels = {r["tag"]: r["relation"] for r in match(sib_index, vllm="0.29.0", model="qwen3.8-flash-next-nvfp4")}
            ck("match 형제(shared-base) · 기반(base-of-query) 관계 · schema 2 항목(grammar 없음) 수용",
               rels.get("hint/x/qwen3.8-flash-next-fp8/a/r") == REL_SIBLING
               and rels.get("hint/y/qwen3.8-flash-next/a/r") == REL_BASE)
            ck("format_match: 숨김 수·안내", "숨김" in format_match(index, match(index, vllm="0.29.0rc6",
                                                                             model="qwen3.8-flash-next-nvfp4"),
                                                                  model="qwen3.8-flash-next-nvfp4"))
            e, _ = err_of(lambda: match(index, vllm="", model="m"))
            ck("★match 빈 질의 거부", e is not None and e.code == "HINT_CATALOG_MATCH_QUERY_EMPTY")

            # ★중앙 권위 플래그 부재 → 거부 · 쓰기 0 (원격도 건드리지 않는다)
            (repo / CENTRAL_FLAG).unlink()
            idx_b, md_b = idx_p.read_bytes(), md_p.read_bytes()
            e, _ = err_of(lambda: derive(repo, "fx", kst, record_missing=True))
            ck("★중앙 권위 플래그 부재 → 파생 거부(D8 권한 비대칭) · 여는 법을 말한다",
               e is not None and e.code == "HINT_CATALOG_CENTRAL_FLAG_ABSENT" and "printf" in (e.remedy or ""))
            ck("★플래그 부재 거부는 아무것도 쓰지 않는다", idx_p.read_bytes() == idx_b and md_p.read_bytes() == md_b)
            (repo / CENTRAL_FLAG).write_text("selftest\n", encoding="utf-8")

            # ★기존 HINTS.md 마커 손상 → 덮어쓰지 않는다(⑬)
            md_p.write_text("# 카탈로그가 아닌 파일\n", encoding="utf-8")
            idx_b = idx_p.read_bytes()
            e, _ = err_of(lambda: derive(repo, "fx", kst, record_missing=True))
            ck("★기존 HINTS.md 마커 손상 → 중단 · index.json 도 쓰지 않는다",
               e is not None and e.code == "HINT_CATALOG_MARKER_BROKEN" and idx_p.read_bytes() == idx_b
               and md_p.read_text(encoding="utf-8") == "# 카탈로그가 아닌 파일\n")
            e, _ = err_of(lambda: derive(repo, "fx", kst, dry_run=True, record_missing=True))
            ck("★dry-run 도 마커 손상에서 멈춘다(실제 실행의 중단을 초록으로 예측하지 않는다)",
               e is not None and e.code == "HINT_CATALOG_MARKER_BROKEN")
            e, _ = err_of(lambda: derive(repo, "--upload-pack=touch x", kst))
            ck("★'-' 로 시작하는 원격 인자는 git 옵션이 되기 전에 거부", e is not None
               and e.code == "HINT_CATALOG_REMOTE_INVALID" and not (repo / "x").exists())
            e, _ = err_of(lambda: derive(repo, "fx", "2026-09-21 19:00"))
            ck("★주입 시각 형식 위반 거부", e is not None and e.code == "HINT_TIME_NOT_INJECTED")

        # ★원격 조회 실패 → 중단(캐시 폴백 금지) · allow_empty 로도 열리지 않는다 · 쓰기 0
        nope = str(root / "does-not-exist.git")
        e, _ = err_of(lambda: remote_hint_tags(repo, nope))
        ck("★음성대조 원격 조회 실패 → 중단(캐시 폴백 금지)", e is not None and e.code == "HINT_CATALOG_REMOTE_QUERY_FAILED")
        e, _ = err_of(lambda: remote_hint_tags(repo, nope, allow_empty=True))
        ck("★음성대조 allow_empty 여도 원격 조회 실패는 중단", e is not None and e.code == "HINT_CATALOG_REMOTE_QUERY_FAILED")
        idx_b = idx_p.read_bytes() if idx_p.exists() else None
        e, _ = err_of(lambda: derive(repo, nope, kst))
        ck("★원격 조회 실패 derive 는 쓰지 않는다", e is not None and (idx_p.read_bytes() if idx_p.exists() else None) == idx_b)
        empty = root / "empty.git"
        g(root, "init", "--bare", "-q", str(empty))
        e, _ = err_of(lambda: remote_hint_tags(repo, str(empty)))
        ck("★음성대조 원격 hint 태그 0건 → 빈 카탈로그 거부", e is not None and e.code == "HINT_CATALOG_REMOTE_EMPTY")
        e, _ = err_of(lambda: remote_hint_tags(repo, str(empty), allow_empty=True))
        ck("allow_empty=True → 0건 정상 통과", e is None and remote_hint_tags(repo, str(empty), allow_empty=True) == {})
        live = remote_hint_tags(repo, str(bare))
        ck("살아 있는 원격에서 (오브젝트, 피일) 광고를 읽는다",
           v6_tag in live and live[v6_tag]["object"] == g(repo, "rev-parse", f"refs/tags/{v6_tag}")
           and live[v6_tag]["peeled"] == v6_commit)

        # ★옛 4세그먼트·어느 세대에도 맞지 않는 이름 → 중단하지 않고 **그대로 나열**한다(plan §4.7 읽기 전용 수용 · P1:
        #   원격 태그는 리콜할 수 없으므로 이름 1건으로 중단하면 카탈로그가 영구히 멈춘다 · 옛 "★4세그먼트 거부"의 반전).
        odd = root / "odd.git"
        g(root, "init", "--bare", "-q", str(odd))
        for nm in ("hint/0.1/m/a", "hint/0.2/m/a/r/extra"):
            atag(repo, nm, "HEAD", "옛 이름의 brief\n")
            g(repo, "push", "-q", str(odd), f"refs/tags/{nm}:refs/tags/{nm}")
        e, _ = err_of(lambda: derive_entries(repo, remote_hint_tags(repo, str(odd)), remote=str(odd)))
        ents = {} if e is not None else {r["tag"]: r for r in derive_entries(repo, remote_hint_tags(repo, str(odd)),
                                                                             remote=str(odd))[0]}
        ck("★옛 4세그먼트 이름은 legacy-4seg 행 · 규약 밖 이름은 unknown 행(필드 추측 ✗) — 중단 ✗",
           e is None and ents.get("hint/0.1/m/a", {}).get("grammar") == naming.GRAMMAR_LEGACY_4SEG
           and ents["hint/0.1/m/a"].get("brief") == "옛 이름의 brief"
           and ents.get("hint/0.2/m/a/r/extra", {}).get("grammar") == naming.GRAMMAR_UNKNOWN
           and ents["hint/0.2/m/a/r/extra"].get("model") is None and "base_model" not in ents["hint/0.2/m/a/r/extra"])
        no_pl = ents.get("hint/0.1/m/a", {})
        ck("★PAYLOAD.json 없는 태그의 결손은 선언 0(`[]`·`—`)이 아니라 None · 표는 '미선언'",
           e is None and no_pl.get("missing") is None and no_pl.get("bench_mode") == BENCH_MODE_ABSENT
           and "PAYLOAD.json 을 읽지 못함" in no_pl.get("bench_mode_source", "")
           and f"| {MISSING_UNDECLARED} |" in render_rows([no_pl]))
        # ★2026-09-22 실측: 최초 형식 페이로드(`missing` 키 없음 — gb10-sim-h100 계열 2건)를 옛 파생기는 `—`(결손 0 선언)으로
        #   적었다. 선언하지 않은 것을 "0건 선언"으로 접지 않는다. 빈 목록 선언(`[]`)은 여전히 `—` 다.
        old_fmt = root / "oldfmt.git"
        g(root, "init", "--bare", "-q", str(old_fmt))
        nokey_tag, zero_tag = "hint/0.18.0/m/gb10-sim-h100/r", "hint/0.18.0/m/gb10-main-native/r"
        atag(repo, nokey_tag, payload_commit(repo, {"identity": {"gpu": "NVIDIA GB10"}}, "nokey"), "옛 형식\n")
        atag(repo, zero_tag, payload_commit(repo, {"missing": []}, "zero"), "결손 0 선언\n")
        g(repo, "push", "-q", str(old_fmt), f"refs/tags/{nokey_tag}:refs/tags/{nokey_tag}",
          f"refs/tags/{zero_tag}:refs/tags/{zero_tag}")
        e, _ = err_of(lambda: derive_entries(repo, remote_hint_tags(repo, str(old_fmt)), remote=str(old_fmt)))
        of = {} if e is not None else {r["tag"]: r for r in derive_entries(repo, remote_hint_tags(repo, str(old_fmt)),
                                                                           remote=str(old_fmt))[0]}
        ck("★missing 키 없는 옛 페이로드 = None('미선언') · 빈 목록 선언 = [](`—`)",
           e is None and of.get(nokey_tag, {}).get("missing") is None and of.get(zero_tag, {}).get("missing") == []
           and f"| {MISSING_UNDECLARED} |" in render_rows([of.get(nokey_tag, {})])
           and "| — |" in render_rows([of.get(zero_tag, {})]))
        # ★lightweight hint 태그 → 기본 모드는 거부(본문이 없으면 brief 를 지어낼 수 없다 · 옛 음성대조 이관)
        lw = root / "lw.git"
        g(root, "init", "--bare", "-q", str(lw))
        lw_tag = "hint/0.19.0/gpt-oss-20b/gb10-main-native/qmxfp4-len131072-kvfp8"
        g(repo, "tag", lw_tag)
        g(repo, "push", "-q", str(lw), f"refs/tags/{lw_tag}:refs/tags/{lw_tag}")
        e, _ = err_of(lambda: derive(repo, str(lw), kst))
        ck("★음성대조 lightweight 태그 거부 · 전용 종료코드가 아니다(원인을 섞지 않는다 · S7)",
           e is not None and e.code == "HINT_CATALOG_NOT_ANNOTATED" and e.exit_code != EXIT_LOCAL_TAG_OBJECT_MISSING)
        # record_missing(D10 경로 — continue 의 push 뒤 파생)은 타 발행처의 lightweight 1건에 영구히 막히지 않는다
        e, out = err_of(lambda: derive_entries(repo, remote_hint_tags(repo, str(lw)), remote=str(lw),
                                               record_missing=True))
        lw_rows = {} if e is not None else {r["tag"]: r for r in derive_entries(
            repo, remote_hint_tags(repo, str(lw)), remote=str(lw), record_missing=True)[0]}
        lwr = lw_rows.get(lw_tag, {})
        ck("★record-missing 은 lightweight 를 not-annotated 행(brief '—' · 합성 ✗)으로 싣는다 · 중단 ✗",
           e is None and lwr.get("object") == OBJECT_NOT_ANNOTATED and lwr.get("brief") == BRIEF_UNRECEIVED
           and lwr.get("anchor") == g(repo, "rev-parse", "HEAD") and lwr.get("published") is True)
        # 로컬에 없는 lightweight 원격 태그(피일 줄 없음) — anchor 는 광고된 오브젝트 그 자체(None 으로 비우지 않는다)
        lw_abs = "hint/0.19.0/gpt-oss-20b/gb10-sub-native/qmxfp4-len131072-kvfp8"
        lw_src = root / "lw-src"            # repo 와 오브젝트를 공유하지 않는 별도 저장소(= 타 발행처)
        lw_src.mkdir()
        g(lw_src, "init", "-q")
        other_c = payload_commit(lw_src, None, "lw-remote-only")
        g(lw_src, "tag", lw_abs, other_c)
        g(lw_src, "push", "-q", str(lw), f"refs/tags/{lw_abs}:refs/tags/{lw_abs}")
        e, _ = err_of(lambda: derive_entries(repo, remote_hint_tags(repo, str(lw)), remote=str(lw), record_missing=True))
        la = {} if e is not None else {r["tag"]: r for r in derive_entries(
            repo, remote_hint_tags(repo, str(lw)), remote=str(lw), record_missing=True)[0]}.get(lw_abs, {})
        ck("★로컬 부재 lightweight 원격 태그의 anchor = 광고 오브젝트(피일 줄 없음 ≠ 대상 모름)",
           e is None and la.get("object") == OBJECT_ABSENT_LOCAL and la.get("anchor") == other_c)
        md_p.unlink()      # 위에서 일부러 망가뜨린 HINTS.md(마커 가드 시험)를 치운다 — 새 파일 생성 경로
        e, out = err_of(lambda: derive(repo, str(lw), kst, dry_run=True, record_missing=True))
        ck("record-missing 의 not-annotated NOTE 가 소리낸다", e is None and "annotated 가 아닌 원격 태그 1건" in out)
    return bad
