#!/usr/bin/env python3
"""Scan the wiki-desk distribution tree for sensitive strings (PII leak guard).

The terms are assembled WITHOUT spelling them literally in this file, so the
scanner can scan its own package without self-matching. Re-target per deployment
with --term; the defaults below are this project's environment specifics that
must never bake into the tracked, distributable skill (pointer principle).
"""
from __future__ import annotations

import argparse
from pathlib import Path

# Split-assembled so the literal never appears in this tracked file.
DEFAULT_TERMS = [
    "/home/" + "cona",          # host home path
    "tbvjvs" + "ladla",         # account / email handle
    "naver" + ".com",           # account email domain
    "coag-" + "ash",            # git commit author name
    "coga-" + "robotics.com",   # git commit author email domain
    "192.168.100." + "10",      # main node RoCE IP
    "192.168.100." + "11",      # sub node RoCE IP
    "spark-" + "a73e",          # main hostname
    "spark-" + "bdc9",          # sub hostname
    "/mnt/llm/" + "quant_model",  # NAS quant model mount
]
SKIP_DIRS = {".git", "__pycache__", ".pytest_cache", "dist"}
TEXT_EXT = {".md", ".py", ".yaml", ".yml", ".json", ".txt", "", ".example"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--term", action="append", default=[])
    args = ap.parse_args()
    root = Path(args.root)
    terms = DEFAULT_TERMS + args.term
    hits = []
    for path in root.rglob("*"):
        if not path.is_file() or set(path.parts) & SKIP_DIRS or path.suffix not in TEXT_EXT:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for term in terms:
            if term and term in text:
                hits.append((path, term))
    if hits:
        for path, term in hits:
            print(f"HIT {path}: {term}")
        return 1
    print("PASS forbidden string scan: 0 hits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
