#!/usr/bin/env python3
"""Deterministic linter for a path-reference __llm-wiki (NO LLM).

Scope is HARD-BOUND to --wiki-root: the linter may READ (stat/hash) project
files referenced by the library, but it NEVER writes, moves, or deletes
anything outside --wiki-root. --fix regenerates only derived caches inside it.

Rules (per the wiki-desk grounding):
  R1 dead-path-ref   every active source_path must resolve under project-root      [critical]
  R2 stale-sha256    recomputed hash must match the recorded hash                  [warning]
  R3 authority/conf  authority_rank matches authority_order; confidence in enum    [warning]
  R4 orphan          source with zero edges; concept node referencing zero paths   [suggestion]
  R5 edge-integrity  every edge endpoint resolves to a known source_id             [critical]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

CONF_ENUM = {"high", "medium", "low"}


def sha256_bytes(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def _sha_file(p: Path) -> str:
    return sha256_bytes(p.read_bytes())


def recompute_sha(project_root: Path, e: dict[str, Any]) -> str | None:
    p = project_root / e["source_path"]
    if not p.exists():
        return None
    if e["document_type"] == "simlog" and p.is_dir():
        # match init_wiki_desk: whole-dir digest (sorted relpath + per-file sha256)
        files = sorted(c for c in p.rglob("*") if c.is_file())
        digest = "\n".join(c.relative_to(p).as_posix() + ":" + _sha_file(c) for c in files)
        return sha256_bytes(digest.encode())
    if p.is_file():
        return sha256_bytes(p.read_text(encoding="utf-8", errors="replace").encode("utf-8", "replace"))
    return None


def regenerate_concepts_index(wiki_root: Path) -> bool:
    cdir = wiki_root / "concepts"
    if not cdir.exists():
        return False
    files = sorted(p.name for p in cdir.glob("*.md") if p.name != "_index.md")
    body = ["# Concepts Index", "",
            "> Synthesis/relationship nodes. Derived cache — rebuilt by lint --fix.", "",
            "| node | summary |", "|---|---|"]
    for n in files:
        body.append(f"| [{n}]({n}) | (see node) |")
    target = cdir / "_index.md"
    new = "\n".join(body) + "\n"
    if not target.exists() or target.read_text(encoding="utf-8") != new:
        target.write_text(new, encoding="utf-8")
        return True
    return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--wiki-root", required=True)
    ap.add_argument("--project-root", required=True)
    ap.add_argument("--fix", action="store_true")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    wiki_root = Path(args.wiki_root).resolve()
    project_root = Path(args.project_root).resolve()

    reg_path = wiki_root / "sources/registry.json"
    if not reg_path.exists():
        raise SystemExit(f"missing registry: {reg_path}")
    data = json.loads(reg_path.read_text(encoding="utf-8"))
    entries = data.get("entries", [])
    active = [e for e in entries if e.get("lifecycle_state") == "active"]
    edges = data.get("edges", [])
    authority = data.get("authority_order", {})

    findings: list[dict[str, str]] = []

    def add(sev: str, rule: str, msg: str):
        findings.append({"severity": sev, "rule": rule, "message": msg})

    ids = {e["source_id"] for e in entries}
    deg: dict[str, int] = {}
    for e in edges:
        deg[e["from"]] = deg.get(e["from"], 0) + 1
        deg[e["to"]] = deg.get(e["to"], 0) + 1
        # R5 edge-integrity
        for end in ("from", "to"):
            if e[end] not in ids:
                add("critical", "edge-integrity", f"edge {end} references unknown source_id: {e[end]}")

    for e in active:
        # R1 dead-path-ref
        if not (project_root / e["source_path"]).exists():
            add("critical", "dead-path-ref", f"source path does not resolve: {e['source_path']}")
            continue
        # R2 stale-sha256
        cur = recompute_sha(project_root, e)
        if cur is not None and cur != e.get("sha256"):
            add("warning", "stale-sha256", f"hash drift (run init --incremental): {e['source_path']}")
        # R3 authority/confidence
        want = authority.get(e["document_type"])
        if want is not None and int(e["authority_rank"]) != int(want):
            add("warning", "authority-range",
                f"authority_rank {e['authority_rank']} != authority_order[{e['document_type']}]={want}: {e['source_path']}")
        if e.get("confidence") not in CONF_ENUM:
            add("warning", "authority-range", f"confidence not in {CONF_ENUM}: {e['source_path']}")
        # R4 orphan (suggestion only)
        if deg.get(e["source_id"], 0) == 0:
            add("suggestion", "orphan", f"source has zero edges: {e['source_path']}")

    # R4 orphan concept nodes
    cdir = wiki_root / "concepts"
    if cdir.exists():
        for p in cdir.glob("*.md"):
            if p.name in ("_index.md", "document-universe-hierarchy-map.md"):
                continue
            body = p.read_text(encoding="utf-8", errors="replace")
            if not re.search(r"\]\([^)]+\)", body) and "`" not in body:
                add("suggestion", "orphan", f"concept node references zero paths: concepts/{p.name}")

    fixed = []
    if args.fix:
        if regenerate_concepts_index(wiki_root):
            fixed.append("regenerated concepts/_index.md")

    counts: dict[str, int] = {}
    for f in findings:
        counts[f["severity"]] = counts.get(f["severity"], 0) + 1

    if args.json:
        print(json.dumps({"findings": findings, "counts": counts, "fixed": fixed}, ensure_ascii=False, indent=2))
    else:
        for f in findings:
            print(f"[{f['severity'].upper()}] {f['rule']}: {f['message']}")
        for x in fixed:
            print(f"[FIXED] {x}")
        crit = counts.get("critical", 0)
        print(f"\nlint: {crit} critical, {counts.get('warning',0)} warning, "
              f"{counts.get('suggestion',0)} suggestion over {len(active)} active sources.")
        print("PASS" if crit == 0 else "FAIL")
    return 1 if counts.get("critical", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
