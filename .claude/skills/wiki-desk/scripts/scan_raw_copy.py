#!/usr/bin/env python3
"""Detect raw-copy anti-patterns in a generated __llm-wiki.

Path-reference principle: the library stores metadata + path references, never
raw source bodies. This guard fails on a raw/ directory or oversized wiki files
that look like copied source content.
"""
from __future__ import annotations

import argparse
from pathlib import Path


# Generated metadata files legitimately grow with the source count — exempt them
# from the oversize rule (which targets unexpected copied source bodies).
EXEMPT = {"registry.json", "source-registry.md", "edge-graph.md"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("wiki_root")
    ap.add_argument("--max-bytes", type=int, default=200_000)
    args = ap.parse_args()
    root = Path(args.wiki_root)
    failures = []
    raw = root / "raw"
    if raw.exists():
        failures.append(f"raw directory exists: {raw}")
    # Check EVERY file (any extension) — a raw copy could use .log/.csv/.html/none.
    for path in root.rglob("*"):
        if not path.is_file() or path.name in EXEMPT:
            continue
        if path.stat().st_size > args.max_bytes:
            failures.append(f"oversized wiki file suggests raw copy: {path} ({path.stat().st_size} bytes)")
    if failures:
        for f in failures:
            print("FAIL", f)
        return 1
    print("PASS raw-copy scan: no raw directory or oversized copied source files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
