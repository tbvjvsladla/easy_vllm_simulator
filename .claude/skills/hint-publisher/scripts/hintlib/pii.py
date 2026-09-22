"""hintlib.pii — PII 스캐너 단일본 · 발췌 기계 치환 (plan_26092119 §4.4 · SPEC §5.3 · 2026-09-21).

무엇을 하나
    hint 평면의 PII 판정을 **한 벌**로 모은다. 옛 스캐너는 세 벌이었고 의미가 서로 달랐다(코드맵 D1):

      (a) `hint_tag.scan_text`          텍스트 단위 · 리터럴+4종 · 섹션앵커 예외 · 패턴마다 첫 검출 1건
      (b) `hint_branch.scan_payload_pii` 파일×줄 · 리터럴+4종 · 섹션앵커 예외 · **셸 기본값 예외** · 바이너리 건너뜀
      (c) `hint_collect._generic_pii_hits` 줄 단위 · 4종만(리터럴 ✗) · 예외 없음(가장 엄격 — env 형상화 백스톱)

    그 결과 `§` 가 붙은 절번호는 (a)(b)를 통과하고 (c)에 걸렸으며, compose 의 `${VAR:-/mnt/…}` 기본값은
    (b)를 통과하고 (a)(c)에 걸렸다. 이제 차이는 **이름 붙은 프로필과 플래그**다:

      프로필   deploy(4종+리터럴) · nondeploy_prose(abs-op-path 제외 3종+리터럴) · identity(deploy − email)
      플래그   section_anchor_exempt(기본 켬 · (c)=끔) · shell_default_exempt(scan_text 기본 끔 · scan_tree 기본 켬 =(b))
               · skip_generic(이름으로 패턴 제외)

    검출은 언제나 **전부**를 돌려준다(패턴마다 첫 1건에서 멈추지 않는다) — (a) 의 "첫 검출" 은 표시 축약이었을 뿐
    판정은 `hits != []` 였으므로 전부 반환은 그 판정을 바꾸지 않는다.

P3(2026-09-21 사용자 결정 "PII 는 논의 대상이 아님"): **체계를 바꾸지 않는다** — 4종 패턴·섹션앵커 예외·용어 파일을
그대로 옮겼다. 강도 표(무엇에 몇 종을 거는가)의 정본은 `.claude/rules/docs.md` §PII 스캔 적용 범위다:
    - 배포 산출물(hint 태그 오브젝트·페이로드 트리·`docs/report/*`·추적 템플릿·서브 전파분) = **4종 전부**
    - 비배포 산문(gitignored `docs/{plan,devlog,testlog,benchmark,request}`) = 3종(abs-op-path 제외 — manifest 정규 필드
      `nas_model_path` 를 인용한 계획·판정 문서가 전부 비준수가 된다 · 정보를 잃는 대신 얻는 안전이 없다)
    - 기계생성 원시 평면(`docs/simlog/*`·`docs/logs/*`·`campaigns/<id>/*`) = **판정 대상 밖**(2026-08-15 plan_26081516 H3 ·
      spark-host 전수 251,982건 중 99.97% 가 이 평면) — 그래서 이 모듈에 그 평면용 프로필이 없다. Docker 기본 브리지
      대역 예외도 그 원시 평면 한정이라 **어느 프로필에도 들어오지 않는다**(배포 프로필이 그 예외를 가지면 유출이다).
    - `pii_scan.passed` 는 **위 강도로 실제 스캔한 결과**만 적는다 — 좁은 패턴으로 스캔하고 통과를 선언하면 그 선언이
      거짓이다(2026-07-31 실제 발생). 호출부는 쓴 프로필 이름을 함께 기록한다.

발췌 기계 치환 (plan §4.4 · D2)
    02 서사의 원문 발췌는 배포된다. 발췌 **전에** 결정론 치환표를 적용한다: 운영자 절대경로 → `<manifest.<field>>`·`<repo>`·
    `<home>` · 노드 호스트·IP → `<node:main>`·`<node:sub>` · NIC·HCA 장치 이름 → `<nic:<role>>`(한 노드에만 있는 이름) ·
    `<nic:cluster>`(최상위 공통 설정 · 여러 노드가 가진 이름) · 그 밖의 사설 IP → `<priv-ip>` · 사람 식별자(pii_terms) → `<redacted>`.
    치환표는 새 체계가 아니라 **manifest 사실 + 4종 패턴 + 용어 파일**의 재사용이다(P3). 치환 후 텍스트는 deploy 프로필
    스캔에 걸리지 않도록 설계되며(자체검사가 그 성질을 확인한다), 린터의 발췌 무결성 검사는 "인용 = 치환된 출처의 부분
    문자열" 로 판정한다(template.lint).

import 부수효과 0 — 정규식 컴파일만 한다(패키지 불변식). 저장소는 언제나 인자로 받는다.
"""
from __future__ import annotations

import functools
import os
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from . import core

# ── 정본 패턴 4종 (옛 `hint_tag.GENERIC_PII` 그대로 이관 · docs.md 가 가리키는 "정본 패턴" 의 새 자리) ──────────
# 리터럴(`pii_terms.txt`) 위에 얹는 이중 안전망. **일부러 좁다** — 버전 문자열("2.11.0" = 3옥텟)이 IPv4 로 오탐되지
# 않게. 사설 IPv4 는 3옥텟만으로도 성립하므로 `§` 뒤의 3단 절번호·`10.x.y` 버전과 모양이 겹친다 → 아래 섹션앵커 예외.
# 감사 N-4(2026-09-01): 트리를 보는 스캐너가 정본과 다른 패턴 집합을 들고 있어 배포 클론에서 4종 중 1종만 살았다.
# 그래서 이 dict 가 **유일한 소유자**다 — 복제하지 말고 이 모듈을 import 한다.
GENERIC_PII: dict[str, re.Pattern] = {
    "private-ipv4": re.compile(r"\b(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}(?:\.\d{1,3})?"),
    "email": re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
    "abs-op-path": re.compile(r"/(?:mnt|home)/[A-Za-z0-9._/-]+"),
    "spark-host": re.compile(r"spark-[0-9a-f]{3,}"),
}

# 리터럴 검출의 패턴 이름(Hit.pattern). 옛 표기 `term:<t>` 와 같은 접두라 `str(hit)` 이 그대로 이어진다.
TERM = "term"

# 강도 프로필 — 값은 GENERIC_PII 이름(순서 = GENERIC_PII 순서). 리터럴은 모든 프로필에 걸린다(주어졌을 때).
# identity: 태그 tagger·페이로드 커밋 author/committer 신원 검사. 신원에는 **반드시** email 이 있고 합성 신원
#   (`core.SYNTHETIC_EMAIL` · `.invalid` 예약 TLD)도 email 패턴에 걸리므로 email 만 뺀다 — 개인 핸들·도메인 같은
#   **알려진 PII 리터럴**은 여전히 잡는다(옛 `skip_generic={"email"}` · claim C2 · 코드맵 O-br1).
PROFILES: dict[str, tuple[str, ...]] = {
    "deploy": ("private-ipv4", "email", "abs-op-path", "spark-host"),
    "nondeploy_prose": ("private-ipv4", "email", "spark-host"),
    "identity": ("private-ipv4", "abs-op-path", "spark-host"),
}

# 문서 절번호("§10.1.x", "#### 10.1 …")는 사설 IPv4 가 아니다. 위 `10\.\d{1,3}\.` 분기는 모양만으로 둘을 가르지
# 못하므로, 판별자는 **같은 줄 안에서 매치 바로 앞의 텍스트**다 — `§` 기호 또는 마크다운 제목 표지.
# 실측: plan_26081410 에서 14/14 매치가 절번호, 진짜 주소 0(plan_26081514 §6.1). 안전 패턴을 좁히는 것은
# **무손실 증명**이 있을 때만 허용된다 — 병합 게이트는 "진짜 IP 검출이 169건 그대로여야 한다"였다(plan_26081516 §4 H1).
# 이 예외를 넓히거나 좁히면 claim_predicates C2 의 음성 픽스처(와 이 모듈 selftest)가 리뷰를 강제하는 tripwire 다.
_SECTION_ANCHOR = re.compile(r"(?:§\s*|^#{1,6}\s+)$")
# 절번호와 모양이 겹치는 것은 IPv4 분기뿐이다. email·abs-op-path·spark-host 는 겹치지 않는다.
_ANCHOR_EXCLUDED: frozenset[str] = frozenset({"private-ipv4"})

# `${VAR:-/mnt/…}` 의 기본값을 가려내는 **구조적 판별자**(2026-09-01 · 옛 hint_branch). 값의 모양이 아니라 **문맥**으로
# 가른다(섹션앵커가 절번호를 사설 IP 와 가르는 것과 같은 규율). compose 는 경로를 baking 하지 않고 치환형으로만 쓰므로
# 그 기본값은 운영자 지문이 아니라 폴백이다 — 실값은 env 로 주입되고 그 env 는 배포되지 않는다(형상 템플릿만 나간다).
# 좁게 잡는다: 매치 **직전** 텍스트가 `${…:-` 로 끝나야만 면제한다. 면제는 **조용히 넘기지 않는다**(EXEMPT 기록).
_SHELL_DEFAULT = re.compile(r"\$\{[A-Za-z_][A-Za-z0-9_]*:-$")
_SHELL_DEFAULT_PATTERNS: frozenset[str] = frozenset({"abs-op-path"})

# 표시용 매치 절단 길이(옛 hint_branch `[:48]`). Hit.match 자체는 자르지 않는다.
_SHOW = 48


@dataclass(frozen=True)
class Hit:
    """검출 1건. pattern = GENERIC_PII 이름 또는 `"term"` · match = 매치 원문 · line = 1-기반 줄 번호.

    줄 번호는 LF 개수로 센다(git·편집기의 줄). 옛 hint_branch 는 `splitlines()` 로 세어 CR 만 있는 진행표시 로그에서
    번호가 어긋났다(2026-09-21 대조: 검출 집합은 같고 번호만 달랐다).

    `str(hit)` = `"<pattern>:<match>"` — 옛 문자열 검출(`term:<t>` · `private-ipv4:<ip>`)과 같은 모양이라
    옛 호출부의 `startswith("private-ipv4:")` 류 판정을 그대로 옮길 수 있다(claim C2 이관).
    """
    pattern: str
    match: str
    line: int

    def __str__(self) -> str:
        return f"{self.pattern}:{self.match}"


def render_hits(hits: Iterable, limit: int = 20) -> str:
    """차단 메시지용 표시(`[rel:]line: pattern:match48`). Hit 또는 (rel, Hit) 목록을 받는다. 초과분은 개수만 적는다."""
    rows: list[str] = []
    items = list(hits)
    for item in items[:limit]:
        if isinstance(item, tuple):
            where, h = item
            rows.append(f"{where}:{h.line}: {h.pattern}:{h.match[:_SHOW]}")
        else:
            rows.append(f"{item.line}: {item.pattern}:{item.match[:_SHOW]}")
    if len(items) > limit:
        rows.append(f"… 외 {len(items) - limit}건")
    return "\n  ".join(rows)


# ── 용어 파일 ──────────────────────────────────────────────────────────────────────────────────
def load_pii_terms(repo: Path) -> list[str] | None:
    """`.claude/pii_terms.txt` 의 리터럴 목록. **부재 = None**(호출부가 결손 기재/차단을 판단한다).

    `wiki-desk/scripts/scan_forbidden_strings.py` 와 같은 원천(포인터 원칙) — 한 줄 하나 · `#` 주석 · 빈 줄 무시.
    파일이 있는데 읽을 수 없으면 부재가 아니라 결함이다(HINT_PII_TERMS_UNREADABLE) — 깨진 파일을 "없음" 으로 읽으면
    리터럴 검사가 조용히 꺼진다.
    """
    p = Path(repo) / core.REL_PII_TERMS
    if not p.exists():
        return None
    if not p.is_file():
        core.fail("HINT_PII_TERMS_UNREADABLE", f"{core.REL_PII_TERMS} 가 파일이 아니다",
                  "한 줄에 리터럴 하나를 둔 텍스트 파일이어야 한다.")
    try:
        raw = p.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        core.fail("HINT_PII_TERMS_UNREADABLE", f"{core.REL_PII_TERMS} 를 읽을 수 없다: {type(e).__name__}",
                  "UTF-8 텍스트로 고친다(리터럴 검사를 조용히 끄지 않는다).")
    terms: list[str] = []
    for line in raw.splitlines():
        s = line.strip()
        if s and not s.startswith("#"):
            terms.append(s)
    return terms


def require_terms(repo: Path) -> list[str]:
    """배포면용 — 용어 파일 부재 = 차단(HINT_PII_TERMS_ABSENT).

    2026-09-21 코드맵 K11: 부재의 의미가 도구마다 반대였다(hint_branch = die · hint_collect = 결손 코드
    `HINT_MISSING_PII_TERMS` 기재 · 계약 §5 = "선언된 결손은 통과"). 그래서 선언 경로는 branch publish 에서 반드시
    죽었다. 이제 하나로 정한다: **배포면은 리터럴 없이 PII-clean 을 인증하지 않는다**(옛 hint_branch 규율 · 감사 ⑫:
    이 파일은 비추적이라 배포 클론의 기본 상태가 부재다). 빈 파일(리터럴 0개)은 "있음" 이다 — 운영자의 선언이다.
    """
    terms = load_pii_terms(repo)
    if terms is None:
        core.fail("HINT_PII_TERMS_ABSENT",
                  f"`{core.REL_PII_TERMS}` 부재 — 배포면 PII-clean 을 인증할 수 없다(fail-closed)",
                  f"`{core.REL_PII_TERMS}` 를 만든다(한 줄에 리터럴 하나 · `#` 주석). 비추적 파일이라 새 클론의 기본 "
                  "상태는 부재다(감사 ⑫). 리터럴이 정말 없으면 빈 파일로 그 사실을 선언한다.")
    return terms


# ── 스캐너 ─────────────────────────────────────────────────────────────────────────────────────
def _patterns_for(profile: str, skip_generic: Iterable[str]) -> tuple[str, ...]:
    if profile not in PROFILES:
        core.fail("HINT_PII_PROFILE_UNKNOWN", f"PII 프로필 {profile!r} 은 없다",
                  f"프로필은 {sorted(PROFILES)} 중 하나다(강도 표 정본 = docs.md §PII 스캔 적용 범위).")
    skip = frozenset(skip_generic)
    unknown = sorted(skip - set(GENERIC_PII))
    if unknown:
        # 오타 난 제외 이름은 아무것도 제외하지 않는다 — 호출자가 믿는 강도와 실제 강도가 갈리므로 소리낸다.
        core.fail("HINT_PII_PATTERN_UNKNOWN", f"skip_generic 에 없는 패턴 이름: {unknown}",
                  f"패턴 이름은 {list(GENERIC_PII)} 중 하나다.")
    return tuple(n for n in PROFILES[profile] if n not in skip)


def _line_prefix(text: str, start: int) -> str:
    return text[text.rfind("\n", 0, start) + 1:start]


def _is_section_anchored(text: str, start: int) -> bool:
    """`start` 의 매치가 **같은 줄 안에서** `§` 또는 마크다운 제목 표지 바로 뒤에 있으면 참 — 주소가 아니라 절번호다.

    접두는 일부러 줄 시작에서 자른다: 앞선 텍스트 전체를 보면 윗줄에 홀로 선 `§` 하나가 한참 아래의 진짜 주소를
    억제한다(옛 hint_tag `_is_section_anchored` · hint_branch 는 줄 단위로 같은 효과)."""
    return _SECTION_ANCHOR.search(_line_prefix(text, start)) is not None


def _is_shell_default(text: str, start: int) -> bool:
    return _SHELL_DEFAULT.search(_line_prefix(text, start)) is not None


def _note_exempt(exempt_log: list | None, kind: str, record: str) -> None:
    """면제는 조용히 넘기지 않는다(2026-09-01 hint_branch `EXEMPT(shell-default)` stderr 출력 이관).
    exempt_log 가 주어지면 거기 적고(호출부가 보고서에 싣는다), 아니면 stderr 에 적는다."""
    if exempt_log is not None:
        exempt_log.append(f"{kind} {record}")
    else:
        print(f"[hint] EXEMPT({kind}) {record}", file=sys.stderr)


def _dedupe(terms: Iterable[str] | None) -> list[str]:
    if isinstance(terms, (str, bytes)):
        # 문자열 하나를 넘기면 글자마다 리터럴이 되어 전 본문이 검출된다 — 목록으로 감싸라고 소리낸다.
        core.fail("HINT_PII_TERMS_SHAPE", "리터럴 목록 자리에 문자열 하나가 왔다", "[리터럴] 목록으로 넘긴다.")
    out: list[str] = []
    seen: set[str] = set()
    for t in terms or ():
        if not isinstance(t, str):
            core.fail("HINT_PII_TERMS_SHAPE", f"리터럴은 문자열이어야 한다: {type(t).__name__}")
        if t and t not in seen:
            seen.add(t)
            out.append(t)
    return out


def scan_text(text: str, terms: Iterable[str] | None, *, profile: str = "deploy",
              skip_generic: Iterable[str] = frozenset(), shell_default_exempt: bool = False,
              section_anchor_exempt: bool = True, exempt_log: list | None = None) -> list[Hit]:
    """텍스트 하나를 리터럴 + 프로필 패턴으로 스캔해 **전부**를 돌려준다(빈 목록 = 무검출).

    terms=None 은 "리터럴 없이" 다 — 배포면 호출부는 `require_terms` 로 먼저 받는다(K11).
    옛 3벌의 의미는 인자로 재현한다:
        (a) hint_tag.scan_text           → 기본값 그대로(terms 주고 · skip_generic 로 identity 대체 가능)
        (b) hint_branch.scan_payload_pii → shell_default_exempt=True (또는 scan_tree 기본값)
        (c) hint_collect 백스톱          → terms=None, section_anchor_exempt=False (가장 엄격)

    매치는 **모두** 걷는다: 예외가 걸린 상태에서 첫 매치에서 멈추면 앞선 오탐(절번호) 하나가 같은 텍스트 뒤쪽의
    진짜 주소를 가린다(옛 scan_text docstring · claim C2 "앵커 뒤 진짜 IP 양성").
    반환 순서는 결정론: (위치, 리터럴 순서 → GENERIC_PII 순서).
    """
    if not isinstance(text, str):
        core.fail("HINT_PII_INPUT_NOT_TEXT", f"scan_text 는 str 을 받는다: {type(text).__name__}",
                  "바이트는 호출부가 UTF-8 로 해독한다(해독 불가 = 바이너리 → scan_tree 규칙).")
    names = _patterns_for(profile, skip_generic)
    found: list[tuple[int, int, Hit]] = []
    lits = _dedupe(terms)
    for order, t in enumerate(lits):
        pos = text.find(t)
        while pos != -1:
            found.append((pos, order, Hit(TERM, t, text.count("\n", 0, pos) + 1)))
            pos = text.find(t, pos + len(t))
    base = len(lits)
    for idx, name in enumerate(GENERIC_PII):
        if name not in names:
            continue
        for m in GENERIC_PII[name].finditer(text):
            start = m.start()
            if section_anchor_exempt and name in _ANCHOR_EXCLUDED and _is_section_anchored(text, start):
                continue  # 문서 절번호(§10.x) — 무손실 증명된 예외(plan_26081516 H1)
            line = text.count("\n", 0, start) + 1
            if shell_default_exempt and name in _SHELL_DEFAULT_PATTERNS and _is_shell_default(text, start):
                _note_exempt(exempt_log, "shell-default", f"{line}: {name}:{m.group(0)[:_SHOW]}")
                continue
            found.append((start, base + idx, Hit(name, m.group(0), line)))
    found.sort(key=lambda x: (x[0], x[1]))
    return [h for _, _, h in found]


def _read_scannable(path: Path) -> str | None:
    """스캔할 텍스트. 심링크 = **링크 문자열**(git 이 blob 으로 싣는 바이트가 그것이다 — 절대경로 대상이면 그 자체가
    운영자 지문이다) · 파일 = UTF-8 해독 · 해독 불가 = None(바이너리 · 확장자가 아니라 내용으로 판정 — 감사 N-1)."""
    if path.is_symlink():
        return os.readlink(path)
    try:
        data = path.read_bytes()
    except OSError as e:
        core.fail("HINT_PII_PATH_UNREADABLE", f"스캔 대상을 읽을 수 없다: {path.name} ({type(e).__name__})")
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _walk(root: Path, base: Path) -> list[tuple[str, Path]]:
    """결정론 순서(이름 정렬)의 재귀 목록. 디렉터리 심링크는 따라가지 않고 링크 자체를 싣는다.
    FIFO·소켓·장치 파일은 여는 순간 멈출 수 있고 배포 트리에 있을 이유가 없다 → 소리내어 거부."""
    out: list[tuple[str, Path]] = []
    with os.scandir(root) as it:
        entries = sorted(it, key=lambda e: e.name)
    for e in entries:
        p = Path(e.path)
        if e.is_symlink():
            out.append((p.relative_to(base).as_posix(), p))
        elif e.is_dir(follow_symlinks=False):
            out.extend(_walk(p, base))
        elif e.is_file(follow_symlinks=False):
            out.append((p.relative_to(base).as_posix(), p))
        else:
            core.fail("HINT_PII_UNSCANNABLE_FILE", f"일반 파일이 아닌 항목: {p.relative_to(base).as_posix()}",
                      "스캔 트리에서 특수 파일(FIFO·소켓·장치)을 치운다.")
    return out


def _selected(root: Path, rels: Iterable[str]) -> list[tuple[str, Path]]:
    """rels(루트 상대) → 목록. 절대경로·상위탈출·부재는 거부(옛 hint_branch `_read_manifest` 규율).
    디렉터리는 펼친다."""
    root_res = root.resolve()
    out: dict[str, Path] = {}
    for r in rels:
        if not isinstance(r, str) or not r or r.startswith("/") or ".." in Path(r).parts:
            core.fail("HINT_PII_PATH_INVALID", f"스캔 경로는 루트 상대여야 한다(절대·상위탈출·빈 값 ✗): {r!r}")
        p = root / r
        if not (p.is_symlink() or p.exists()):
            # 부재를 "무검출" 로 읽으면 스캔하지 않은 것을 통과로 적게 된다(2026-07-31 pii_scan.passed 거짓 선례).
            core.fail("HINT_PII_PATH_ABSENT", f"스캔 대상 부재: {r}")
        try:
            p.parent.resolve().relative_to(root_res)
        except ValueError:
            core.fail("HINT_PII_PATH_INVALID", f"스캔 경로가 루트 밖으로 이어진다(상위 심링크): {r}")
        if p.is_dir() and not p.is_symlink():
            for rr, pp in _walk(p, root):
                out[rr] = pp
        else:
            out[Path(r).as_posix()] = p
    return sorted(out.items())


def scan_tree(root: Path, terms: Iterable[str] | None, *, profile: str = "deploy",
              skip_generic: Iterable[str] = frozenset(), shell_default_exempt: bool = True,
              section_anchor_exempt: bool = True, rels: Iterable[str] | None = None,
              exempt_log: list | None = None) -> list[tuple[str, Hit]]:
    """트리(또는 그 안의 rels)를 스캔해 `(루트 상대 posix 경로, Hit)` 전부를 돌려준다.

    - **shell_default_exempt 기본값이 참**이다 — 트리 스캐너의 옛 의미(hint_branch.scan_payload_pii · 페이로드에
      compose 파일이 실린다)를 보존한다. 끄려면 명시한다. scan_text 의 기본값(거짓)과 다르다는 점에 주의.
    - 바이너리(UTF-8 해독 불가)는 건너뛴다. exempt_log 가 주어지면 건너뛴 사실을 거기 적는다.
    - 심링크는 따라가지 않고 링크 문자열을 스캔한다(git 이 싣는 바이트).
    - terms=None 은 "리터럴 없이" 다. 배포면은 `require_terms(repo)` 로 받은 목록을 넘긴다(K11).
    """
    root = Path(root)
    if not root.is_dir():
        core.fail("HINT_PII_ROOT_ABSENT", f"스캔 루트가 디렉터리가 아니다: {root.name}")
    _patterns_for(profile, skip_generic)   # 빈 트리여도 프로필 오타는 잡는다
    entries = _walk(root, root) if rels is None else _selected(root, rels)
    lits = _dedupe(terms)
    out: list[tuple[str, Hit]] = []
    for rel_path, path in entries:
        text = _read_scannable(path)
        if text is None:
            if exempt_log is not None:
                exempt_log.append(f"binary {rel_path}: UTF-8 해독 불가 — 건너뜀")
            continue
        local: list[str] = []
        hits = scan_text(text, lits, profile=profile, skip_generic=skip_generic,
                         shell_default_exempt=shell_default_exempt,
                         section_anchor_exempt=section_anchor_exempt, exempt_log=local)
        for rec in local:
            kind, _, rest = rec.partition(" ")
            _note_exempt(exempt_log, kind, f"{rel_path}:{rest}")
        out.extend((rel_path, h) for h in hits)
    return out


# ── 발췌 기계 치환 (plan §4.4) ─────────────────────────────────────────────────────────────────
# 치환 표지 어휘의 단일 소유자. 옛 env 형상화는 `<manifest.nodes[].<key>>` 를 냈고 plan §4.4 는 `<node:main|sub>` 를
# 원했다 — 어휘를 여기로 모은다(코드맵 hint_collect §10 "Unify the placeholder vocabulary in pii").
PH_REPO = "<repo>"
PH_HOME = "<home>"
PH_PRIV_IP = "<priv-ip>"
PH_HOST = "<host>"
PH_ABS_PATH = "<abs-path>"
PH_REDACTED = "<redacted>"
# 치환표가 놓친 잔여를 4종 패턴으로 가리는 표지. email 은 사람 식별자라 제거(`<redacted>`).
RESIDUAL_PLACEHOLDERS: dict[str, str] = {
    "private-ipv4": PH_PRIV_IP,
    "email": PH_REDACTED,
    "abs-op-path": PH_ABS_PATH,
    "spark-host": PH_HOST,
}


def node_placeholder(label: str) -> str:
    return f"<node:{label}>"


def manifest_placeholder(field: str) -> str:
    return f"<manifest.{field}>"


def nic_placeholder(label: str) -> str:
    return f"<nic:{label}>"


# ── NIC·HCA 장치 이름 (2026-09-22 · plan_26092119 S2 round 2 · F8) ────────────────────────────────────────────
# 왜: 1.5 PROMPT 는 "NIC 이름을 적지 마라" 고 하는데 치환표가 NIC 이름을 몰라 엔진 로그 발췌(NCCL `NET/IB : Using [0]<hca>…`
#   · `SOCKET_IFNAME … <iface>`)가 장치 이름을 그대로 실었다(S2 1차 저작자 template_defects #3). **스캐너 체계 변경이 아니라
#   치환 추가**다(P3) — 4종 패턴·용어 파일은 그대로이고, NIC 이름은 스캔 대상이 아니다(장치 이름은 사람 식별자가 아니라 환경
#   지문이므로 발췌에서 자리표시로 바꿀 뿐 차단하지 않는다).
# 무엇이 NIC 필드인가(닫힌 키 토큰 · tripwire): 키를 `_` 로 자른 토큰 중 하나가 아래 목록이면 그 값이 장치 이름 후보다
#   (manifest 실측 2026-09-22: `interconnect.socket_iface` · `interconnect.hca_devices[]`). 값은 장치 이름 모양 + **숫자를 하나 이상**
#   품은 것만 받는다(`auto`·`RoCE v2` 같은 낱말이 온 본문에서 깎이지 않게) · NCCL 목록 문법(`^lo,eth0` · `=eth0`)은 쉼표로 가르고
#   앞의 `^`·`=` 를 뗀다.
# 라벨: 이름 하나가 **한 노드에만** 속할 때만 그 노드의 role 라벨(`nodes[]` 안의 필드 · `<nic:sub>`)이다. 최상위 필드(예
#   `interconnect.*`)는 클러스터 공통 설정이고, 이름 하나를 두 노드 이상이 가지면 그 이름도 클러스터 공통이다 — 둘 다
#   `<nic:cluster>`(모든 노드가 같은 이름으로 쓰는 장치)다.
#   2026-09-22 S2 round 2 적대 리뷰: 1판은 최상위를 manifest 를 쓴 노드(`self_role` = main)로 붙였다. 그런데 렌더러는 최상위
#   `interconnect` 하나로 `.env.interconnect` 를 만들어 **두 노드에 같이** 보낸다(render_dockerfile `--nccl-envfile`) — 서브 Ray
#   worker 로그 줄의 같은 장치 이름이 `<nic:main>` 이 되어 "서브가 메인의 NIC 를 썼다" 로 읽혔다(도구가 만든 오귀속 · 오도 바이트).
#   nodes[] 둘이 같은 이름을 적었을 때 사전순으로 `<nic:main>` 이 이기던 것도 같은 오귀속이다.
_NIC_KEY_TOKENS = frozenset({"iface", "ifaces", "ifname", "interface", "interfaces", "nic", "nics", "netdev", "hca", "hcas"})
_NIC_VALUE = re.compile(r"[A-Za-z][A-Za-z0-9._:-]{1,31}")
NIC_LABEL_SHARED = "cluster"


def _is_nic_key(key: str) -> bool:
    base = re.sub(r"\[[^\]]*\]$", "", str(key)).lower()
    return any(tok in _NIC_KEY_TOKENS for tok in base.split("_"))


def _nic_values(val) -> list[str]:
    """NIC 필드 값 → 장치 이름 목록(문자열 · 문자열 목록 · NCCL 쉼표 목록). 모양이 아니거나 숫자가 없으면 버린다."""
    items = val if isinstance(val, list) else [val]
    out: list[str] = []
    for it in items:
        if not isinstance(it, str):
            continue
        for part in it.split(","):
            v = part.strip().lstrip("^=").strip()
            if _NIC_VALUE.fullmatch(v) and any(ch.isdigit() for ch in v) and not _is_ipv4(v):
                out.append(v)
    return out


def _walk_nic(obj, key: str, out: list[str]) -> None:
    """dict 를 훑어 NIC 키의 값을 모은다(nodes 목록은 호출부가 따로 — 라벨이 다르다)."""
    if isinstance(obj, dict):
        for k in sorted(obj, key=str):
            if str(k) == "nodes" and key == "":
                continue
            v = obj[k]
            if _is_nic_key(str(k)) and (isinstance(v, str) or (isinstance(v, list) and all(isinstance(x, str) for x in v))):
                out.extend(_nic_values(v))
            else:
                _walk_nic(v, str(k), out)
    elif isinstance(obj, list):
        for item in obj:
            _walk_nic(item, key, out)


# 치환표 행의 우선순위(같은 리터럴이 두 출처에서 오면 낮은 값이 이긴다 · 동률은 표지 사전순) — 더 많이 말하는 표지가 이긴다.
_PRIO_REPO, _PRIO_NODE, _PRIO_NIC, _PRIO_FIELD, _PRIO_HOME, _PRIO_GENERIC, _PRIO_TERM = range(7)
_PRIO_NAME = {_PRIO_REPO: "repo", _PRIO_NODE: "manifest.nodes", _PRIO_NIC: "manifest NIC 필드", _PRIO_FIELD: "manifest 경로 필드",
              _PRIO_HOME: "home", _PRIO_GENERIC: "manifest 값", _PRIO_TERM: "pii_terms"}
_IPV4 = re.compile(r"\d{1,3}(?:\.\d{1,3}){3}")
_HOSTNAME = re.compile(r"[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?")
_ROLE = re.compile(r"[a-z][a-z0-9_-]*")
_USER = re.compile(r"[A-Za-z0-9._-]+")
_LOOPBACK = frozenset({"localhost", "0.0.0.0"})


def _is_ipv4(v: str) -> bool:
    return bool(_IPV4.fullmatch(v)) and all(int(o) <= 255 for o in v.split("."))


def _is_host_key(key: str) -> bool:
    k = key.lower()
    return k in ("host", "hostname", "ssh_host") or k.endswith("_host") or k.endswith("_hostname")


def _is_abs_path(v: str) -> bool:
    """운영자 절대경로 후보: `/` 로 시작 · 성분 2개 이상(`/tmp` 같은 한 성분 경로를 온 본문에서 치환하지 않는다) · 공백 없음."""
    if not v.startswith("/") or v.startswith("//") or any(c.isspace() for c in v):
        return False
    return len([p for p in v.split("/") if p]) >= 2


def _home_of(path: str) -> str | None:
    """`/home/<user>/…` → `/home/<user>`. 저장소 경로에서 파생한다(환경변수 HOME 을 읽지 않는다 — 결정론)."""
    parts = path.split("/")
    if len(parts) >= 3 and parts[0] == "" and parts[1] == "home" and _USER.fullmatch(parts[2] or ""):
        return f"/home/{parts[2]}"
    return None


def _node_labels(nodes: list) -> list[str]:
    """노드 표지 라벨: role(main/sub…) · 같은 role 이 둘 이상이면 서수(sub1·sub2) · role 이 없거나 이상하면 인덱스."""
    roles = [n.get("role") if isinstance(n, dict) else None for n in nodes]
    ok = [r if isinstance(r, str) and _ROLE.fullmatch(r) else None for r in roles]
    counts: dict[str, int] = {}
    for r in ok:
        if r:
            counts[r] = counts.get(r, 0) + 1
    seen: dict[str, int] = {}
    labels: list[str] = []
    for i, r in enumerate(ok):
        if not r:
            labels.append(str(i))
        elif counts[r] == 1:
            labels.append(r)
        else:
            seen[r] = seen.get(r, 0) + 1
            labels.append(f"{r}{seen[r]}")
    return labels


def _walk_strings(obj, prefix: str, node_labels: list[str] | None = None) -> list[tuple[str, str]]:
    """manifest 의 모든 문자열 값을 (점 경로, 값) 으로 — 키 정렬(결정론). nodes 목록은 role 라벨로 색인한다."""
    out: list[tuple[str, str]] = []
    if isinstance(obj, dict):
        for k in sorted(obj, key=str):
            key = str(k)
            path = f"{prefix}.{key}" if prefix else key
            v = obj[k]
            if key == "nodes" and not prefix and isinstance(v, list):
                labels = _node_labels(v)
                for i, item in enumerate(v):
                    out.extend(_walk_strings(item, f"nodes[{labels[i]}]"))
            else:
                out.extend(_walk_strings(v, path))
    elif isinstance(obj, list):
        for i, item in enumerate(obj):
            out.extend(_walk_strings(item, f"{prefix}[{i}]"))
    elif isinstance(obj, str):
        out.append((prefix, obj))
    return out


def substitution_table(repo: Path, manifest: dict | None) -> list[tuple[str, str]]:
    """발췌 기계 치환표 `[(리터럴, 표지)]` — 결정론 · **긴 리터럴 우선** 정렬(길이 내림차순 → 사전순).

    행의 출처(같은 리터럴이면 앞의 것이 이긴다):
        repo       저장소 절대경로(주어진 모양과 resolve 모양)                    → `<repo>`
        nodes      manifest `nodes[]` 의 host·hostname(·*_host)·IPv4 값           → `<node:main>` · `<node:sub>`
        NIC        manifest NIC·HCA 장치 이름(`*_iface` · `hca_devices[]` 등)      → `<nic:<role>>`(한 노드의 nodes[] 에만
                   있는 이름) · `<nic:cluster>`(최상위 공통 설정 · 둘 이상의 노드가 가진 이름) · 2026-09-22 S2 round 2
        경로 필드  manifest 의 절대경로 값(성분 ≥ 2 · 끝 `/` 제거)                → `<manifest.<점 경로>>`
        home       저장소 경로의 `/home/<user>` · 노드 `ssh_user` 의 `/home/<user>` → `<home>`
        그 밖      nodes 밖의 사설 IPv4 전체값 → `<priv-ip>` · spark-host 전체값 → `<host>`
        pii_terms  `.claude/pii_terms.txt` 리터럴(사람 식별자 · 호스트명 등)       → `<redacted>`

    - manifest 는 호출부가 파싱한 dict 다(이 모듈은 YAML 을 읽지 않는다 — stdlib 전용). None 이면 repo·home·terms 만.
    - 용어 파일이 없으면 terms 행이 없다. 배포면 스캔은 어차피 `require_terms` 로 차단되므로 여기서 두 번 막지 않는다.
    - 리터럴이 어떤 표지의 부분 문자열이면 차단(HINT_PII_SUBST_COLLISION) — 치환이 멱등이 아니게 되고, 용어가 표지 안에
      남아 배포 스캔이 치환된 본문을 다시 잡는다. 메시지에 리터럴 자체는 싣지 않는다(그것이 PII 다).
    - 노드 `ssh_user` 맨몸은 행이 되지 않는다: 짧은 계정명이 본문의 다른 낱말을 깎는다. 사람 이름은 pii_terms 의 몫이다.
    """
    best: dict[str, tuple[int, str]] = {}

    def add(prio: int, literal: str, ph: str) -> None:
        if not literal:
            return
        cur = best.get(literal)
        if cur is None or (prio, ph) < cur:
            best[literal] = (prio, ph)

    repo_forms = dict.fromkeys([Path(os.path.abspath(repo)).as_posix(), Path(repo).resolve().as_posix()])
    for rp in repo_forms:
        if _is_abs_path(rp):
            add(_PRIO_REPO, rp.rstrip("/"), PH_REPO)
        home = _home_of(rp)
        if home:
            add(_PRIO_HOME, home, PH_HOME)

    if manifest is not None:
        if not isinstance(manifest, dict):
            core.fail("HINT_PII_MANIFEST_SHAPE", f"manifest 는 dict 여야 한다: {type(manifest).__name__}")
        nodes = manifest.get("nodes") or []
        if not isinstance(nodes, list):
            core.fail("HINT_PII_MANIFEST_SHAPE", "manifest.nodes 는 목록이어야 한다")
        labels = _node_labels(nodes)
        nic_owner: dict[str, set[str]] = {}          # NIC 리터럴 → 그 이름을 가진 라벨들(한 노드만 = 그 노드 · 그 밖 = 공통)
        for i, node in enumerate(nodes):
            if not isinstance(node, dict):
                core.fail("HINT_PII_MANIFEST_SHAPE", f"manifest.nodes[{i}] 가 mapping 이 아니다")
            for key in sorted(node, key=str):
                val = node[key]
                if not isinstance(val, str):
                    continue
                v = val.strip()
                if v in _LOOPBACK or v.startswith("127."):
                    continue  # 루프백은 노드 지문이 아니다 — 본문의 루프백 언급을 노드로 오귀속하지 않는다
                if _is_ipv4(v) or (_is_host_key(str(key)) and len(v) >= 3 and _HOSTNAME.fullmatch(v)):
                    add(_PRIO_NODE, v, node_placeholder(labels[i]))
                if str(key) == "ssh_user" and _USER.fullmatch(v):
                    add(_PRIO_HOME, f"/home/{v}", PH_HOME)
            node_nics: list[str] = []
            _walk_nic(node, "node", node_nics)
            for nic in node_nics:
                nic_owner.setdefault(nic, set()).add(labels[i])
        top_nics: list[str] = []
        _walk_nic(manifest, "", top_nics)
        for nic in top_nics:
            nic_owner.setdefault(nic, set()).add(NIC_LABEL_SHARED)
        for nic, owners in sorted(nic_owner.items()):
            add(_PRIO_NIC, nic, nic_placeholder(next(iter(owners)) if len(owners) == 1 else NIC_LABEL_SHARED))
        for field, val in _walk_strings(manifest, ""):
            v = val.strip()
            if _is_abs_path(v):
                add(_PRIO_FIELD, v.rstrip("/"), manifest_placeholder(field))
            elif _is_ipv4(v) and GENERIC_PII["private-ipv4"].fullmatch(v):
                add(_PRIO_GENERIC, v, PH_PRIV_IP)
            elif GENERIC_PII["spark-host"].fullmatch(v):
                add(_PRIO_GENERIC, v, PH_HOST)

    for t in _dedupe(load_pii_terms(repo)):
        add(_PRIO_TERM, t, PH_REDACTED)

    placeholders = {ph for _, ph in best.values()} | set(RESIDUAL_PLACEHOLDERS.values()) | {
        PH_REPO, PH_HOME, PH_PRIV_IP, PH_HOST, PH_ABS_PATH, PH_REDACTED}
    for literal, (prio, _) in sorted(best.items()):
        clash = sorted(ph for ph in placeholders if literal in ph)
        if clash:
            core.fail("HINT_PII_SUBST_COLLISION",
                      f"{_PRIO_NAME[prio]} 리터럴(길이 {len(literal)})이 치환 표지 {clash[0]} 안에 들어 있다 — "
                      "치환이 멱등이 아니게 되고 배포 스캔이 치환된 본문을 다시 잡는다",
                      f"`{core.REL_PII_TERMS}` 의 그 리터럴을 더 구체적으로 적거나 manifest 값을 확인한다.")
    return sorted(((lit, ph) for lit, (_, ph) in best.items()), key=lambda kv: (-len(kv[0]), kv[0]))


def _literal_regex(literal: str, placeholder: str | None = None) -> str:
    """리터럴의 경계 규칙 — **모양으로** 정한다(표 행은 (리터럴, 표지) 두 칸뿐이다):
    IPv4 = 양쪽 숫자 경계(`…0.1` 이 `…0.12` 안에서 노드로 오귀속되지 않게) · 경로 = 오른쪽 경로문자 경계
    (`<home>/u` 리터럴이 `<home>/u2` 를 깎지 않게) · NIC 장치 이름(표지 `<nic:…>`) = 양쪽 식별자 경계(`eth1` 이 `eth10` 을
    깎지 않게 · 2026-09-22 S2 round 2 — NIC 이름은 스캔 대상이 아니므로 부분 문자열 단위일 이유가 없다) · 그 밖(용어·호스트명) =
    부분 문자열 — 스캐너가 리터럴을 부분 문자열로 잡으므로(옛 규율) 치환도 같은 단위여야 치환된 본문이 스캔을 통과한다."""
    esc = re.escape(literal)
    if placeholder is not None and placeholder.startswith("<nic:"):
        return rf"(?<![A-Za-z0-9_]){esc}(?![A-Za-z0-9_])"
    if _is_ipv4(literal):
        return rf"(?<![0-9.]){esc}(?![0-9]|\.[0-9])"
    if literal.startswith("/"):
        return rf"{esc}(?![A-Za-z0-9._-])"
    return esc


def _table_rows(table) -> tuple[tuple[str, str], ...]:
    rows: list[tuple[str, str]] = []
    for row in table:
        if (not isinstance(row, (tuple, list)) or len(row) != 2 or not isinstance(row[0], str)
                or not isinstance(row[1], str) or not row[0]):
            core.fail("HINT_PII_SUBST_TABLE_SHAPE", "치환표 행은 (비어 있지 않은 리터럴, 표지) 문자열 쌍이다",
                      "substitution_table() 의 반환값을 그대로 넘긴다.")
        rows.append((row[0], row[1]))
    return tuple(rows)


@functools.lru_cache(maxsize=16)
def _compiled(table: tuple[tuple[str, str], ...]) -> tuple[re.Pattern, dict[str, str]]:
    repl: dict[str, str] = {}
    for row in table:
        if row[0] in repl and repl[row[0]] != row[1]:
            core.fail("HINT_PII_SUBST_TABLE_SHAPE", f"같은 리터럴(길이 {len(row[0])})에 표지가 둘이다")
        repl[row[0]] = row[1]
    order = sorted(repl, key=lambda s: (-len(s), s))
    return re.compile("|".join(_literal_regex(s, repl[s]) for s in order)), repl


def _mask_residual(text: str, name: str, placeholder: str) -> str:
    pat = GENERIC_PII[name]
    pieces: list[str] = []
    last = 0
    for m in pat.finditer(text):
        if name in _ANCHOR_EXCLUDED and _is_section_anchored(text, m.start()):
            continue  # 절번호는 스캐너가 판정하지 않는 것과 똑같이 치환도 하지 않는다(발췌가 원문과 어긋나지 않게)
        pieces.append(text[last:m.start()])
        pieces.append(placeholder)
        last = m.end()
    pieces.append(text[last:])
    return "".join(pieces)


def substitute(text: str, table: list[tuple[str, str]], *, residual: bool = True) -> str:
    """발췌 기계 치환(결정론). 두 단계:

    1. 치환표 — 한 번의 정규식 통과(교대 순서 = 긴 리터럴 우선)라 치환 결과를 다시 훑지 않는다(연쇄 치환 ✗).
    2. residual(기본 켬) — 표가 놓친 4종 패턴 잔여를 표지로 가린다(`RESIDUAL_PLACEHOLDERS` · 절번호 예외는 스캐너와
       같다). 표에 없는 LAN 주소·다른 NAS 경로가 발췌를 막지 않게 하는 자리다. 과치환은 정보 손실일 뿐 유출이 아니다.
       env 형상화의 "조용히 덧칠하지 않는다"(2026-09-06 Q9)는 **키 축 규칙이 놓친 것을 숨기지 말라**는 형상화 전용
       규율이다 — 그쪽은 artifacts 가 `scan_text(..., section_anchor_exempt=False)` 백스톱으로 차단한다.

    멱등이다: substitute(substitute(x)) == substitute(x) (표지는 어떤 리터럴·패턴에도 걸리지 않는다 — 표 생성 시 검사).
    """
    if not isinstance(text, str):
        core.fail("HINT_PII_INPUT_NOT_TEXT", f"substitute 는 str 을 받는다: {type(text).__name__}")
    rows = _table_rows(table)
    if rows:
        rx, repl = _compiled(rows)
        text = rx.sub(lambda m: repl[m.group(0)], text)
    if residual:
        for name, ph in RESIDUAL_PLACEHOLDERS.items():
            text = _mask_residual(text, name, ph)
    return text


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
def _ip(*octets: int) -> str:
    """픽스처 주소 조립기. 이 파일은 추적·배포 파일이라 IPv4 모양 리터럴을 쓰지 않는다 — 쓰면 픽스처가 자기가 지키는
    게이트에 걸린다(2026-08-06 docs.md 자기스캔 사고 · claim C2 의 `'10.' + '1.2'` 조립과 같은 자기참조 방지)."""
    return ".".join(str(o) for o in octets)


def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 라이브 용어 파일·manifest·태그에 의존하지 않는다(임시 디렉터리 픽스처).

    픽스처 값은 **합성**이다(2026-09-04 실측: 운영자 실호스트명을 쓴 픽스처 줄을 배포면 스캔이 `term:` 으로 잡았다 —
    합성 값으로도 generic 패턴 발화는 똑같이 증명된다). 패턴에 걸리는 픽스처 문자열은 조각으로 조립한다.
    """
    bad: list[str] = []

    def ck(name: str, cond: bool) -> None:
        if not cond:
            bad.append(f"pii: {name}")

    def code_of(fn) -> str | None:
        try:
            fn()
        except core.HintError as e:
            return e.code
        return None

    def names(hits) -> list[str]:
        return [h.pattern for h in hits]

    terms = ["forbidden-secret-token"]
    genuine_ip = _ip(192, 168, 1, 5)          # RFC1918 양성 픽스처(조립)
    ten_ip = _ip(10, 20, 30, 40)
    s172_ip = _ip(172, 20, 1, 2)
    sec = '10.' + '1.2'                       # 절번호 — claim C2 픽스처와 같은 조립(추적 파일에 IPv4 모양 리터럴 ✗)
    nas = "/" + "mnt/fixture-nas/Model/x"
    spark = "spark-" + "0f0f"
    email = "bob" + "@" + "example.com"

    # ── 정본 4종 · 프로필 구조
    ck("정본 패턴 4종(이름·순서)", list(GENERIC_PII) == ["private-ipv4", "email", "abs-op-path", "spark-host"])
    ck("deploy = 4종 전부", PROFILES["deploy"] == tuple(GENERIC_PII))
    ck("nondeploy_prose = deploy − abs-op-path",
       PROFILES["nondeploy_prose"] == tuple(n for n in GENERIC_PII if n != "abs-op-path"))
    ck("identity = deploy − email", PROFILES["identity"] == tuple(n for n in GENERIC_PII if n != "email"))

    # ── claim C2 픽스처 의미 보존 (plan_26081514 §6.4 · plan_26081516 H1)
    ck("C2 용어 양성", scan_text("this text contains forbidden-secret-token here", terms) != [])
    ck("C2 RFC1918 양성", scan_text(f"{genuine_ip} is a private ip", terms) != [])
    ck("C2 깨끗한 본문 음성", scan_text("nothing sensitive here at all", terms) == [])
    ck("★C2 §-앵커 절번호는 사설 IPv4 가 아니다", scan_text(f"본문 §{sec} 를 참조", terms) == [])
    ck("★C2 제목-앵커 절번호는 사설 IPv4 가 아니다", scan_text(f"#### {sec} 관측된 부작용", terms) == [])
    after = scan_text(f"§{sec} 요약\n서브 노드 {genuine_ip} 도달", terms)
    ck("★C2 앵커 뒤(다음 줄) 진짜 IP 는 여전히 양성",
       [str(h) for h in after if str(h).startswith("private-ipv4:")] == [f"private-ipv4:{genuine_ip}"])
    same_line = scan_text(f"§{sec} 와 {genuine_ip}", terms)
    ck("★앵커 뒤(같은 줄) 진짜 IP 양성 — 첫 매치에서 멈추지 않는다", names(same_line) == ["private-ipv4"])
    ck("★윗줄의 홀로 선 § 는 아랫줄 진짜 IP 를 억제하지 못한다(줄 시작 절단)",
       names(scan_text(f"§\n{ten_ip}", None)) == ["private-ipv4"])
    ident = scan_text("Alice <" + "alice" + "@example.com>", ["Alice"], skip_generic=frozenset({"email"}))
    ck("C2 신원 검사: email 제외해도 리터럴은 잡는다", any(str(h).startswith("term:") for h in ident))
    ck("C2 신원 검사: skip_generic email", not any(h.pattern == "email"
                                                  for h in scan_text(email, [], skip_generic=frozenset({"email"}))))
    ck("identity 프로필은 email 을 보지 않는다", scan_text(email, [], profile="identity") == [])
    ck("★음성대조 deploy 프로필은 email 을 잡는다", names(scan_text(email, [])) == ["email"])
    syn = f"{core.SYNTHETIC_NAME} <{core.SYNTHETIC_EMAIL}>"
    ck("합성 신원은 identity 프로필 무검출", scan_text(syn, terms, profile="identity") == [])
    ck("★합성 신원도 deploy 프로필 email 에는 걸린다(그래서 identity 프로필이 있다)",
       names(scan_text(syn, terms)) == ["email"])

    # ── 옛 hint_tag 자체검사 이관
    ck("★PII 절대경로 검출", any(h.pattern == "abs-op-path" for h in scan_text(f"경로 {nas} 참조", None)))
    ck("★PII 호스트명 검출(합성 값)", any(h.pattern == "spark-host" for h in scan_text(f"노드 {spark} 에서", None)))
    ck("깨끗한 본문은 무검출", scan_text("GB10 2노드 TP=2 · 53.92 t/s", None) == [])
    # ── 좁은 패턴의 음성 경계(버전 문자열 · 사설 대역 밖)
    ck("버전 문자열은 IPv4 가 아니다", scan_text("vllm 0.29.0rc6 · torch 2.11.0 · CUDA 13.2", None) == [])
    ck("172.15 는 사설 대역 밖", scan_text(_ip(172, 15, 0, 1), None) == [])
    ck("★10.x · 172.16-31 사설 대역 양성",
       names(scan_text(f"{ten_ip} {s172_ip}", None)) == ["private-ipv4", "private-ipv4"])

    # ── 전부 반환 · 줄 번호 · 결정론 순서
    multi = scan_text(f"a {nas}\nb {nas}2 {spark}\n", ["b "])
    ck("검출은 전부 반환(패턴마다 1건에서 멈추지 않는다)",
       [(h.pattern, h.line) for h in multi] == [("abs-op-path", 1), ("term", 2), ("abs-op-path", 2),
                                                 ("spark-host", 2)])
    ck("str(Hit) = pattern:match", str(Hit("term", "x", 1)) == "term:x")
    ck("render_hits 절단·경로 표기", render_hits([("a.md", Hit("abs-op-path", "/" + "mnt/" + "y" * 80, 3))])
       == "a.md:3: abs-op-path:" + ("/" + "mnt/" + "y" * 80)[:_SHOW])

    # ── 비배포 산문 프로필(docs.md 3종)
    ck("nondeploy_prose 는 abs-op-path 를 보지 않는다", scan_text(nas, None, profile="nondeploy_prose") == [])
    ck("★nondeploy_prose 도 사설 IP·호스트명은 잡는다",
       names(scan_text(f"{genuine_ip} {spark}", None, profile="nondeploy_prose")) == ["private-ipv4", "spark-host"])

    # ── 셸 기본값 예외(옛 hint_branch 음성대조 이관 · 2026-09-01)
    compose_line = "- ${NAS_MODEL_PATH:-" + "/" + "mnt/models}:/app/models:ro\n"
    log: list[str] = []
    ck("★shell 기본값은 면제된다(플래그 켬)", scan_text(compose_line, None, shell_default_exempt=True, exempt_log=log) == [])
    ck("면제는 조용히 넘기지 않는다(EXEMPT 기록)", len(log) == 1 and log[0].startswith("shell-default 1: abs-op-path:"))
    ck("★플래그 끔(기본)이면 셸 기본값도 검출", names(scan_text(compose_line, None)) == ["abs-op-path"])
    baked = "NAS_MODEL_PATH=" + "/" + "mnt/fixture-nas/Model/real\n"
    ck("★음성대조 baking 된 실경로는 여전히 잡힌다",
       names(scan_text(baked, None, shell_default_exempt=True, exempt_log=[])) == ["abs-op-path"])
    near = "설명: ${VAR} 뒤에 " + "/" + "mnt/fixture-nas/Model/real 이 있다\n"
    ck("★음성대조 치환구문 근처라도 기본값이 아니면 잡힌다",
       names(scan_text(near, None, shell_default_exempt=True, exempt_log=[])) == ["abs-op-path"])

    # ── 엄격 모드(옛 hint_collect 백스톱 의미 — 섹션앵커 예외 없음)
    ck("★엄격 모드(section_anchor_exempt=False)는 절번호도 잡는다",
       names(scan_text(f"§{sec}", None, section_anchor_exempt=False)) == ["private-ipv4"])

    # ── 입력 검증(소리내어 실패)
    ck("★모르는 프로필 차단", code_of(lambda: scan_text("x", None, profile="deploy_body")) == "HINT_PII_PROFILE_UNKNOWN")
    ck("★모르는 skip 이름 차단", code_of(lambda: scan_text("x", None, skip_generic={"mail"})) == "HINT_PII_PATTERN_UNKNOWN")
    ck("★바이트 입력 차단", code_of(lambda: scan_text(b"x", None)) == "HINT_PII_INPUT_NOT_TEXT")

    # ── 자기스캔: 이 추적 파일은 배포 강도 4종에 걸리지 않는다(2026-08-06 자기스캔 사고 방지) — 같은 원칙을 이 모듈과 함께
    #    배포되는 명명·카탈로그·어휘표에도 건다(카탈로그 머리말·어휘표 원문이 배포 파일로 나간다 · 2026-09-22 감사).
    here = Path(__file__).resolve()
    own_files = [here, here.parent / "naming.py", here.parent / "catalog.py",
                 here.parents[5] / core.REL_VOCAB]
    for f in own_files:
        try:
            hits = scan_text(f.read_text(encoding="utf-8"), None)
            ck(f"자기스캔 — {f.name} 에 4종 패턴 리터럴 0({render_hits(hits, 3)})", hits == [])
        except OSError:
            ck(f"자기스캔 — {f.name} 읽기", False)

    # ── import 부수효과 0: 새 프로세스(PATH 끊음)에서 적재 · 감사 훅으로 프로세스 실행·비 .py 파일 열기 관측
    probe = ("import json, sys\n"
             "ev = []\n"
             "W = ('subprocess.Popen', 'os.system', 'os.exec', 'os.posix_spawn', 'os.spawn', 'os.fork')\n"
             "def h(e, a):\n"
             "    if e in W: ev.append(e)\n"
             "    elif e == 'open':\n"
             "        p = a[0].decode('utf-8', 'replace') if isinstance(a[0], bytes) else a[0]\n"
             "        if isinstance(p, str) and not p.endswith(('.py', '.pyc')): ev.append(p)\n"
             "sys.addaudithook(h)\n"
             "sys.path.insert(0, sys.argv[1])\n"
             "import hintlib.pii\n"
             "print(json.dumps(ev))\n")
    try:
        import subprocess
        r = subprocess.run([sys.executable, "-B", "-c", probe, str(core.SCRIPTS_DIR)], capture_output=True,
                           text=True, env={"PATH": "/nonexistent"}, timeout=60)
        ck(f"★import 가 프로세스를 띄우거나 파일을 열지 않는다({r.stdout.strip()[-200:]}{r.stderr[-200:]})",
           r.returncode == 0 and r.stdout.strip() == "[]")
    except (OSError, subprocess.SubprocessError) as e:
        ck(f"import 탐침 실행({e!r})", False)

    with tempfile.TemporaryDirectory(prefix="hintpii-") as td:
        base = Path(td)
        # ── 용어 파일
        repo = base / "repo"
        (repo / ".claude").mkdir(parents=True)
        ck("용어 파일 부재 = None", load_pii_terms(repo) is None)
        ck("★용어 파일 부재 = 배포면 차단(K11)", code_of(lambda: require_terms(repo)) == "HINT_PII_TERMS_ABSENT")
        tf = repo / core.REL_PII_TERMS
        tf.write_text("# 주석\n\n  forbidden-secret-token  \nOpFixtureName\n", encoding="utf-8")
        ck("용어 파일 파싱(주석·빈 줄·공백)", load_pii_terms(repo) == ["forbidden-secret-token", "OpFixtureName"])
        ck("require_terms 는 목록", require_terms(repo) == ["forbidden-secret-token", "OpFixtureName"])
        broken = base / "broken"
        (broken / ".claude").mkdir(parents=True)
        (broken / core.REL_PII_TERMS).write_bytes(b"\xff\xfe\x00bad")
        ck("★깨진 용어 파일은 부재가 아니라 결함", code_of(lambda: load_pii_terms(broken)) == "HINT_PII_TERMS_UNREADABLE")
        empty = base / "empty"
        (empty / ".claude").mkdir(parents=True)
        (empty / core.REL_PII_TERMS).write_text("# 리터럴 없음\n", encoding="utf-8")
        ck("빈 용어 파일은 '있음'(선언)", require_terms(empty) == [])

        # ── 트리 스캔
        pay = base / "payload"
        (pay / "artifacts" / "compose").mkdir(parents=True)
        (pay / "00-hint.md").write_text(f"# 지도\n§{sec} 참조\n경로 {nas}\n", encoding="utf-8")
        (pay / "artifacts" / "compose" / "docker-compose.yaml").write_text(compose_line, encoding="utf-8")
        (pay / "artifacts" / "blob.bin").write_bytes(b"\xff\xfe\x00" + nas.encode())
        (pay / "clean.md").write_text("정상 문서\n", encoding="utf-8")
        tlog: list[str] = []
        th = scan_tree(pay, ["forbidden-secret-token"], exempt_log=tlog)
        ck("트리 스캔: 상대경로·줄·패턴", [(r, h.pattern, h.line) for r, h in th] == [("00-hint.md", "abs-op-path", 3)])
        ck("트리 스캔: 셸 기본값 면제(기본 켬) 기록", any(e.startswith("shell-default artifacts/compose/docker-compose.yaml:1:")
                                              for e in tlog))
        ck("트리 스캔: 바이너리 건너뜀 기록", any(e.startswith("binary artifacts/blob.bin") for e in tlog))
        ck("★트리 스캔 셸 기본값 면제를 끄면 compose 도 검출",
           ("artifacts/compose/docker-compose.yaml", "abs-op-path") in
           [(r, h.pattern) for r, h in scan_tree(pay, None, shell_default_exempt=False, exempt_log=[])])
        ck("트리 스캔: 결정론(두 번 같음)", scan_tree(pay, None, exempt_log=[]) == scan_tree(pay, None, exempt_log=[]))
        os.symlink("/" + "home/op-fixture/secret", pay / "link")
        ck("★심링크는 링크 문자열(git 이 싣는 바이트)을 스캔한다",
           ("link", "abs-op-path") in [(r, h.pattern) for r, h in scan_tree(pay, None, exempt_log=[])])
        (pay / "link").unlink()
        ck("rels 한정 스캔", [r for r, _ in scan_tree(pay, None, rels=["clean.md", "00-hint.md"], exempt_log=[])]
           == ["00-hint.md"])
        ck("rels 디렉터리는 펼친다", scan_tree(pay, None, rels=["artifacts"], exempt_log=[]) == [])
        ck("★rels 절대경로 거부", code_of(lambda: scan_tree(pay, None, rels=["/etc/x"])) == "HINT_PII_PATH_INVALID")
        ck("★rels 상위탈출 거부", code_of(lambda: scan_tree(pay, None, rels=["../x"])) == "HINT_PII_PATH_INVALID")
        ck("★rels 부재는 무검출이 아니라 차단", code_of(lambda: scan_tree(pay, None, rels=["nope.md"]))
           == "HINT_PII_PATH_ABSENT")
        ck("★루트 부재 차단", code_of(lambda: scan_tree(base / "none", None)) == "HINT_PII_ROOT_ABSENT")
        ck("nondeploy_prose 트리", scan_tree(pay, None, profile="nondeploy_prose", exempt_log=[]) == [])
        os.mkfifo(pay / "fifo")
        ck("★특수 파일(FIFO)은 열지 않고 거부", code_of(lambda: scan_tree(pay, None, exempt_log=[]))
           == "HINT_PII_UNSCANNABLE_FILE")
        (pay / "fifo").unlink()

        # ── 치환표 · 치환
        main_ip, sub_ip = _ip(10, 250, 0, 1), _ip(10, 250, 0, 2)
        lan_ip = _ip(192, 168, 7, 9)
        other_ip = _ip(172, 16, 9, 9)
        sub_host = "spark-" + "0e0e"
        home = "/" + "home/op-fixture"
        sub_work = home + "/easy_vllm_sub"
        nas_root = "/" + "mnt/fixture-nas/Models"
        tik = "/" + "mnt/fixture-nas/tiktoken/"
        repo_s = repo.resolve().as_posix()
        manifest = {
            "topology": "multi",
            "nas_model_path": nas_root,
            "tiktoken_host_path": tik,
            "nodes": [
                {"role": "main", "host": main_ip, "hostname": spark, "ssh_user": "op-fixture", "work_dir": repo_s},
                {"role": "sub", "host": sub_ip, "hostname": sub_host, "ssh_user": "op-fixture", "work_dir": sub_work},
            ],
            "propagation_authorization": {"ssh_host": sub_ip, "work_dir": sub_work, "misc_ip": other_ip},
            "interconnect": {"socket_iface": "enp1s0f0np0", "gid_index": 3, "type": "RoCE v2",
                             "hca_devices": ["rocep1s0f0", "roceP2p1s0f0"], "nccl_socket_ifname": "^lo,enp9s0"},
            "self_role": "main",
            "serving_ip": "0.0.0.0",
        }
        manifest["nodes"][1]["socket_iface"] = "enp7s0f1np1"      # 노드별 NIC 필드 = 그 노드 라벨
        table = substitution_table(repo, manifest)
        tmap = dict(table)
        ck("치환표 결정론(두 번 같음)", table == substitution_table(repo, manifest))
        ck("치환표 정렬 = 긴 리터럴 우선", [len(lit) for lit, _ in table] == sorted((len(lit) for lit, _ in table),
                                                                            reverse=True))
        ck("노드 IP·호스트명 → <node:role>", tmap.get(main_ip) == "<node:main>" and tmap.get(sub_ip) == "<node:sub>"
           and tmap.get(spark) == "<node:main>" and tmap.get(sub_host) == "<node:sub>")
        ck("repo 가 노드 work_dir 보다 우선", tmap.get(repo_s) == PH_REPO)
        ck("같은 리터럴은 사전순 앞 표지(nodes[sub] < propagation_authorization)",
           tmap.get(sub_work) == "<manifest.nodes[sub].work_dir>")
        ck("manifest 경로 필드(끝 / 제거)", tmap.get(nas_root) == "<manifest.nas_model_path>"
           and tmap.get(tik.rstrip("/")) == "<manifest.tiktoken_host_path>")
        ck("ssh_user 홈 → <home>", tmap.get(home) == PH_HOME)
        ck("nodes 밖 사설 IP → <priv-ip>", tmap.get(other_ip) == PH_PRIV_IP)
        ck("pii_terms → <redacted>", tmap.get("OpFixtureName") == PH_REDACTED)
        ck("★루프백·비경로 값·ssh_user 맨몸은 행이 되지 않는다", "0.0.0.0" not in tmap and "op-fixture" not in tmap)
        # 2026-09-22 S2 round 2(F8 · 적대 리뷰 정정): NIC·HCA 장치 이름 → 한 노드의 nodes[] 에만 있는 이름 = `<nic:<role>>` ·
        #   최상위 공통 설정(interconnect — 렌더러가 두 노드에 같이 보낸다) · 둘 이상의 노드가 가진 이름 = `<nic:cluster>`
        ck("★NIC 최상위(interconnect · 클러스터 공통) → <nic:cluster>(self_role=main 이어도 <nic:main> ✗)",
           tmap.get("enp1s0f0np0") == "<nic:cluster>" and tmap.get("rocep1s0f0") == "<nic:cluster>"
           and tmap.get("roceP2p1s0f0") == "<nic:cluster>" and "<nic:main>" not in tmap.values())
        ck("NIC NCCL 목록 문법(^lo,enp9s0) → 장치만 · `lo` 는 숫자가 없어 행이 아니다", tmap.get("enp9s0") == "<nic:cluster>"
           and "lo" not in tmap and "^lo" not in tmap)
        ck("NIC nodes[sub] 에만 있는 이름 → <nic:sub>", tmap.get("enp7s0f1np1") == "<nic:sub>")
        both = dict(manifest, nodes=[dict(n) for n in manifest["nodes"]])      # 노드 행만 사본(원 manifest 불변)
        both["nodes"][0]["socket_iface"] = "enp7s0f1np1"
        both["nodes"][0]["rdma_hca"] = "mlx5x9"
        tb = dict(substitution_table(repo, both))
        ck("★두 노드가 같은 이름을 가지면 <nic:cluster>(사전순으로 <nic:main> 이 이기지 않는다) · 한 노드만 = 그 노드",
           tb.get("enp7s0f1np1") == "<nic:cluster>" and tb.get("mlx5x9") == "<nic:main>")
        ck("★NIC 키의 낱말 값(`RoCE v2` · gid_index 숫자)은 행이 아니다", "RoCE v2" not in tmap and "RoCE" not in tmap
           and "3" not in tmap)
        nic_doc = "NET/IB : Using [0]rocep1s0f0:1/RoCE [1]roceP2p1s0f0:1/RoCE ; OOB enp1s0f0np0:x · enp1s0f0np01 · xenp1s0f0np0"
        nic_out = substitute(nic_doc, table)
        ck(f"NIC 치환: 로그 줄의 장치 이름 → 자리표시({nic_out})", nic_out.startswith(
            "NET/IB : Using [0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE ; OOB <nic:cluster>:x"))
        ck("★NIC 경계: 닮은 이름(뒤·앞에 식별자 문자가 붙은 것)은 깎지 않는다", nic_out.endswith("· enp1s0f0np01 · xenp1s0f0np0"))
        no_self = dict(manifest)
        no_self.pop("self_role")
        ck("self_role 유무와 무관(최상위 = 공통 설정) — 치환표가 같다", substitution_table(repo, no_self) == table)
        ck("★NIC 는 스캐너 체계 밖(치환 추가일 뿐 · P3) — 장치 이름 본문은 deploy 스캔 무검출",
           scan_text("OOB enp1s0f0np0 rocep1s0f0", None) == [])
        ck("_home_of 는 저장소 경로에서 파생", _home_of(home + "/ws/repo") == home and _home_of("/srv/x/y") is None)

        doc = "\n".join([
            f"메인 {main_ip} 서브 {sub_ip} 호스트 {spark}",
            f"모델 {nas_root}/qwen3 · tiktoken {tik}o200k",
            f"작업 {repo_s}/docs/testlog/x.md",
            f"옆 경로 {nas_root}2/other",
            f"LAN {lan_ip} · 닮은 주소 {main_ip}2",
            f"§{sec} 참조",
            "작성 OpFixtureName <op" + "@fixture.example>",
            f"홈 {home}/other · 서브 {sub_work}/logs",
            "- ${NAS_MODEL_PATH:-" + "/" + "mnt/models}",
        ]) + "\n"
        out = substitute(doc, table)
        ck("치환: 노드", f"메인 <node:main> 서브 <node:sub> 호스트 <node:main>" in out)
        ck("치환: manifest 경로 필드", "<manifest.nas_model_path>/qwen3" in out
           and "<manifest.tiktoken_host_path>/o200k" in out)
        ck("치환: repo 가 home 보다 긴 리터럴로 이긴다", "<repo>/docs/testlog/x.md" in out)
        ck("★경로 경계: 닮은 경로는 필드로 오귀속되지 않는다(잔여 가림)", "옆 경로 <abs-path>" in out)
        ck("★IP 경계: 닮은 주소는 노드로 오귀속되지 않는다(잔여 가림)", "LAN <priv-ip> · 닮은 주소 <priv-ip>" in out)
        ck("★절번호는 치환하지 않는다(스캐너와 같은 예외)", f"§{sec} 참조" in out)
        ck("치환: 사람 식별자·email 제거", "작성 <redacted> <<redacted>>" in out)
        ck("치환: home·서브 work_dir", "홈 <home>/other · 서브 <manifest.nodes[sub].work_dir>/logs" in out)
        ck("치환: 셸 기본값 잔여도 가린다", "${NAS_MODEL_PATH:-<abs-path>}" in out)
        ck("★치환 결과는 deploy 스캔(용어 포함) 무검출", scan_text(out, require_terms(repo)) == [])
        ck("★치환 결과는 엄격 스캔에서도 절번호만 남는다",
           [str(h) for h in scan_text(out, None, section_anchor_exempt=False)] == [f"private-ipv4:{sec}"])
        ck("치환 멱등", substitute(out, table) == out)
        ck("치환 결정론", substitute(doc, table) == out)
        ck("★residual 끄면 표 밖 잔여가 남는다(잔여 단계가 실제로 일한다)",
           lan_ip in substitute(doc, table, residual=False))
        ck("manifest 없는 치환표 = repo·home·terms", {ph for _, ph in substitution_table(repo, None)}
           <= {PH_REPO, PH_HOME, PH_REDACTED})
        ck("★치환표 행 모양 검사", code_of(lambda: substitute("x", [("", "<a>")])) == "HINT_PII_SUBST_TABLE_SHAPE"
           and code_of(lambda: substitute("x", [{"a": 1}])) == "HINT_PII_SUBST_TABLE_SHAPE")
        ck("★같은 리터럴에 표지 둘 차단", code_of(lambda: substitute("x", [("a", "<p>"), ("a", "<q>")]))
           == "HINT_PII_SUBST_TABLE_SHAPE")
        ck("★리터럴 목록 자리의 문자열 하나 차단", code_of(lambda: scan_text("x", "abc")) == "HINT_PII_TERMS_SHAPE")
        ck("★manifest 모양 검사", code_of(lambda: substitution_table(repo, {"nodes": "x"})) == "HINT_PII_MANIFEST_SHAPE")

        # 같은 role 둘 → 서수 라벨
        dup = {"nodes": [{"role": "main", "host": main_ip}, {"role": "sub", "host": sub_ip},
                         {"role": "sub", "host": _ip(10, 250, 0, 3)}]}
        dmap = dict(substitution_table(repo, dup))
        ck("같은 role 둘 → sub1·sub2", dmap.get(sub_ip) == "<node:sub1>" and dmap.get(_ip(10, 250, 0, 3)) == "<node:sub2>")

        # ★용어가 표지 안에 들어가면 차단(멱등 붕괴 · 치환된 본문이 다시 걸림)
        coll = base / "coll"
        (coll / ".claude").mkdir(parents=True)
        (coll / core.REL_PII_TERMS).write_text("node\n", encoding="utf-8")
        ck("★용어-표지 충돌 차단", code_of(lambda: substitution_table(coll, manifest)) == "HINT_PII_SUBST_COLLISION")
        try:
            substitution_table(coll, manifest)
        except core.HintError as e:
            ck("충돌 메시지는 출처 범주·길이로 말한다(리터럴 인용 ✗)", "pii_terms 리터럴(길이 4)" in e.message)
    return bad
