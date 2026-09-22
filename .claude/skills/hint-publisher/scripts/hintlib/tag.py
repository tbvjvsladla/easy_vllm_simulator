"""hintlib.tag — hint 태그 봉인 · 이 태그 하나의 로컬 검증 · 정확한 refspec 1개 push
(plan_26092119 §4.8 · SPEC §5.8 · 옛 `hint_tag.py` 의 footer·seal·verify·push·자격증명 부분 이관).

무엇을 하나
    annotation(brief 1문단 + zip 포인터 + 증거 footer v1)을 짓고, hint 브랜치의 **페이로드 커밋**에 annotated 태그로
    봉인하고, push **전에** 그 태그 하나만 로컬에서 검증하고, `refs/tags/<tag>:refs/tags/<tag>` 하나만 원격에 민다.
    페이로드 트리(= 배포물)의 계약은 `branch.py` 가 소유하고 여기서는 그 판정(`commit_violations`)을 부른다.

불변식 (날짜 = 사고·결정 · 삭제하지 말고 옮겨 적는다)
    footer v1 (2026-07-25 리뷰 교정 blocker 3건)
        - 증거 바인딩은 **태그 오브젝트 본문**에 산다(소스 파일 ✗). `<!-- hint-evidence-binding:v1 … -->` 안의 평평한
          `key: value` 6필드 · 전부 필수 · 중복·미지 키 ✗ · **블록 부재(MISSING)와 형식 결함(MALFORMED)은 다른 code**.
        - 2026-09-03(plan_26090222 F-6a): `*_sha256` 3종 제거 — footer 는 증거의 **주소**를 묶는다. 무결성은 git 의 일
          (anchor = git 이 해시하는 커밋)이고, 손으로 옮긴 digest 는 첫 권위에서 갈라질 수만 있는 두 번째 권위다.
        - 2026-09-04(CP7.5): 폐기 키를 면제받던 태그가 사라져 `LEGACY_FOOTER_TAG_PINS`·`LEGACY_CERTIFICATE_REF_ALIASES`
          를 걷어냈다. 폐기 키는 **어떤 태그에서도** 차단된다(`retired_ok` 기본 = 빈 집합 · 읽기 호환은 명시 인자뿐).
    annotation (D4 · 2026-09-21)
        - 본문 = brief 1문단(00-hint §0.1 첫 문단 ≤3줄의 **기계 추출**) + zip 포인터 + footer. 그 밖의 바이트 0.
          검증은 모양을 정본 렌더와 **바이트 대조**하고, brief 를 페이로드 트리의 00-hint 에서 **다시 추출**해 대조한다 —
          옛 `unsealed_reason` 은 옛 본문 린터로 "도구가 만들었나"를 물었는데, 새 annotation 은 그 린터에 전부
          UNSEALED 로 읽힌다(코드맵 §1.6 ⚠). 새 질문은 "정본 모양인가 · brief 가 페이로드에서 왔나" 다.
    봉인 (seal)
        - `git tag -a -F - --cleanup=verbatim`: 메시지는 stdin 으로만(추적 파일에 쓰지 않는다 · claim C1).
          ★ 2026-08-20: git 기본 cleanup=strip 은 `#` 로 시작하는 줄을 **전부** 지워 `## N.` 제목이 사라졌다(git 2.43.0
          샌드박스 재현) — "49/49 태그에 헤딩 0개" 가 저자 탓으로 오인됐었다.
        - tagger = 합성 신원(`core.SYNTHETIC_*`) · 시각 = 주입(`GIT_COMMITTER_DATE`). 실명 override 경로는 두지 않는다
          (2026-09-01 tagger 실명 유출을 push 직전에 회수한 선례). 같은 입력 → 같은 태그 SHA(K2).
        - `--no-sign` + `tag.gpgSign=false`: 운영자 서명이 끼면 본문이 정본 모양에서 벗어나고 SHA 가 키의 함수가 된다.
        - ★ 2026-09-22: 봉인 사후조건이 매번 실패했다 — 태그 본문을 **텍스트 모드 + strip** 으로 읽어 끝 개행이
          사라졌다. 이제 `cat-file` **바이트**를 그대로 읽고, 기대 오브젝트 바이트(`_tag_object_bytes`)의 SHA 와
          실제 SHA 를 대조한다. 멱등 재개(X13)는 같은 페이로드 커밋 · 같은 annotation 바이트 · 합성 tagger 일 때만이다
          (tagger 시각은 빼고 본다 — 끊긴 continue 를 새 주입 시각으로 다시 돌린 경우) · 그 밖은 HINT_NAME_COLLISION.
    검증 (verify_local · D10)
        - **이 태그 하나만** 본다. 옛 verify/push 는 로컬 태그 전수를 봐서 과거·타 PC 결함(`REF_ABSENT` 18 등)이 신규
          발행을 영구히 막았다(F12). 게이트 범위 = 이번 발행분.
        - 판정은 **code 로만** 한다(각 항목 `CODE: 설명` · 첫 `:` 앞). 2026-09-01 감사 ①-②: 메시지 substring 판정에서
          발행자가 통제하는 footer 값으로 차단이 경고로 강등됐다(주입 실증 blocking 1→0).
        - taggerdate 기반 면제 없음(2026-09-01 감사 ①-③ "피감사자가 자기 검사를 끌 수 있는 구조").
        - 앵커 게이트(2026-09-07): 태그는 hint 브랜치 **페이로드 커밋**을 가리킨다 — `branch.require_payload_anchor`.
    push (O2 · 2026-08-20 · 2026-09-04)
        - refspec 은 `refs/tags/<그 태그>:refs/tags/<그 태그>` **정확히 1개**. `--tags` ✗(로컬 `last-good-*` 롤백 앵커
          유출 · 계약 C5) · `hint/` 접두 가드(2026-08-20 `--tag '*'` → `refs/tags/*` 를 이 가드가 잡았다) · glob ✗
          (fnmatch 선택과 git glob 이 `?`·`[…]` 에서 갈려 검증 집합 ≠ 전송 집합) · 브랜치 ✗(O2: 발행 push 는 태그뿐).
        - 검증 범위 = 전송 범위(2026-08-20 "drift 16건이 hy3 7건을 막았다" → D10): push 는 그 태그의 verify_local 이
          0 일 때만 민다. 로그에는 **실제 refspec** 을 찍는다(B11: 옛 로그는 늘 `refs/tags/hint/*` 라고 적었다).
        - 원격에 같은 이름의 **다른** 오브젝트가 있으면 밀지 않는다(강제 ✗ · P1 리콜 금지 — 개정판은 새 이름).
        - ★ 2026-09-22: refspec 이 정확히 1개여도 운영자 설정 `push.followTags=true` 는 밀리는 커밋에서 닿는
          **다른 annotated 태그**(검증 안 된 옛 hint 태그 · 로컬 `last-good-*`)를 함께 보낸다 — 격리 저장소 실측으로
          태그 1개 push 가 3개를 올렸다. `--tags` 금지(C5)를 설정 한 줄이 조용히 되살리는 경로이므로 push 는 늘
          `--no-follow-tags`(+ `--no-recurse-submodules`) 를 명시한다. 설정에 맡기지 않는다.
    자격증명 (2026-09-04 신설 · 2026-09-21 스킴 인식)
        - 무인 push 에 자격증명 배선이 아예 없어서 `could not read Username` 이 세션마다 재발했고, `envs/.env` 의
          `GITHUB_TOKEN` 을 읽는 코드가 없었다("만든 것과 도는 것은 다르다").
        - ★ 토큰은 argv·URL·로그·reflog·remote.url 어디에도 두지 않는다. helper 가 **환경변수**에서 읽는다. URL 에 박는
          형태는 쓰지 않고, 원격 URL 에 userinfo 가 있으면 거부한다. `credential.helper=`(빈 값)로 상속 helper 를 먼저
          비운다. `GIT_TERMINAL_PROMPT=0`(프롬프트 대기 대신 즉시 실패 · 무인 교착 방지).
        - ★ dry-run 도 실패를 삼키지 않는다(2026-09-04 감사 C-3: 옛 `check=not dry_run` 은 인증 실패를 exit 0 으로 냈다).
        - 스킴 인식(B12 · H7): https 만 토큰이 필요하다 — ssh·scp형(`user@host:path`)·로컬 경로는 불요. 옛 코드는 SSH
          원격에서도 토큰이 없으면 죽었다. 판정은 **선언된** URL(`remote.<n>.pushurl` → `url`, insteadOf 전개 전)로 한다:
          전개 후 URL 로 판정하면 insteadOf 한 줄이 자격증명 요구를 조용히 끈다. 대가: https 를 ssh 로 돌리는
          insteadOf 설정에서는 불요한 토큰을 요구한다(fail-closed 방향 · 그때는 ssh 원격 이름을 직접 쓴다).
        - 평문 http 원격에는 토큰을 보내지 않는다(거부).
        - `git_push_authenticated` 는 **공개 API** 다(upstream `push_branches.py` 가 브랜치 push 에 소비 — 두 번째 사본을
          두지 않는다). 성공 = CompletedProcess · 실패 = HintError(메시지에 사유 · 자격증명 부재는 "자격증명").
"""
from __future__ import annotations

import contextlib
import io
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlsplit

from . import branch, core, naming, pii

# ── footer v1 (옛 hint_tag `_FOOTER_*` 이관 · wire format 불변) ─────────────────────────────────
FOOTER_MARKER_OPEN = "<!-- hint-evidence-binding:v1"
FOOTER_MARKER_CLOSE = "-->"
FOOTER_VERSION = "1"
# 순서가 곧 wire format 이다(claim C3 가 이 튜플을 고정한다).
FOOTER_FIELDS = ("version", "tag", "topology", "anchor", "manifest_ref", "certificate_ref")
# 2026-09-03 에 스키마에서 제거된 digest 키. 닫힌 목록(tripwire) — 이름을 붙여 두는 이유는 "무엇을 차단하는가" 를
# 자체검사가 단언하기 위해서다. 읽기 호환은 `parse_footer(retired_ok=…)` 명시 인자뿐이고 값은 **읽고 버린다**.
RETIRED_FOOTER_KEYS = frozenset({"manifest_sha256", "identity_sha256", "certificate_sha256"})
_FOOTER_BLOCK_RE = re.compile(re.escape(FOOTER_MARKER_OPEN) + r"\s*\n(?P<body>.*?)\n" + re.escape(FOOTER_MARKER_CLOSE),
                              re.S)
_FOOTER_LINE_RE = re.compile(r"^([a-z0-9_]+): (.*)$")
_SHA40_RE = re.compile(r"^[0-9a-f]{40}$")

# ── annotation (SPEC §2.2 · D4) ────────────────────────────────────────────────────────────────
ANNOTATION_POINTER = "전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다."
ANNOTATION_BRIEF_MAX_LINES = 3

# ── push 자격증명 ──────────────────────────────────────────────────────────────────────────────
TOKEN_KEY = "GITHUB_TOKEN"            # 운영자가 두는 이름(환경변수 또는 비추적 envs/.env)
TOKEN_ENV_FILE_REL = "envs/.env"      # 비추적 평면(.gitignore) — 배포 클론에는 없다
PUSH_TOKEN_ENV = "HINT_PUSH_TOKEN"    # helper 가 읽을 임시 변수명(원본 이름과 분리 · push 프로세스 env 에만)
_GLOB_CHARS = frozenset("*?[")
_SCHEME_RE = re.compile(r"^([A-Za-z][A-Za-z0-9+.-]*)://")
# 선언된 URL 스킴 → 전송. 여기 없는 스킴(원격 helper `x::` 포함)은 판정하지 않고 거부한다(판정 불가 ≠ 토큰 불요).
_SCHEME_TRANSPORT = {"https": "https", "http": "http", "ssh": "ssh", "git+ssh": "ssh", "ssh+git": "ssh",
                     "file": "local", "git": "git"}


def _log(msg: str) -> None:
    print(f"[hint] {msg}", file=sys.stderr)


def _problem(code: str, msg: str) -> str:
    return f"{code}: {msg}"


def problem_codes(problems) -> list[str]:
    """verify_local 결과 → code 목록. **판정은 이것으로만 한다**(메시지 substring ✗ · 감사 ①-②)."""
    return sorted({str(p).split(":", 1)[0] for p in problems})


class HintProblemsError(core.HintError):
    """여러 검사 결과를 한 번에 거부하는 실패(HINT_SEAL_REFUSED · HINT_PUSH_UNVERIFIED).

    `problems` = `CODE: 설명` 목록 원본. 호출부는 `problem_codes(e.problems)` 로 **code 를 구조로** 받는다 —
    메시지 본문에서 하위 code 를 찾아 분류하면 감사 ①-② 의 substring 판정이 되살아난다(발행자가 통제하는 footer·
    brief 문자열이 메시지에 섞인다)."""

    def __init__(self, code: str, message: str, remedy: str | None, problems):
        super().__init__(code, message, remedy)
        self.problems = tuple(problems)


def _fail_problems(code: str, what: str, problems, remedy: str):
    raise HintProblemsError(code, f"{what} {len(problems)}건:\n  " + "\n  ".join(problems), remedy, problems)


# ── footer ──────────────────────────────────────────────────────────────────────────────────
class HintEvidenceBindingError(core.HintError):
    """footer 결함. 안정 (code, message) — 호출부는 **code 로만** 분류한다. HintError 라서 CLI 가 한 자리에서 처리한다."""

    def __init__(self, code: str, message: str):
        super().__init__(code, message, "annotation 은 손으로 쓰지 않는다 — tag.annotation(brief, fields) 로 짓는다.")


def _malformed(msg: str):
    raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED", msg)


def _checked_fields(fields) -> dict[str, str]:
    """6필드 정합(빌드·파싱 공용 — 두 경로의 규칙이 같아야 왕복이 성립한다)."""
    if not isinstance(fields, dict):
        _malformed(f"footer 필드는 사전이어야 한다: {type(fields).__name__}")
    missing = [k for k in FOOTER_FIELDS if k not in fields]
    extra = sorted(set(fields) - set(FOOTER_FIELDS))
    if missing or extra:
        _malformed(f"footer 필드 불일치 — 누락 {missing} · 초과 {extra}")
    out: dict[str, str] = {}
    for k in FOOTER_FIELDS:
        v = fields[k]
        if not isinstance(v, str):
            _malformed(f"footer 필드 {k!r} 는 문자열이어야 한다: {type(v).__name__}")
        v = v.strip()
        # 줄바꿈은 평평한 key: value 를 깨고, `-->`·`<!--` 는 블록 경계를 위조한다.
        if any(bad in v for bad in ("\n", "\r", "-->", "<!--")):
            _malformed(f"footer 필드 {k!r} 에 줄바꿈·주석 경계 문자열이 있다")
        out[k] = v
    if out["version"] != FOOTER_VERSION:
        _malformed(f"지원하지 않는 footer version: {out['version']!r}")
    if not _SHA40_RE.fullmatch(out["anchor"]):
        _malformed(f"anchor 는 40자 커밋 SHA 여야 한다: {out['anchor']!r}")
    for k in ("tag", "topology", "manifest_ref", "certificate_ref"):
        if not out[k]:
            _malformed(f"footer 필드 {k!r} 가 비었다")
    for k in ("manifest_ref", "certificate_ref"):
        # 주소는 저장소(또는 manifest 디렉터리) 상대다 — 절대경로는 운영자 환경 지문이고 수신 클론에서 뜻이 없다.
        if out[k].startswith("/"):
            _malformed(f"footer 필드 {k!r} 는 상대경로여야 한다")
    return out


def build_footer(**fields: str) -> str:
    """footer v1 블록(끝 개행 포함). 6필드 순서 고정 · 폐기 키를 쓰는 경로는 없다(자체검사가 단언)."""
    clean = _checked_fields(fields)
    return "\n".join([FOOTER_MARKER_OPEN, *(f"{k}: {clean[k]}" for k in FOOTER_FIELDS), FOOTER_MARKER_CLOSE]) + "\n"


def parse_footer(text: str, *, retired_ok: frozenset[str] = frozenset()) -> dict[str, str]:
    """footer 를 **정확히 하나** 엄격 파싱. MISSING = 여는 표지 부재 · 그 밖의 모든 결함 = MALFORMED.

    옛 파서는 첫 블록을 채택했다(`search`). 새 봉인은 footer 가 1개이므로 둘 이상이면 위조로 읽는다 — brief 가
    footer 를 흉내 내면 "첫 블록 우선" 규칙이 가짜 주소를 채택하게 된다. 여는 표지는 있는데 닫는 줄이 없으면 부재가
    아니라 결함이다(MISSING ≠ MALFORMED 의 경계를 여기서 긋는다).
    """
    if not isinstance(text, str):
        _malformed(f"footer 입력은 문자열이어야 한다: {type(text).__name__}")
    opens = text.count(FOOTER_MARKER_OPEN)
    if opens == 0:
        raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MISSING", "hint-evidence-binding footer 가 없다")
    blocks = list(_FOOTER_BLOCK_RE.finditer(text))
    if not blocks:
        _malformed("footer 여는 표지는 있으나 닫는 `-->` 줄이 없다")
    if opens != 1 or len(blocks) != 1:
        _malformed(f"footer 가 둘 이상이다(여는 표지 {opens}개)")
    fields: dict[str, str] = {}
    for raw in blocks[0].group("body").split("\n"):
        line = raw.rstrip("\r")
        if not line.strip():
            continue
        m = _FOOTER_LINE_RE.match(line)
        if not m:
            _malformed(f"해석할 수 없는 footer 줄: {line[:80]!r}")
        key, value = m.group(1), m.group(2).strip()
        if key not in FOOTER_FIELDS:
            if key in RETIRED_FOOTER_KEYS and key in retired_ok:
                continue  # 폐기 키 — 명시 인자에 한해 읽고 버린다(값은 판정에 쓰지 않는다)
            _malformed(f"알 수 없는 footer 키: {key!r}")
        if key in fields:
            _malformed(f"중복 footer 키: {key!r}")
        fields[key] = value
    missing = [k for k in FOOTER_FIELDS if k not in fields]
    if missing:
        _malformed(f"footer 필수 필드 누락: {missing}")
    return _checked_fields(fields)


# ── annotation ────────────────────────────────────────────────────────────────────────────────
def _canonical_brief(brief) -> str:
    """brief 정규형: 앞뒤 공백 제거 · 줄 끝 공백 제거 · 한 문단(빈 줄 ✗) · ≤3줄 · 주석 경계·포인터 문구 ✗."""
    if not isinstance(brief, str) or not brief.strip():
        core.fail("HINT_ANNOTATION_BRIEF_ABSENT", "annotation brief 가 비었다(00-hint.md §0.1 첫 문단).",
                  "template.brief(payload_dir) 로 기계 추출한다.")
    lines = [ln.rstrip() for ln in brief.strip().split("\n")]
    if any(not ln.strip() for ln in lines):
        core.fail("HINT_ANNOTATION_BRIEF_SHAPE", "brief 는 한 문단이다(빈 줄 ✗) — 카탈로그는 첫 문단만 읽는다.")
    if len(lines) > ANNOTATION_BRIEF_MAX_LINES:
        core.fail("HINT_ANNOTATION_BRIEF_TOO_LONG", f"brief 는 최대 {ANNOTATION_BRIEF_MAX_LINES}줄이다({len(lines)}줄).")
    text = "\n".join(lines)
    if "<!--" in text or "-->" in text or "\r" in text or ANNOTATION_POINTER in text:
        # `<!--` 가 카탈로그 렌더에서 뒤 행을 삼킨 사고(⑤ · 59항목 중 11 brief 손상)와 footer 위조를 함께 막는다.
        core.fail("HINT_ANNOTATION_BRIEF_SHAPE", "brief 에 주석 경계·CR·포인터 문구가 있다.")
    return text


def annotation(brief: str, fields: dict) -> str:
    """태그 메시지 정본(SPEC §2.2): `<brief>\\n\\n<포인터>\\n\\n<footer>`. 이 함수의 출력만 봉인된다."""
    return f"{_canonical_brief(brief)}\n\n{ANNOTATION_POINTER}\n\n{build_footer(**fields)}"


def parse_annotation(body: str) -> dict:
    """annotation → {"brief", "footer"}. footer 결함은 그 code(MISSING/MALFORMED) · 모양이 정본 렌더와 한 바이트라도
    다르면 HINT_ANNOTATION_SHAPE(손 편집 · 서명 덧붙임 · 뒤꼬리)."""
    footer = parse_footer(body)
    sep = f"\n\n{ANNOTATION_POINTER}\n\n"
    i = body.find(sep)
    if i < 0:
        core.fail("HINT_ANNOTATION_SHAPE", "zip 포인터 문단이 없다 — 수신자는 zip 을 열 이유를 모른다.")
    brief = body[:i]
    try:
        canonical = annotation(brief, footer)
    except core.HintError as e:
        core.fail("HINT_ANNOTATION_SHAPE", f"brief 가 정본 규칙을 어긴다({e.code}: {e.message})")
    if canonical != body:
        core.fail("HINT_ANNOTATION_SHAPE", "annotation 이 정본 모양(brief · 포인터 · footer)과 바이트가 다르다 — "
                                           "손 편집·서명·덧붙임은 봉인 산출물이 아니다.")
    return {"brief": _canonical_brief(brief), "footer": footer}


# ── 태그 오브젝트 읽기 ─────────────────────────────────────────────────────────────────────────
def _tag_ref(tag: str) -> str:
    return f"refs/tags/{tag}"


def _git_b(repo: Path, *args: str, input_bytes: bytes | None = None,
           env_extra: dict | None = None) -> subprocess.CompletedProcess:
    """바이트 입출력 git(한글 메시지를 로캘 인코딩에 맡기지 않는다 · 결과를 strip 하지 않는다)."""
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    env.update(env_extra or {})
    try:
        return subprocess.run(["git", *args], input=input_bytes, capture_output=True, cwd=str(repo), env=env)
    except OSError as e:
        core.fail("HINT_GIT_UNAVAILABLE", f"git 실행 불가: {e}")


def _ref_sha(repo: Path, ref: str) -> str | None:
    r = core.git(repo, "rev-parse", "--verify", "--quiet", ref, check=False)
    sha = r.stdout.strip()
    return sha if r.returncode == 0 and sha else None


def tag_ref_kind(repo: Path, tag: str) -> str | None:
    """refs/tags/<tag> 가 직접 가리키는 오브젝트 종류(`tag` = annotated · `commit` = lightweight · None = 부재)."""
    sha = _ref_sha(repo, _tag_ref(tag))
    if sha is None:
        return None
    return core.git(repo, "cat-file", "-t", sha, check=False).stdout.strip() or None


def read_tag(repo: Path, tag: str) -> dict | None:
    """annotated 태그 → {object_sha, target, target_type, name, tagger, body, raw}. 부재·lightweight = None
    (구분이 필요하면 `tag_ref_kind`). body 는 **바이트 그대로의 UTF-8 해독**이다 — strip 하지 않는다(2026-09-22)."""
    sha = _ref_sha(repo, _tag_ref(tag))
    if sha is None or core.git(repo, "cat-file", "-t", sha, check=False).stdout.strip() != "tag":
        return None
    raw = core.git_bytes(repo, "cat-file", "tag", sha)
    head, sep, body = raw.partition(b"\n\n")
    if not sep:
        core.fail("HINT_TAG_OBJECT_MALFORMED", f"태그 오브젝트에 헤더/본문 구분이 없다: {tag}")
    try:
        head_text, body_text = head.decode("utf-8"), body.decode("utf-8")
    except UnicodeDecodeError:
        core.fail("HINT_TAG_OBJECT_MALFORMED", f"태그 오브젝트가 UTF-8 이 아니다: {tag}")
    headers: dict[str, str] = {}
    for line in head_text.split("\n"):
        k, _, v = line.partition(" ")
        headers.setdefault(k, v)
    target = headers.get("object", "")
    if not _SHA40_RE.fullmatch(target):
        core.fail("HINT_TAG_OBJECT_MALFORMED", f"태그 대상이 40자 SHA 가 아니다: {target!r}")
    return {"object_sha": sha, "target": target, "target_type": headers.get("type"), "name": headers.get("tag"),
            "tagger": branch.parse_ident_headers(head_text).get("tagger"), "body": body_text, "raw": raw}


# ── 이름 · ref 형식 ───────────────────────────────────────────────────────────────────────────
def check_ref_format(repo: Path, tag: str) -> None:
    """push·봉인 대상 태그 이름의 **ref 수준** 검사(문법 세대 판정은 naming.validate_new_name).

    `hint/` 접두만 · `refs/` 로 시작하는 입력 ✗(브랜치 refspec 위장 · `refs/heads/hint`) · glob 문자 ✗ ·
    refspec 구분자 `:` ✗ · `git check-ref-format refs/tags/<tag>` 통과."""
    if not isinstance(tag, str) or not tag:
        core.fail("HINT_TAG_NAME_ABSENT", "태그 이름이 비었다.")
    if tag.startswith("refs/") or not tag.startswith(core.HINT_TAG_PREFIX):
        core.fail("HINT_TAG_NAMESPACE", f"hint/ 밖 이름은 다루지 않는다(브랜치·다른 태그 ✗): {tag!r}",
                  "태그 이름은 `hint/<vllm>/<model>/<arch>/<recipe>` 이다(refs/ 접두 없이).")
    if any(c in tag for c in _GLOB_CHARS):
        core.fail("HINT_TAG_GLOB_FORBIDDEN", f"glob 문자가 있는 태그 이름: {tag!r}",
                  "정확한 태그 1개만 다룬다(glob 선택과 git glob 은 `?`·`[…]` 에서 갈린다).")
    if ":" in tag or any(c.isspace() or ord(c) < 0x20 for c in tag):
        core.fail("HINT_TAG_REF_FORMAT", f"refspec 구분자·공백·제어문자가 있는 태그 이름: {tag!r}")
    if core.git(repo, "check-ref-format", _tag_ref(tag), check=False).returncode != 0:
        core.fail("HINT_TAG_REF_FORMAT", f"git check-ref-format 이 거부한다: {tag!r}")


def _local_hint_refs(repo: Path) -> list[str]:
    out = core.git(repo, "for-each-ref", "--format=%(refname)", "refs/tags/hint", check=False)
    return [ln.strip() for ln in out.stdout.splitlines() if ln.strip()]


def _prefix_conflict(ref: str, refs) -> str | None:
    """디렉터리/파일 충돌(`a/b` 와 `a/b/c` 는 공존할 수 없다 · 옛 validate_name D/F prefix 검사)."""
    for r in refs:
        if r != ref and (r.startswith(ref + "/") or ref.startswith(r + "/")):
            return r
    return None


def name_collision(repo: Path, tag: str, remote: str | None = None) -> str | None:
    """이름 충돌(X13): `local` · `local-prefix` · `remote` · `remote-prefix` · None(충돌 없음).

    로컬 태그가 **이 draft 가 봉인한 것**인지(재개)는 여기서 묻지 않는다 — `seal` 이 대상·annotation 바이트·tagger 로 판정한다.
    원격 조회는 `ls-remote` 읽기뿐이다. 조회 실패는 충돌 없음으로 접지 않고 실패한다(부재 ≠ 조회 불가)."""
    check_ref_format(repo, tag)
    ref = _tag_ref(tag)
    if _ref_sha(repo, ref) is not None:
        return "local"
    if _prefix_conflict(ref, _local_hint_refs(repo)):
        return "local-prefix"
    if remote is not None:
        refs = _ls_remote_refs(repo, remote, "refs/tags/hint/*")
        if ref in refs:
            return "remote"
        if _prefix_conflict(ref, refs):
            return "remote-prefix"
    return None


# ── 봉인 ─────────────────────────────────────────────────────────────────────────────────────
def _tag_object_bytes(anchor: str, tag: str, utc: str, message: str) -> bytes:
    """`git tag -a` 가 이 입력으로 써야 하는 오브젝트 바이트(기대값). tagger 줄 = 합성 신원 + 주입 시각(+0000)."""
    epoch = int(core.parse_utc(utc).timestamp())
    head = f"object {anchor}\ntype commit\ntag {tag}\ntagger {branch.synthetic_identity()} {epoch} +0000\n\n"
    return head.encode("utf-8") + message.encode("utf-8")


def _hash_tag_object(repo: Path, data: bytes) -> str:
    r = _git_b(repo, "hash-object", "-t", "tag", "--stdin", input_bytes=data)
    sha = r.stdout.decode("ascii", "replace").strip()
    if r.returncode != 0 or not _SHA40_RE.fullmatch(sha):
        core.fail("HINT_TAG_OBJECT_MALFORMED", f"기대 태그 오브젝트를 해시할 수 없다: "
                                               f"{r.stderr.decode('utf-8', 'replace').strip()}")
    return sha


def _brief_problems(repo: Path, anchor: str, brief: str) -> list[str]:
    """annotation brief == 페이로드 트리 00-hint.md §0.1 첫 문단의 기계 추출(`template.brief` · 추출기 1벌)."""
    data = branch.read_blob(repo, anchor, "00-hint.md")
    if data is None:
        return []  # 필수 부재는 tree_violations 가 보고한다
    from . import template  # 지연 import — 봉인·검증에서만 필요하다
    try:
        with tempfile.TemporaryDirectory(prefix="hint-brief-") as td:
            (Path(td) / "00-hint.md").write_bytes(data)
            extracted = _canonical_brief(template.brief(Path(td)))
    except core.HintError as e:
        return [_problem(e.code, f"00-hint.md §0.1 에서 brief 를 추출할 수 없다 — {e.message}")]
    except (UnicodeDecodeError, OSError) as e:
        return [_problem("HINT_BRIEF_ABSENT", f"00-hint.md 를 읽을 수 없다({type(e).__name__})")]
    if extracted != brief:
        return [_problem("HINT_ANNOTATION_BRIEF_MISMATCH",
                         "annotation brief 가 페이로드 00-hint.md §0.1 첫 문단의 추출과 다르다 — brief 는 손으로 쓰지 않는다.")]
    return []


def _facts_problems(repo: Path, anchor: str, tag: str) -> list[str]:
    """PAYLOAD.naming 에 naming facts(SPEC §3.3)가 실려 있으면 어휘표로 **다시 파생**해 태그와 대조한다.
    (축 재조립 대조는 `branch.payload_doc_problems` 가 늘 한다 · 이것은 facts 가 있을 때의 한 단계 더 깊은 대조)"""
    doc, _err = branch.read_json_blob(repo, anchor, branch.PAYLOAD_JSON)
    nm = doc.get("naming") if isinstance(doc, dict) else None
    facts = nm.get("facts") if isinstance(nm, dict) else None
    if not isinstance(facts, dict):
        return []
    try:
        derived = naming.derive_name(facts, naming.load_vocab(repo))
    except core.HintError as e:
        return [_problem(e.code, f"PAYLOAD.naming.facts 재파생 실패 — {e.message}")]
    if derived.tag != tag:
        return [_problem("HINT_DERIVED_NAME_MISMATCH", f"naming facts 재파생 {derived.tag!r} ≠ 태그 {tag!r}")]
    return []


def _anchor_problems(repo: Path, tag: str, anchor: str, brief: str | None, body: str, terms: list[str]) -> list[str]:
    """봉인 전(seal)·후(verify_local) 공용: 페이로드 커밋 계약 + brief 원천 + annotation PII + 신원 tripwire + facts."""
    probs = list(branch.commit_violations(repo, anchor, tag, terms))
    if brief is not None:
        probs += _brief_problems(repo, anchor, brief)
    hits = pii.scan_text(body, terms, profile="deploy")
    if hits:
        probs.append(_problem("HINT_TAG_PII", "annotation PII(배포 4종+리터럴): " + pii.render_hits(hits, limit=5)))
    if branch.identity_pii_hits(terms):
        probs.append(_problem("HINT_TAGGER_IDENTITY_PII", "합성 tagger 신원이 PII 목록에 걸린다(tripwire)"))
    probs += _facts_problems(repo, anchor, tag)
    return probs


def _rollback_created_tag(repo: Path, tag: str, sha: str) -> None:
    """이 호출이 방금 만든 태그 **하나**를 CAS 로 지운다(사후조건 실패 = 부분 적용 ✗). 남의 값이면 지우지 않는다."""
    core.git(repo, "update-ref", "-d", _tag_ref(tag), sha, check=False)


def seal(repo: Path, tag: str, anchor: str, message: str, *, generated_utc: str) -> str:
    """annotated 태그 봉인 → 태그 오브젝트 SHA. 모든 거부는 태그 생성 전(부작용 0)에 난다.

    순서: ① ref·문법(v6)·시각·앵커 모양 ② annotation 정본 모양 + footer.tag/anchor == 봉인 대상
          ③ 기존 태그: 같은 대상·같은 annotation 바이트·합성 tagger = 재개(멱등 · 기존 오브젝트 반환) ·
             그 밖(lightweight 포함) = HINT_NAME_COLLISION(X13)
          ④ 앵커 게이트(v5 코드) · 페이로드 커밋 계약 · brief 원천 · PII(annotation · 신원) — 하나라도 있으면 HINT_SEAL_REFUSED
          ⑤ `git tag -a --no-sign --cleanup=verbatim -F -`(합성 tagger · 주입 시각) ⑥ 사후조건: SHA·바이트 = 기대값
    """
    check_ref_format(repo, tag)
    naming.validate_new_name(tag)
    utc = core.require_utc(generated_utc)
    if not isinstance(anchor, str) or not _SHA40_RE.fullmatch(anchor):
        core.fail("HINT_ANCHOR_SHAPE", f"앵커는 40자 커밋 SHA 여야 한다: {anchor!r}")
    if not isinstance(message, str):
        core.fail("HINT_ANNOTATION_SHAPE", "태그 메시지는 문자열이어야 한다(tag.annotation 산출물).")
    parsed = parse_annotation(message)
    footer = parsed["footer"]
    if footer["tag"] != tag:
        core.fail("HINT_EVIDENCE_BINDING_TAG_MISMATCH", f"footer.tag={footer['tag']!r} ≠ 봉인 대상 {tag!r}")
    if footer["anchor"] != anchor:
        core.fail("HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH",
                  f"footer.anchor={footer['anchor'][:12]} ≠ 봉인 앵커 {anchor[:12]} — footer 의 anchor 는 이 태그가 "
                  "가리키는 **페이로드 커밋**이다(K3).")
    cur = _ref_sha(repo, _tag_ref(tag))
    if cur is not None:
        # X13 재개 판정: **이 draft 가 봉인한 것** = 같은 페이로드 커밋 · 같은 annotation 바이트 · 합성 tagger.
        # tagger 시각은 판정에 넣지 않는다 — 태그 생성과 state 기록 사이에서 끊긴 뒤 새 `--generated-utc` 로 재실행하면
        # 시각만 달라진다. 그때는 기존 봉인을 그대로 쓴다(태그는 불변 · 새로 만들지 않는다).
        old = read_tag(repo, tag)
        if (old is not None and old["target"] == anchor and old["body"] == message
                and (old["tagger"] or {}).get("ident") == branch.synthetic_identity()):
            same_time = cur == _hash_tag_object(repo, _tag_object_bytes(anchor, tag, utc, message))
            _log(f"{tag} 는 이미 이 페이로드·annotation 으로 봉인돼 있다({cur[:12]}"
                 + (")" if same_time else " · tagger 시각만 다르다 — 기존 봉인 유지)") + " — 재개(새 오브젝트 없음).")
            return cur
        why = "lightweight 태그" if old is None else " · ".join(
            w for w, differs in (("대상 커밋", old["target"] != anchor), ("본문", old["body"] != message),
                                 ("tagger", (old["tagger"] or {}).get("ident") != branch.synthetic_identity()))
            if differs) or "오브젝트"
        core.fail("HINT_NAME_COLLISION", f"로컬에 같은 이름의 다른 태그가 있다({tag} · 다른 것: {why}) — 태그는 불변이다.",
                  "이 draft 가 봉인한 것이 아니다. 같은 셀의 개정판은 새 이름으로 발행한다(P1 리콜 금지 · 덮어쓰기 ✗).")
    branch.require_payload_anchor(repo, tag, anchor)
    terms = pii.require_terms(repo)
    probs = sorted(set(_anchor_problems(repo, tag, anchor, parsed["brief"], message, terms)))
    if probs:
        _fail_problems("HINT_SEAL_REFUSED", "봉인 전 검사", probs,
                       "각 항목의 code 를 고친다(페이로드는 hint.py publish 스캐폴드 · 커밋은 hint.py continue).")
    sha = _create_tag(repo, tag, anchor, message, utc)
    _log(f"봉인 {tag} → {sha[:12]} (대상 {anchor[:12]} · tagger {core.SYNTHETIC_NAME})")
    return sha


def _create_tag(repo: Path, tag: str, anchor: str, message: str, utc: str) -> str:
    """git 수준 생성 + 사후조건(봉인의 마지막 두 단계 · 검사는 호출자 몫). 반환 = 태그 오브젝트 SHA.

    메시지는 stdin 으로만(`"-F", "-"`) · `--cleanup=verbatim`(2026-08-20: 기본 strip 이 `#` 줄·빈 줄·줄끝 공백을 지웠다) ·
    서명 ✗ · tagger = 합성 신원 + 주입 시각. 결과 오브젝트를 **바이트로** 다시 읽어 기대값과 같아야 한다 — 다르면 방금
    만든 태그 하나를 CAS 로 되돌린다(부분 적용 ✗)."""
    expected = _tag_object_bytes(anchor, tag, utc, message)
    want = _hash_tag_object(repo, expected)
    env = {"GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL,
           "GIT_COMMITTER_DATE": core.git_date(utc)}
    r = _git_b(repo, "-c", "tag.gpgSign=false", "-c", "tag.forceSignAnnotated=false",
               "tag", "-a", "--no-sign", "--cleanup=verbatim", "-F", "-", tag, anchor,
               input_bytes=message.encode("utf-8"), env_extra=env)
    if r.returncode != 0:
        if _ref_sha(repo, _tag_ref(tag)) is not None:
            core.fail("HINT_NAME_COLLISION", f"봉인 도중 같은 이름의 태그가 생겼다(경합): {tag}")
        core.fail("HINT_TAG_CREATE_FAILED", f"git tag 실패(rc={r.returncode}): "
                                            f"{r.stderr.decode('utf-8', 'replace').strip()}")
    got = _ref_sha(repo, _tag_ref(tag))
    raw = core.git_bytes(repo, "cat-file", "tag", got, check=False) if got else None
    if got != want or raw != expected:
        if got:
            _rollback_created_tag(repo, tag, got)
        core.fail("HINT_TAG_SEAL_POSTCONDITION",
                  f"봉인된 오브젝트가 기대 바이트와 다르다({tag} · 기대 {want[:12]} · 실제 {str(got)[:12]}) — 방금 만든 태그를 "
                  "되돌렸다(부분 적용 ✗).", "git 설정(서명·cleanup·신원)이 끼어들었는지 확인한다.")
    return want


# ── 로컬 검증 (이 태그 하나 · D10) ────────────────────────────────────────────────────────────
def _lint_findings(fn, repo: Path, anchor: str, tag: str) -> list[str]:
    try:
        found = fn(repo=repo, anchor=anchor, tag=tag) or []
    except core.HintError as e:
        return [_problem(e.code, e.message)]
    out = []
    for f in found:
        if isinstance(f, dict):
            code, msg = f.get("code"), f.get("message", "")
        elif isinstance(f, str):
            code, _, msg = f.partition(":")
        else:
            code, msg = getattr(f, "code", None), getattr(f, "message", "")
        out.append(_problem(str(code or "HINT_LINT_FINDING"), str(msg).strip()))
    return out


def verify_local(repo: Path, tag: str, *, lint_fn=None) -> list[str]:
    """push 전 로컬 봉인 검증 — **이 태그 하나만**(D10). 반환 = `CODE: 설명` 정렬 목록(빈 = 통과). 읽기만 한다.

    묻는 것: ref 형식·v6 문법 · annotated · 태그 헤더 이름 · 대상이 커밋 · tagger = 합성 신원(+PII tripwire) ·
    annotation 정본 모양 · footer 파싱(MISSING/MALFORMED) · footer.tag/anchor == 이 태그/대상 · 앵커 게이트(hint 브랜치
    페이로드 커밋) · 트리 allowlist · 커밋 신원 · PAYLOAD/PROVENANCE.tag == tag · 이름 재조립 · PROMPT 잔재 0 · PII(배포:
    annotation + 트리 + 경로 + 커밋 메시지) · brief 원천 · (있으면) naming facts 재파생 · lint_fn 발견.
    lint_fn(repo=…, anchor=…, tag=…) → 발견 목록(dict{code,message} · `.code` 객체 · `CODE: 설명` 문자열) — 서사 린터 주입구.
    """
    try:
        check_ref_format(repo, tag)
    except core.HintError as e:
        return [_problem(e.code, e.message)]
    kind = tag_ref_kind(repo, tag)
    if kind is None:
        return [_problem("HINT_TAG_ABSENT", f"로컬에 태그가 없다: {tag}")]
    if kind != "tag":
        return [_problem("HINT_TAG_NOT_ANNOTATED", f"annotated 태그가 아니다({kind}) — 본문(footer)이 없는 태그는 봉인 산출물이 아니다.")]
    try:
        obj = read_tag(repo, tag)
    except core.HintError as e:
        return [_problem(e.code, e.message)]
    if obj is None:
        return [_problem("HINT_TAG_NOT_ANNOTATED", f"태그 오브젝트를 읽을 수 없다: {tag}")]
    probs: list[str] = []
    try:
        naming.validate_new_name(tag)
    except core.HintError as e:
        probs.append(_problem(e.code, e.message))
    if obj["name"] != tag:
        probs.append(_problem("HINT_TAG_OBJECT_NAME_MISMATCH",
                              f"태그 오브젝트의 이름 {obj['name']!r} ≠ ref 이름 {tag!r}(다른 태그 오브젝트를 옮겨 단 ref)"))
    # 반환 계약은 **목록**이다(push_tag·hint.py verify 가 code 로 분류한다). 하위 판정이 HintError 로 죽으면 그 code 를
    # 문제로 싣는다 — 예외로 새면 호출부는 "검증 결과" 가 아니라 "검증기 사고" 를 받는다(2026-09-22 리뷰).
    try:
        terms = pii.load_pii_terms(repo)
    except core.HintError as e:
        probs.append(_problem(e.code, e.message))
        terms = []
    if terms is None:
        probs.append(_problem("HINT_PII_TERMS_ABSENT", f"`{core.REL_PII_TERMS}` 부재 — 리터럴 없이 PII-clean 을 인증하지 "
                                                       "않는다(4종 패턴은 그대로 검사했다)."))
        terms = []
    tagger = obj["tagger"]
    if not tagger or tagger.get("ident") != branch.synthetic_identity():
        probs.append(_problem("HINT_TAGGER_NOT_SYNTHETIC",
                              f"tagger 가 합성 신원이 아니다({'부재' if not tagger else '다른 신원'}) — 운영자 신원이 "
                              "배포된다(2026-09-01 실명 유출 선례)."))
    if tagger and branch.identity_pii_hits(terms, tagger.get("ident")):
        probs.append(_problem("HINT_TAGGER_IDENTITY_PII", "tagger 신원이 PII 목록에 걸린다"))
    body, brief, footer = obj["body"], None, None
    try:
        parsed = parse_annotation(body)
        brief, footer = parsed["brief"], parsed["footer"]
    except core.HintError as e:
        probs.append(_problem(e.code, e.message))
        if e.code == "HINT_ANNOTATION_SHAPE":
            with contextlib.suppress(core.HintError):
                footer = parse_footer(body)  # 모양만 어긋났으면 바인딩 대조는 계속한다
    if footer is not None:
        if footer["tag"] != tag:
            probs.append(_problem("HINT_EVIDENCE_BINDING_TAG_MISMATCH", "footer.tag ≠ 이 태그"))
        if footer["anchor"] != obj["target"]:
            probs.append(_problem("HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH",
                                  "footer.anchor ≠ 태그가 가리키는 페이로드 커밋(ⓑ축 — 옛날엔 한 번도 검사되지 않았다 · K3)"))
    if obj["target_type"] != "commit":
        probs.append(_problem("HINT_TAG_TARGET_NOT_COMMIT", f"태그 대상이 커밋이 아니다({obj['target_type']})"))
        hits = pii.scan_text(body, terms, profile="deploy")
        if hits:
            probs.append(_problem("HINT_TAG_PII", "annotation PII: " + pii.render_hits(hits, limit=5)))
    else:
        try:
            branch.require_payload_anchor(repo, tag, obj["target"])
        except core.HintError as e:
            probs.append(_problem(e.code, e.message))
        try:
            probs += _anchor_problems(repo, tag, obj["target"], brief, body, terms)
        except core.HintError as e:   # 트리·blob 을 읽지 못함(부분 클론·손상 오브젝트) — 통과로 접지 않는다
            probs.append(_problem(e.code, e.message))
        if lint_fn is not None:
            probs += _lint_findings(lint_fn, repo, obj["target"], tag)
    return sorted(set(probs))


# ── 원격 · 자격증명 ───────────────────────────────────────────────────────────────────────────
def _redact_url(url: str) -> str:
    """메시지용 URL(userinfo 제거). 토큰이 URL 에 박혀 있었어도 로그로 새지 않게."""
    return re.sub(r"(://)[^/@\s]*@", r"\1***@", url)


def _require_remote_arg(remote) -> None:
    if not isinstance(remote, str) or not remote.strip():
        core.fail("HINT_PUSH_REMOTE_UNRESOLVED", "원격 이름(또는 URL·경로)이 비었다.")
    if remote.startswith("-") or any(ord(c) < 0x20 for c in remote):
        core.fail("HINT_PUSH_REMOTE_UNRESOLVED", "원격 인자가 옵션처럼 보이거나 제어문자를 담았다(옵션 주입 ✗).")


def _is_remote_nick(name: str) -> bool:
    """git 규칙(remote.c valid_remote_nick): 비어 있지 않고 `.`·`..` 아니고 경로 구분자가 없으면 원격 이름 후보."""
    return bool(name) and name not in (".", "..") and "/" not in name and "\\" not in name


def _declared_urls(repo: Path, remote: str, *, push: bool) -> list[str]:
    """**선언된** URL(insteadOf 전개 전). 원격 이름이면 `remote.<n>.pushurl`(push 만) → `remote.<n>.url` ·
    설정이 없으면 git 과 같이 인자 자체를 URL/경로로 본다."""
    if _is_remote_nick(remote):
        keys = ([f"remote.{remote}.pushurl"] if push else []) + [f"remote.{remote}.url"]
        for key in keys:
            r = core.git(repo, "config", "--get-all", key, check=False)
            vals = [v.strip() for v in r.stdout.splitlines() if v.strip()]
            if vals:
                return vals
    return [remote]


def _transport_of(url: str) -> str:
    """선언 URL → `https`·`http`·`ssh`·`local`·`git`. 판정할 수 없으면 거부(판정 불가 ≠ 토큰 불요)."""
    m = _SCHEME_RE.match(url)
    if m:
        t = _SCHEME_TRANSPORT.get(m.group(1).lower())
        if t is None:
            core.fail("HINT_PUSH_REMOTE_SCHEME_UNKNOWN", f"판정할 수 없는 원격 스킴: {m.group(1)!r}",
                      "https · ssh · scp형(user@host:path) · 로컬 경로만 다룬다.")
        try:
            parts = urlsplit(url)
        except ValueError as e:   # 예: 닫히지 않은 IPv6 `[` — 판정 불가를 통과(토큰 불요)로 접지 않는다
            core.fail("HINT_PUSH_REMOTE_URL_INVALID", f"원격 URL 을 해석할 수 없다({e}): {_redact_url(url)}",
                      "remote.<이름>.url 을 고친다.")
        # ssh 의 사용자명(`ssh://git@host/…`)은 정상이다 — 그러나 **비밀번호 자리**는 어느 스킴에서든 자격증명이다.
        if parts.password is not None or (t in ("https", "http") and parts.username is not None):
            # GitHub 은 토큰을 사용자명 자리로도 받는다 — https userinfo 는 무엇이든 토큰일 수 있다.
            core.fail("HINT_PUSH_URL_EMBEDS_CREDENTIAL",
                      f"원격 URL 에 자격증명(userinfo)이 박혀 있다: {_redact_url(url)}",
                      f"remote.url 에서 userinfo 를 걷어내고 `{TOKEN_ENV_FILE_REL}`(비추적)의 `{TOKEN_KEY}` 를 쓴다 — "
                      "URL 에 토큰을 박으면 reflog·설정·로그에 남는다.")
        return t
    if "::" in url:
        core.fail("HINT_PUSH_REMOTE_SCHEME_UNKNOWN", "원격 helper 형식(`<transport>::<address>`)은 판정하지 않는다.")
    head, colon, _ = url.partition(":")
    if colon and "/" not in head:
        return "ssh"  # scp형 `[user@]host:path` — git 도 첫 콜론 앞에 `/` 가 없을 때만 이렇게 읽는다
    return "local"


def _remote_transports(repo: Path, remote: str, *, push: bool) -> set[str]:
    _require_remote_arg(remote)
    return {_transport_of(u) for u in _declared_urls(repo, remote, push=push)}


def remote_needs_token(repo: Path, remote: str) -> bool:
    """push 에 토큰이 필요한가: https 만 참 · ssh/scp형/로컬 경로 = 거짓. 평문 http · 판정 불가 스킴 · URL 속 자격증명은
    거짓으로 접지 않고 실패한다(HINT_PUSH_INSECURE_TRANSPORT 등)."""
    ts = _remote_transports(repo, remote, push=True)
    if "http" in ts:
        core.fail("HINT_PUSH_INSECURE_TRANSPORT", "평문 http 원격에는 push 자격증명을 보내지 않는다.",
                  "https 또는 ssh 원격을 쓴다.")
    return "https" in ts


def read_push_token(repo: Path, environ: dict | None = None) -> str | None:
    """push 용 PAT: 환경변수 `GITHUB_TOKEN` → `<repo>/envs/.env` 의 같은 키(`#` 주석 · 빈 값 · 따옴표 처리). 없으면 None.
    값은 **반환만** 하고 어디에도 출력하지 않는다. 파일이 있는데 읽을 수 없으면 부재가 아니라 결함이다."""
    environ = os.environ if environ is None else environ
    direct = str(environ.get(TOKEN_KEY) or "").strip()
    if direct:
        return direct
    f = Path(repo) / TOKEN_ENV_FILE_REL
    if not f.is_file():
        return None
    try:
        text = f.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        core.fail("HINT_PUSH_TOKEN_FILE_UNREADABLE", f"`{TOKEN_ENV_FILE_REL}` 를 읽을 수 없다({type(e).__name__})",
                  "UTF-8 텍스트 `GITHUB_TOKEN=<PAT>` 한 줄로 고친다(값은 출력하지 않는다).")
    for line in text.splitlines():
        st = line.strip()
        if st.startswith("#") or not st.startswith(TOKEN_KEY + "="):
            continue
        val = st.split("=", 1)[1].strip().strip('"').strip("'")
        if val:
            return val
    return None


def push_credential_args() -> list[str]:
    """`git -c` 인자만 산출한다(토큰 없음 — 순수 함수라 자체검사가 argv 를 직접 단언할 수 있다).
    앞의 빈 값이 상속된 helper 목록을 **비운다** — 호스트에 다른 helper 가 있어도 우리 것만 쓴다. helper 는 `get` 에만
    응답하고 `store`·`erase` 에는 아무것도 하지 않는다."""
    helper = ('!f() { test "$1" = get || exit 0; echo username=x-access-token; '
              'echo "password=$%s"; }; f' % PUSH_TOKEN_ENV)
    return ["-c", "credential.helper=", "-c", "credential.helper=" + helper]


def _scrub(text: str, token: str | None) -> str:
    return text.replace(token, "***") if token and text else (text or "")


def _remote_env(repo: Path, remote: str, *, push: bool) -> tuple[dict, list[str], str | None]:
    """(env, git 앞 인자, 토큰). https 이고 토큰이 있으면 helper 를 싣는다. 토큰은 env 에만 둔다."""
    ts = _remote_transports(repo, remote, push=push)
    env = dict(os.environ)
    env.pop(PUSH_TOKEN_ENV, None)
    env["GIT_TERMINAL_PROMPT"] = "0"
    token = read_push_token(repo) if "https" in ts else None
    if token:
        env[PUSH_TOKEN_ENV] = token
        return env, push_credential_args(), token
    return env, [], None


def _ls_remote_refs(repo: Path, remote: str, pattern: str) -> dict[str, str]:
    """`git ls-remote --refs <remote> <pattern>` → {ref: 직접 오브젝트 SHA}(피일 줄 `^{}` 제외 · 읽기)."""
    env, pre, token = _remote_env(repo, remote, push=False)
    try:
        r = subprocess.run(["git", *pre, "ls-remote", "--refs", remote, pattern], cwd=str(repo), env=env,
                           capture_output=True, text=True, encoding="utf-8", errors="replace")
    except OSError as e:
        core.fail("HINT_GIT_UNAVAILABLE", f"git 실행 불가: {e}")
    if r.returncode != 0:
        core.fail("HINT_REMOTE_QUERY_FAILED", f"ls-remote {_redact_url(remote)} 실패(rc={r.returncode}): "
                                              f"{_scrub(r.stderr, token).strip()[-600:]}",
                  "원격 이름·경로와 네트워크를 확인한다(https 비공개 원격이면 자격증명 — envs/.env 의 GITHUB_TOKEN). "
                  "조회 실패는 '없음' 으로 읽지 않는다.")
    out: dict[str, str] = {}
    for line in r.stdout.splitlines():
        sha, _, ref = line.partition("\t")
        if ref and _SHA40_RE.fullmatch(sha.strip()):
            out[ref.strip()] = sha.strip()
    return out


def remote_tag_object(repo: Path, remote: str, tag: str) -> str | None:
    """원격의 refs/tags/<tag> 가 광고하는 **피일 전** 오브젝트 SHA(annotated 면 태그 오브젝트 SHA · 없으면 None).
    로컬 `read_tag()["object_sha"]` 와 그대로 대조된다(K5: 원격 SHA 를 버리고 이름만 보면 동명 다른 태그를 못 가린다)."""
    check_ref_format(repo, tag)
    return _ls_remote_refs(repo, remote, _tag_ref(tag)).get(_tag_ref(tag))


def _require_safe_refspec(repo: Path, refspec) -> None:
    """공개 push API 의 refspec 규율: `refs/…:refs/…` 한 쌍 · 양쪽 같음 · 강제(`+`)·옵션(`-`)·glob ✗ · ref 형식 통과."""
    bad = None
    if not isinstance(refspec, str) or not refspec:
        bad = "비었다"
    elif refspec.startswith(("+", "-")):
        bad = "강제(+)·옵션(-) 형태(`--tags` 포함)"
    elif any(c in refspec for c in _GLOB_CHARS):
        bad = "glob 문자"
    elif refspec.count(":") != 1 or any(c.isspace() for c in refspec):
        bad = "`src:dst` 한 쌍이 아니다"
    else:
        src, dst = refspec.split(":")
        if src != dst or not src.startswith("refs/"):
            bad = "src ≠ dst 이거나 `refs/` 전체 이름이 아니다(이름 바꿔 밀기 ✗)"
        elif core.git(repo, "check-ref-format", src, check=False).returncode != 0:
            bad = "git check-ref-format 거부"
    if bad:
        core.fail("HINT_PUSH_REFSPEC_FORBIDDEN", f"push refspec 거부({bad}): {refspec!r}",
                  "정확한 ref 1개를 `refs/<…>:refs/<…>` 로 민다.")


def _push_env(repo: Path, remote: str) -> tuple[dict, list[str], str | None]:
    """push 자격증명 판정 + env(한 자리). https 인데 토큰 없음 = HINT_PUSH_CREDENTIAL_ABSENT(메시지에 "자격증명").
    push_tag 는 원격 조회 **전에** 이것을 부른다 — 비공개 https 원격이면 ls-remote 가 먼저 인증 실패로 죽어서
    자격증명 부재가 '원격 조회 실패' 로 둔갑한다(2026-09-22 리뷰 실측)."""
    needs = remote_needs_token(repo, remote)
    env, pre, token = _remote_env(repo, remote, push=True)
    if needs and not token:
        core.fail("HINT_PUSH_CREDENTIAL_ABSENT",
                  f"https 원격 push 자격증명이 없다 — `{TOKEN_ENV_FILE_REL}`(비추적)에 `{TOKEN_KEY}=<PAT>` 한 줄을 두거나 "
                  f"환경변수 `{TOKEN_KEY}` 를 준다. 이 배선이 없으면 무인 실행이 `could not read Username` 으로 멈춘다.",
                  "ssh 원격(scp형 포함)을 쓰면 토큰이 필요 없다.")
    return env, pre, token


# push 에 늘 붙이는 옵션 — 운영자 설정이 "정확한 refspec 1개" 를 넓히지 못하게(모듈 docstring 2026-09-22).
PUSH_SCOPE_ARGS = ("--no-follow-tags", "--no-recurse-submodules")


def git_push_authenticated(repo: Path, remote: str, refspec: str, *, dry_run: bool) -> subprocess.CompletedProcess:
    """**공개 API**(upstream push_branches 가 브랜치 push 에 소비). 스킴 인식 자격증명으로 refspec 하나를 민다.

    https = 토큰 필수(없으면 HINT_PUSH_CREDENTIAL_ABSENT · 메시지에 "자격증명") · ssh/scp형/로컬 = 토큰 불요 ·
    평문 http = 거부. `--no-follow-tags` 로 설정(`push.followTags`)이 딸린 태그를 얹지 못하게 한다.
    성공 = CompletedProcess(stdout/stderr 에서 토큰 문자열은 가린다) · 실패 = HintError
    (HINT_PUSH_FAILED · dry-run 도 같다 — 삼키지 않는다 · 2026-09-04 C-3)."""
    _require_remote_arg(remote)
    _require_safe_refspec(repo, refspec)
    env, pre, token = _push_env(repo, remote)
    args = ["git", *pre, "push", *PUSH_SCOPE_ARGS, *(["--dry-run"] if dry_run else []), remote, refspec]
    try:
        r = subprocess.run(args, cwd=str(repo), env=env, capture_output=True, text=True, encoding="utf-8",
                           errors="replace")
    except OSError as e:
        core.fail("HINT_GIT_UNAVAILABLE", f"git 실행 불가: {e}")
    r = subprocess.CompletedProcess(r.args, r.returncode, _scrub(r.stdout, token), _scrub(r.stderr, token))
    if r.returncode != 0:
        core.fail("HINT_PUSH_FAILED",
                  f"git push{' --dry-run' if dry_run else ''} {_redact_url(remote)} {refspec} → rc={r.returncode}: "
                  f"{r.stderr.strip()[-800:]}",
                  "원격 거부·인증·네트워크를 확인한다(dry-run 실패도 실패다 — 삼키지 않는다).")
    return r


def push_tag(repo: Path, remote: str, tag: str, *, dry_run: bool = False, lint_fn=None) -> dict:
    """태그 **하나**를 정확한 refspec `refs/tags/<tag>:refs/tags/<tag>` 로 민다(O2: 브랜치 ✗ · `--tags` ✗ · glob ✗).

    ① ref 형식(`hint/` · glob ✗ · refs/heads ✗) ② annotated ③ verify_local(이 태그 · lint_fn 전달) 0 이어야 한다
    (검증 범위 = 전송 범위) ④ 원격 사전 조회: 같은 오브젝트면 이미 반영(멱등 · push 생략) · 다른 오브젝트면
    HINT_REMOTE_TAG_CONFLICT(강제 ✗) ⑤ 실제 refspec 로그 → push ⑥ (dry-run 아니면) 원격 SHA == 로컬 SHA.
    반환 {remote, tag, refspec, dry_run, status(pushed|dry-run|already-on-remote), local_object, remote_object}."""
    check_ref_format(repo, tag)
    kind = tag_ref_kind(repo, tag)
    if kind is None:
        core.fail("HINT_TAG_ABSENT", f"로컬에 태그가 없다: {tag}")
    if kind != "tag":
        core.fail("HINT_TAG_NOT_ANNOTATED", f"annotated 태그가 아니다({kind}) — lightweight 태그는 밀지 않는다.")
    probs = verify_local(repo, tag, lint_fn=lint_fn)
    if probs:
        _fail_problems("HINT_PUSH_UNVERIFIED", "push 전 로컬 검증(이 태그)", probs,
                       "hint.py verify --tag <태그> 로 확인하고 고친다 — 검증되지 않은 것은 밀지 않는다.")
    ref = _tag_ref(tag)
    refspec = f"{ref}:{ref}"
    if refspec.count(":") != 1 or "refs/heads/" in refspec or any(c in refspec for c in _GLOB_CHARS):
        core.fail("HINT_PUSH_REFSPEC_FORBIDDEN", f"태그 refspec 가 정확한 태그 1개가 아니다: {refspec!r}")
    _require_remote_arg(remote)
    _push_env(repo, remote)          # 자격증명 판정을 원격 조회보다 먼저(부재가 조회 실패로 둔갑하지 않게)
    local_obj = str(read_tag(repo, tag)["object_sha"])
    before = remote_tag_object(repo, remote, tag)
    result = {"remote": remote, "tag": tag, "refspec": refspec, "dry_run": bool(dry_run),
              "local_object": local_obj, "remote_object": before}
    if before is not None:
        if before == local_obj:
            _log(f"{_redact_url(remote)} 에 이미 같은 오브젝트가 있다({local_obj[:12]}) — push 생략(재개).")
            return {**result, "status": "already-on-remote"}
        core.fail("HINT_REMOTE_TAG_CONFLICT",
                  f"원격 {_redact_url(remote)} 의 {ref} 가 다른 오브젝트다(원격 {before[:12]} ≠ 로컬 {local_obj[:12]}) — "
                  "덮어쓰지 않는다.", "태그는 불변이다(P1 리콜 금지 · 강제 push ✗). 개정판은 새 이름으로 발행한다.")
    _log(f"push {_redact_url(remote)} {refspec}{' (dry-run)' if dry_run else ''} — 태그 1개 · --tags ✗ · 브랜치 ✗")
    git_push_authenticated(repo, remote, refspec, dry_run=dry_run)
    if dry_run:
        return {**result, "status": "dry-run"}
    after = remote_tag_object(repo, remote, tag)
    if after != local_obj:
        core.fail("HINT_REMOTE_SHA_MISMATCH", f"push 뒤 원격 {ref} = {str(after)[:12]} ≠ 로컬 {local_obj[:12]}")
    _log(f"pushed {refspec} → {_redact_url(remote)} ({local_obj[:12]})")
    return {**result, "status": "pushed", "remote_object": after}


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
# 픽스처 문자열은 **조각으로 조립**한다 — 추적 파일에 메일·IPv4·운영자 경로 모양 리터럴을 쓰면 배포 4종 스캔이 이
# 파일 자신에 걸린다(2026-08-06 자기스캔 사고). 원격 주소는 전부 예약 TLD(`.invalid`)이고 insteadOf 로 로컬 bare 로
# 돌린다 — 네트워크 0.
_FX_UTC = "2026-09-21T10:21:01Z"
_FX_UTC2 = "2026-09-21T11:00:00Z"
_FX_TOKEN = "fixture-token-" + "5f3a9c0e71d24b68"
_FX_HOST = "fixture.invalid"


def _fx_fields(tag: str, anchor: str) -> dict:
    return {"version": "1", "tag": tag, "topology": "multi TP=2(Ray)", "anchor": anchor,
            "manifest_ref": "docs/_evidence/fixture.work-manifest.json",
            "certificate_ref": "../benchmark/benchmark_fixture.yaml"}


@contextlib.contextmanager
def _spy_git(log: list):
    """자체검사 전용: git 서브프로세스의 argv·env 를 기록한다(토큰이 argv 에 없는지 · refspec 이 정확한지 단언용)."""
    real = subprocess.run

    def spy(args, *a, **kw):
        if isinstance(args, (list, tuple)) and args and args[0] == "git":
            log.append({"argv": [str(x) for x in args], "env": dict(kw.get("env") or {})})
        return real(args, *a, **kw)

    subprocess.run = spy
    try:
        yield
    finally:
        subprocess.run = real


def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 격리 임시 저장소 + 로컬 bare 원격만 쓴다(라이브 태그·브랜치·네트워크 무관)."""
    bad: list[str] = []

    def ck(name: str, cond) -> None:
        if not cond:
            bad.append(f"tag: {name}")

    try:
        _selftest_pure(ck)
    except Exception as e:  # noqa: BLE001 — 자체검사는 죽지 않고 실패로 보고한다(러너가 전 모듈을 이어서 돌린다)
        bad.append(f"tag: 순수 자체검사 중 예외 {type(e).__name__}: {str(e)[:300]}")
    sink = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="hint-tag-selftest-") as td, contextlib.redirect_stderr(sink):
        tmp = Path(td)
        with branch.selftest_env(tmp):
            try:
                _selftest_git(tmp, ck)
            except core.HintError as e:
                bad.append(f"tag: 자체검사 중 예기치 않은 HintError {e.code}: {e.message[:400]}")
            except Exception as e:  # noqa: BLE001
                bad.append(f"tag: 자체검사 중 예외 {type(e).__name__}: {str(e)[:300]}")
    return bad


def _selftest_pure(ck) -> None:
    code = branch.code_of
    fields = _fx_fields("hint/x/y/z/w", "a" * 40)
    wire = build_footer(**fields)
    ck("footer 왕복", parse_footer(wire) == fields)
    ck("FOOTER_FIELDS 순서 고정(claim C3)",
       FOOTER_FIELDS == ("version", "tag", "topology", "anchor", "manifest_ref", "certificate_ref"))
    ck("★footer 부재 = MISSING", code(lambda: parse_footer("요약만 있다\n")) == "HINT_EVIDENCE_BINDING_MISSING")
    malformed = {
        "닫는 줄 없음": wire.replace("\n-->", ""),
        "중복 키": wire.replace("tag: ", "tag: a\ntag: ", 1),
        "미지 키": wire.replace("version: 1\n", "version: 1\nextra_key: x\n"),
        "짧은 anchor": wire.replace("anchor: " + "a" * 40, "anchor: " + "a" * 12),
        "빈 필드": wire.replace("topology: multi TP=2(Ray)", "topology: "),
        "version 2": wire.replace("version: 1", "version: 2"),
        "필드 누락": wire.replace("certificate_ref: ../benchmark/benchmark_fixture.yaml\n", ""),
        "footer 둘": wire + wire,
        "해석 불가 줄": wire.replace("version: 1\n", "version: 1\nnot a field line\n"),
        "절대경로 주소": wire.replace("manifest_ref: docs/", "manifest_ref: /docs/"),
    }
    for label, text in malformed.items():
        ck(f"★footer {label} = MALFORMED(≠ MISSING)", code(lambda t=text: parse_footer(t)) == "HINT_EVIDENCE_BINDING_MALFORMED")
    ck("footer 결함은 HintError 로도 잡힌다(CLI 단일 처리)", isinstance(HintEvidenceBindingError("X", "y"), core.HintError))
    # ── 폐기 키(옛 hint_tag 음성대조 이관 · 2026-09-04 CP7.5)
    legacy = wire.replace("certificate_ref:", "manifest_sha256: " + "0" * 64 + "\ncertificate_ref:")
    ck("★명시 인자가 있으면 폐기 키를 읽고 버린다",
       "manifest_sha256" not in parse_footer(legacy, retired_ok=RETIRED_FOOTER_KEYS))
    ck("★음성대조 명시 인자 없으면 폐기 키는 여전히 차단", code(lambda: parse_footer(legacy)) == "HINT_EVIDENCE_BINDING_MALFORMED")
    ck("★음성대조 폐기 키가 아닌 미지 키는 명시 인자가 있어도 차단",
       code(lambda: parse_footer(wire.replace("version: 1\n", "version: 1\ntotally_unknown: x\n"),
                                 retired_ok=RETIRED_FOOTER_KEYS)) == "HINT_EVIDENCE_BINDING_MALFORMED")
    ck("★봉인이 폐기 키를 발행하는 경로는 없다",
       not (set(FOOTER_FIELDS) & RETIRED_FOOTER_KEYS)
       and not any(k in build_footer(**fields) for k in RETIRED_FOOTER_KEYS))
    # 이름을 **파일 텍스트**에서 찾으면 이 주석이 매칭돼 가드가 스스로 무력화된다 — 모듈 네임스페이스를 본다.
    ck("★CP7.5: 폐기 키 면제 상수가 존재하지 않는다",
       "LEGACY_FOOTER_TAG_PINS" not in globals() and "LEGACY_CERTIFICATE_REF_ALIASES" not in globals())
    for label, bad_fields in (("여러 줄 값", {**fields, "topology": "a\nb"}), ("주석 경계 값", {**fields, "tag": "x-->"}),
                              ("필드 누락", {k: v for k, v in fields.items() if k != "tag"}),
                              ("필드 초과", {**fields, "extra": "x"})):
        ck(f"★build_footer {label} 거부", code(lambda f=bad_fields: build_footer(**f)) == "HINT_EVIDENCE_BINDING_MALFORMED")
    # ── annotation 정본 모양
    msg = annotation("한 줄 요약", fields)
    ck("annotation 정본 모양(SPEC §2.2)", msg == f"한 줄 요약\n\n{ANNOTATION_POINTER}\n\n{wire}")
    ck("parse_annotation 왕복", parse_annotation(msg) == {"brief": "한 줄 요약", "footer": fields})
    ck("brief 줄 끝 공백 정규화", annotation("첫 줄  \n둘째 줄", fields).startswith("첫 줄\n둘째 줄\n\n"))
    ck("★brief 4줄 = TOO_LONG", code(lambda: annotation("1\n2\n3\n4", fields)) == "HINT_ANNOTATION_BRIEF_TOO_LONG")
    ck("★빈 brief = ABSENT", code(lambda: annotation("  \n", fields)) == "HINT_ANNOTATION_BRIEF_ABSENT")
    ck("★brief 속 주석 경계 = SHAPE", code(lambda: annotation("요약 <!-- x", fields)) == "HINT_ANNOTATION_BRIEF_SHAPE")
    ck("★brief 속 빈 줄(두 문단) = SHAPE", code(lambda: annotation("a\n\nb", fields)) == "HINT_ANNOTATION_BRIEF_SHAPE")
    ck("★뒤꼬리 덧붙임 = HINT_ANNOTATION_SHAPE", code(lambda: parse_annotation(msg + "덧붙임\n")) == "HINT_ANNOTATION_SHAPE")
    ck("★포인터 부재 = HINT_ANNOTATION_SHAPE",
       code(lambda: parse_annotation(msg.replace(ANNOTATION_POINTER, "다른 문장"))) == "HINT_ANNOTATION_SHAPE")
    ck("★footer 없는 annotation = MISSING",
       code(lambda: parse_annotation(f"요약\n\n{ANNOTATION_POINTER}\n")) == "HINT_EVIDENCE_BINDING_MISSING")
    try:
        from . import catalog as _cat  # noqa: PLC0415 — 교차검증(카탈로그 brief 추출기 = annotation 첫 문단)
        ck("카탈로그 brief 추출 = annotation brief(다줄은 공백 이음)",
           _cat.annotation_brief(annotation("첫 줄\n둘째 줄", fields)) == "첫 줄 둘째 줄")
    except ImportError as e:
        ck(f"카탈로그 교차검증 불가 — {e!r}", False)
    # ── push 토큰 판독(옛 hint_tag 자체검사 6건 이관)
    with tempfile.TemporaryDirectory(prefix="hint-tag-token-") as td:
        t = Path(td)
        envf = t / TOKEN_ENV_FILE_REL
        ck("파일 없으면 None", read_push_token(t, environ={}) is None)
        envf.parent.mkdir(parents=True)
        envf.write_text("# 주석\nHF_TOKEN=hf_x\nGITHUB_TOKEN=ghp_FILE\n", encoding="utf-8")
        ck("envs/.env 에서 판독", read_push_token(t, environ={}) == "ghp_FILE")
        ck("★환경변수가 파일을 이긴다", read_push_token(t, environ={TOKEN_KEY: "ghp_ENV"}) == "ghp_ENV")
        envf.write_text("GITHUB_TOKEN=\n", encoding="utf-8")
        ck("★음성대조 빈 값은 토큰이 아니다", read_push_token(t, environ={}) is None)
        envf.write_text("#GITHUB_TOKEN=ghp_commented\n", encoding="utf-8")
        ck("★음성대조 주석 처리된 줄은 무시", read_push_token(t, environ={}) is None)
        envf.write_text('GITHUB_TOKEN="ghp_Q"\n', encoding="utf-8")
        ck("따옴표 제거", read_push_token(t, environ={}) == "ghp_Q")
    # ── credential helper argv(옛 4건 이관) — ★ 토큰이 argv 에 실리면 ps·reflog 로 샌다
    args = push_credential_args()
    ck("상속 helper 를 먼저 비운다", args[:2] == ["-c", "credential.helper="])
    helper = args[3] if len(args) == 4 else ""
    ck("인자는 `-c` 두 쌍이다", len(args) == 4 and args[0] == args[2] == "-c")
    ck("우리 helper 가 환경변수에서 읽는다", "$" + PUSH_TOKEN_ENV in helper)
    ck("★토큰 값이 argv 에 없다", not any("ghp_" in a or _FX_TOKEN in a for a in args))
    ck("helper 는 get 이외 연산에 응답하지 않는다", 'test "$1" = get' in helper)
    # ── 원격 스킴 판정(선언 URL · 순수)
    scp = "git" + "@" + _FX_HOST + ":org/r.git"
    for url, want in ((f"https://{_FX_HOST}/r.git", "https"), (scp, "ssh"), (f"ssh://{_FX_HOST}/r.git", "ssh"),
                      ("/srv/remote/r.git", "local"), ("../r.git", "local"), ("r.git", "local"),
                      ("file:///srv/r.git", "local"), (f"git://{_FX_HOST}/r.git", "git"),
                      (f"http://{_FX_HOST}/r.git", "http")):
        ck(f"스킴 판정 {url.split(':')[0]}… = {want}", code(lambda u=url: _transport_of(u)) is None and _transport_of(url) == want)
    cred_url = "https://" + "user:secret" + "@" + _FX_HOST + "/r.git"
    tok_url = "https://" + "tok" + "@" + _FX_HOST + "/r.git"
    ck("★URL 속 user:pw = HINT_PUSH_URL_EMBEDS_CREDENTIAL", code(lambda: _transport_of(cred_url)) == "HINT_PUSH_URL_EMBEDS_CREDENTIAL")
    ck("★URL 속 토큰 사용자명 = HINT_PUSH_URL_EMBEDS_CREDENTIAL", code(lambda: _transport_of(tok_url)) == "HINT_PUSH_URL_EMBEDS_CREDENTIAL")
    ck("★ssh URL 의 비밀번호 자리 = HINT_PUSH_URL_EMBEDS_CREDENTIAL",
       code(lambda: _transport_of("ssh://" + "git:pw" + "@" + _FX_HOST + "/r.git")) == "HINT_PUSH_URL_EMBEDS_CREDENTIAL")
    ck("ssh URL 의 사용자명만은 정상", _transport_of("ssh://" + "git" + "@" + _FX_HOST + "/r.git") == "ssh")
    ck("★원격 helper 형식 = SCHEME_UNKNOWN", code(lambda: _transport_of("ext::sh -c x")) == "HINT_PUSH_REMOTE_SCHEME_UNKNOWN")
    ck("★미지 스킴 = SCHEME_UNKNOWN", code(lambda: _transport_of("foo://x/r")) == "HINT_PUSH_REMOTE_SCHEME_UNKNOWN")
    ck("메시지용 URL 은 userinfo 를 가린다", "secret" not in _redact_url(cred_url))
    ck("★해석 불가 URL(닫히지 않은 IPv6) = HINT_PUSH_REMOTE_URL_INVALID(예외로 새지 않는다)",
       code(lambda: _transport_of("https://[::1/r.git")) == "HINT_PUSH_REMOTE_URL_INVALID")


def _selftest_git(tmp: Path, ck) -> None:
    code = branch.code_of
    U = _FX_UTC
    repo = branch.selftest_repo(tmp)              # 운영자 신원이 저장소 설정에 있다(일부러)
    op_name, op_email = branch._FX_OPERATOR_NAME, branch._FX_OPERATOR_EMAIL
    op_env = {"GIT_AUTHOR_NAME": op_name, "GIT_AUTHOR_EMAIL": op_email,
              "GIT_COMMITTER_NAME": op_name, "GIT_COMMITTER_EMAIL": op_email}
    source = core.git_out(repo, "rev-parse", "HEAD")
    tag = branch._FX_TAG

    def variant(n: int) -> str:
        return tag.replace("len262144", f"len{n}")

    def commit_new(r: Path, t: str, pdir: Path, *, brief: str = branch.FX_BRIEF, extra=None, sa=source) -> str:
        branch.write_fixture_payload(pdir, t, source_anchor=sa, brief=brief, extra_files=extra)
        return branch.commit_payload(r, pdir, message=branch.commit_message(pdir, generated_utc=U), generated_utc=U)

    # ── 커밋 → (거부들) → 봉인. 운영자 신원 env 를 **일부러** 심은 채로 한다.
    with branch.selftest_env(tmp, op_env):
        anchor = commit_new(repo, tag, tmp / "p1")
        msg = annotation(branch.FX_BRIEF, _fx_fields(tag, anchor))
        ck("★footer.tag ≠ 봉인 대상 = TAG_MISMATCH",
           code(lambda: seal(repo, tag, anchor, annotation(branch.FX_BRIEF, _fx_fields(variant(1024), anchor)),
                             generated_utc=U)) == "HINT_EVIDENCE_BINDING_TAG_MISMATCH")
        ck("★footer.anchor ≠ 봉인 앵커 = ANCHOR_MISMATCH",
           code(lambda: seal(repo, tag, anchor, annotation(branch.FX_BRIEF, _fx_fields(tag, source)),
                             generated_utc=U)) == "HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH")
        ck("★소스 커밋에 봉인 = HINT_ANCHOR_NOT_ON_HINT_BRANCH(2026-09-07 native 3종)",
           code(lambda: seal(repo, tag, source, annotation(branch.FX_BRIEF, _fx_fields(tag, source)),
                             generated_utc=U)) == "HINT_ANCHOR_NOT_ON_HINT_BRANCH")
        refused = None
        try:
            seal(repo, tag, anchor, annotation("손으로 쓴 다른 요약", _fx_fields(tag, anchor)), generated_utc=U)
        except core.HintError as e:
            refused = e
        ck("★brief 가 00-hint §0.1 추출과 다르면 봉인 거부",
           isinstance(refused, HintProblemsError) and refused.code == "HINT_SEAL_REFUSED"
           and "HINT_ANNOTATION_BRIEF_MISMATCH" in problem_codes(refused.problems))
        ck("★정본 아닌 annotation(덧붙임) = HINT_ANNOTATION_SHAPE",
           code(lambda: seal(repo, tag, anchor, msg + "덧붙임\n", generated_utc=U)) == "HINT_ANNOTATION_SHAPE")
        ck("★옛 문법 이름은 봉인하지 않는다",
           code(lambda: seal(repo, "hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8", anchor, msg,
                             generated_utc=U)) == "HINT_ARCH_NODE_AXIS_ABSENT")
        ck("★비-UTC 시각 = HINT_TIME_NOT_INJECTED",
           code(lambda: seal(repo, tag, anchor, msg, generated_utc="2026-09-21 10:21")) == "HINT_TIME_NOT_INJECTED")
        ck("거부들 뒤 태그 없음(부작용 0)", tag_ref_kind(repo, tag) is None)
        obj = seal(repo, tag, anchor, msg, generated_utc=U)
        ck("멱등 재봉인 = 같은 오브젝트", seal(repo, tag, anchor, msg, generated_utc=U) == obj)

    rt = read_tag(repo, tag) or {}
    ck("annotated 태그가 생겼다", tag_ref_kind(repo, tag) == "tag" and rt.get("object_sha") == obj)
    ck("★본문 바이트 = 봉인 메시지(끝 개행까지 · 2026-09-22 사후조건 결함 회귀)", rt.get("body") == msg)
    ck("태그 대상 = 페이로드 커밋", rt.get("target") == anchor and rt.get("target_type") == "commit")
    tg = rt.get("tagger") or {}
    ck("tagger = 합성 신원 · 시각 = 주입", tg.get("ident") == branch.synthetic_identity()
       and tg.get("epoch") == int(core.parse_utc(U).timestamp()) and tg.get("tz") == "+0000")
    raw_commit = core.git_bytes(repo, "cat-file", "commit", anchor)
    ck("★운영자 신원이 태그·커밋 오브젝트 어디에도 없다(config·env 에 심었는데도)",
       all(op_name.encode() not in b and op_email.encode() not in b for b in (rt.get("raw", b""), raw_commit)))
    ck("verify_local = 0", verify_local(repo, tag) == [])
    ck("★부재 태그 = HINT_TAG_ABSENT", problem_codes(verify_local(repo, variant(2048))) == ["HINT_TAG_ABSENT"])

    # ── ★verbatim(2026-08-20: git 기본 cleanup=strip 이 `#` 줄·빈 줄 연속·줄끝 공백·끝 빈 줄을 지웠다) — 생성 단계를
    #   직접 친다(brief 모양 규칙은 template 소관이라 그 규칙과 무관하게 git 층의 바이트 보존만 증명한다).
    tag_v = variant(8192)
    msg_v = "## 헤딩처럼 보이는 줄\n# 주석처럼 보이는 줄\n\n\n줄끝 공백   \n  \n끝\n\n"
    sha_v = _create_tag(repo, tag_v, anchor, msg_v, U)
    ck("★verbatim: `#` 줄·빈 줄·공백이 본문에 바이트 그대로 남는다",
       core.git_bytes(repo, "cat-file", "tag", sha_v) == _tag_object_bytes(anchor, tag_v, U, msg_v))
    core.git(repo, "update-ref", "-d", _tag_ref(tag_v), sha_v)   # 격리 저장소 픽스처 정리

    # ── 결정론: 다른 저장소 · 같은 입력·주입 시각 = 같은 커밋 SHA · 같은 태그 SHA
    repo2 = tmp / "repo2"
    repo2.mkdir()
    core.git(repo2, "init", "-q", "-b", "main")
    (repo2 / core.REL_PII_TERMS).parent.mkdir(parents=True)
    (repo2 / core.REL_PII_TERMS).write_text(f"{branch.FIXTURE_TERM}\n", encoding="utf-8")
    anchor2 = commit_new(repo2, tag, tmp / "p2")
    ck("★결정론: 태그 SHA 가 저장소·운영자와 무관하다",
       anchor2 == anchor and seal(repo2, tag, anchor2, msg, generated_utc=U) == obj)

    # ── ★X13: 이 draft 가 봉인한 것(같은 바이트)만 재개 · 나머지는 충돌
    ck("★같은 앵커·다른 brief = HINT_NAME_COLLISION",
       code(lambda: seal(repo, tag, anchor, annotation("다른 요약", _fx_fields(tag, anchor)), generated_utc=U))
       == "HINT_NAME_COLLISION")
    ck("같은 앵커·같은 annotation·다른 봉인 시각 = 재개(기존 오브젝트 유지 · 끊긴 continue 재실행)",
       seal(repo, tag, anchor, msg, generated_utc=_FX_UTC2) == obj and _ref_sha(repo, _tag_ref(tag)) == obj)
    t_op = _git_b(repo, "mktag", input_bytes=(
        f"object {anchor}\ntype commit\ntag {tag}\ntagger {op_name} <{op_email}> "
        f"{int(core.parse_utc(U).timestamp())} +0000\n\n").encode("utf-8") + msg.encode("utf-8")).stdout.decode().strip()
    core.git(repo, "update-ref", _tag_ref(tag), t_op, obj)
    try:
        ck("★같은 앵커·같은 본문이라도 tagger 가 합성 신원이 아니면 재개가 아니다 = HINT_NAME_COLLISION",
           code(lambda: seal(repo, tag, anchor, msg, generated_utc=U)) == "HINT_NAME_COLLISION")
    finally:
        core.git(repo, "update-ref", _tag_ref(tag), obj, t_op)
    anchor_b = commit_new(repo, tag, tmp / "pb", extra={"artifacts/triplet/extra.yaml": "k: v\n"})
    ck("★다른 앵커(같은 태그의 다른 페이로드 커밋) = HINT_NAME_COLLISION",
       code(lambda: seal(repo, tag, anchor_b, annotation(branch.FX_BRIEF, _fx_fields(tag, anchor_b)), generated_utc=U))
       == "HINT_NAME_COLLISION")
    ck("브랜치가 더 전진해도 봉인된 앵커는 조상 — verify_local = 0", verify_local(repo, tag) == [])

    # ── lint_fn 주입구
    seen: dict = {}

    def lint_ok(**kw):
        seen.update(kw)
        return [{"code": "HINT_LINT_FIXTURE", "message": "픽스처 발견"}]

    def lint_boom(**kw):
        core.fail("HINT_LINT_BOOM", "린터 자체 실패")

    ck("lint_fn 발견 = 문제(code 보존)", "HINT_LINT_FIXTURE" in problem_codes(verify_local(repo, tag, lint_fn=lint_ok)))
    ck("lint_fn 인자 = repo·anchor·tag", seen.get("anchor") == anchor and seen.get("tag") == tag and seen.get("repo") == repo)
    ck("★lint_fn 의 HintError 도 문제로 남는다", "HINT_LINT_BOOM" in problem_codes(verify_local(repo, tag, lint_fn=lint_boom)))

    # ── 이름·ref 형식
    ck("name_collision 로컬", name_collision(repo, tag) == "local")
    ck("name_collision 없음", name_collision(repo, variant(4096)) is None)
    ck("★D/F 접두 충돌", name_collision(repo, tag + "/x") == "local-prefix")
    for badname, want in (("other/x", "HINT_TAG_NAMESPACE"), ("refs/heads/hint", "HINT_TAG_NAMESPACE"),
                          ("refs/tags/hint/x", "HINT_TAG_NAMESPACE"), ("hint/*", "HINT_TAG_GLOB_FORBIDDEN"),
                          ("hint/a?b", "HINT_TAG_GLOB_FORBIDDEN"), ("hint/[x]", "HINT_TAG_GLOB_FORBIDDEN"),
                          ("hint/a b", "HINT_TAG_REF_FORMAT"), ("hint/a:b", "HINT_TAG_REF_FORMAT"),
                          ("hint/a..b", "HINT_TAG_REF_FORMAT"), ("hint/x.lock", "HINT_TAG_REF_FORMAT"),
                          ("", "HINT_TAG_NAME_ABSENT")):
        ck(f"★ref 형식 거부 {badname!r} = {want}", code(lambda n=badname: check_ref_format(repo, n)) == want)

    # ── push: 로컬 bare 원격(토큰 불요) · 정확한 refspec 1개
    bare = tmp / "remote.git"
    core.git(tmp, "init", "-q", "--bare", str(bare))
    calls: list = []
    with _spy_git(calls):
        dry = push_tag(repo, str(bare), tag, dry_run=True)
        ck("dry-run 은 원격을 바꾸지 않는다", dry.get("status") == "dry-run" and remote_tag_object(repo, str(bare), tag) is None)
        res = push_tag(repo, str(bare), tag)
    pushes = [c["argv"] for c in calls if "push" in c["argv"]]
    want_spec = f"refs/tags/{tag}:refs/tags/{tag}"
    ck("push 2회(dry-run·실제) 모두 정확한 refspec 1개",
       len(pushes) == 2 and all(sum(a == want_spec for a in p) == 1 and not any(
           a.startswith("refs/") and a != want_spec for a in p) for p in pushes))
    ck("★push argv 에 --tags·refs/heads 없음", all("--tags" not in p and not any("refs/heads" in a for a in p) for p in pushes))
    ck("★push argv 가 설정의 태그 딸려보내기를 끈다(--no-follow-tags)",
       pushes and all(all(o in p for o in PUSH_SCOPE_ARGS) for p in pushes))
    ck("pushed · 원격 오브젝트 = 로컬 오브젝트",
       res.get("status") == "pushed" and res.get("remote_object") == obj and remote_tag_object(repo, str(bare), tag) == obj)
    ck("원격 태그 바이트 = 로컬 태그 바이트", core.git_bytes(bare, "cat-file", "tag", obj) == rt.get("raw"))
    calls.clear()
    with _spy_git(calls):
        again = push_tag(repo, str(bare), tag)
    ck("재push = already-on-remote(push 생략 · 재개)",
       again.get("status") == "already-on-remote" and not any("push" in c["argv"] for c in calls))
    repo3 = tmp / "repo3"
    repo3.mkdir()
    core.git(repo3, "init", "-q", "-b", "main")
    ck("name_collision 원격", name_collision(repo3, tag, remote=str(bare)) == "remote")
    ck("★name_collision 원격 D/F 접두", name_collision(repo3, tag + "/x", remote=str(bare)) == "remote-prefix")
    ck("★원격 조회 실패는 '없음' 이 아니다",
       code(lambda: name_collision(repo3, tag, remote=str(tmp / "absent.git"))) == "HINT_REMOTE_QUERY_FAILED")

    # ── ★원격에 같은 이름의 다른 오브젝트 = 밀지 않는다
    repo4 = tmp / "repo4"
    repo4.mkdir()
    core.git(repo4, "init", "-q", "-b", "main")
    (repo4 / core.REL_PII_TERMS).parent.mkdir(parents=True)
    (repo4 / core.REL_PII_TERMS).write_text(f"{branch.FIXTURE_TERM}\n", encoding="utf-8")
    a4 = commit_new(repo4, tag, tmp / "p4", brief="다른 PC 가 같은 셀을 다르게 요약했다.")
    seal(repo4, tag, a4, annotation("다른 PC 가 같은 셀을 다르게 요약했다.", _fx_fields(tag, a4)), generated_utc=U)
    ck("★원격 동명 다른 오브젝트 = HINT_REMOTE_TAG_CONFLICT(강제 ✗)",
       code(lambda: push_tag(repo4, str(bare), tag)) == "HINT_REMOTE_TAG_CONFLICT"
       and remote_tag_object(repo, str(bare), tag) == obj)

    # ── ★refspec·이름 규율
    ck("★push_tag glob = HINT_TAG_GLOB_FORBIDDEN", code(lambda: push_tag(repo, str(bare), "hint/*")) == "HINT_TAG_GLOB_FORBIDDEN")
    ck("★push_tag hint/ 밖 = HINT_TAG_NAMESPACE", code(lambda: push_tag(repo, str(bare), "other/x")) == "HINT_TAG_NAMESPACE")
    ck("★push_tag 브랜치 = HINT_TAG_NAMESPACE", code(lambda: push_tag(repo, str(bare), "refs/heads/hint")) == "HINT_TAG_NAMESPACE")
    for spec in ("+" + want_spec, "refs/tags/hint/*:refs/tags/hint/*", "--tags", f"refs/heads/hint:refs/tags/{tag}",
                 "refs/tags/a:refs/tags/a refs/tags/b:refs/tags/b", f"refs/tags/{tag}"):
        ck(f"★git_push_authenticated refspec 거부 {spec[:28]!r}",
           code(lambda s=spec: git_push_authenticated(repo, str(bare), s, dry_run=True)) == "HINT_PUSH_REFSPEC_FORBIDDEN")
    ck("★원격 인자 옵션 주입 거부", code(lambda: git_push_authenticated(repo, "--mirror", want_spec, dry_run=True))
       == "HINT_PUSH_REMOTE_UNRESOLVED")
    tag_lw = variant(512)
    core.git(repo, "tag", tag_lw, anchor)                          # 격리 저장소의 lightweight 픽스처
    ck("★lightweight 태그는 밀지 않는다", code(lambda: push_tag(repo, str(bare), tag_lw)) == "HINT_TAG_NOT_ANNOTATED")
    ck("★lightweight 태그 verify = HINT_TAG_NOT_ANNOTATED", problem_codes(verify_local(repo, tag_lw)) == ["HINT_TAG_NOT_ANNOTATED"])
    ck("★dry-run 도 실패를 삼키지 않는다(C-3)",
       code(lambda: git_push_authenticated(repo, str(tmp / "absent.git"), want_spec, dry_run=True)) == "HINT_PUSH_FAILED")

    # ── ★스킴 인식 자격증명(선언 URL 판정 · insteadOf 로 로컬 bare 에 돌려 네트워크 0)
    https_url = f"https://{_FX_HOST}/r.git"
    core.git(repo, "remote", "add", "https-fixture", https_url)
    core.git(repo, "config", f"url.{bare}.insteadOf", https_url)
    tag_c = variant(16384)
    a_c = commit_new(repo, tag_c, tmp / "pc")
    seal(repo, tag_c, a_c, annotation(branch.FX_BRIEF, _fx_fields(tag_c, a_c)), generated_utc=U)
    ck("https 선언 원격은 토큰이 필요하다(insteadOf 전개 전 판정)", remote_needs_token(repo, "https-fixture") is True)
    absent = None
    try:
        git_push_authenticated(repo, "https-fixture", f"refs/tags/{tag_c}:refs/tags/{tag_c}", dry_run=True)
    except core.HintError as e:
        absent = e
    ck("★https·토큰 없음 = HINT_PUSH_CREDENTIAL_ABSENT(메시지에 '자격증명')",
       absent is not None and absent.code == "HINT_PUSH_CREDENTIAL_ABSENT" and "자격증명" in absent.message)
    ck("★push_tag 도 같은 거부 · 원격 불변",
       code(lambda: push_tag(repo, "https-fixture", tag_c)) == "HINT_PUSH_CREDENTIAL_ABSENT"
       and remote_tag_object(repo, str(bare), tag_c) is None)
    calls.clear()
    with branch.selftest_env(tmp, {TOKEN_KEY: _FX_TOKEN}), _spy_git(calls):
        res_c = push_tag(repo, "https-fixture", tag_c)
        proc = git_push_authenticated(repo, "https-fixture", f"refs/tags/{tag_c}:refs/tags/{tag_c}", dry_run=False)
    ck("토큰이 있으면 https 원격 push 성공", res_c.get("status") == "pushed" and remote_tag_object(repo, str(bare), tag_c)
       == (read_tag(repo, tag_c) or {}).get("object_sha"))
    pc = [c for c in calls if "push" in c["argv"]]
    ck("★토큰 값이 어떤 git argv 에도 없다", calls and not any(_FX_TOKEN in a for c in calls for a in c["argv"]))
    ck("토큰은 push 프로세스 env 로만 · 상속 helper 비움 · 프롬프트 ✗",
       pc and all(c["env"].get(PUSH_TOKEN_ENV) == _FX_TOKEN and c["env"].get("GIT_TERMINAL_PROMPT") == "0"
                  and c["argv"][1:5] == push_credential_args() for c in pc))
    ck("★remote.url 은 그대로(토큰 ✗)", core.git_out(repo, "config", "--get-all", "remote.https-fixture.url") == https_url)
    ck("★push 출력에 토큰 없음", _FX_TOKEN not in (proc.stdout + proc.stderr))
    leaked = [p for root in (repo / ".git", bare) for p in root.rglob("*")
              if p.is_file() and _FX_TOKEN.encode() in p.read_bytes()]
    ck("★토큰이 reflog·설정·ref 어디에도 없다(두 저장소 전수)", leaked == [])
    # 실제 helper 를 git 이 부르게 한다(네트워크 없이 `git credential fill`) — 상속 helper 가 비워지는지까지.
    core.git(repo, "config", "credential.helper", "!f() { echo username=leak; echo password=LEAKED; }; f")
    probe = f"protocol=https\nhost={_FX_HOST}\n\n"
    env_t = {PUSH_TOKEN_ENV: _FX_TOKEN, "GIT_TERMINAL_PROMPT": "0"}
    ours = core.git(repo, *push_credential_args(), "credential", "fill", input_text=probe, env_extra=env_t, check=False)
    ctrl = core.git(repo, "credential", "fill", input_text=probe, env_extra={"GIT_TERMINAL_PROMPT": "0"}, check=False)
    ck("★우리 helper 만 응답한다(상속 helper 비움)", f"password={_FX_TOKEN}" in ours.stdout and "LEAKED" not in ours.stdout)
    ck("대조: 비우지 않으면 상속 helper 가 응답한다(픽스처가 살아 있다)", "LEAKED" in ctrl.stdout)
    core.git(repo, "config", "--unset", "credential.helper")
    core.git(repo, "remote", "add", "http-fixture", f"http://{_FX_HOST}/r.git")
    ck("★평문 http = HINT_PUSH_INSECURE_TRANSPORT",
       code(lambda: git_push_authenticated(repo, "http-fixture", want_spec, dry_run=True)) == "HINT_PUSH_INSECURE_TRANSPORT")
    core.git(repo, "remote", "add", "cred-fixture", "https://" + "user:secret" + "@" + _FX_HOST + "/r.git")
    ck("★URL 에 자격증명이 박힌 원격 = 거부",
       code(lambda: git_push_authenticated(repo, "cred-fixture", want_spec, dry_run=True)) == "HINT_PUSH_URL_EMBEDS_CREDENTIAL")
    for name, url in (("ssh-fixture", f"ssh://{_FX_HOST}/r.git"), ("scp-fixture", "git" + "@" + _FX_HOST + ":r.git")):
        core.git(repo, "remote", "add", name, url)
        core.git(repo, "config", "--add", f"url.{bare}.insteadOf", url)
        ck(f"{name}: 토큰 불요", remote_needs_token(repo, name) is False)
        ck(f"★{name}: 토큰 없이 push(dry-run) 가 돈다(옛 코드는 SSH 여도 토큰 부재로 죽었다 · B12)",
           code(lambda n=name: git_push_authenticated(repo, n, want_spec, dry_run=True)) is None)

    # ── ★자격증명 판정은 원격 조회보다 먼저: 비공개 https 원격이면 ls-remote 가 인증 실패로 먼저 죽어 부재가
    #   '원격 조회 실패' 로 둔갑했다(2026-09-22 리뷰). 도달 불가 https 원격으로 재현한다(insteadOf → 없는 경로).
    dead = f"https://{_FX_HOST}/dead.git"
    core.git(repo, "remote", "add", "dead-https", dead)
    core.git(repo, "config", "--add", f"url.{tmp / 'no-such-remote.git'}.insteadOf", dead)
    ck("대조: 그 원격은 실제로 조회되지 않는다(픽스처가 살아 있다)",
       code(lambda: remote_tag_object(repo, "dead-https", tag_c)) == "HINT_REMOTE_QUERY_FAILED")
    ck("★https·토큰 없음 · 원격 조회 불가여도 push_tag = HINT_PUSH_CREDENTIAL_ABSENT(≠ REMOTE_QUERY_FAILED)",
       code(lambda: push_tag(repo, "dead-https", tag_c, dry_run=True)) == "HINT_PUSH_CREDENTIAL_ABSENT")

    # ── ★운영자 설정 push.followTags=true 가 딸린 태그를 얹지 못한다(2026-09-22 실측: 태그 1개 push 가 3개를 올렸다)
    core.git(repo, "tag", "-a", "-m", "local rollback anchor", "last-good-fixture", anchor)   # 격리 저장소 픽스처
    core.git(repo, "config", "push.followTags", "true")
    try:
        ctrl_bare, ft_bare = tmp / "ctrl-follow.git", tmp / "follow.git"
        for b in (ctrl_bare, ft_bare):
            core.git(tmp, "init", "-q", "--bare", str(b))
        core.git(repo, "push", "-q", str(ctrl_bare), f"refs/tags/{tag_c}:refs/tags/{tag_c}")
        ctrl_refs = core.git_out(ctrl_bare, "for-each-ref", "--format=%(refname)").splitlines()
        ck("대조: 옵션 없는 git push 는 followTags 로 다른 태그까지 올린다(픽스처가 살아 있다)",
           "refs/tags/last-good-fixture" in ctrl_refs and f"refs/tags/{tag}" in ctrl_refs)
        ft = push_tag(repo, str(ft_bare), tag_c)
        ft_refs = core.git_out(ft_bare, "for-each-ref", "--format=%(refname)").splitlines()
        ck("★followTags 설정에서도 push_tag 는 정확히 그 태그 1개만 올린다(last-good·옛 hint 태그 ✗)",
           ft.get("status") == "pushed" and ft_refs == [f"refs/tags/{tag_c}"])
        br_bare = tmp / "follow-branch.git"
        core.git(tmp, "init", "-q", "--bare", str(br_bare))
        git_push_authenticated(repo, str(br_bare), f"{core.HINT_BRANCH_REF}:{core.HINT_BRANCH_REF}", dry_run=False)
        ck("★브랜치 push(공개 API · push_branches 경로)도 태그를 딸려 보내지 않는다",
           core.git_out(br_bare, "for-each-ref", "--format=%(refname)").splitlines() == [core.HINT_BRANCH_REF])
    finally:
        core.git(repo, "config", "--unset", "push.followTags")

    # ── ★봉인 뒤 변조 → verify_local 이 잡는다(격리 저장소에서 ref 를 변조 오브젝트로 잠시 옮긴다)
    def mktag(body: str, *, tagger: str | None = None, target: str = anchor, name: str = tag) -> str:
        who = tagger or branch.synthetic_identity()
        data = (f"object {target}\ntype commit\ntag {name}\ntagger {who} "
                f"{int(core.parse_utc(U).timestamp())} +0000\n\n").encode("utf-8") + body.encode("utf-8")
        r = _git_b(repo, "mktag", input_bytes=data)
        return r.stdout.decode().strip()

    f_ok = _fx_fields(tag, anchor)
    tampers = {
        "footer.anchor": (annotation(branch.FX_BRIEF, {**f_ok, "anchor": source}), None, "HINT_EVIDENCE_BINDING_ANCHOR_MISMATCH"),
        "footer.tag": (annotation(branch.FX_BRIEF, {**f_ok, "tag": variant(1024)}), None, "HINT_EVIDENCE_BINDING_TAG_MISMATCH"),
        "footer 삭제": (f"{branch.FX_BRIEF}\n\n{ANNOTATION_POINTER}\n", None, "HINT_EVIDENCE_BINDING_MISSING"),
        "중복 키": (msg.replace("topology: ", "topology: x\ntopology: ", 1), None, "HINT_EVIDENCE_BINDING_MALFORMED"),
        "폐기 키": (msg.replace("certificate_ref:", "manifest_sha256: " + "0" * 64 + "\ncertificate_ref:"), None,
                  "HINT_EVIDENCE_BINDING_MALFORMED"),
        "뒤꼬리": (msg + "덧붙임\n", None, "HINT_ANNOTATION_SHAPE"),
        "brief 변조": (annotation("변조된 요약", f_ok), None, "HINT_ANNOTATION_BRIEF_MISMATCH"),
        "brief PII": (annotation(f"요약 {branch.FIXTURE_TERM}", f_ok), None, "HINT_TAG_PII"),
        "운영자 tagger": (msg, f"{op_name} <{op_email}>", "HINT_TAGGER_NOT_SYNTHETIC"),
    }
    for label, (body, who, want) in tampers.items():
        t_sha = mktag(body, tagger=who)
        core.git(repo, "update-ref", _tag_ref(tag), t_sha, obj)
        try:
            codes = problem_codes(verify_local(repo, tag))
            ck(f"★변조({label}) → {want}", want in codes)
            if label == "footer.anchor":
                ck("★변조된 태그는 push 되지 않는다", code(lambda: push_tag(repo, str(tmp / "other.git"), tag))
                   == "HINT_PUSH_UNVERIFIED")
        finally:
            core.git(repo, "update-ref", _tag_ref(tag), obj, t_sha)
    ck("변조 복원 뒤 verify_local = 0", verify_local(repo, tag) == [])

    # ── ★verify_local 은 하위 판정의 HintError 를 **문제 목록**으로 돌려준다(예외로 새지 않는다)
    tp = repo / core.REL_PII_TERMS
    tp.rename(tmp / "terms.moved")
    tp.mkdir()
    try:
        vr = None
        try:
            vr = verify_local(repo, tag)
        except core.HintError as e:
            vr = f"raised {e.code}"
        ck("★깨진 pii_terms(디렉터리) → verify 결과에 HINT_PII_TERMS_UNREADABLE(예외 ✗)",
           isinstance(vr, list) and "HINT_PII_TERMS_UNREADABLE" in problem_codes(vr))
    finally:
        tp.rmdir()
        (tmp / "terms.moved").rename(tp)

    # ── ★정상 모양의 페이로드 커밋이라도 hint 브랜치 **밖**이면 봉인·검증 모두 거부(2026-09-07 계약 §6 · 이 태그만)
    t_off = variant(1029)
    p_off = tmp / "p-off"
    branch.write_fixture_payload(p_off, t_off, source_anchor=source)
    off_tree = branch.build_tree(repo, p_off, branch.payload_files(p_off))
    c_off = core.git(repo, "commit-tree", off_tree, "--no-gpg-sign", input_text=branch.commit_message(p_off, generated_utc=U),
                     env_extra=branch.identity_env(U)).stdout.strip()   # ref 를 옮기지 않은 고아 커밋
    m_off = annotation(branch.FX_BRIEF, _fx_fields(t_off, c_off))
    ck("★브랜치 밖 페이로드 커밋 봉인 = HINT_ANCHOR_NOT_ON_HINT_BRANCH",
       code(lambda: seal(repo, t_off, c_off, m_off, generated_utc=U)) == "HINT_ANCHOR_NOT_ON_HINT_BRANCH"
       and tag_ref_kind(repo, t_off) is None)
    off_sha = mktag(m_off, target=c_off, name=t_off)
    core.git(repo, "update-ref", _tag_ref(t_off), off_sha, "0" * 40)
    ck("★브랜치 밖 페이로드 커밋 태그 verify = HINT_ANCHOR_NOT_ON_HINT_BRANCH",
       problem_codes(verify_local(repo, t_off)) == ["HINT_ANCHOR_NOT_ON_HINT_BRANCH"])

    # ── ★blob 이 없는 커밋(부분 클론·손상)도 verify 는 목록으로 답한다 — 읽지 못함을 통과로도, 예외로도 접지 않는다
    t_miss = variant(1028)
    m_tree = core.git(repo, "mktree", "--missing", input_text=f"100644 blob {'1' * 40}\tPAYLOAD.json\n").stdout.strip()
    c_miss = core.git(repo, "commit-tree", m_tree, "--no-gpg-sign", input_text=f"hint: {t_miss}\n",
                      env_extra=branch.identity_env(U)).stdout.strip()
    core.git(repo, "update-ref", _tag_ref(t_miss), mktag(annotation(branch.FX_BRIEF, _fx_fields(t_miss, c_miss)),
                                                        target=c_miss, name=t_miss), "0" * 40)
    try:
        vm = verify_local(repo, t_miss)
    except core.HintError as e:
        vm = f"raised {e.code}"
    ck("★누락 blob → verify 결과에 HINT_TREE_READ_FAILED(예외 ✗ · 통과 ✗)",
       isinstance(vm, list) and "HINT_TREE_READ_FAILED" in problem_codes(vm))

    # ── ★도구를 우회한 페이로드 커밋(배관 직접) → 봉인 거부 + verify 검출
    def plumb(t: str, pdir: Path, *, extra=None, env=None, outside: bool = False) -> str:
        branch.write_fixture_payload(pdir, t, source_anchor=None, extra_files=extra)
        rels = sorted(p.relative_to(pdir).as_posix() for p in pdir.rglob("*") if p.is_file())
        tree = branch.build_tree(repo, pdir, rels)
        if outside:  # allowlist 밖 최상위 파일을 트리에 끼워 넣는다(build_tree 는 거부하므로 mktree 로)
            blob = core.git(repo, "hash-object", "-w", "--stdin", input_text="메모\n").stdout.strip()
            lines = core.git(repo, "ls-tree", tree).stdout
            tree = core.git(repo, "mktree", input_text=lines + f"100644 blob {blob}\tnotes.txt\n").stdout.strip()
        tip = branch.hint_tip(repo)
        c = core.git(repo, "commit-tree", tree, "-p", tip, "--no-gpg-sign",
                     input_text=branch.commit_message(pdir, generated_utc=U),
                     env_extra=env or branch.identity_env(U)).stdout.strip()
        branch._update_hint_ref(repo, c, tip)
        return c

    op_commit_env = {**op_env, "GIT_AUTHOR_DATE": core.git_date(U), "GIT_COMMITTER_DATE": core.git_date(U)}
    for n, label, kw, want in ((1030, "allowlist 밖 파일", {"outside": True}, "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"),
                               (1031, "페이로드 PII", {"extra": {"02-narrative.md": f"메모 {branch.FIXTURE_TERM}\n"}},
                                "HINT_PAYLOAD_PII"),
                               (1032, "운영자 신원 커밋", {"env": op_commit_env}, "HINT_COMMIT_IDENTITY_NOT_SYNTHETIC"),
                               (1033, "PROMPT 잔재", {"extra": {"02-narrative.md": "<!-- PROMPT\n질문\n-->\n"}},
                                "HINT_PROMPT_RESIDUE")):
        tb = variant(n)
        cb = plumb(tb, tmp / f"pp{n}", **kw)
        mb = annotation(branch.FX_BRIEF, _fx_fields(tb, cb))
        err = None
        try:
            seal(repo, tb, cb, mb, generated_utc=U)
        except core.HintError as e:
            err = e
        ck(f"★봉인 거부({label}) → {want}", isinstance(err, HintProblemsError) and err.code == "HINT_SEAL_REFUSED"
           and want in problem_codes(err.problems)
           and tag_ref_kind(repo, tb) is None)
        t_sha = mktag(mb, target=cb, name=tb)
        core.git(repo, "update-ref", _tag_ref(tb), t_sha, "0" * 40)
        ck(f"★verify 검출({label}) → {want}", want in problem_codes(verify_local(repo, tb)))
