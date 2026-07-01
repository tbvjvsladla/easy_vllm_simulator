#!/usr/bin/env python3
"""Adaptive init + warm-start shelving for a path-reference __llm-wiki.

Path-reference principle: this NEVER copies a source body. It records, per
source, a path reference + sha256 + role + authority_rank + concise summary +
DETERMINISTIC relationship edges extracted from the docs naming/cross-reference
convention (NO probabilistic inference, NO deep body interpretation).

Modes:
  (default)        full bootstrap scan — first install, or a forced re-index.
  --incremental    warm-start: re-summarize/re-extract only changed/new sources,
                   carry unchanged entries verbatim, mark removed as archived.

Output (all under --wiki-root, untracked live library):
  config.yaml, SCHEMA.md, index.md, log.md,
  sources/source-registry.md (human),  sources/registry.json (machine),
  relationships/edge-graph.md,  concepts/_index.md,
  concepts/document-universe-hierarchy-map.md,  warm-start/<thread>.md
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
from pathlib import Path
from typing import Any

try:
    import yaml
except Exception:  # pragma: no cover
    yaml = None

# Path-substring → (document_type, role). First match wins; order matters.
TYPE_HINTS = [
    ("docs/devlog", "devlog", "work-log"),
    ("docs/testlog", "testlog", "verification-evidence"),
    ("docs/plan", "plan", "plan-context"),
    ("docs/simlog", "simlog", "raw-evidence"),
    ("sync_staging/sub_docs", "sub-doc", "sub-insight"),
    ("sub_docs", "sub-doc", "sub-insight"),
    ("seed", "seed", "bootstrap-lineage"),
]

# A doc stem = "<type>_<YYYYMMDDHH>_<seq>" (plan/devlog/testlog) or, for simlog,
# the run-dir name "<YYYYMMDDHH>_<seq>" (NO type prefix).
STEM_RE = re.compile(r"(plan|devlog|testlog)_(\d{10})_(\d+)")
SIMLOG_RE = re.compile(r"(\d{10})_(\d+)")
# plan/devlog/testlog citation tokens: bare stem, backticked, or `docs/<type>/`-prefixed
# (topic suffix, if present, is ignored). Brace-glob is supported for simlog only.
CITE_TOKEN_RE = re.compile(
    r"(?:docs/(?:plan|devlog|testlog)/)?(plan|devlog|testlog)_(\d{10})_(\d+)"
)
# simlog citations: `docs/simlog/<YYYYMMDDHH>_<seq>` with optional brace-glob {1,2,3}.
SIMLOG_CITE_RE = re.compile(r"docs/simlog/(\d{10})_(\{[\d,]+\}|\d+)")
SEEDISH_RE = re.compile(r"(seed_[0-9a-f]{6,}|interview_\d{8}_\d{6})")
# A plan is `realizes` (vs generic `cites`) only when named on a 계획/대상 line —
# the docs convention's "this is the plan this work realizes" header label.
REALIZE_LABEL_RE = re.compile(r"(계획|대상)")
# Retroactive supersede banner (docs.md §3 소급 배너 — literal, deterministic).
# "⚠ SUPERSEDED-IN-PART by `<doc>`" (partial) / "⛔ SUPERSEDED by `<doc>`" (full).
# HEADER-scoped: banners live in the doc header; body text merely *quoting* the
# convention (e.g. a devlog narrating that it added a banner) must not create edges.
SUPERSEDE_RE = re.compile(r"SUPERSEDED(-IN-PART)?\s+by\b")
SUPERSEDE_HEADER_LINES = 12


def read_answers(path: Path) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8")
    if yaml:
        return yaml.safe_load(text)
    raise SystemExit("PyYAML required for init answers parsing")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


TITLE_CAP = 160


def _cap(s: str) -> str:
    s = s.strip()
    return (s[:TITLE_CAP].rstrip() + "…") if len(s) > TITLE_CAP else s


def title_hint(path: Path, text: str | None = None) -> str:
    raw = path.stem.replace("_", " ").replace("-", " ").title()
    try:
        body = text if text is not None else path.read_text(encoding="utf-8", errors="replace")
        for line in body.splitlines():
            if line.startswith("# "):
                raw = line[2:].strip()
                break
            if line.lower().startswith("title:"):
                raw = line.split(":", 1)[1].strip().strip('"')
                break
    except Exception:
        pass
    return _cap(raw)  # bounded single heading line — never paragraph/body prose


def classify(rel: str, authority: dict[str, int]) -> tuple[str, str, int]:
    for marker, doc_type, role in TYPE_HINTS:
        if marker in rel:
            return doc_type, role, int(authority.get(doc_type, 0))
    return "reference", "workspace-reference", int(authority.get("reference", 0))


def stem_of(source_path: str, doc_type: str) -> str | None:
    """Rename-stable key used to resolve citations to a source."""
    base = source_path.rsplit("/", 1)[-1]
    if doc_type == "simlog":
        m = SIMLOG_RE.match(base)
        return f"{m.group(1)}_{m.group(2)}" if m else None
    m = STEM_RE.match(base)
    return f"{m.group(1)}_{m.group(2)}_{m.group(3)}" if m else None


def topic_slug(source_path: str, doc_type: str) -> str | None:
    base = source_path.rsplit("/", 1)[-1]
    base = base[:-3] if base.endswith(".md") else base
    if doc_type == "simlog":
        m = re.match(r"\d{10}_\d+_(.+)", base)
    else:
        m = re.match(r"(?:plan|devlog|testlog)_\d{10}_\d+_(.+)", base)
    return m.group(1) if m else None


def summarize(doc_type: str, title: str) -> str:
    return f"{doc_type}: {title}"


def confidence_of(entry: dict[str, Any], edge_count: int) -> str:
    if edge_count >= 2:
        return "high"
    if edge_count >= 1 or entry.get("title_hint"):
        return "medium"
    return "low"


def iter_sources(project_root: Path, root_name: str, reg: dict[str, Any]):
    """Yield (path, is_dir_node) for one configured root. Graceful skip if absent."""
    base = project_root / root_name
    if not base.exists():
        return
    include_ext = set(reg.get("include_extensions", [".md", ".yaml", ".yml", ".json", ".txt"]))
    exclude_dirs = set(reg.get("exclude_dirs", []))
    exclude_basenames = set(reg.get("exclude_basenames", []))
    dir_node_roots = set(reg.get("dir_node_roots", []))
    if root_name in dir_node_roots:
        # Directory-node mode: each immediate child directory = ONE node.
        for child in sorted(base.iterdir()):
            if child.is_dir() and child.name not in exclude_dirs:
                yield child, True
        return
    for path in sorted(base.rglob("*")):
        if not path.is_file():
            continue
        rel_parts = set(path.relative_to(project_root).parts)
        if rel_parts & exclude_dirs:
            continue
        if path.name in exclude_basenames:
            continue
        if path.suffix and path.suffix not in include_ext:
            continue
        yield path, False


def scan_sources(project_root: Path, roots: list[str], authority: dict[str, int],
                 reg: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, str]]:
    """Return (entries, bodies). bodies maps source_id -> text for edge extraction."""
    entries: list[dict[str, Any]] = []
    bodies: dict[str, str] = {}
    for root_name in roots:
        for path, is_dir_node in iter_sources(project_root, root_name, reg):
            rel = path.relative_to(project_root).as_posix()
            doc_type, role, rank = classify(rel, authority)
            source_id = re.sub(r"[/_.]", "-", rel)
            if is_dir_node:
                # Hash the whole run directory deterministically (sorted filename +
                # per-file sha256) so ANY content change is detected, independent of
                # whether run_summary.json is present. Do not interpret raw run logs.
                files = sorted(c for c in path.rglob("*") if c.is_file())
                digest = "\n".join(c.relative_to(path).as_posix() + ":" + sha256_file(c) for c in files)
                sha = sha256_bytes(digest.encode())
                nbytes = sum(c.stat().st_size for c in files)
                nlines, body = len(files), ""
                title = topic_slug(rel, doc_type) or path.name
            else:
                text = path.read_text(encoding="utf-8", errors="replace")
                sha = sha256_bytes(text.encode("utf-8", "replace"))
                nbytes, nlines = path.stat().st_size, text.count("\n") + 1
                body = text
                title = title_hint(path, text)
            entry = {
                "source_id": source_id, "workspace": project_root.name, "source_path": rel,
                "document_type": doc_type, "role": role, "authority_rank": rank,
                "copy_policy": "path_reference", "sha256": sha, "bytes": nbytes, "lines": nlines,
                "title_hint": title, "topical_summary": summarize(doc_type, title),
                "stem": stem_of(rel, doc_type), "topic_slug": topic_slug(rel, doc_type),
                "lifecycle_state": "active", "out_edges": [],
            }
            entries.append(entry)
            bodies[source_id] = body
    return entries, bodies


def _expand_brace(stem_num: str) -> list[str]:
    """'{1,2,3}' -> ['1','2','3']; '1' -> ['1']."""
    if stem_num.startswith("{"):
        return [s for s in stem_num.strip("{}").split(",") if s]
    return [stem_num]


def extract_edges(entries: list[dict[str, Any]], bodies: dict[str, str]) -> list[dict[str, Any]]:
    """Deterministic, citation-grounded edges. Every edge carries its evidence token."""
    by_stem: dict[str, str] = {}
    by_id: dict[str, dict[str, Any]] = {}
    for e in entries:
        by_id[e["source_id"]] = e
        if e.get("stem"):
            by_stem[e["stem"]] = e["source_id"]
    edges: list[dict[str, Any]] = []
    seen: set[tuple[str, str, str]] = set()

    def add(frm: str, to: str, etype: str, evidence: str):
        if frm == to:
            return
        key = (frm, to, etype)
        if key in seen:
            return
        seen.add(key)
        edges.append({"from": frm, "to": to, "edge_type": etype, "evidence": evidence})

    def realize_targets(body: str) -> set[str]:
        # plan stems named on a 계획/대상 line = the plan(s) this work realizes
        # (vs a plan merely mentioned/blamed elsewhere in the body → cites).
        out: set[str] = set()
        for line in body.splitlines():
            if REALIZE_LABEL_RE.search(line):
                for m in CITE_TOKEN_RE.finditer(line):
                    if m.group(1) == "plan":
                        out.add(f"plan_{m.group(2)}_{m.group(3)}")
        return out

    for e in entries:
        sid, s_type = e["source_id"], e["document_type"]
        body = bodies.get(sid, "")
        if not body:
            continue
        rset = realize_targets(body) if s_type in ("devlog", "testlog") else set()
        # plan/devlog/testlog citations
        for m in CITE_TOKEN_RE.finditer(body):
            target_stem = f"{m.group(1)}_{m.group(2)}_{m.group(3)}"
            tid = by_stem.get(target_stem)
            if not tid:
                continue
            t_type = by_id[tid]["document_type"]
            tok = m.group(0)
            if t_type == "plan" and s_type in ("devlog", "testlog"):
                add(sid, tid, "realizes" if target_stem in rset else "cites", tok)
            elif t_type == "testlog" and s_type == "devlog":
                add(tid, sid, "evidences", tok)
            else:
                add(sid, tid, "cites", tok)
        # simlog directory citations (brace-glob expands to runs)
        for m in SIMLOG_CITE_RE.finditer(body):
            for num in _expand_brace(m.group(2)):
                tid = by_stem.get(f"{m.group(1)}_{num}")
                if tid and by_id[tid]["document_type"] == "simlog":
                    add(tid, sid, "evidences", m.group(0))
        # seed_/interview_ lineage tokens (only if such a node is registered)
        for m in SEEDISH_RE.finditer(body):
            tid = by_stem.get(m.group(1))
            if tid:
                add(sid, tid, "cites", m.group(0))
        # superseded-by: retroactive banner in the doc HEADER (docs.md §3 — 앵커링 방지).
        # Direction: this doc (overturned) → the superseding doc. Still v1-deterministic:
        # the banner literal is the grep-grounded evidence, no inference.
        for line in body.splitlines()[:SUPERSEDE_HEADER_LINES]:
            sm = SUPERSEDE_RE.search(line)
            if not sm:
                continue
            for m in CITE_TOKEN_RE.finditer(line[sm.end():]):
                tid = by_stem.get(f"{m.group(1)}_{m.group(2)}_{m.group(3)}")
                if tid:
                    add(sid, tid, "superseded-by", _cap(line.strip()))

    # part-of: a source nested under another source's dir-node path
    dir_nodes = [e for e in entries if e["document_type"] == "simlog"]
    for e in entries:
        for d in dir_nodes:
            if e["source_id"] != d["source_id"] and e["source_path"].startswith(d["source_path"] + "/"):
                add(e["source_id"], d["source_id"], "part-of", d["source_path"])
    # same-thread: identical topic_slug (best-effort, secondary)
    by_slug: dict[str, list[str]] = {}
    for e in entries:
        if e.get("topic_slug"):
            by_slug.setdefault(e["topic_slug"], []).append(e["source_id"])
    for ids in by_slug.values():
        for i, a in enumerate(ids):
            for b in ids[i + 1:]:
                add(a, b, "same-thread", "topic-slug")
                add(b, a, "same-thread", "topic-slug")
    # drop a `cites` edge shadowed by a stronger evidences/realizes/superseded-by on the same pair
    strong = {(e["from"], e["to"]) for e in edges if e["edge_type"] in ("evidences", "realizes", "superseded-by")}
    edges = [e for e in edges if not (e["edge_type"] == "cites" and (e["from"], e["to"]) in strong)]
    return edges


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def write_wiki(wiki_root: Path, answers: dict[str, Any], entries: list[dict[str, Any]],
               edges: list[dict[str, Any]], delta: dict[str, int] | None,
               schema_template: Path | None) -> None:
    wiki_root.mkdir(parents=True, exist_ok=True)
    for d in ["sources", "concepts", "relationships", "warm-start"]:
        (wiki_root / d).mkdir(parents=True, exist_ok=True)
    now = now_iso()
    project = answers.get("project", {})
    authority = answers.get("authority_order", {})
    retrieval = answers.get("retrieval", {})

    # recompute per-source out_edges + degree from the final (filtered) edge set
    out_map: dict[str, list[dict[str, Any]]] = {}
    deg: dict[str, int] = {}
    for e in edges:
        out_map.setdefault(e["from"], []).append(
            {"to": e["to"], "edge_type": e["edge_type"], "evidence": e["evidence"]})
        deg[e["from"]] = deg.get(e["from"], 0) + 1
        deg[e["to"]] = deg.get(e["to"], 0) + 1
    for e in entries:
        e["out_edges"] = out_map.get(e["source_id"], [])
        e["confidence"] = confidence_of(e, deg.get(e["source_id"], 0))
        e.setdefault("indexed_at", now)

    entries_sorted = sorted(entries, key=lambda e: (-e["authority_rank"], e["source_path"]))

    # config.yaml (= answers + wiki defaults)
    config = dict(answers)
    config.setdefault("wiki", {"mode": "path-reference", "raw_copy": False})
    config_text = (yaml.safe_dump(config, sort_keys=False, allow_unicode=True)
                   if yaml else json.dumps(config, indent=2, ensure_ascii=False))
    (wiki_root / "config.yaml").write_text(config_text, encoding="utf-8")

    # SCHEMA.md (copy template if available)
    if schema_template and schema_template.exists():
        (wiki_root / "SCHEMA.md").write_text(schema_template.read_text(encoding="utf-8"), encoding="utf-8")

    # machine registry
    (wiki_root / "sources/registry.json").write_text(
        json.dumps({"generated_at": now, "project": project.get("name"),
                    "authority_order": authority, "retrieval": retrieval,
                    "entries": entries_sorted, "edges": edges}, ensure_ascii=False, indent=2),
        encoding="utf-8")

    # human registry table
    rows = ["| source_id | path | type | auth | conf | edges | summary |",
            "|---|---|---|---:|---|---:|---|"]
    for e in entries_sorted:
        rows.append(f"| {e['source_id']} | `{e['source_path']}` | {e['document_type']} | "
                    f"{e['authority_rank']} | {e['confidence']} | {len(e.get('out_edges', []))} | "
                    f"{e['topical_summary']} |")
    (wiki_root / "sources/source-registry.md").write_text(
        "# Source Registry (path-reference)\n\n"
        "> Metadata + path references only — no raw source bodies. Machine copy: `sources/registry.json`.\n\n"
        + "\n".join(rows) + "\n", encoding="utf-8")

    # edge graph
    eg = ["# Relationship Edge Graph (deterministic)", "",
          f"> {len(edges)} edges over {len(entries)} sources. Types: cites/realizes/evidences/part-of/same-thread/superseded-by.",
          "> Every edge is grep-grounded (evidence token shown); no inference.", "",
          "| from | edge | to | evidence |", "|---|---|---|---|"]
    for e in sorted(edges, key=lambda x: (x["edge_type"], x["from"])):
        if e["edge_type"] == "same-thread" and e["from"] > e["to"]:
            continue  # show each same-thread pair once
        eg.append(f"| {e['from']} | {e['edge_type']} | {e['to']} | `{e['evidence']}` |")
    (wiki_root / "relationships/edge-graph.md").write_text("\n".join(eg) + "\n", encoding="utf-8")

    # index.md (orientation entrypoint)
    by_type: dict[str, int] = {}
    for e in entries:
        by_type[e["document_type"]] = by_type.get(e["document_type"], 0) + 1
    idx = ["# Wiki Index", "", f"> Project: {project.get('name', '')}",
           f"> Sources: {len(entries)} · Edges: {len(edges)} · Updated: {now[:19]}", "",
           "## Navigate (orientation order)", "",
           "1. [[SCHEMA]] — data model & rules",
           "2. [[concepts/_index]] — synthesis nodes (relationships)",
           "3. [[sources/source-registry]] — all sources (authority-ranked)",
           "4. [[relationships/edge-graph]] — typed edges",
           "5. [[concepts/document-universe-hierarchy-map]] — authority & query routing", "",
           "## Sources by type", ""]
    for k, v in sorted(by_type.items(), key=lambda kv: -int(authority.get(kv[0], 0))):
        idx.append(f"- {k}: {v} (authority {authority.get(k, 0)})")
    (wiki_root / "index.md").write_text("\n".join(idx) + "\n", encoding="utf-8")

    # concepts/_index.md (derived cache, regenerable)
    concept_files = sorted(p.name for p in (wiki_root / "concepts").glob("*.md")
                           if p.name not in ("_index.md",))
    ci = ["# Concepts Index", "",
          "> Synthesis/relationship nodes. Derived cache — rebuild from node frontmatter if stale.", "",
          "| node | summary |", "|---|---|"]
    for name in concept_files:
        ci.append(f"| [{name}]({name}) | (see node) |")
    (wiki_root / "concepts/_index.md").write_text("\n".join(ci) + "\n", encoding="utf-8")

    # hierarchy / query-routing map
    hm = ["---", "title: document-universe-hierarchy-map", "type: concept",
          "confidence: high", "---", "", "# Document Universe Hierarchy Map", "",
          "## Authority order", ""]
    for k, v in sorted(authority.items(), key=lambda kv: -int(kv[1])):
        hm.append(f"- {k}: {v}")
    hm += ["", "## Query routing", "",
           f"- completion ('did it work'): {', '.join(retrieval.get('completion_first', []))}",
           f"- intent ('what approach'): {', '.join(retrieval.get('intent_first', []))}",
           "- Resolve via metadata/edges first; open a source body only when the summary is insufficient.",
           "- Negative honesty: if nothing clears the authority/score gate, return 'no evidence' — never fabricate a path.",
           "", "## Constitution is NOT indexed", "",
           "The project ROOT constitution (/CLAUDE.md, /.claude/) is deliberately excluded (by-root; vendored",
           "subtrees under seed/ too) so the library stays a neutral evidence base that can surface patterns",
           "not yet codified (anti-confirmation-bias)."]
    (wiki_root / "concepts/document-universe-hierarchy-map.md").write_text("\n".join(hm) + "\n", encoding="utf-8")

    # warm-start durable packets per active thread (highest-authority entry per slug)
    threads: dict[str, list[dict[str, Any]]] = {}
    for e in entries:
        if e.get("topic_slug") and e["lifecycle_state"] == "active":
            threads.setdefault(e["topic_slug"], []).append(e)
    for slug, members in threads.items():
        members = sorted(members, key=lambda x: -x["authority_rank"])
        safe = re.sub(r"[^0-9A-Za-z가-힣_-]", "-", slug)[:80]
        pk = [f"# Warm-start packet — {slug}", "",
              "> Highest-authority context for this thread (path-reference).", ""]
        for e in members:
            pk.append(f"- [{e['document_type']}] `{e['source_path']}` (auth {e['authority_rank']}, {e['confidence']})")
        (wiki_root / f"warm-start/{safe}.md").write_text("\n".join(pk) + "\n", encoding="utf-8")

    # append-only log
    log_path = wiki_root / "log.md"
    head = "" if log_path.exists() else "# Wiki Log\n\n"
    mode = "incremental" if delta else "full"
    dline = (f" (added {delta.get('added',0)}, changed {delta.get('changed',0)}, "
             f"archived {delta.get('archived',0)})" if delta else "")
    with log_path.open("a", encoding="utf-8") as f:
        if head:
            f.write(head)
        f.write(f"## [{now[:19]}] {mode} | sources={len(entries)} edges={len(edges)}{dline}\n\n")

    (wiki_root / "warm-start/README.md").write_text(
        "# Warm-start\n\nDurable per-thread context packets are written here on (re)index.\n"
        "Create extra packets only when an interview/query benefits.\n", encoding="utf-8")


def load_existing(wiki_root: Path) -> dict[str, dict[str, Any]]:
    reg = wiki_root / "sources/registry.json"
    if not reg.exists():
        return {}
    data = json.loads(reg.read_text(encoding="utf-8"))
    return {e["source_path"]: e for e in data.get("entries", [])}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--project-root", required=True)
    ap.add_argument("--wiki-root", required=True)
    ap.add_argument("--answers", required=True)
    ap.add_argument("--incremental", action="store_true",
                    help="warm-start: re-extract only changed/new sources, carry unchanged, archive removed")
    args = ap.parse_args()
    project_root = Path(args.project_root).resolve()
    wiki_root = Path(args.wiki_root).resolve()
    answers = read_answers(Path(args.answers))
    roots = list(answers.get("document_roots", []))
    reg = answers.get("registry", {})
    authority = {str(k): int(v) for k, v in answers.get("authority_order", {}).items()}
    schema_template = Path(__file__).resolve().parent.parent / "templates/__llm-wiki/SCHEMA.md"

    entries, bodies = scan_sources(project_root, roots, authority, reg)
    delta = None

    if args.incremental:
        prior = load_existing(wiki_root)
        cur_paths = {e["source_path"] for e in entries}
        added = changed = 0
        # Preserve indexed_at for unchanged sources (freshness key); edges are
        # always fully re-extracted from current bodies (cheap for markdown,
        # and correct: a new doc can add edges onto older ones).
        for e in entries:
            p = prior.get(e["source_path"])
            if p and p.get("sha256") == e["sha256"]:
                e["indexed_at"] = p.get("indexed_at")
            else:
                if p:
                    changed += 1
                else:
                    added += 1
                e["indexed_at"] = now_iso()
        # archived: present before, gone now
        archived = []
        for path, p in prior.items():
            if path not in cur_paths and p.get("lifecycle_state") != "archived":
                p["lifecycle_state"] = "archived"
                p["out_edges"] = []
                archived.append(p)
        entries.extend(archived)
        delta = {"added": added, "changed": changed, "archived": len(archived)}

    edges = extract_edges([e for e in entries if e["lifecycle_state"] == "active"], bodies)
    write_wiki(wiki_root, answers, entries, edges, delta, schema_template)
    print(json.dumps({"wiki_root": str(wiki_root), "sources": len(entries),
                      "edges": len(edges), "mode": "incremental" if args.incremental else "full",
                      "raw_copy": False, "delta": delta}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
