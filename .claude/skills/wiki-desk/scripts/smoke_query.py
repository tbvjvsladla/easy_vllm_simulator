#!/usr/bin/env python3
"""Path-reference retrieval / smoke over __llm-wiki/sources/registry.json.

Two retrieval modes, both authority-ranked and EDGE-TRAVERSABLE:
  • direct lookup  — the query names a doc stem (e.g. devlog_26062412):
                     return that node's path/authority/confidence + its edges.
  • topical        — score active entries by query-token overlap, gate by
                     min_authority/min_score, rank by intent → authority → score,
                     and surface supporting docs via realizes/evidences/same-thread edges.

Negative honesty: if nothing clears the gates, emit an explicit NO-EVIDENCE
verdict and return non-zero — NEVER print a path the query did not select.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
from pathlib import Path
from typing import Any

STEM_RE = re.compile(r"(?:plan|devlog|testlog)_\d{8}(?:_\d{2}_\d{2})?(?!\d)|(?<!\d)\d{8}(?:_\d{2}_\d{2})?(?!\d)")
TOKEN_RE = re.compile(r"[0-9A-Za-z가-힣.]+")
COMPLETION_HINTS = {"됐", "검증", "통과", "성공", "실패", "오류", "스모크", "evidence",
                    "verify", "verified", "pass", "fail", "smoke", "result", "확인"}
INTENT_HINTS = {"계획", "전략", "왜", "이유", "접근", "방향", "설계", "plan", "strategy",
                "intent", "approach", "design", "why", "rationale"}


def load_registry(wiki_root: Path) -> dict[str, Any]:
    reg = wiki_root / "sources/registry.json"
    if not reg.exists():
        raise SystemExit(f"missing registry: {reg} (run init_wiki_desk.py first)")
    return json.loads(reg.read_text(encoding="utf-8"))


def tokens(text: str) -> set[str]:
    return {t.lower() for t in TOKEN_RE.findall(text) if len(t) >= 2}


def edges_of(sid: str, edges: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for e in edges:
        if e["from"] == sid:
            out.append({"dir": "→", "other": e["to"], "edge_type": e["edge_type"], "evidence": e["evidence"]})
        elif e["to"] == sid:
            out.append({"dir": "←", "other": e["from"], "edge_type": e["edge_type"], "evidence": e["evidence"]})
    return out


def render_node(e: dict[str, Any], edges: list[dict[str, Any]], by_id: dict[str, Any]) -> list[str]:
    lines = [f"- path: `{e['source_path']}` | type: `{e['document_type']}` | "
             f"authority: {e['authority_rank']} | confidence: {e['confidence']} | "
             f"summary: {e['topical_summary']}"]
    rel = edges_of(e["source_id"], edges)
    # anchoring guard: a doc carrying a retroactive supersede banner surfaces WITH a warning —
    # authority rank is unchanged (v1), but the reader is told newer verdicts override this one.
    sup = [r for r in rel if r["dir"] == "→" and r["edge_type"] == "superseded-by"]
    if sup:
        tgts = ", ".join(f"`{by_id.get(r['other'], {}).get('source_path', r['other'])}`" for r in sup)
        lines.append(f"    ⚠ SUPERSEDED(부분/전체) — 후속 판정 우선: {tgts} (이 노드의 결론을 그대로 이월하지 말 것)")
    for r in rel:
        tgt = by_id.get(r["other"], {})
        lines.append(f"    {r['dir']} {r['edge_type']}: `{tgt.get('source_path', r['other'])}` "
                     f"(evidence `{r['evidence']}`)")
    return lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--wiki-root", required=True)
    ap.add_argument("--query", required=True)
    ap.add_argument("--report")
    ap.add_argument("--top", type=int, default=5)
    ap.add_argument("--min-authority", type=int, default=None)
    ap.add_argument("--min-score", type=int, default=None)
    args = ap.parse_args()

    data = load_registry(Path(args.wiki_root))
    entries = [e for e in data.get("entries", []) if e.get("lifecycle_state") == "active"]
    edges = data.get("edges", [])
    retrieval = data.get("retrieval", {})
    by_id = {e["source_id"]: e for e in data.get("entries", [])}
    by_stem = {e["stem"]: e for e in entries if e.get("stem")}
    min_auth = args.min_authority if args.min_authority is not None else int(retrieval.get("min_authority", 0))
    min_score = args.min_score if args.min_score is not None else int(retrieval.get("min_score", 1))

    out = ["# Wiki Query Result", "", f"Query: {args.query}", ""]
    verdict_pass = True

    # --- direct lookup: query names a doc stem ---
    named = [m.group(0) for m in STEM_RE.finditer(args.query)]
    hit = next((by_stem[s] for s in named if s in by_stem), None)
    if hit:
        out += ["## Direct node lookup", ""]
        out += render_node(hit, edges, by_id)
        out += ["", "## Verdict", "", f"PASS — node resolved by stem with {len(edges_of(hit['source_id'], edges))} edge(s)."]
    else:
        # --- topical retrieval ---
        qtok = tokens(args.query)
        intent = "intent" if (qtok & INTENT_HINTS) and not (qtok & COMPLETION_HINTS) else "completion"
        order = retrieval.get(f"{intent}_first", [])
        scored = []
        for e in entries:
            hay = tokens(" ".join([e["title_hint"], e["topical_summary"], e["source_path"],
                                   e["document_type"], e.get("topic_slug") or ""]))
            score = len(qtok & hay)
            if score >= min_score and e["authority_rank"] >= min_auth:
                tprio = order.index(e["document_type"]) if e["document_type"] in order else len(order)
                scored.append((tprio, -e["authority_rank"], -score, e))
        scored.sort(key=lambda x: (x[0], x[1], x[2], x[3]["source_path"]))
        top = [s[3] for s in scored[:args.top]]
        if not top:
            verdict_pass = False
            out += ["## Retrieved path-reference context", "",
                    "(none)", "",
                    "## Verdict", "",
                    "INSUFFICIENT EVIDENCE — no source cleared the authority/relevance gate "
                    f"(min_authority={min_auth}, min_score={min_score}); no path returned (negative honesty)."]
        else:
            out += [f"## Retrieved path-reference context (intent={intent})", ""]
            for e in top:
                out += render_node(e, edges, by_id)
            out += ["", "## Verdict", "",
                    f"PASS — {len(top)} authority-ranked path reference(s) with edge traversal, no raw copy."]

    text = "\n".join(out) + "\n"
    print(text)
    if args.report:
        rp = Path(args.report)
        rp.parent.mkdir(parents=True, exist_ok=True)
        rp.write_text(text + f"\nGenerated: {dt.datetime.now(dt.timezone.utc).isoformat()}\n", encoding="utf-8")
    return 0 if verdict_pass else 2


if __name__ == "__main__":
    raise SystemExit(main())
