#!/usr/bin/env python3
"""Scan a tree for sensitive strings (PII leak guard).

Terms come from an UNTRACKED shared term file (default: <repo>/.claude/pii_terms.txt,
one literal per line, '#' comments) so no PII literal ever bakes into this tracked,
distributable script (pointer principle — plan_26070208 Phase 1). The file is
shared with .claude/skills/upstream-version-watch/scripts/smoke_clone.sh (A4):
single source, no cross-copy drift.  (2026-09-10: 경로 갱신 — 옛 루트 `scripts/` 표기는
서브에서 은퇴한 이름이라 retirement 감사가 이 줄을 소비자로 셌다.)
When the term file is absent (fresh deployment skeleton), the generic patterns
below still guard private-IP leaks; deployments inject their own literals by
creating the term file (or via --term).
"""
from __future__ import annotations

import argparse
import re
from pathlib import Path

# Generic guards (patterns, not PII) — active even without a term file.
GENERIC_PATTERNS = [
    re.compile(r"192\.168\."),  # private IPv4 prefix (RFC1918) — same scope as smoke_clone.sh fallback
]
SKIP_DIRS = {".git", "__pycache__", ".pytest_cache", "dist"}
# 확장자 allowlist 를 쓰지 않는다 (2026-09-01 교정). 이전 목록은 `.sh` 를 담지 않아
# 추적 배포 빌딩블럭 190개 중 38개(.sh 28 · .template/.patch/.golden/.service 등 10)가
# **스캔 사각지대**였고, 그 결과 `.claude/**` 셸 스크립트 주석의 운영자 절대경로 3건이
# 게이트를 통과한 채 원격에 배포됐다(실측 · docs.md §PII 적용범위는 `.claude/**` 를
# 배포 산출물 = 4종 전부 강도로 규정한다). 바이너리는 아래 read_text 의
# UnicodeDecodeError 가 이미 거르므로 확장자 판정은 중복이면서 유해했다.
MAX_BYTES = 4 * 1024 * 1024  # 이보다 큰 파일은 스캔하지 않고 **소리내어** 보고한다(침묵 생략 금지)

# 행 단위 면제 마커. 탐지기 자신의 양성 픽스처(RFC1918 리터럴 등)가 자기 스캔에 걸리는 문제를
# **픽스처 삭제로 풀지 않는다** — 지우면 "탐지기가 실제로 발화하는가"를 증명하던 시험이 함께 죽는다
# (역-오라클). 대신 그 한 줄만 면제하고, 면제분은 EXEMPT 로 **출력한다**. 파일 단위가 아니라
# 행 단위인 이유는 파일 하나를 통째로 여는 마커가 곧 우회 수단이 되기 때문이다.
EXEMPT_MARK = "pii-scan-fixture"


def _default_terms_file() -> Path:
    # scripts/ → wiki-desk → skills → .claude
    return Path(__file__).resolve().parents[3] / "pii_terms.txt"


def scan_lines(text: str, terms: list[str]) -> list[tuple[int, str, bool]]:
    """(lineno, what, exempt) for every hit — the single scanning kernel.

    호출자가 둘이다: 이 파일의 `main()`(사람이 트리를 훑을 때)과 `runtime_selftest.py` 의
    tripwire ⑤(커밋마다 추적 배포면을 훑을 때). 두 번째 호출자가 없던 동안 이 스캐너는
    **실행자가 아예 없는 가드**였고, 그래서 운영자 호스트명 2건이 추적 파일로 배포됐다
    (2026-09-04 실측 · 헌법 §노드 제어 5불변식 3 — "처방을 누가 실행하는가를 먼저 적는다").
    커널을 함수로 노출해 두 호출자가 **같은 패턴·같은 면제 규칙**을 쓰게 한다.
    """
    out: list[tuple[int, str, bool]] = []
    for lineno, line in enumerate(text.splitlines(), 1):
        exempt = EXEMPT_MARK in line
        found = [f"term:{t}" for t in terms if t and t in line]
        found += [f"generic:{m.group(0)}" for pat in GENERIC_PATTERNS
                  if (m := pat.search(line))]
        out.extend((lineno, what, exempt) for what in found)
    return out


def load_terms(path: Path) -> list[str]:
    if not path.is_file():
        return []
    terms = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            terms.append(line)
    return terms


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--term", action="append", default=[])
    ap.add_argument(
        "--terms-file",
        default=None,
        help="PII term file (default: <repo>/.claude/pii_terms.txt; absent → generic patterns only)",
    )
    args = ap.parse_args()
    root = Path(args.root)
    terms_file = Path(args.terms_file) if args.terms_file else _default_terms_file()
    terms = load_terms(terms_file) + args.term
    hits = []
    skipped_large: list[Path] = []
    unreadable: list[tuple[Path, OSError]] = []
    exempted: list[tuple[Path, int, str]] = []
    for path in root.rglob("*"):
        if not path.is_file() or set(path.parts) & SKIP_DIRS:
            continue
        if path.resolve() == terms_file.resolve():
            continue  # the untracked term source itself is not a leak
        try:
            if path.stat().st_size > MAX_BYTES:
                skipped_large.append(path)
                continue
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue  # 바이너리 — 확장자가 아니라 내용으로 판정한다
        except OSError as exc:
            unreadable.append((path, exc))
            continue
        for lineno, what, exempt in scan_lines(text, terms):
            (exempted if exempt else hits).append((path, lineno, what))
    # 스캔하지 못한 것을 먼저 고지한다 — 조용히 건너뛴 파일은 "깨끗한 것"과 구분되지 않는다.
    for path in skipped_large:
        print(f"SKIP(too-large>{MAX_BYTES}B) {path}")
    for path, exc in unreadable:
        print(f"SKIP(unreadable) {path}: {exc}")
    for path, lineno, what in exempted:
        print(f"EXEMPT({EXEMPT_MARK}) {path}:{lineno}: {what}")
    if hits:
        for path, lineno, what in hits:
            print(f"HIT {path}:{lineno}: {what}")
        return 1
    print(f"PASS forbidden string scan: 0 hits "
          f"(면제 {len(exempted)} · 미스캔 large={len(skipped_large)} unreadable={len(unreadable)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
