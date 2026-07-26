#!/usr/bin/env python3
"""Scan a tree for sensitive strings (PII leak guard).

Terms come from an UNTRACKED shared term file (default: <repo>/.claude/pii_terms.txt,
one literal per line, '#' comments) so no PII literal ever bakes into this tracked,
distributable script (pointer principle — plan_26070208 Phase 1). The file is
shared with scripts/smoke_clone.sh (A4): single source, no cross-copy drift.
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
TEXT_EXT = {".md", ".py", ".yaml", ".yml", ".json", ".txt", "", ".example"}


def _default_terms_file() -> Path:
    # scripts/ → wiki-desk → skills → .claude
    return Path(__file__).resolve().parents[3] / "pii_terms.txt"


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
    for path in root.rglob("*"):
        if not path.is_file() or set(path.parts) & SKIP_DIRS or path.suffix not in TEXT_EXT:
            continue
        if path.resolve() == terms_file.resolve():
            continue  # the untracked term source itself is not a leak
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for term in terms:
            if term and term in text:
                hits.append((path, f"term:{term}"))
        for pat in GENERIC_PATTERNS:
            m = pat.search(text)
            if m:
                hits.append((path, f"generic:{m.group(0)}"))
    if hits:
        for path, what in hits:
            print(f"HIT {path}: {what}")
        return 1
    print("PASS forbidden string scan: 0 hits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
